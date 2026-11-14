// P7.14 — Plugin artifact signing, provenance, and SBOM metadata (report
// §9.3, §5.6 `Discovered → Verified` gate, §17.3–§17.4; ledger
// `ledger_phase_7.md` P7.14).
//
// WHAT IS IMPLEMENTED HERE: the digest-and-record layer of the plugin
// supply chain — the versioned provenance record
// (`flexcore-plugin-provenance-v1`), the pluggable [`DigestVerifier`]
// port, the fail-closed verification gate that produces the P7.07
// [`GateOutcome`] consumed by the lifecycle's `Discovered → Verified`
// transition, and the audit records for every verification and refusal.
// The design follows the P6.20 signed-update precedent
// (`ledger/phase-06-edge-protocol/20_deployment_profile.md`): the
// verification STRUCTURE is implemented; the production signing identity
// is a DEPLOYMENT REQUIREMENT, recorded honestly and not claimed.
//
// WHAT DOES NOT EXIST (the P6.20 honesty pattern, restated): no real
// cryptographic signatures, no PKI, no release keys, no revocation
// infrastructure (revocation is P7.38), and no in-crate SHA-256. A plain
// FNV-1a digest (the repo's `fnv1a64` integrity-checksum pattern, P4.28/
// P6.14) is INSUFFICIENT for supply-chain integrity and is provided ONLY
// as [`UnsafeTestOnlyDigest`], loudly labelled for tests; the crate's
// DEFAULT verifier is [`NoVerifier`], which refuses EVERYTHING
// (fail-closed). Wiring a production SHA-256 (or better) verifier through
// the [`DigestVerifier`] port at the composition root is a recorded
// DEPLOYMENT REQUIREMENT — exactly like P6.20's PKI. Nothing in this
// module warns instead of rejecting (the ledger's named risk); every
// refusal is a named outcome.
//
// Record design (the structural guarantees):
// - The signer key appears as a REFERENCE only (`signer-key-reference`,
//   scheme-prefixed like manifest secret references). The closed key set
//   makes the record STRUCTURALLY INCAPABLE of carrying key material:
//   there is no field a key could hide in, and a reference without a
//   scheme prefix is rejected as likely inline key material (the value is
//   never echoed back, mirroring the P7.05 secret-reference rule).
// - The SBOM appears as a REFERENCE only (`sbom-reference`, REQUIRED).
//   Kernel-side the SBOM is metadata; its GENERATION is P7.27's CI
//   configuration (config-only, the P6.27 pattern) — the seam is recorded
//   here, generation is NOT implemented in this module.
// - No self-upgrade / no runtime code fetch (§9.3) are enforced
//   structurally elsewhere (the kernel has no fetch port at all, and
//   plugins have no capability surface that reaches the artifact store);
//   this module contributes the gate that refuses unprovenanced
//   artifacts BEFORE any compatibility check (the P7.14 step-2 rule: an
//   unsigned or bad-signature package is rejected before compatibility).
//
// Audit: every verification and every refusal emits a
// [`LifecycleAuditRecord`] through the in-module [`AuditSink`] port
// (reused from [`crate::lifecycle`] — the kernel-local audit port feeds
// the host's audit path), with timestamps from the injected [`Clock`]
// only. Acceptance is additionally recorded by the lifecycle itself when
// the caller commits `Discovered → Verified` with the returned
// [`GateOutcome::Passed`]; refusal records here carry the attempted edge
// explicitly.
//
// Determinism: parsing is fail-closed over closed key sets with sorted
// unknown-field selection (the [`crate::manifest`] style); digests are
// compared as exact strings over injected bytes. No `unwrap`/`expect` on
// untrusted input anywhere.

use std::fmt;

use serde_json::Value;

use crate::lifecycle::{AuditSink, Clock, GateOutcome, LifecycleAuditKind, LifecycleAuditRecord, LifecycleState};
use crate::manifest::{validate_plugin_id, SemVer};

/// The versioned provenance-record envelope id (P7.14 step 1). A record
/// whose `provenance-schema` field is not exactly this value is rejected —
/// never parsed permissively.
pub const PROVENANCE_SCHEMA_ID: &str = "flexcore-plugin-provenance-v1";

/// Maximum accepted provenance-record source size in bytes (fail-closed
/// bound; a provenance record is metadata, not a payload).
pub const MAX_PROVENANCE_BYTES: usize = 64 * 1024;

/// The digest algorithm a PRODUCTION deployment must wire (DEPLOYMENT
/// REQUIREMENT — not implemented in this crate; see the module header and
/// the decision record). The name is accepted in records so the deployment
/// wiring does not need a schema change; acceptance is meaningless until a
/// real SHA-256 [`DigestVerifier`] is configured — and [`NoVerifier`] (the
/// default) refuses everything regardless of the declared algorithm.
pub const DIGEST_ALGORITHM_SHA256: &str = "sha-256";

/// The TEST-ONLY digest algorithm: FNV-1a 64 over the artifact bytes (the
/// repo's non-cryptographic integrity-checksum pattern). NOT a security
/// primitive; never acceptable outside tests. It exists so the gate,
/// mismatch, and audit paths are exercisable without adding a crypto
/// dependency to the kernel crate.
pub const DIGEST_ALGORITHM_FNV1A64_TEST: &str = "fnv1a64-test";

/// The closed digest-algorithm vocabulary for provenance records. Anything
/// else is [`SigningError::RecordDigestAlgorithmUnknown`].
pub const DIGEST_ALGORITHMS: [&str; 2] = [DIGEST_ALGORITHM_SHA256, DIGEST_ALGORITHM_FNV1A64_TEST];

/// The accepted key-reference schemes (mirrors the P7.05 secret-reference
/// scheme rule; `key-store:` is the deployment key store the production
/// signer writes references into).
pub const KEY_REFERENCE_SCHEMES: [&str; 3] = ["key-store:", "env:", "file:"];

/// The P7.14 honesty statement (recorded so tests and reviewers can pin
/// what IS and IS NOT claimed — the P4.28 statement pattern).
pub const SIGNING_STATEMENT: &str = "the P7.14 plugin supply-chain layer is a digest-and-record \
     structure: provenance records, a pluggable digest-verifier port, a fail-closed \
     Discovered-to-Verified gate, and audit on every verify/refuse. No cryptographic \
     signature, PKI, release key, or revocation (P7.38) exists; the default verifier \
     refuses everything; wiring a production SHA-256 (or stronger) verifier is a \
     DEPLOYMENT REQUIREMENT. No certification or security claim is made.";

// --- Refusal reason tags (stable; they ride the audit reason and the
// --- GateOutcome failure reason, and the tests pin them).

/// No provenance record was supplied for the artifact.
pub const REFUSAL_UNSIGNED: &str = "UNSIGNED_ARTIFACT";
/// The record's digest does not match the artifact bytes.
pub const REFUSAL_DIGEST_MISMATCH: &str = "DIGEST_MISMATCH";
/// The record names a manifest id/version other than the one verified.
pub const REFUSAL_MANIFEST_MISMATCH: &str = "MANIFEST_MISMATCH";
/// The record's declared algorithm is not the configured verifier's.
pub const REFUSAL_ALGORITHM_MISMATCH: &str = "DIGEST_ALGORITHM_MISMATCH";
/// The configured verifier refused (e.g. the fail-closed [`NoVerifier`]).
pub const REFUSAL_VERIFIER_REFUSED: &str = "VERIFIER_REFUSED";

/// The actor attributed to provenance-gate audit records.
const ACTOR_PROVENANCE_VERIFIER: &str = "provenance-verifier";

// --- The digest-verifier port ----------------------------------------------------

/// The outcome of one digest verification. Fail-closed: an implementation
/// that cannot verify reports [`DigestVerdict::Refused`], never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestVerdict {
    /// The artifact bytes match the declared digest under this verifier's
    /// algorithm.
    Matches,
    /// The artifact bytes do NOT match the declared digest.
    Mismatch,
    /// The verifier could not verify at all (not configured, backend
    /// unavailable). Never treated as a pass.
    Refused { reason: String },
}

/// The pluggable digest verifier (P7.14 step 1). The kernel defines the
/// port; the composition root injects the implementation. The DEFAULT is
/// [`NoVerifier`] (refuses everything); [`UnsafeTestOnlyDigest`] is the
/// labelled test implementation; a production SHA-256 implementation is a
/// DEPLOYMENT REQUIREMENT (module header).
pub trait DigestVerifier {
    /// The algorithm name this verifier computes (compared against the
    /// record's `digest-algorithm` field; a mismatch is a refusal).
    fn algorithm_name(&self) -> &'static str;
    /// Verify `artifact` against the declared hex digest.
    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> DigestVerdict;
}

/// The fail-closed DEFAULT verifier: refuses EVERY artifact. A deployment
/// that wires nothing verifies nothing — and nothing passes the
/// `Discovered → Verified` gate. This is the recorded safety posture: the
/// absence of signing infrastructure can never look like success.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoVerifier;

impl DigestVerifier for NoVerifier {
    fn algorithm_name(&self) -> &'static str {
        "none"
    }

    fn verify(&self, _artifact: &[u8], _declared_digest_hex: &str) -> DigestVerdict {
        DigestVerdict::Refused {
            reason: "no digest verifier is configured; verification is refused (fail-closed default; production SHA-256 wiring is a deployment requirement)"
                .to_string(),
        }
    }
}

/// The FNV-1a 64-bit hash (the repo's existing non-cryptographic pattern —
/// `crates/flexcore-application/src/dispatch_security.rs`, P4.28/P6.14;
/// deterministic, dependency-free, explicitly NOT a security primitive).
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
/// production has wired nothing — and should be treated as [`NoVerifier`].
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsafeTestOnlyDigest;

impl DigestVerifier for UnsafeTestOnlyDigest {
    fn algorithm_name(&self) -> &'static str {
        DIGEST_ALGORITHM_FNV1A64_TEST
    }

    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> DigestVerdict {
        let computed = hex_u64(fnv1a64(artifact));
        if computed == declared_digest_hex {
            DigestVerdict::Matches
        } else {
            DigestVerdict::Mismatch
        }
    }
}

// --- The provenance record ---------------------------------------------------------

/// A validated provenance record (`flexcore-plugin-provenance-v1`): the
/// digest-and-record metadata a signer attaches to a plugin artifact. The
/// signer key is a REFERENCE, never material; the SBOM is a REFERENCE to
/// the artifact whose GENERATION is P7.27 CI tooling (config-only; the
/// seam is recorded, not implemented here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRecord {
    /// The signed artifact's id (path or content address; a reference,
    /// never the artifact itself).
    pub artifact_id: String,
    /// The digest algorithm, from [`DIGEST_ALGORITHMS`] (closed).
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
    /// The manifest plugin id this record binds to (must match the
    /// manifest being verified, or the gate refuses).
    pub manifest_id: String,
    /// The manifest version this record binds to.
    pub manifest_version: SemVer,
    /// The builder/provenance metadata (e.g. the CI builder reference that
    /// produced the artifact — the reproducibility seam).
    pub builder_id: String,
    /// The SBOM artifact reference (path/id). REQUIRED: an artifact
    /// without an SBOM reference is not provenanced (§9.3; P7.27 generates
    /// the SBOM itself).
    pub sbom_reference: String,
}

impl ProvenanceRecord {
    /// The manifest identity this record binds, as `(plugin_id, version)`
    /// — the gate's expected-identity input.
    pub fn manifest_identity(&self) -> (&str, SemVer) {
        (&self.manifest_id, self.manifest_version)
    }
}

// --- Record errors -----------------------------------------------------------------

/// Every provenance-record violation is a DISTINCT named error
/// ([`SigningError::name`]). Error text never echoes a rejected key
/// reference VALUE (it may be key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    /// The source text was not valid JSON at all (also: oversize source).
    RecordJsonInvalid { reason: String },
    /// The JSON document parsed but is not an object.
    RecordNotAnObject,
    /// `provenance-schema` is present but not [`PROVENANCE_SCHEMA_ID`].
    RecordSchemaUnsupported { found: String },
    /// A field appeared that the closed key set does not declare.
    RecordUnknownField { location: String, field: String },
    /// A required field was absent.
    RecordMissingField { field: String, location: String },
    /// A field was present with the wrong JSON type.
    RecordWrongType { location: String, field: String, expected: &'static str, found: &'static str },
    /// `manifest-id` is not a valid namespaced plugin id.
    RecordManifestIdMalformed { value: String, reason: String },
    /// `manifest-version` is not a valid SemVer core.
    RecordManifestVersionInvalid { value: String, reason: String },
    /// `digest` is not a lowercase hex string of plausible length.
    RecordDigestInvalid { reason: String },
    /// `digest-algorithm` is not in [`DIGEST_ALGORITHMS`].
    RecordDigestAlgorithmUnknown { value: String },
    /// `signer-key-reference` does not name a scheme-prefixed location
    /// (likely inline key material). Deliberately does NOT echo the value.
    RecordKeyReferenceInvalid { reason: String },
    /// `sbom-reference` is absent or empty — an artifact without an SBOM
    /// reference is not provenanced (§9.3).
    RecordSbomReferenceInvalid { reason: String },
    /// A scalar field is empty or overlong (artifact-id, builder-id).
    RecordFieldInvalid { field: String, reason: String },
}

impl SigningError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            SigningError::RecordJsonInvalid { .. } => "PROVENANCE_RECORD_JSON_INVALID",
            SigningError::RecordNotAnObject => "PROVENANCE_RECORD_NOT_AN_OBJECT",
            SigningError::RecordSchemaUnsupported { .. } => "PROVENANCE_SCHEMA_UNSUPPORTED",
            SigningError::RecordUnknownField { .. } => "PROVENANCE_UNKNOWN_FIELD",
            SigningError::RecordMissingField { .. } => "PROVENANCE_MISSING_FIELD",
            SigningError::RecordWrongType { .. } => "PROVENANCE_WRONG_TYPE",
            SigningError::RecordManifestIdMalformed { .. } => "PROVENANCE_MANIFEST_ID_MALFORMED",
            SigningError::RecordManifestVersionInvalid { .. } => "PROVENANCE_MANIFEST_VERSION_INVALID",
            SigningError::RecordDigestInvalid { .. } => "PROVENANCE_DIGEST_INVALID",
            SigningError::RecordDigestAlgorithmUnknown { .. } => "PROVENANCE_DIGEST_ALGORITHM_UNKNOWN",
            SigningError::RecordKeyReferenceInvalid { .. } => "PROVENANCE_KEY_REFERENCE_INVALID",
            SigningError::RecordSbomReferenceInvalid { .. } => "PROVENANCE_SBOM_REFERENCE_INVALID",
            SigningError::RecordFieldInvalid { .. } => "PROVENANCE_FIELD_INVALID",
        }
    }
}

impl fmt::Display for SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SigningError::RecordJsonInvalid { reason } => {
                write!(f, "provenance record is not valid JSON: {reason}")
            }
            SigningError::RecordNotAnObject => write!(f, "provenance record must be a JSON object"),
            SigningError::RecordSchemaUnsupported { found } => write!(
                f,
                "unsupported provenance-schema id '{found}' (expected '{PROVENANCE_SCHEMA_ID}')"
            ),
            SigningError::RecordUnknownField { location, field } => write!(
                f,
                "unknown field '{field}' at {location} (unknown fields are rejected, not warned; the closed key set is what makes key material structurally impossible)"
            ),
            SigningError::RecordMissingField { field, location } => {
                write!(f, "missing required field '{field}' at {location}")
            }
            SigningError::RecordWrongType { location, field, expected, found } => {
                write!(f, "field '{field}' at {location} must be {expected}, found {found}")
            }
            SigningError::RecordManifestIdMalformed { value, reason } => {
                write!(f, "provenance manifest-id '{value}' is malformed: {reason}")
            }
            SigningError::RecordManifestVersionInvalid { value, reason } => {
                write!(f, "provenance manifest-version '{value}' is invalid: {reason}")
            }
            SigningError::RecordDigestInvalid { reason } => {
                write!(f, "provenance digest is invalid: {reason}")
            }
            SigningError::RecordDigestAlgorithmUnknown { value } => write!(
                f,
                "digest algorithm '{value}' is not in the closed vocabulary ({}); unknown algorithms are refused, never accepted permissively",
                DIGEST_ALGORITHMS.join(", ")
            ),
            SigningError::RecordKeyReferenceInvalid { reason } => {
                // The reference VALUE is deliberately not echoed: a
                // rejected reference may be inline key material.
                write!(f, "signer key reference is invalid: {reason}")
            }
            SigningError::RecordSbomReferenceInvalid { reason } => {
                write!(f, "SBOM reference is invalid: {reason}")
            }
            SigningError::RecordFieldInvalid { field, reason } => {
                write!(f, "field '{field}' is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for SigningError {}

// --- Record parsing (the manifest.rs validation style) ------------------------------

/// Parse and validate a provenance record from its JSON source text
/// (fail-closed: oversize input is rejected before parsing).
pub fn parse_provenance(text: &str) -> Result<ProvenanceRecord, SigningError> {
    if text.len() > MAX_PROVENANCE_BYTES {
        return Err(SigningError::RecordJsonInvalid {
            reason: format!("provenance record exceeds the {} byte limit", MAX_PROVENANCE_BYTES),
        });
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|err| SigningError::RecordJsonInvalid { reason: err.to_string() })?;
    parse_provenance_value(&value)
}

/// Parse and validate a provenance record from an already-parsed JSON
/// value (used by tests and by callers that receive structured JSON).
pub fn parse_provenance_value(value: &Value) -> Result<ProvenanceRecord, SigningError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => return Err(SigningError::RecordNotAnObject),
    };

    // 1. Envelope first.
    let schema_id = required_string(obj, "provenance-schema", "provenance")?;
    if schema_id != PROVENANCE_SCHEMA_ID {
        return Err(SigningError::RecordSchemaUnsupported { found: schema_id });
    }

    // 2. Closed top-level key set. There is NO field a signer key could
    //    hide in — key material is structurally impossible (module header).
    const ALLOWED_TOP_LEVEL: [&str; 10] = [
        "provenance-schema",
        "artifact-id",
        "digest-algorithm",
        "digest",
        "signer-key-reference",
        "signed-at-millis",
        "manifest-id",
        "manifest-version",
        "builder-id",
        "sbom-reference",
    ];
    reject_unknown_fields(obj, &ALLOWED_TOP_LEVEL, "provenance")?;

    for field in ALLOWED_TOP_LEVEL {
        if !obj.contains_key(field) {
            return Err(SigningError::RecordMissingField {
                field: field.to_string(),
                location: "provenance".to_string(),
            });
        }
    }

    // 3. Artifact identity.
    let artifact_id = required_string(obj, "artifact-id", "provenance")?;
    if artifact_id.is_empty() || artifact_id.len() > 256 {
        return Err(SigningError::RecordFieldInvalid {
            field: "artifact-id".to_string(),
            reason: "artifact-id must be a non-empty string of at most 256 characters".to_string(),
        });
    }

    // 4. Digest: closed algorithm vocabulary + lowercase hex of plausible
    //    length. Length-vs-algorithm consistency is the VERIFIER's check
    //    (it owns the algorithm's digest shape); the record only refuses
    //    strings that cannot be a digest at all.
    let digest_algorithm = required_string(obj, "digest-algorithm", "provenance")?;
    if !DIGEST_ALGORITHMS.contains(&digest_algorithm.as_str()) {
        return Err(SigningError::RecordDigestAlgorithmUnknown { value: digest_algorithm });
    }
    let digest_hex = required_string(obj, "digest", "provenance")?;
    validate_digest_hex(&digest_hex)?;

    // 5. Signer key REFERENCE (scheme-prefixed; value never echoed on
    //    rejection — it may be key material).
    let signer_key_reference = required_string(obj, "signer-key-reference", "provenance")?;
    let scheme_ok = KEY_REFERENCE_SCHEMES.iter().any(|scheme| {
        signer_key_reference
            .strip_prefix(*scheme)
            .map(|rest| !rest.is_empty())
            .unwrap_or(false)
    });
    if !scheme_ok {
        return Err(SigningError::RecordKeyReferenceInvalid {
            reason: "a signer key reference must use the key-store:, env:, or file: scheme and name a location; provenance records carry key REFERENCES only and can never carry key material"
                .to_string(),
        });
    }

    // 6. Signing time (recorded data from the signer's clock).
    let signed_at_millis = match obj.get("signed-at-millis") {
        Some(Value::Number(n)) => match n.as_u64() {
            Some(v) => v,
            None => {
                return Err(SigningError::RecordWrongType {
                    location: "provenance".to_string(),
                    field: "signed-at-millis".to_string(),
                    expected: "unsigned integer",
                    found: "number",
                })
            }
        },
        Some(other) => {
            return Err(SigningError::RecordWrongType {
                location: "provenance".to_string(),
                field: "signed-at-millis".to_string(),
                expected: "unsigned integer",
                found: json_type_name(other),
            })
        }
        None => {
            return Err(SigningError::RecordMissingField {
                field: "signed-at-millis".to_string(),
                location: "provenance".to_string(),
            })
        }
    };

    // 7. The manifest identity this record binds.
    let manifest_id = required_string(obj, "manifest-id", "provenance")?;
    validate_plugin_id(&manifest_id).map_err(|reason| SigningError::RecordManifestIdMalformed {
        value: manifest_id.clone(),
        reason,
    })?;
    let manifest_version_text = required_string(obj, "manifest-version", "provenance")?;
    let manifest_version =
        SemVer::parse(&manifest_version_text).map_err(|err| SigningError::RecordManifestVersionInvalid {
            value: manifest_version_text.clone(),
            reason: err.reason,
        })?;

    // 8. Builder/provenance metadata.
    let builder_id = required_string(obj, "builder-id", "provenance")?;
    if builder_id.is_empty() || builder_id.len() > 128 {
        return Err(SigningError::RecordFieldInvalid {
            field: "builder-id".to_string(),
            reason: "builder-id must be a non-empty string of at most 128 characters".to_string(),
        });
    }

    // 9. SBOM reference (REQUIRED — the P7.27 seam, metadata here).
    let sbom_reference = required_string(obj, "sbom-reference", "provenance")?;
    if sbom_reference.is_empty() || sbom_reference.len() > 256 {
        return Err(SigningError::RecordSbomReferenceInvalid {
            reason: "sbom-reference must be a non-empty string of at most 256 characters naming the artifact's SBOM (generation is P7.27 CI tooling; kernel-side it is metadata)"
                .to_string(),
        });
    }

    Ok(ProvenanceRecord {
        artifact_id,
        digest_algorithm,
        digest_hex,
        signer_key_reference,
        signed_at_millis,
        manifest_id,
        manifest_version,
        builder_id,
        sbom_reference,
    })
}

/// Lowercase hex, even length, 8..=512 digits (covers FNV-1a 64 at 16 and
/// SHA-256 at 64; the verifier owns the algorithm-exact shape).
fn validate_digest_hex(hex: &str) -> Result<(), SigningError> {
    if hex.len() < 8 || hex.len() > 512 || hex.len() % 2 != 0 {
        return Err(SigningError::RecordDigestInvalid {
            reason: "digest must be an even-length lowercase hex string of 8..=512 digits".to_string(),
        });
    }
    if !hex.bytes().all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b.is_ascii_hexdigit())) {
        return Err(SigningError::RecordDigestInvalid {
            reason: "digest must be lowercase hex".to_string(),
        });
    }
    Ok(())
}

// --- Verification --------------------------------------------------------------------

/// The verification outcome for one artifact against its provenance
/// record. `Accepted` is the ONLY outcome that can produce
/// [`GateOutcome::Passed`]; every refusal carries a stable reason tag
/// (the constants above).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceVerdict {
    Accepted,
    Refused { reason: String },
}

impl ProvenanceVerdict {
    pub fn refused(&self) -> bool {
        matches!(self, ProvenanceVerdict::Refused { .. })
    }
}

/// Verify one artifact against its provenance record:
///
/// 1. the record must bind the manifest being verified (id AND version) —
///    otherwise `REFUSAL_MANIFEST_MISMATCH`;
/// 2. the record's declared algorithm must be the configured verifier's —
///    otherwise `REFUSAL_ALGORITHM_MISMATCH`;
/// 3. the verifier must report [`DigestVerdict::Matches`] — a mismatch is
///    `REFUSAL_DIGEST_MISMATCH`, a verifier refusal is
///    `REFUSAL_VERIFIER_REFUSED`.
///
/// Pure: no clock, no audit, no state — the gate wrapper below adds the
/// side effects.
pub fn verify_artifact(
    record: &ProvenanceRecord,
    artifact: &[u8],
    expected_manifest_id: &str,
    expected_manifest_version: SemVer,
    verifier: &dyn DigestVerifier,
) -> ProvenanceVerdict {
    if record.manifest_id != expected_manifest_id || record.manifest_version != expected_manifest_version {
        return ProvenanceVerdict::Refused {
            reason: format!(
                "{REFUSAL_MANIFEST_MISMATCH}: the provenance record binds manifest '{}', not the manifest being verified ('{}' {})",
                record.manifest_id, expected_manifest_id, expected_manifest_version
            ),
        };
    }
    if record.digest_algorithm != verifier.algorithm_name() {
        return ProvenanceVerdict::Refused {
            reason: format!(
                "{REFUSAL_ALGORITHM_MISMATCH}: the record declares '{}' but the configured verifier computes '{}'",
                record.digest_algorithm,
                verifier.algorithm_name()
            ),
        };
    }
    match verifier.verify(artifact, &record.digest_hex) {
        DigestVerdict::Matches => ProvenanceVerdict::Accepted,
        DigestVerdict::Mismatch => ProvenanceVerdict::Refused {
            reason: format!(
                "{REFUSAL_DIGEST_MISMATCH}: the artifact bytes do not match the record's declared digest under '{}'",
                record.digest_algorithm
            ),
        },
        DigestVerdict::Refused { reason } => ProvenanceVerdict::Refused {
            reason: format!("{REFUSAL_VERIFIER_REFUSED}: {reason}"),
        },
    }
}

/// The `Discovered → Verified` provenance gate (P7.14 step 2): consumes the
/// P7.07 types additively and produces the [`GateOutcome`] the lifecycle's
/// [`crate::lifecycle::PluginLifecycle::verify`] consumes. An artifact with
/// NO record is refused before anything else (the P7.14 rule: an unsigned
/// package is rejected before any compatibility check); a refusal is
/// audited here AND produces `GateOutcome::Failed`, so the lifecycle
/// refuses the transition with the gate outcome (fail-closed, state
/// unchanged) — a signature verification never warns instead of
/// rejecting (the ledger's named risk).
///
/// Audit: the acceptance and every refusal emit a [`LifecycleAuditRecord`]
/// through the in-module [`AuditSink`] port with a timestamp from the
/// injected [`Clock`]. The refusal records carry the attempted
/// `Discovered → Verified` edge explicitly.
pub fn provenance_gate(
    record: Option<&ProvenanceRecord>,
    artifact: &[u8],
    expected_manifest_id: &str,
    expected_manifest_version: SemVer,
    verifier: &dyn DigestVerifier,
    clock: &dyn Clock,
    audit: &dyn AuditSink,
) -> GateOutcome {
    let record = match record {
        Some(record) => record,
        None => {
            let reason = format!(
                "{REFUSAL_UNSIGNED}: no provenance record was supplied; an unsigned artifact is rejected before any compatibility check (report §9.3)"
            );
            emit_gate_record(
                audit,
                clock,
                expected_manifest_id,
                &expected_manifest_version.to_string(),
                LifecycleAuditKind::Refused,
                &reason,
            );
            return GateOutcome::Failed { reason };
        }
    };
    match verify_artifact(record, artifact, expected_manifest_id, expected_manifest_version, verifier) {
        ProvenanceVerdict::Accepted => {
            let reason = format!(
                "provenance verified: artifact '{}' digest '{}' under '{}' (signer key reference '{}'; SBOM '{}'; builder '{}')",
                record.artifact_id,
                record.digest_hex,
                record.digest_algorithm,
                record.signer_key_reference,
                record.sbom_reference,
                record.builder_id,
            );
            emit_gate_record(
                audit,
                clock,
                expected_manifest_id,
                &expected_manifest_version.to_string(),
                LifecycleAuditKind::Transition,
                &reason,
            );
            GateOutcome::Passed
        }
        ProvenanceVerdict::Refused { reason } => {
            emit_gate_record(
                audit,
                clock,
                expected_manifest_id,
                &expected_manifest_version.to_string(),
                LifecycleAuditKind::Refused,
                &reason,
            );
            GateOutcome::Failed { reason }
        }
    }
}

/// Emit one gate audit record (plugin identity = the manifest being
/// verified; from/to name the attempted `Discovered → Verified` edge).
fn emit_gate_record(
    audit: &dyn AuditSink,
    clock: &dyn Clock,
    manifest_id: &str,
    manifest_version: &str,
    kind: LifecycleAuditKind,
    reason: &str,
) {
    let record = LifecycleAuditRecord {
        kind,
        plugin_id: manifest_id.to_string(),
        version: manifest_version.to_string(),
        from: Some(LifecycleState::Discovered),
        to: Some(LifecycleState::Verified),
        actor: ACTOR_PROVENANCE_VERIFIER.to_string(),
        reason: reason.to_string(),
        timestamp_millis: clock.now_millis(),
    };
    audit.record(record);
}

// --- Shared parsing helpers (the manifest.rs style, local copies) -------------------

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
) -> Result<String, SigningError> {
    match obj.get(field) {
        None => Err(SigningError::RecordMissingField {
            field: field.to_string(),
            location: location.to_string(),
        }),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(SigningError::RecordWrongType {
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
) -> Result<(), SigningError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(SigningError::RecordUnknownField {
            location: location.to_string(),
            field: field.clone(),
        }),
    }
}
