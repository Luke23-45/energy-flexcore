// P8.32 — Model-artifact signing, provenance, and verification at load.
//         Ledger `ledger/phase-08-advanced-coordination/ledger_phase_8.md`
//         §P8.32; report trace §9.3 (supply chain, by analogy), §17.4
//         (supply-chain compromise), §6.5 (artifact storage); decision
//         record: `ledger/phase-08-advanced-coordination/
//         32_model_artifact_signing.md`.
//
// WHAT THIS IS: the research-side counterpart of the kernel's P7.14
// plugin signing layer — the digest-and-record supply-chain structure for
// MODEL artifacts (the ledger's named gap: "the research equivalent of
// P7.14 plugin signing, which previously had no counterpart for models").
//
// THE KERNEL ALIGNMENT (mirror by citation, NO kernel dependency — the
// recorded alignment seam of this crate, see `lib.rs` and `Cargo.toml`):
// this module MIRRORS the type shape and the enforcement rules of
// `crates/flexcore-kernel/src/signing.rs` (P7.14) —
//
// 1. THE VERSIONED PROVENANCE RECORD (`flexcore-model-artifact-provenance-v1`).
//    The record binds (artifact ref, digest, digest algorithm, signer key
//    REFERENCE, signing time, worker family) AND the provenance the ledger
//    requires to be LINKED TO THE SIGNATURE, not stored beside it (ledger
//    step 3: "training-data reference, training config/seed, framework
//    versions — linked to the signature, not stored separately where it
//    can drift"): `training-data-ref`, `training-config-ref`, and
//    `framework-version-ref` are REQUIRED record fields — a record without
//    them does not parse, so the provenance cannot silently detach from
//    the signature. The signer key appears as a scheme-prefixed REFERENCE
//    only; the closed key set makes key MATERIAL structurally impossible
//    (there is no field a key could hide in), and a reference without a
//    scheme prefix is rejected as likely inline key material (the value is
//    never echoed back — the P7.05 secret-reference rule, mirrored).
// 2. THE FAIL-CLOSED VERIFIER PORT. [`ModelArtifactDigestVerifier`] is the
//    pluggable digest verifier; the DEFAULT is [`NoArtifactVerifier`],
//    which refuses EVERYTHING — the absence of signing infrastructure can
//    never look like success (the kernel [`NoVerifier`] posture, mirrored).
//    [`UnsafeTestOnlyArtifactDigest`] is the loudly-labelled FNV-1a 64
//    test implementation (NOT a security primitive). A production SHA-256
//    (or better) verifier is a DEPLOYMENT REQUIREMENT recorded as a
//    DECISION POINT in the decision record — no crypto crate is added to
//    this workspace in this wave (the crate stays serde/serde_json only).
// 3. THE LOAD GATE, FAIL-CLOSED AND COMPOSED. [`verify_model_artifact_at_load`]
//    is the one load path: the P8.31 tenant-isolation gate FIRST (the
//    artifact must be registered in the [`ModelServingRegister`] and the
//    worker's host-stamped stamp must cover the serving scope, with a
//    recorded approval for cross-tenant serving), THEN the revocation
//    register, THEN provenance verification. An unsigned artifact, a
//    ref-mismatched record, a tampered (digest-mismatched) artifact, a
//    wrong-algorithm record, and a revoked artifact are each a DISTINCT
//    named refusal — a verification never warns instead of rejecting (the
//    ledger's named risk, closed).
// 4. REVOCATION, APPEND-ONLY (ledger step 1's "how revocation is
//    handled"). [`ModelArtifactRevocationRegister`] records revocations
//    with a REQUIRED reason and actor; there is NO un-revoke — the only
//    direction implemented is the safe one (the kernel UNREVOCATION_SEAM
//    honesty, mirrored): a revocation cannot silently disappear.
// 5. AUDIT ON EVERY OUTCOME. Every load acceptance, every load refusal,
//    and every revocation record/refusal emits an audit record through the
//    in-module sink port, timestamps from the injected clock ONLY
//    ([`crate::validation::HarnessClock`] — the crate's one clock
//    vocabulary; `SystemTime::now()` appears nowhere here). The ledger's
//    "with the refusal audited" is enforced structurally.
//
// WHAT IS A RECORDED SEAM (NOT implemented — the honesty lines):
//
// - NO REAL SIGNATURES. Like the kernel module, this is the
//   digest-and-record STRUCTURE: no PKI, no release keys, no signature
//   algorithm. The production signing identity is a DEPLOYMENT
//   REQUIREMENT (decision record, DEFAULT/RECOMMENDATION recorded there).
// - THE WORKER LOAD SITES ARE NOT WIRED HERE. The Python worker load
//   sites (`apps/worker/python` GNN/MARL artifact loads — security-review
//   findings F-1/F-2/F-3 of `25_security_review.md`) are READ-ONLY in this
//   subtask: this module provides the flexcore-research-side record/verify
//   types that the with-framework activation wave wires into those load
//   sites. Finding F-4 ("signing seam referenced-not-implemented") is
//   addressed at the RECORD level — the seam now HAS an implementation to
//   reference — but the END-TO-END LOAD PATH IS NOT LIVE, and no statement
//   here claims it.
//
// Determinism: parsing is fail-closed over closed key sets; digests are
// exact strings over injected bytes; register iteration is `BTreeMap`-
// ordered. No `unwrap`/`expect` on untrusted input anywhere.
//
// Control status: research only, simulation/shadow (ADR-010). Nothing
// here authorizes, issues, approves, or dispatches anything; a verified
// artifact is a DATA-INTEGRITY fact, never a dispatch authority. No
// safety, certification, or field-authority claim is made.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

use crate::tenant_isolation::{
    ModelServingRegister, ResearchWorkerFamily, ResearchWorkerScope, TenantIsolationError,
    WorkerScopeStamp, WORKER_FAMILIES,
};
use crate::validation::HarnessClock;

// --- Honesty / seam statements -------------------------------------------------------

/// The P8.32 honesty statement (recorded so tests and reviewers can pin
/// what IS and IS NOT claimed — the P4.28/P7.14 statement pattern).
pub const MODEL_SIGNING_STATEMENT: &str = "the P8.32 model-artifact supply-chain layer is a \
     digest-and-record structure: provenance records with LINKED provenance references, a \
     pluggable digest-verifier port, an append-only revocation register, a fail-closed load \
     gate composed with the P8.31 tenant-isolation serving register, and audit on every \
     verify/refuse. No cryptographic signature, PKI, release key, or signature algorithm \
     exists; the default verifier refuses everything; wiring a production SHA-256 (or \
     stronger) verifier is a DEPLOYMENT REQUIREMENT (decision point, no crypto crate added \
     in this wave). The Python worker load sites are NOT wired here — the end-to-end load \
     path is NOT live in this delivery loop. No certification or security claim is made.";

/// The kernel alignment seam (recorded — do not drift silently): this
/// module MIRRORS `crates/flexcore-kernel/src/signing.rs` (P7.14 —
/// provenance-v1 envelope, pluggable verifier port, `NoVerifier` fail-closed
/// default, labelled test-grade digest, refusal-tag constants) BY CITATION.
/// This crate does not depend on flexcore-kernel (composition-crate policy,
/// `Cargo.toml`); if the kernel shape changes, the mirror update is
/// deliberate, fail-closed in both directions until then.
pub const KERNEL_SIGNING_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/signing.rs (P7.14 provenance-v1, DigestVerifier \
     port, NoVerifier fail-closed, UnsafeTestOnlyDigest test-grade FNV-1a 64) BY CITATION; \
     kernel unchanged; no flexcore-kernel dependency";

/// The load-composition rule (the P8.31/P8.32 integration, recorded and
/// pinned by tests): an artifact is loadable ONLY when it passes BOTH
/// gates, in this order, on ONE path.
pub const MODEL_LOAD_COMPOSITION_RULE: &str = "a model artifact is loadable ONLY when it is \
     BOTH registered in the P8.31 ModelServingRegister (owner scope covered, recorded \
     approval for cross-tenant serving) AND provenance-verified here (not revoked, not \
     unsigned, digest matching under the configured verifier). One load path, two gates, \
     audit on every outcome.";

/// The recorded worker-load-site seam (the F-4 answer, stated so no reader
/// can mistake this module for a live end-to-end control).
pub const WORKER_LOAD_SITE_SEAM: &str = "RECORDED SEAM — the Python worker load sites \
     (apps/worker/python GNN/MARL artifact loads; security-review findings F-1/F-2/F-3 of \
     ledger/phase-08-advanced-coordination/25_security_review.md) do NOT call this module \
     yet: the with-framework activation wave wires these record/verify types into those \
     load sites and re-runs the hostile-artifact cases. Until then the end-to-end load \
     path is NOT live; F-4 is addressed at the RECORD level only.";

// --- Closed vocabularies --------------------------------------------------------------

/// The versioned provenance-record envelope id (ledger P8.32 step 1). A
/// record whose `provenance-schema` field is not exactly this value is
/// rejected — never parsed permissively.
pub const MODEL_PROVENANCE_SCHEMA_ID: &str = "flexcore-model-artifact-provenance-v1";

/// Maximum accepted provenance-record source size in bytes (fail-closed
/// bound; a provenance record is metadata, not a payload).
pub const MAX_MODEL_PROVENANCE_BYTES: usize = 64 * 1024;

/// The digest algorithm a PRODUCTION deployment must wire (DEPLOYMENT
/// REQUIREMENT — not implemented in this crate; see the module header and
/// the decision record). The name is accepted in records so the deployment
/// wiring does not need a schema change; acceptance is meaningless until a
/// real SHA-256 [`ModelArtifactDigestVerifier`] is configured — and
/// [`NoArtifactVerifier`] (the default) refuses everything regardless of
/// the declared algorithm.
pub const MODEL_DIGEST_ALGORITHM_SHA256: &str = "sha-256";

/// The TEST-ONLY digest algorithm: FNV-1a 64 over the artifact bytes (the
/// repo's non-cryptographic integrity-checksum pattern, P4.28/P6.14/P7.14).
/// NOT a security primitive; never acceptable outside tests. It exists so
/// the gate, tamper, mismatch, and audit paths are exercisable without
/// adding a crypto dependency to this crate.
pub const MODEL_DIGEST_ALGORITHM_FNV1A64_TEST: &str = "fnv1a64-test";

/// The closed digest-algorithm vocabulary for model provenance records.
/// Anything else is [`ArtifactSigningError::RecordDigestAlgorithmUnknown`].
pub const MODEL_DIGEST_ALGORITHMS: [&str; 2] =
    [MODEL_DIGEST_ALGORITHM_SHA256, MODEL_DIGEST_ALGORITHM_FNV1A64_TEST];

/// The accepted signer-key-reference schemes (mirrors the kernel
/// signing.rs `KEY_REFERENCE_SCHEMES`; `key-store:` is the deployment key
/// store the production signer writes references into).
pub const MODEL_SIGNER_KEY_REFERENCE_SCHEMES: [&str; 3] = ["key-store:", "env:", "file:"];

/// Maximum reference/id length in a provenance record (the crate's
/// 256-character id bound posture, validation.rs `MAX_ID_CHARS` aligned).
pub const MAX_MODEL_REF_CHARS: usize = 256;

// --- Refusal reason tags (stable; they ride the refusal reasons and the
// --- audit details, and the tests pin them) ------------------------------------------

/// No provenance record was supplied for the artifact (the unsigned case).
pub const MODEL_REFUSAL_UNSIGNED: &str = "UNSIGNED_MODEL_ARTIFACT";
/// The record's `artifact-ref` does not name the artifact being loaded.
pub const MODEL_REFUSAL_REF_MISMATCH: &str = "MODEL_ARTIFACT_REF_MISMATCH";
/// The record's declared algorithm is not the configured verifier's.
pub const MODEL_REFUSAL_ALGORITHM_MISMATCH: &str = "MODEL_DIGEST_ALGORITHM_MISMATCH";
/// The record's digest does not match the artifact bytes.
pub const MODEL_REFUSAL_DIGEST_MISMATCH: &str = "MODEL_DIGEST_MISMATCH";
/// The configured verifier refused (e.g. the fail-closed [`NoArtifactVerifier`]).
pub const MODEL_REFUSAL_VERIFIER_REFUSED: &str = "MODEL_VERIFIER_REFUSED";
/// The artifact is in the append-only revocation register.
pub const MODEL_REFUSAL_REVOKED: &str = "MODEL_ARTIFACT_REVOKED";

/// The actor attributed to load-gate audit records.
const ACTOR_MODEL_LOAD_GATE: &str = "model-artifact-load-gate";
/// The actor attributed to revocation-register audit records.
const ACTOR_MODEL_REVOCATION_REGISTER: &str = "model-artifact-revocation-register";

// --- The digest-verifier port ---------------------------------------------------------

/// The outcome of one digest verification. Fail-closed: an implementation
/// that cannot verify reports [`ModelDigestVerdict::Refused`], never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelDigestVerdict {
    /// The artifact bytes match the declared digest under this verifier's
    /// algorithm.
    Matches,
    /// The artifact bytes do NOT match the declared digest.
    Mismatch,
    /// The verifier could not verify at all (not configured, backend
    /// unavailable). Never treated as a pass.
    Refused { reason: String },
}

/// The pluggable digest verifier for model artifacts (the P7.14 port,
/// mirrored). This crate defines the port; the composition root injects
/// the implementation. The DEFAULT is [`NoArtifactVerifier`] (refuses
/// everything); [`UnsafeTestOnlyArtifactDigest`] is the labelled test
/// implementation; a production SHA-256 implementation is a DEPLOYMENT
/// REQUIREMENT (module header; decision point in the decision record).
pub trait ModelArtifactDigestVerifier {
    /// The algorithm name this verifier computes (compared against the
    /// record's `digest-algorithm` field; a mismatch is a refusal).
    fn algorithm_name(&self) -> &'static str;
    /// Verify `artifact` against the declared hex digest.
    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> ModelDigestVerdict;
}

/// The fail-closed DEFAULT verifier: refuses EVERY artifact. A deployment
/// that wires nothing verifies nothing — and nothing loads. This is the
/// recorded safety posture: the absence of signing infrastructure can
/// never look like success.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoArtifactVerifier;

impl ModelArtifactDigestVerifier for NoArtifactVerifier {
    fn algorithm_name(&self) -> &'static str {
        "none"
    }

    fn verify(&self, _artifact: &[u8], _declared_digest_hex: &str) -> ModelDigestVerdict {
        ModelDigestVerdict::Refused {
            reason: "no digest verifier is configured; verification is refused \
                     (fail-closed default; production SHA-256 wiring is a deployment \
                     requirement recorded in the P8.32 decision record)"
                .to_string(),
        }
    }
}

/// The FNV-1a 64-bit hash (the repo's non-cryptographic pattern —
/// P4.28/P6.14/P7.14/P8.31; deterministic, dependency-free, explicitly NOT
/// a security primitive).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Lowercase hex of a `u64` (16 digits, no padding surprises).
fn hex_u64(value: u64) -> String {
    format!("{value:016x}")
}

/// The TEST-ONLY digest verifier: FNV-1a 64 over the artifact bytes.
/// UNSAFE FOR SUPPLY-CHAIN INTEGRITY by construction (a 64-bit
/// non-cryptographic checksum; forged collisions are practical). Exists
/// ONLY so the verification structure, tamper path, and audit trail are
/// testable without a crypto dependency. A deployment that wires this in
/// production has wired nothing — and must be treated as
/// [`NoArtifactVerifier`].
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsafeTestOnlyArtifactDigest;

impl ModelArtifactDigestVerifier for UnsafeTestOnlyArtifactDigest {
    fn algorithm_name(&self) -> &'static str {
        MODEL_DIGEST_ALGORITHM_FNV1A64_TEST
    }

    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> ModelDigestVerdict {
        let computed = hex_u64(fnv1a64(artifact));
        if computed == declared_digest_hex {
            ModelDigestVerdict::Matches
        } else {
            ModelDigestVerdict::Mismatch
        }
    }
}

// --- The provenance record ------------------------------------------------------------

/// A validated model-artifact provenance record
/// (`flexcore-model-artifact-provenance-v1`): the digest-and-record
/// metadata a signer attaches to a model artifact. The signer key is a
/// REFERENCE, never material; the training provenance
/// (data/config/framework references) is IN the signed record — linked to
/// the signature, not stored separately where it can drift (ledger step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactProvenance {
    /// The signed artifact's reference (the P8.31 register key this record
    /// binds; a reference, never the artifact itself).
    pub artifact_ref: String,
    /// The digest algorithm, from [`MODEL_DIGEST_ALGORITHMS`] (closed).
    pub digest_algorithm: String,
    /// The artifact digest as lowercase hex (the verifier compares this
    /// against its own computation over the artifact bytes).
    pub digest_hex: String,
    /// The signer key REFERENCE (scheme-prefixed). Never key material —
    /// the closed key set makes material structurally impossible.
    pub signer_key_reference: String,
    /// Signing time in milliseconds of the SIGNER's clock (recorded data;
    /// the verifier never derives it).
    pub signed_at_millis: u64,
    /// The worker family the artifact serves (closed vocabulary;
    /// [`ResearchWorkerFamily`]).
    pub worker_family: ResearchWorkerFamily,
    /// The training-data reference (LINKED provenance, REQUIRED — P8.35
    /// owns the full dataset record; this is its reference).
    pub training_data_ref: String,
    /// The training-config/seed reference (LINKED provenance, REQUIRED —
    /// the P8.20 reproducibility record carries the config and seeds).
    pub training_config_ref: String,
    /// The framework-versions reference (LINKED provenance, REQUIRED —
    /// the pinned framework pair from P8.03/P8.10).
    pub framework_version_ref: String,
}

// --- Record errors --------------------------------------------------------------------

/// Every provenance-record violation is a DISTINCT named error
/// ([`ArtifactSigningError::name`]). Error text never echoes a rejected
/// key-reference VALUE (it may be key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactSigningError {
    /// The source text was not valid JSON at all (also: oversize source).
    RecordJsonInvalid { reason: String },
    /// The JSON document parsed but is not an object.
    RecordNotAnObject,
    /// `provenance-schema` is present but not [`MODEL_PROVENANCE_SCHEMA_ID`].
    RecordSchemaUnsupported { found: String },
    /// A field appeared that the closed key set does not declare.
    RecordUnknownField { location: String, field: String },
    /// A required field was absent.
    RecordMissingField { field: String, location: String },
    /// A field was present with the wrong JSON type.
    RecordWrongType {
        location: String,
        field: String,
        expected: &'static str,
        found: &'static str,
    },
    /// `digest` is not a lowercase hex string of plausible length.
    RecordDigestInvalid { reason: String },
    /// `digest-algorithm` is not in [`MODEL_DIGEST_ALGORITHMS`].
    RecordDigestAlgorithmUnknown { value: String },
    /// `signer-key-reference` does not name a scheme-prefixed location
    /// (likely inline key material). Deliberately does NOT echo the value.
    RecordKeyReferenceInvalid { reason: String },
    /// `worker-family` is not in the closed [`WORKER_FAMILIES`] vocabulary.
    RecordFamilyUnknown { value: String },
    /// A reference/scalar field is empty or overlong.
    RecordFieldInvalid { field: String, reason: String },
    /// A revocation record is structurally invalid (empty artifact ref,
    /// reason, or actor).
    RevocationInvalid { reason: String },
    /// The artifact is already in the revocation register (the register is
    /// append-only; exactly one record per artifact ref).
    RevocationAlreadyRecorded { artifact_ref: String },
}

impl ArtifactSigningError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactSigningError::RecordJsonInvalid { .. } => "MODEL_PROVENANCE_RECORD_JSON_INVALID",
            ArtifactSigningError::RecordNotAnObject => "MODEL_PROVENANCE_RECORD_NOT_AN_OBJECT",
            ArtifactSigningError::RecordSchemaUnsupported { .. } => "MODEL_PROVENANCE_SCHEMA_UNSUPPORTED",
            ArtifactSigningError::RecordUnknownField { .. } => "MODEL_PROVENANCE_UNKNOWN_FIELD",
            ArtifactSigningError::RecordMissingField { .. } => "MODEL_PROVENANCE_MISSING_FIELD",
            ArtifactSigningError::RecordWrongType { .. } => "MODEL_PROVENANCE_WRONG_TYPE",
            ArtifactSigningError::RecordDigestInvalid { .. } => "MODEL_PROVENANCE_DIGEST_INVALID",
            ArtifactSigningError::RecordDigestAlgorithmUnknown { .. } => {
                "MODEL_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN"
            }
            ArtifactSigningError::RecordKeyReferenceInvalid { .. } => {
                "MODEL_PROVENANCE_SIGNER_KEY_REFERENCE_INVALID"
            }
            ArtifactSigningError::RecordFamilyUnknown { .. } => "MODEL_PROVENANCE_WORKER_FAMILY_UNKNOWN",
            ArtifactSigningError::RecordFieldInvalid { .. } => "MODEL_PROVENANCE_FIELD_INVALID",
            ArtifactSigningError::RevocationInvalid { .. } => "MODEL_ARTIFACT_REVOCATION_INVALID",
            ArtifactSigningError::RevocationAlreadyRecorded { .. } => {
                "MODEL_ARTIFACT_REVOCATION_ALREADY_RECORDED"
            }
        }
    }
}

impl fmt::Display for ArtifactSigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactSigningError::RecordJsonInvalid { reason } => {
                write!(f, "model provenance record is not valid JSON: {reason}")
            }
            ArtifactSigningError::RecordNotAnObject => {
                write!(f, "model provenance record must be a JSON object")
            }
            ArtifactSigningError::RecordSchemaUnsupported { found } => write!(
                f,
                "unsupported provenance-schema id '{found}' (expected '{MODEL_PROVENANCE_SCHEMA_ID}')"
            ),
            ArtifactSigningError::RecordUnknownField { location, field } => write!(
                f,
                "unknown field '{field}' at {location} (unknown fields are rejected, not \
                 warned; the closed key set is what makes signer key material structurally \
                 impossible)"
            ),
            ArtifactSigningError::RecordMissingField { field, location } => {
                write!(f, "missing required field '{field}' at {location}")
            }
            ArtifactSigningError::RecordWrongType { location, field, expected, found } => {
                write!(f, "field '{field}' at {location} must be {expected}, found {found}")
            }
            ArtifactSigningError::RecordDigestInvalid { reason } => {
                write!(f, "model provenance digest is invalid: {reason}")
            }
            ArtifactSigningError::RecordDigestAlgorithmUnknown { value } => write!(
                f,
                "digest algorithm '{value}' is not in the closed vocabulary ({}); unknown \
                 algorithms are refused, never accepted permissively",
                MODEL_DIGEST_ALGORITHMS.join(", ")
            ),
            ArtifactSigningError::RecordKeyReferenceInvalid { reason } => {
                // The reference VALUE is deliberately not echoed: a
                // rejected reference may be inline key material.
                write!(f, "signer key reference is invalid: {reason}")
            }
            ArtifactSigningError::RecordFamilyUnknown { value } => write!(
                f,
                "worker family '{value}' is not in the closed research worker vocabulary \
                 ({}); unknown families are refused, never guessed",
                WORKER_FAMILIES.join(", ")
            ),
            ArtifactSigningError::RecordFieldInvalid { field, reason } => {
                write!(f, "field '{field}' is invalid: {reason}")
            }
            ArtifactSigningError::RevocationInvalid { reason } => {
                write!(f, "the model-artifact revocation is invalid: {reason}")
            }
            ArtifactSigningError::RevocationAlreadyRecorded { artifact_ref } => write!(
                f,
                "the model artifact '{artifact_ref}' is already revoked (the register is \
                 append-only; exactly one revocation record per artifact reference)"
            ),
        }
    }
}

impl std::error::Error for ArtifactSigningError {}

// --- Record parsing (the kernel signing.rs validation style, mirrored) --

/// Parse and validate a model provenance record from its JSON source text
/// (fail-closed: oversize input is rejected before parsing).
pub fn parse_model_provenance(text: &str) -> Result<ModelArtifactProvenance, ArtifactSigningError> {
    if text.len() > MAX_MODEL_PROVENANCE_BYTES {
        return Err(ArtifactSigningError::RecordJsonInvalid {
            reason: format!(
                "model provenance record exceeds the {} byte limit",
                MAX_MODEL_PROVENANCE_BYTES
            ),
        });
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|err| ArtifactSigningError::RecordJsonInvalid { reason: err.to_string() })?;
    parse_model_provenance_value(&value)
}

/// Parse and validate a model provenance record from an already-parsed
/// JSON value (used by tests and by callers that receive structured JSON).
pub fn parse_model_provenance_value(value: &Value) -> Result<ModelArtifactProvenance, ArtifactSigningError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => return Err(ArtifactSigningError::RecordNotAnObject),
    };

    // 1. Envelope first.
    let schema_id = required_string(obj, "provenance-schema", "model-provenance")?;
    if schema_id != MODEL_PROVENANCE_SCHEMA_ID {
        return Err(ArtifactSigningError::RecordSchemaUnsupported { found: schema_id });
    }

    // 2. Closed top-level key set. There is NO field a signer key could
    //    hide in — key material is structurally impossible (module header).
    //    The three `*-ref` provenance fields are REQUIRED (ledger step 3:
    //    provenance linked to the signature, never stored apart).
    const ALLOWED_TOP_LEVEL: [&str; 10] = [
        "provenance-schema",
        "artifact-ref",
        "digest-algorithm",
        "digest",
        "signer-key-reference",
        "signed-at-millis",
        "worker-family",
        "training-data-ref",
        "training-config-ref",
        "framework-version-ref",
    ];
    reject_unknown_fields(obj, &ALLOWED_TOP_LEVEL, "model-provenance")?;

    for field in ALLOWED_TOP_LEVEL {
        if !obj.contains_key(field) {
            return Err(ArtifactSigningError::RecordMissingField {
                field: field.to_string(),
                location: "model-provenance".to_string(),
            });
        }
    }

    // 3. Artifact identity.
    let artifact_ref = required_string(obj, "artifact-ref", "model-provenance")?;
    validate_ref(&artifact_ref, "artifact-ref")?;

    // 4. Digest: closed algorithm vocabulary + lowercase hex of plausible
    //    length. Length-vs-algorithm consistency is the VERIFIER's check
    //    (it owns the algorithm's digest shape); the record only refuses
    //    strings that cannot be a digest at all.
    let digest_algorithm = required_string(obj, "digest-algorithm", "model-provenance")?;
    if !MODEL_DIGEST_ALGORITHMS.contains(&digest_algorithm.as_str()) {
        return Err(ArtifactSigningError::RecordDigestAlgorithmUnknown { value: digest_algorithm });
    }
    let digest_hex = required_string(obj, "digest", "model-provenance")?;
    validate_digest_hex(&digest_hex)?;

    // 5. Signer key REFERENCE (scheme-prefixed; value never echoed on
    //    rejection — it may be key material).
    let signer_key_reference = required_string(obj, "signer-key-reference", "model-provenance")?;
    let scheme_ok = MODEL_SIGNER_KEY_REFERENCE_SCHEMES.iter().any(|scheme| {
        signer_key_reference
            .strip_prefix(*scheme)
            .map(|rest| !rest.is_empty())
            .unwrap_or(false)
    });
    if !scheme_ok {
        return Err(ArtifactSigningError::RecordKeyReferenceInvalid {
            reason: "a signer key reference must use the key-store:, env:, or file: scheme \
                     and name a location; model provenance records carry key REFERENCES only \
                     and can never carry key material"
                .to_string(),
        });
    }

    // 6. Signing time (recorded data from the signer's clock).
    let signed_at_millis = match obj.get("signed-at-millis") {
        Some(Value::Number(n)) => match n.as_u64() {
            Some(v) => v,
            None => {
                return Err(ArtifactSigningError::RecordWrongType {
                    location: "model-provenance".to_string(),
                    field: "signed-at-millis".to_string(),
                    expected: "unsigned integer",
                    found: "number",
                })
            }
        },
        Some(other) => {
            return Err(ArtifactSigningError::RecordWrongType {
                location: "model-provenance".to_string(),
                field: "signed-at-millis".to_string(),
                expected: "unsigned integer",
                found: json_type_name(other),
            })
        }
        None => {
            return Err(ArtifactSigningError::RecordMissingField {
                field: "signed-at-millis".to_string(),
                location: "model-provenance".to_string(),
            })
        }
    };

    // 7. The worker family (closed vocabulary).
    let worker_family_text = required_string(obj, "worker-family", "model-provenance")?;
    let worker_family = ResearchWorkerFamily::parse(&worker_family_text).ok_or_else(|| {
        ArtifactSigningError::RecordFamilyUnknown { value: worker_family_text.clone() }
    })?;

    // 8. The LINKED provenance references (all required, non-empty).
    let training_data_ref = required_string(obj, "training-data-ref", "model-provenance")?;
    validate_ref(&training_data_ref, "training-data-ref")?;
    let training_config_ref = required_string(obj, "training-config-ref", "model-provenance")?;
    validate_ref(&training_config_ref, "training-config-ref")?;
    let framework_version_ref = required_string(obj, "framework-version-ref", "model-provenance")?;
    validate_ref(&framework_version_ref, "framework-version-ref")?;

    Ok(ModelArtifactProvenance {
        artifact_ref,
        digest_algorithm,
        digest_hex,
        signer_key_reference,
        signed_at_millis,
        worker_family,
        training_data_ref,
        training_config_ref,
        framework_version_ref,
    })
}

/// A provenance reference/id: non-empty, at most 256 characters.
fn validate_ref(value: &str, field: &str) -> Result<(), ArtifactSigningError> {
    if value.trim().is_empty() || value.len() > MAX_MODEL_REF_CHARS {
        return Err(ArtifactSigningError::RecordFieldInvalid {
            field: field.to_string(),
            reason: format!(
                "{field} must be a non-empty string of at most {MAX_MODEL_REF_CHARS} characters"
            ),
        });
    }
    Ok(())
}

/// Lowercase hex, even length, 8..=512 digits (covers FNV-1a 64 at 16 and
/// SHA-256 at 64; the verifier owns the algorithm-exact shape).
fn validate_digest_hex(hex: &str) -> Result<(), ArtifactSigningError> {
    if hex.len() < 8 || hex.len() > 512 || hex.len() % 2 != 0 {
        return Err(ArtifactSigningError::RecordDigestInvalid {
            reason: "digest must be an even-length lowercase hex string of 8..=512 digits"
                .to_string(),
        });
    }
    if !hex
        .bytes()
        .all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b.is_ascii_hexdigit()))
    {
        return Err(ArtifactSigningError::RecordDigestInvalid {
            reason: "digest must be lowercase hex".to_string(),
        });
    }
    Ok(())
}

// --- Verification ----------------------------------------------------------------------

/// The verification outcome for one artifact against its provenance
/// record. `Accepted` is the ONLY outcome that lets the load gate pass;
/// every refusal carries a stable reason tag (the constants above).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvenanceVerdict {
    Accepted,
    Refused { reason: String },
}

impl ModelProvenanceVerdict {
    pub fn refused(&self) -> bool {
        matches!(self, ModelProvenanceVerdict::Refused { .. })
    }
}

/// Verify one artifact against its provenance record:
///
/// 1. the record must bind the artifact being loaded (exact ref) —
///    otherwise `MODEL_ARTIFACT_REF_MISMATCH`;
/// 2. the record's declared algorithm must be the configured verifier's —
///    otherwise `MODEL_DIGEST_ALGORITHM_MISMATCH`;
/// 3. the verifier must report [`ModelDigestVerdict::Matches`] — a
///    mismatch is `MODEL_DIGEST_MISMATCH`, a verifier refusal is
///    `MODEL_VERIFIER_REFUSED`.
///
/// Pure: no clock, no audit, no state — the load gate below adds the
/// side effects.
pub fn verify_model_artifact(
    record: &ModelArtifactProvenance,
    expected_artifact_ref: &str,
    artifact: &[u8],
    verifier: &dyn ModelArtifactDigestVerifier,
) -> ModelProvenanceVerdict {
    if record.artifact_ref != expected_artifact_ref {
        return ModelProvenanceVerdict::Refused {
            reason: format!(
                "{MODEL_REFUSAL_REF_MISMATCH}: the provenance record binds artifact '{}', \
                 not the artifact being loaded ('{}')",
                record.artifact_ref, expected_artifact_ref
            ),
        };
    }
    if record.digest_algorithm != verifier.algorithm_name() {
        return ModelProvenanceVerdict::Refused {
            reason: format!(
                "{MODEL_REFUSAL_ALGORITHM_MISMATCH}: the record declares '{}' but the \
                 configured verifier computes '{}'",
                record.digest_algorithm,
                verifier.algorithm_name()
            ),
        };
    }
    match verifier.verify(artifact, &record.digest_hex) {
        ModelDigestVerdict::Matches => ModelProvenanceVerdict::Accepted,
        ModelDigestVerdict::Mismatch => ModelProvenanceVerdict::Refused {
            reason: format!(
                "{MODEL_REFUSAL_DIGEST_MISMATCH}: the artifact bytes do not match the \
                 record's declared digest under '{}' — a tampered artifact is refused, \
                 never trusted",
                record.digest_algorithm
            ),
        },
        ModelDigestVerdict::Refused { reason } => ModelProvenanceVerdict::Refused {
            reason: format!("{MODEL_REFUSAL_VERIFIER_REFUSED}: {reason}"),
        },
    }
}

// --- The append-only revocation register (ledger step 1) -------------------------------

/// One model-artifact revocation record (append-only). The reason and
/// actor are REQUIRED — an unattributed revocation is refused, never
/// recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactRevocation {
    /// The revoked artifact reference.
    pub artifact_ref: String,
    /// Why the artifact was revoked (decision-record reference — REQUIRED).
    pub reason: String,
    /// Who revoked it (REQUIRED).
    pub actor: String,
    /// When (injected-clock millis; set by the register, never by the caller).
    pub revoked_at_millis: i64,
}

/// The append-only model-artifact revocation register (ledger step 1's
/// "how revocation is handled"). There is NO un-revoke and NO edit: the
/// only direction implemented is the safe one — a revocation cannot
/// silently disappear (the kernel `UNREVOCATION_SEAM` honesty, mirrored;
/// the P8.31 serving-approval note applies in the same direction: the
/// append-only sides are each the SAFE direction of their control).
#[derive(Debug, Default)]
pub struct ModelArtifactRevocationRegister {
    revoked: BTreeMap<String, ModelArtifactRevocation>,
    append_order: Vec<ModelArtifactRevocation>,
}

impl ModelArtifactRevocationRegister {
    pub fn new() -> ModelArtifactRevocationRegister {
        ModelArtifactRevocationRegister {
            revoked: BTreeMap::new(),
            append_order: Vec::new(),
        }
    }

    /// Records one revocation (host-side). The reason and actor are
    /// REQUIRED; a duplicate artifact ref is refused. Both the recording
    /// and the refusal are audited.
    pub fn revoke(
        &mut self,
        artifact_ref: &str,
        reason: &str,
        actor: &str,
        clock: &dyn HarnessClock,
        audit: &dyn ModelArtifactAuditSink,
    ) -> Result<(), ArtifactSigningError> {
        let outcome = self.revoke_inner(artifact_ref, reason, actor, clock);
        match &outcome {
            Ok(()) => audit.record(ModelArtifactAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: ModelArtifactAuditKind::RevocationRecorded,
                actor: ACTOR_MODEL_REVOCATION_REGISTER.to_string(),
                detail: format!(
                    "model artifact '{artifact_ref}' revoked (by '{actor}'): {reason}"
                ),
            }),
            Err(err) => audit.record(ModelArtifactAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: ModelArtifactAuditKind::RevocationRefused,
                actor: ACTOR_MODEL_REVOCATION_REGISTER.to_string(),
                detail: format!("revocation refused ({}): {err}", err.name()),
            }),
        }
        outcome
    }

    fn revoke_inner(
        &mut self,
        artifact_ref: &str,
        reason: &str,
        actor: &str,
        clock: &dyn HarnessClock,
    ) -> Result<(), ArtifactSigningError> {
        if artifact_ref.trim().is_empty() {
            return Err(ArtifactSigningError::RevocationInvalid {
                reason: "the artifact reference is empty — a revocation binds an artifact"
                    .to_string(),
            });
        }
        if reason.trim().is_empty() {
            return Err(ArtifactSigningError::RevocationInvalid {
                reason: "a revocation requires a non-empty reason (an unattributed \
                         revocation is never recorded)"
                    .to_string(),
            });
        }
        if actor.trim().is_empty() {
            return Err(ArtifactSigningError::RevocationInvalid {
                reason: "a revocation requires a non-empty actor".to_string(),
            });
        }
        if self.revoked.contains_key(artifact_ref) {
            return Err(ArtifactSigningError::RevocationAlreadyRecorded {
                artifact_ref: artifact_ref.to_string(),
            });
        }
        let record = ModelArtifactRevocation {
            artifact_ref: artifact_ref.to_string(),
            reason: reason.to_string(),
            actor: actor.to_string(),
            revoked_at_millis: clock.now_millis(),
        };
        self.revoked.insert(artifact_ref.to_string(), record.clone());
        self.append_order.push(record);
        Ok(())
    }

    /// Whether the artifact reference is revoked (the load gate's check).
    pub fn is_revoked(&self, artifact_ref: &str) -> bool {
        self.revoked.contains_key(artifact_ref)
    }

    /// The revocations in append order (audit/replay view).
    pub fn revocations_in_append_order(&self) -> &[ModelArtifactRevocation] {
        &self.append_order
    }
}

// --- The composed load gate (P8.31 gate FIRST, then signing) ------------

/// The LINKED provenance a successful load hands to the caller: the loader
/// MUST carry these references with the loaded artifact (the ledger's
/// "every loaded artifact carries linked provenance" — there is no accepted
/// load without them, by construction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactLoadAcceptance {
    pub artifact_ref: String,
    pub digest_algorithm: String,
    pub digest_hex: String,
    pub signer_key_reference: String,
    pub training_data_ref: String,
    pub training_config_ref: String,
    pub framework_version_ref: String,
    pub worker_family: ResearchWorkerFamily,
}

/// Every load-gate refusal is a DISTINCT named error
/// ([`ModelArtifactLoadError::name`]). The tenant-isolation variant
/// delegates its name to the inner P8.31 error so audit queries span both
/// paths (the P8.31 alignment discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelArtifactLoadError {
    /// The P8.31 gate refused (unknown stamp, binding mismatch, unknown
    /// artifact, scope mismatch, unapproved cross-tenant serving). The
    /// inner error's stable name IS this error's name.
    TenantIsolation(TenantIsolationError),
    /// The artifact is in the append-only revocation register.
    ArtifactRevoked { artifact_ref: String },
    /// No provenance record was supplied — the unsigned artifact case.
    Unsigned,
    /// The record does not bind the artifact being loaded.
    ArtifactRefMismatch { record_ref: String, loaded_ref: String },
    /// Verification refused (algorithm mismatch, digest mismatch/tamper,
    /// verifier refusal). The reason carries the stable refusal tag.
    VerificationRefused { reason: String },
}

impl ModelArtifactLoadError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            // Name-aligned with the P8.31 refusal so audit queries span
            // both gates (the tenant-isolation alignment discipline).
            ModelArtifactLoadError::TenantIsolation(inner) => inner.name(),
            ModelArtifactLoadError::ArtifactRevoked { .. } => "MODEL_ARTIFACT_REVOKED",
            ModelArtifactLoadError::Unsigned => "MODEL_ARTIFACT_UNSIGNED",
            ModelArtifactLoadError::ArtifactRefMismatch { .. } => {
                "MODEL_PROVENANCE_ARTIFACT_REF_MISMATCH"
            }
            ModelArtifactLoadError::VerificationRefused { .. } => {
                "MODEL_ARTIFACT_VERIFICATION_REFUSED"
            }
        }
    }
}

impl fmt::Display for ModelArtifactLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelArtifactLoadError::TenantIsolation(inner) => write!(f, "{inner}"),
            ModelArtifactLoadError::ArtifactRevoked { artifact_ref } => write!(
                f,
                "the model artifact '{artifact_ref}' is REVOKED ({MODEL_REFUSAL_REVOKED}); \
                 a revoked artifact is refused at load even with valid provenance"
            ),
            ModelArtifactLoadError::Unsigned => write!(
                f,
                "no provenance record was supplied ({MODEL_REFUSAL_UNSIGNED}); an unsigned \
                 model artifact is refused before any use (report §9.3, ledger P8.32 step 2)"
            ),
            ModelArtifactLoadError::ArtifactRefMismatch { record_ref, loaded_ref } => write!(
                f,
                "the provenance record binds artifact '{record_ref}' but the artifact being \
                 loaded is '{loaded_ref}' ({MODEL_REFUSAL_REF_MISMATCH})"
            ),
            ModelArtifactLoadError::VerificationRefused { reason } => {
                write!(f, "model artifact verification refused: {reason}")
            }
        }
    }
}

impl std::error::Error for ModelArtifactLoadError {}

/// The ONE model-artifact load path (ledger step 2, composed with P8.31
/// per [`MODEL_LOAD_COMPOSITION_RULE`]). Order is deterministic; each step
/// is a distinct named refusal; EVERY outcome (acceptance and refusal) is
/// audited with a timestamp from the injected clock.
///
/// 1. the worker stamp binding is re-verified (P8.31);
/// 2. the P8.31 serving register enforces registration, scope, and the
///    recorded cross-tenant approval;
/// 3. the revocation register is consulted (a revoked artifact is refused
///    even with valid provenance);
/// 4. the provenance record must EXIST (unsigned refused — never "treat
///    as trusted");
/// 5. the record must bind the artifact being loaded;
/// 6. [`verify_model_artifact`] must accept (algorithm + digest).
pub fn verify_model_artifact_at_load(
    stamp: &WorkerScopeStamp,
    serving_register: &ModelServingRegister,
    serving_scope: &ResearchWorkerScope,
    revocations: &ModelArtifactRevocationRegister,
    provenance: Option<&ModelArtifactProvenance>,
    artifact_ref: &str,
    artifact: &[u8],
    verifier: &dyn ModelArtifactDigestVerifier,
    clock: &dyn HarnessClock,
    audit: &dyn ModelArtifactAuditSink,
) -> Result<ModelArtifactLoadAcceptance, ModelArtifactLoadError> {
    let outcome = verify_model_artifact_at_load_inner(
        stamp,
        serving_register,
        serving_scope,
        revocations,
        provenance,
        artifact_ref,
        artifact,
        verifier,
        clock,
    );
    let (kind, detail) = match &outcome {
        Ok(acceptance) => (
            ModelArtifactAuditKind::LoadAccepted,
            format!(
                "worker '{}' ({}) loaded artifact '{}' verified under '{}' (signer key \
                 reference '{}'; training data '{}'; training config '{}'; framework \
                 versions '{}')",
                stamp.worker_id(),
                stamp.family().as_str(),
                acceptance.artifact_ref,
                acceptance.digest_algorithm,
                acceptance.signer_key_reference,
                acceptance.training_data_ref,
                acceptance.training_config_ref,
                acceptance.framework_version_ref,
            ),
        ),
        Err(err) => (
            ModelArtifactAuditKind::LoadRefused,
            format!(
                "worker '{}' ({}) load of artifact '{}' refused ({}): {err}",
                stamp.worker_id(),
                stamp.family().as_str(),
                artifact_ref,
                err.name(),
            ),
        ),
    };
    audit.record(ModelArtifactAuditRecord {
        timestamp_millis: clock.now_millis(),
        kind,
        actor: ACTOR_MODEL_LOAD_GATE.to_string(),
        detail,
    });
    outcome
}

#[allow(clippy::too_many_arguments)]
fn verify_model_artifact_at_load_inner(
    stamp: &WorkerScopeStamp,
    serving_register: &ModelServingRegister,
    serving_scope: &ResearchWorkerScope,
    revocations: &ModelArtifactRevocationRegister,
    provenance: Option<&ModelArtifactProvenance>,
    artifact_ref: &str,
    artifact: &[u8],
    verifier: &dyn ModelArtifactDigestVerifier,
    clock: &dyn HarnessClock,
) -> Result<ModelArtifactLoadAcceptance, ModelArtifactLoadError> {
    let _ = clock; // the clock rides the caller's audit emission; kept explicit
    // 1. + 2. The P8.31 gate first: stamp binding, registration, scope,
    // recorded cross-tenant approval. Its refusals pass through with
    // their own stable names.
    serving_register
        .enforce_model_serving(stamp, artifact_ref, serving_scope)
        .map_err(ModelArtifactLoadError::TenantIsolation)?;

    // 3. Revocation (before verification: a revoked artifact is refused
    //    even if its provenance would verify).
    if revocations.is_revoked(artifact_ref) {
        return Err(ModelArtifactLoadError::ArtifactRevoked {
            artifact_ref: artifact_ref.to_string(),
        });
    }

    // 4. The provenance record must exist.
    let record = match provenance {
        Some(record) => record,
        None => return Err(ModelArtifactLoadError::Unsigned),
    };

    // 5. + 6. Verify (ref binding, algorithm, digest).
    match verify_model_artifact(record, artifact_ref, artifact, verifier) {
        ModelProvenanceVerdict::Accepted => Ok(ModelArtifactLoadAcceptance {
            artifact_ref: record.artifact_ref.clone(),
            digest_algorithm: record.digest_algorithm.clone(),
            digest_hex: record.digest_hex.clone(),
            signer_key_reference: record.signer_key_reference.clone(),
            training_data_ref: record.training_data_ref.clone(),
            training_config_ref: record.training_config_ref.clone(),
            framework_version_ref: record.framework_version_ref.clone(),
            worker_family: record.worker_family,
        }),
        ModelProvenanceVerdict::Refused { reason } => {
            // Split the ref-mismatch class into its own named error so
            // the ledger's tamper/unsigned/revoked triad stays distinct
            // from a record bound to a DIFFERENT artifact.
            if reason.starts_with(MODEL_REFUSAL_REF_MISMATCH) {
                Err(ModelArtifactLoadError::ArtifactRefMismatch {
                    record_ref: record.artifact_ref.clone(),
                    loaded_ref: artifact_ref.to_string(),
                })
            } else {
                Err(ModelArtifactLoadError::VerificationRefused { reason })
            }
        }
    }
}

// --- The audit port ---------------------------------------------------------------------

/// The kinds of model-artifact audit records (SCREAMING_SNAKE stable style;
/// every load outcome and every revocation outcome is audited).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArtifactAuditKind {
    /// A model-artifact load was accepted (all gates passed; the detail
    /// carries the linked provenance references).
    LoadAccepted,
    /// A model-artifact load was REFUSED (unsigned, revoked, tampered,
    /// ref mismatch, verifier refusal, tenant-isolation refusal — the
    /// detail names the refusal class).
    LoadRefused,
    /// A revocation was recorded (append-only register).
    RevocationRecorded,
    /// A revocation was refused (duplicate, missing reason/actor).
    RevocationRefused,
}

impl ModelArtifactAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            ModelArtifactAuditKind::LoadAccepted => "MODEL_ARTIFACT_LOAD_ACCEPTED",
            ModelArtifactAuditKind::LoadRefused => "MODEL_ARTIFACT_LOAD_REFUSED",
            ModelArtifactAuditKind::RevocationRecorded => "MODEL_ARTIFACT_REVOCATION_RECORDED",
            ModelArtifactAuditKind::RevocationRefused => "MODEL_ARTIFACT_REVOCATION_REFUSED",
        }
    }
}

/// One model-artifact audit record. `actor` is the gate or register —
/// never a worker-supplied identity — and `detail` carries reference
/// strings and refusal names only (no artifact payload, no key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactAuditRecord {
    /// When the event was recorded (injected-clock millis).
    pub timestamp_millis: i64,
    pub kind: ModelArtifactAuditKind,
    /// The emitting gate/register (audit attribution).
    pub actor: String,
    /// Human-readable detail (reference renderings and refusal names — no
    /// payload data, no key material).
    pub detail: String,
}

/// The model-artifact audit sink port (crate-local, the kernel
/// `LifecycleAuditRecord` sink posture mirrored). Implementations MUST be
/// append-only (§5.1 audit integrity).
pub trait ModelArtifactAuditSink {
    fn record(&self, record: ModelArtifactAuditRecord);
}

/// An append-only in-memory [`ModelArtifactAuditSink`] — the deterministic
/// collector used by tests and in-process compositions.
#[derive(Debug, Default)]
pub struct MemoryModelArtifactAuditSink {
    records: std::cell::RefCell<Vec<ModelArtifactAuditRecord>>,
}

impl MemoryModelArtifactAuditSink {
    pub fn new() -> MemoryModelArtifactAuditSink {
        MemoryModelArtifactAuditSink {
            records: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// The records in emission order (append-only; the borrow is brief and
    /// single-threaded).
    pub fn records(&self) -> std::cell::Ref<Vec<ModelArtifactAuditRecord>> {
        self.records.borrow()
    }
}

impl ModelArtifactAuditSink for MemoryModelArtifactAuditSink {
    fn record(&self, record: ModelArtifactAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}

// --- Shared parsing helpers (the kernel signing.rs style, local copy) ---

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn required_string(
    obj: &serde_json::Map<String, Value>,
    field: &str,
    location: &str,
) -> Result<String, ArtifactSigningError> {
    match obj.get(field) {
        None => Err(ArtifactSigningError::RecordMissingField {
            field: field.to_string(),
            location: location.to_string(),
        }),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(ArtifactSigningError::RecordWrongType {
            location: location.to_string(),
            field: field.to_string(),
            expected: "string",
            found: json_type_name(other),
        }),
    }
}

fn reject_unknown_fields(
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    location: &str,
) -> Result<(), ArtifactSigningError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(ArtifactSigningError::RecordUnknownField {
            location: location.to_string(),
            field: field.clone(),
        }),
    }
}

// --- The closed error-name table ---------------------------------------------------------

/// All stable error names of this module's two enums (sorted; the
/// pairwise-distinct property is pinned by tests, the house style). The
/// P8.31 tenant-isolation names are NOT duplicated here: the
/// [`ModelArtifactLoadError::TenantIsolation`] variant delegates to the
/// inner error's name (the alignment discipline), and those names live in
/// the P8.31 table.
pub const MODEL_ARTIFACT_SIGNING_ERROR_NAMES: [&str; 17] = [
    "MODEL_ARTIFACT_REVOCATION_ALREADY_RECORDED",
    "MODEL_ARTIFACT_REVOCATION_INVALID",
    "MODEL_ARTIFACT_UNSIGNED",
    "MODEL_ARTIFACT_VERIFICATION_REFUSED",
    "MODEL_PROVENANCE_ARTIFACT_REF_MISMATCH",
    "MODEL_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN",
    "MODEL_PROVENANCE_DIGEST_INVALID",
    "MODEL_PROVENANCE_FIELD_INVALID",
    "MODEL_PROVENANCE_MISSING_FIELD",
    "MODEL_PROVENANCE_RECORD_JSON_INVALID",
    "MODEL_PROVENANCE_RECORD_NOT_AN_OBJECT",
    "MODEL_PROVENANCE_SCHEMA_UNSUPPORTED",
    "MODEL_PROVENANCE_SIGNER_KEY_REFERENCE_INVALID",
    "MODEL_PROVENANCE_UNKNOWN_FIELD",
    "MODEL_PROVENANCE_WORKER_FAMILY_UNKNOWN",
    "MODEL_PROVENANCE_WRONG_TYPE",
    "MODEL_ARTIFACT_REVOKED",
];

/// The error names taken from the enums themselves (sorted, deduplicated
/// via `BTreeSet`) — the test-side consistency check against
/// [`MODEL_ARTIFACT_SIGNING_ERROR_NAMES`], so a variant added without
/// updating the table is a named test failure.
pub fn error_names_from_enum() -> std::collections::BTreeSet<&'static str> {
    let signing: [&str; 13] = [
        ArtifactSigningError::RecordJsonInvalid { reason: String::new() }.name(),
        ArtifactSigningError::RecordNotAnObject.name(),
        ArtifactSigningError::RecordSchemaUnsupported { found: String::new() }.name(),
        ArtifactSigningError::RecordUnknownField {
            location: String::new(),
            field: String::new(),
        }
        .name(),
        ArtifactSigningError::RecordMissingField {
            field: String::new(),
            location: String::new(),
        }
        .name(),
        ArtifactSigningError::RecordWrongType {
            location: String::new(),
            field: String::new(),
            expected: "string",
            found: "string",
        }
        .name(),
        ArtifactSigningError::RecordDigestInvalid { reason: String::new() }.name(),
        ArtifactSigningError::RecordDigestAlgorithmUnknown { value: String::new() }.name(),
        ArtifactSigningError::RecordKeyReferenceInvalid { reason: String::new() }.name(),
        ArtifactSigningError::RecordFamilyUnknown { value: String::new() }.name(),
        ArtifactSigningError::RecordFieldInvalid {
            field: String::new(),
            reason: String::new(),
        }
        .name(),
        ArtifactSigningError::RevocationInvalid { reason: String::new() }.name(),
        ArtifactSigningError::RevocationAlreadyRecorded { artifact_ref: String::new() }.name(),
    ];
    let load: [&str; 5] = [
        ModelArtifactLoadError::TenantIsolation(TenantIsolationError::ScopeInvalid {
            reason: String::new(),
        })
        .name(),
        ModelArtifactLoadError::ArtifactRevoked { artifact_ref: String::new() }.name(),
        ModelArtifactLoadError::Unsigned.name(),
        ModelArtifactLoadError::ArtifactRefMismatch {
            record_ref: String::new(),
            loaded_ref: String::new(),
        }
        .name(),
        ModelArtifactLoadError::VerificationRefused { reason: String::new() }.name(),
    ];
    // The load enum's tenant-isolation name is delegated (it belongs to
    // the P8.31 table); every other name must be in this module's table.
    let mut names: std::collections::BTreeSet<&'static str> = signing.into_iter().collect();
    for name in load {
        if name != "RESEARCH_SCOPE_INVALID" {
            names.insert(name);
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the rules above — the closed key sets, the scheme
// rules, the refusal tags — or a literal copied from this module's own
// constants. The one numeric digest expectation is DEFINITIONAL: FNV-1a 64
// over ZERO bytes is the offset basis 0xcbf29ce484222325 — no bytes are
// processed, so the digest equals the basis. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed injected clock (the injected-clock discipline — no
    /// `SystemTime` anywhere in this module).
    struct FixedClock {
        now: i64,
    }

    impl HarnessClock for FixedClock {
        fn now_millis(&self) -> i64 {
            self.now
        }
    }

    const ARTIFACT_REF: &str = "artifact://models/gnn-warmstart.v1";

    /// A well-formed record JSON (hand-built; the digest hex is filled in
    /// per test). Every field is one of the closed top-level keys.
    fn record_json(digest_hex: &str) -> serde_json::Value {
        serde_json::json!({
            "provenance-schema": MODEL_PROVENANCE_SCHEMA_ID,
            "artifact-ref": ARTIFACT_REF,
            "digest-algorithm": MODEL_DIGEST_ALGORITHM_FNV1A64_TEST,
            "digest": digest_hex,
            "signer-key-reference": "key-store://release/model-signing",
            "signed-at-millis": 1_000u64,
            "worker-family": "gnn",
            "training-data-ref": "training-data://tenant-a/ds-1",
            "training-config-ref": "config://experiments/gnn-warmstart.v1",
            "framework-version-ref": "repro://runs/run-1/framework-versions"
        })
    }

    /// A record parsed from [`record_json`], with the digest computed over
    /// `artifact` by the module's own test-grade primitive (self-consistent;
    /// the tamper tests mutate the bytes, not the record).
    fn parsed_record(artifact: &[u8]) -> ModelArtifactProvenance {
        let digest = hex_u64(fnv1a64(artifact));
        parse_model_provenance_value(&record_json(&digest)).expect("the fixture record parses")
    }

    #[test]
    fn the_test_grade_digest_of_empty_input_is_the_fnv_offset_basis() {
        // DEFINITIONAL: FNV-1a 64 processes no bytes for empty input, so
        // the digest IS the offset basis 0xcbf29ce484222325. This is the
        // hand-computable pin for the digest primitive.
        let verdict = UnsafeTestOnlyArtifactDigest.verify(&[], "cbf29ce484222325");
        assert_eq!(verdict, ModelDigestVerdict::Matches);
        // Any other 16-digit hex is a mismatch, not a refusal.
        let verdict = UnsafeTestOnlyArtifactDigest.verify(&[], "0000000000000000");
        assert_eq!(verdict, ModelDigestVerdict::Mismatch);
    }

    #[test]
    fn a_well_formed_record_parses_with_linked_provenance() {
        let record = parsed_record(b"model-artifact-bytes");
        assert_eq!(record.artifact_ref, ARTIFACT_REF);
        assert_eq!(record.digest_algorithm, MODEL_DIGEST_ALGORITHM_FNV1A64_TEST);
        assert_eq!(record.worker_family, ResearchWorkerFamily::Gnn);
        assert_eq!(record.training_data_ref, "training-data://tenant-a/ds-1");
        assert_eq!(record.training_config_ref, "config://experiments/gnn-warmstart.v1");
        assert_eq!(record.framework_version_ref, "repro://runs/run-1/framework-versions");
        assert_eq!(record.signer_key_reference, "key-store://release/model-signing");
        assert_eq!(record.signed_at_millis, 1_000);
    }

    #[test]
    fn a_wrong_schema_id_is_refused_never_parsed_permissively() {
        let mut json = record_json("cbf29ce484222325");
        json["provenance-schema"] = serde_json::json!("flexcore-plugin-provenance-v1");
        let err = parse_model_provenance_value(&json).expect_err("wrong envelope refused");
        assert_eq!(err.name(), "MODEL_PROVENANCE_SCHEMA_UNSUPPORTED");
    }

    #[test]
    fn an_unknown_field_is_refused_the_closed_key_set_holds() {
        let mut json = record_json("cbf29ce484222325");
        json["signing-key-material"] = serde_json::json!("DEADBEEF");
        let err = parse_model_provenance_value(&json).expect_err("unknown field refused");
        assert_eq!(err.name(), "MODEL_PROVENANCE_UNKNOWN_FIELD");
        assert_eq!(
            err,
            ArtifactSigningError::RecordUnknownField {
                location: "model-provenance".to_string(),
                field: "signing-key-material".to_string(),
            }
        );
    }

    #[test]
    fn a_missing_required_field_is_refused_including_the_linked_provenance() {
        // Dropping any of the three LINKED provenance references must
        // refuse: provenance is never stored apart from the signature.
        for field in ["training-data-ref", "training-config-ref", "framework-version-ref"] {
            let mut json = record_json("cbf29ce484222325");
            // Remove the key entirely to exercise the missing path.
            let obj = json.as_object_mut().expect("object");
            obj.remove(field);
            let err = parse_model_provenance_value(&json).expect_err("missing field refused");
            assert_eq!(err.name(), "MODEL_PROVENANCE_MISSING_FIELD");
            assert!(format!("{err}").contains(field));
        }
    }

    #[test]
    fn an_inline_signer_key_reference_is_refused_and_the_value_never_echoed() {
        let mut json = record_json("cbf29ce484222325");
        json["signer-key-reference"] = serde_json::json!("RAWKEY-0123456789abcdef");
        let err = parse_model_provenance_value(&json).expect_err("inline key refused");
        assert_eq!(err.name(), "MODEL_PROVENANCE_SIGNER_KEY_REFERENCE_INVALID");
        let text = format!("{err}");
        assert!(
            !text.contains("RAWKEY"),
            "a rejected key reference VALUE is never echoed back (it may be key material)"
        );
    }

    #[test]
    fn an_unknown_digest_algorithm_is_refused() {
        let mut json = record_json("cbf29ce484222325");
        json["digest-algorithm"] = serde_json::json!("md5");
        let err = parse_model_provenance_value(&json).expect_err("unknown algorithm refused");
        assert_eq!(err.name(), "MODEL_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN");
    }

    #[test]
    fn an_unknown_worker_family_is_refused() {
        let mut json = record_json("cbf29ce484222325");
        json["worker-family"] = serde_json::json!("generic");
        let err = parse_model_provenance_value(&json).expect_err("unknown family refused");
        assert_eq!(err.name(), "MODEL_PROVENANCE_WORKER_FAMILY_UNKNOWN");
    }

    #[test]
    fn the_default_verifier_refuses_everything_fail_closed() {
        // A record that would verify under the test-grade verifier STILL
        // fails under the default: the absence of signing infrastructure
        // can never look like success.
        let record = parsed_record(b"model-artifact-bytes");
        let verdict = verify_model_artifact(&record, ARTIFACT_REF, b"model-artifact-bytes", &NoArtifactVerifier);
        match &verdict {
            ModelProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(MODEL_REFUSAL_VERIFIER_REFUSED));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn tampered_bytes_are_refused_with_the_digest_mismatch_tag() {
        let record = parsed_record(b"model-artifact-bytes");
        let tampered = b"model-artifact-byteZ"; // one byte flipped
        let verdict = verify_model_artifact(&record, ARTIFACT_REF, tampered, &UnsafeTestOnlyArtifactDigest);
        match &verdict {
            ModelProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(MODEL_REFUSAL_DIGEST_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_record_bound_to_a_different_artifact_is_refused() {
        let record = parsed_record(b"model-artifact-bytes");
        let verdict = verify_model_artifact(
            &record,
            "artifact://models/OTHER.v1",
            b"model-artifact-bytes",
            &UnsafeTestOnlyArtifactDigest,
        );
        match &verdict {
            ModelProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(MODEL_REFUSAL_REF_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_algorithm_mismatch_between_record_and_verifier_is_refused() {
        // A record declaring sha-256 (accepted IN the record vocabulary)
        // against the test-grade verifier: the verifier owns the shape
        // match, so this is the ALGORITHM_MISMATCH refusal.
        let mut json = record_json(&format!("{:064x}", 0u64)); // 64 hex digits
        json["digest-algorithm"] = serde_json::json!(MODEL_DIGEST_ALGORITHM_SHA256);
        let record = parse_model_provenance_value(&json).expect("parses");
        let verdict = verify_model_artifact(&record, ARTIFACT_REF, b"model-artifact-bytes", &UnsafeTestOnlyArtifactDigest);
        match &verdict {
            ModelProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(MODEL_REFUSAL_ALGORITHM_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn revocation_requires_reason_and_actor_and_refuses_duplicates() {
        let mut register = ModelArtifactRevocationRegister::new();
        let clock = FixedClock { now: 5_000 };
        let audit = MemoryModelArtifactAuditSink::new();

        let err = register
            .revoke(ARTIFACT_REF, "  ", "security", &clock, &audit)
            .expect_err("empty reason refused");
        assert_eq!(err.name(), "MODEL_ARTIFACT_REVOCATION_INVALID");
        let err = register
            .revoke(ARTIFACT_REF, "bad artifact", "", &clock, &audit)
            .expect_err("empty actor refused");
        assert_eq!(err.name(), "MODEL_ARTIFACT_REVOCATION_INVALID");

        register
            .revoke(ARTIFACT_REF, "tampered artifact found in store", "security", &clock, &audit)
            .expect("the revocation records");
        assert!(register.is_revoked(ARTIFACT_REF));
        let err = register
            .revoke(ARTIFACT_REF, "again", "security", &clock, &audit)
            .expect_err("duplicate refused");
        assert_eq!(err.name(), "MODEL_ARTIFACT_REVOCATION_ALREADY_RECORDED");

        // Audit: two refusals then one recording, in emission order.
        let records = audit.records();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].kind, ModelArtifactAuditKind::RevocationRefused);
        assert_eq!(records[1].kind, ModelArtifactAuditKind::RevocationRefused);
        assert_eq!(records[2].kind, ModelArtifactAuditKind::RevocationRecorded);
        assert_eq!(records[2].timestamp_millis, 5_000);
    }

    #[test]
    fn the_error_name_table_equals_the_enum_names() {
        let from_table: std::collections::BTreeSet<&'static str> =
            MODEL_ARTIFACT_SIGNING_ERROR_NAMES.into_iter().collect();
        assert_eq!(from_table, error_names_from_enum());
        // Pairwise distinct (the table length equals the set size).
        assert_eq!(MODEL_ARTIFACT_SIGNING_ERROR_NAMES.len(), from_table.len());
    }
}
