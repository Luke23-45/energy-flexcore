// P15.01 — The forecasting architecture record: the forecasts-as-inputs
//         boundary (ADR-010 extension as an architecture record), the
//         forecast artifact types and horizons, and the provenance +
//         signing mirror of the P8.32 model-artifact signing layer.
//
// Phase 15 plan §2 row P15.01; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 01_p15_01_forecasting_architecture.md`.
// Sources: [S-41] (GE Vernova GridOS DERMS forecasting model library),
// [S-50] (Tesla Autobidder/Opticaster 15-minute price forecasts),
// [S-51] (GridBeyond price + consumption forecasts) — the commercial
// evidence that forecasting is table stakes; none of these vendors is a
// FlexCore source or integration.
//
// THE BOUNDARY, STRUCTURALLY:
//
// 1. **Forecasts are INPUTS, never commands** (ADR-010 extension, program
//    conventions §6.3). Every forecast artifact, model, and score this
//    phase produces is an INPUT, RECORD, or PROPOSAL. There is no
//    function in this module that constructs, mutates, or carries a
//    dispatch command; the artifact envelope's closed field vocabulary
//    (below) cannot express one, and the permission vocabulary a forecast
//    producer may hold (`telemetry.read`, `forecast.publish`) has no
//    dispatch member. The never-granted permission set is mirrored here
//    VERBATIM and pinned byte-identical by a test (it lives in
//    `crates/flexcore-kernel/src/manifest.rs`; the mirror discipline is
//    recorded, and a change on either side is a deliberate, recorded seam
//    update — never a silent drift).
// 2. **THE ARTIFACT TYPES** (closed): load, PV generation, and price —
//    per site or per portfolio — over day-ahead or intra-day horizons.
//    The price kind is an INGESTION seam only (P15.04): this platform
//    produces no price forecast in this phase (P15-E-2 OPEN; a vendor
//    forecast series is ingested DATA, never a bid input — ADR-011
//    boundary unchanged).
// 3. **THE PROVENANCE + SIGNING MIRROR** (mirror by citation, NO
//    flexcore-research dependency — the manifest policy of this crate):
//    this module MIRRORS the type shape and enforcement rules of
//    `crates/flexcore-research/src/artifact_signing.rs` (P8.32): a
//    versioned provenance record binding (artifact ref, digest, digest
//    algorithm, signer key REFERENCE, signing time) WITH the provenance
//    LINKED TO THE SIGNATURE as REQUIRED record fields (input-data ref,
//    producing-model ref, eval-set ref — a record without them does not
//    parse, so provenance cannot silently detach), a fail-closed default
//    verifier that refuses EVERYTHING, a loudly-labelled FNV-1a 64
//    test-grade digest (NOT a security primitive), and distinct named
//    refusal tags. The application crate imports no serialization
//    library, so the mirror is TYPED (constructed from validated fields),
//    not a JSON parse; the JSON record path remains the research crate's
//    pattern, and the composition root may implement it over this shape.
// 4. **NO SKILL, EVER, UNTIL EVALUATED**: no artifact here claims
//    accuracy or skill; the evaluation framework (P15.05) owns every
//    result cell, and every cell starts EMPTY.
//
// Control status: records and inputs for the simulation-only 0.x line.
// Nothing here authorizes, issues, approves, or dispatches anything; a
// verified forecast artifact is a DATA-INTEGRITY fact, never a dispatch
// authority. No safety, certification, or field-authority claim is made.

use std::fmt;

// --- Honesty / boundary statements ---------------------------------------------------

/// The P15.01 architecture statement (pinned by tests; the P4.28/P8.32
/// statement pattern).
pub const FORECASTING_ARCHITECTURE_STATEMENT: &str = "forecasting in FlexCore produces INPUTS, \
     RECORDS, and PROPOSALS — never commands (ADR-010 extension, program conventions 6.3). \
     Forecast artifacts are load, PV-generation, and price series per site or per portfolio \
     over day-ahead or intra-day horizons; every artifact carries a versioned provenance \
     record with the provenance LINKED to the signature; a forecast artifact is verified \
     against a fail-closed digest gate before use. No forecast claims skill: every \
     evaluation cell is EMPTY until an X-8-gated evaluation run fills it (P15.05). Nothing \
     here authorizes, issues, or dispatches anything.";

/// The permission classes a forecast producer may hold (closed; from the
/// `forecast-worker` manifest's permission set,
/// `plugins/manifests/forecast-worker.manifest.json`: NO dispatch
/// permission). This is the exact producer vocabulary of the ADR-010
/// extension — anything else is refused.
pub const FORECAST_PRODUCER_PERMISSIONS: [&str; 2] = ["telemetry.read", "forecast.publish"];

/// The never-granted permission set, mirrored VERBATIM from
/// `crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS`
/// (alignment seam — kernel side untouched). A test pins this constant
/// byte-identical to the mirror's spelling.
pub const NEVER_GRANTED_PERMISSIONS_MIRROR: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];

/// The alignment seam statement (recorded — do not drift silently; the
/// same discipline as the research crate's kernel-signing mirror).
pub const KERNEL_PERMISSION_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS and \
     apps/plugin-host/src/broker.rs::ALLOWED_PERMISSIONS/NEVER_GRANTED_PERMISSIONS BY \
     CITATION; kernel and host unchanged; no flexcore-kernel dependency";

// --- Closed vocabularies: artifact kinds, scopes, horizons ----------------------------

/// The forecast artifact kind (closed). The price kind exists so a price
/// series can be REFERENCED and PROVENANCED in the artifact vocabulary;
/// the platform produces no price forecast in this phase (P15-E-2 OPEN)
/// and a price series is never a bid input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastArtifactKind {
    /// Forecast load (active power) — MW, consumption negative
    /// (`P > 0` export, §10.2).
    Load,
    /// Forecast PV generation (active power) — MW, non-negative (export).
    PvGeneration,
    /// A price series reference (ingested DATA; P15.04 seam; never a bid
    /// input, never a platform accuracy claim).
    Price,
}

impl ForecastArtifactKind {
    /// The wire spelling of the kind (records and audit details).
    pub const fn wire_name(self) -> &'static str {
        match self {
            ForecastArtifactKind::Load => "load",
            ForecastArtifactKind::PvGeneration => "pv_generation",
            ForecastArtifactKind::Price => "price",
        }
    }

    /// The canonical unit of the kind's values (§10.2; the price kind
    /// carries a currency per MWh declared PER SERIES — see
    /// `flexcore-ports::price_signal` — never a platform default).
    pub const fn unit(self) -> &'static str {
        match self {
            ForecastArtifactKind::Load => "MW",
            ForecastArtifactKind::PvGeneration => "MW",
            ForecastArtifactKind::Price => "currency/MWh (declared per series)",
        }
    }
}

/// The scope a forecast artifact covers (closed). Site-scoped and
/// portfolio-scoped artifacts are distinct records — a portfolio artifact
/// is never silently derived from site artifacts (the constrained-
/// aggregation rule of the P3.12 envelope applies at the consumer, not
/// here).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ForecastScope {
    /// One site.
    PerSite { site_id: String },
    /// One portfolio (whole-fleet aggregation).
    PerPortfolio { portfolio_id: String },
}

impl ForecastScope {
    /// The scope's reference string (records, lineage keys).
    pub fn ref_string(&self) -> String {
        match self {
            ForecastScope::PerSite { site_id } => format!("site:{site_id}"),
            ForecastScope::PerPortfolio { portfolio_id } => {
                format!("portfolio:{portfolio_id}")
            }
        }
    }

    /// Whether the scope reference is non-empty (validation helper).
    pub fn is_valid(&self) -> bool {
        match self {
            ForecastScope::PerSite { site_id } => !site_id.trim().is_empty(),
            ForecastScope::PerPortfolio { portfolio_id } => !portfolio_id.trim().is_empty(),
        }
    }
}

/// The forecast horizon class (closed): day-ahead or intra-day. A third
/// "long-term" class does NOT exist in this phase — adding one is a
/// deliberate vocabulary extension, never a string field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastHorizon {
    /// Day-ahead: the next delivery day, published before the market
    /// gate-closure convention of the deployment (the exact schedule is a
    /// deployment setting, never assumed here).
    DayAhead,
    /// Intra-day: rolling shorter-horizon updates within the delivery day.
    IntraDay,
}

impl ForecastHorizon {
    /// The wire spelling of the horizon.
    pub const fn wire_name(self) -> &'static str {
        match self {
            ForecastHorizon::DayAhead => "day_ahead",
            ForecastHorizon::IntraDay => "intra_day",
        }
    }
}

// --- The provenance + signing mirror (P8.32 shape, typed) -----------------------------

/// The versioned provenance-record envelope id. A record built with a
/// different schema id is refused — never accepted permissively.
pub const FORECAST_PROVENANCE_SCHEMA_ID: &str = "flexcore-forecast-artifact-provenance-v1";

/// The digest algorithm a PRODUCTION deployment must wire (DEPLOYMENT
/// REQUIREMENT — not implemented in this crate; the same posture as the
/// P8.32 record: no crypto crate is added in this wave). The name is
/// accepted in records so deployment wiring needs no schema change; the
/// default verifier refuses everything regardless.
pub const FORECAST_DIGEST_ALGORITHM_SHA256: &str = "sha-256";

/// The TEST-ONLY digest algorithm: FNV-1a 64 over the artifact bytes (the
/// repo's non-cryptographic integrity-checksum pattern). NOT a security
/// primitive; never acceptable outside tests.
pub const FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST: &str = "fnv1a64-test";

/// The closed digest-algorithm vocabulary for forecast provenance records.
pub const FORECAST_DIGEST_ALGORITHMS: [&str; 2] =
    [FORECAST_DIGEST_ALGORITHM_SHA256, FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST];

/// The accepted signer-key-reference schemes (mirrors the P8.32 record's
/// scheme set; the value is never echoed back on rejection — it may be
/// key material, the P7.05 secret-reference rule).
pub const FORECAST_SIGNER_KEY_REFERENCE_SCHEMES: [&str; 3] = ["key-store:", "env:", "file:"];

/// Maximum reference/id length in a provenance record (the crate's
/// 256-character id bound posture).
pub const MAX_FORECAST_REF_CHARS: usize = 256;

/// A validated forecast-artifact provenance record
/// (`flexcore-forecast-artifact-provenance-v1`), mirroring the P8.32
/// `ModelArtifactProvenance` shape BY CITATION: the digest-and-record
/// metadata a signer attaches to a forecast artifact. The signer key is a
/// REFERENCE, never material; the LINKED provenance (input data,
/// producing model, eval set) is REQUIRED — a record without it cannot be
/// constructed, so provenance cannot silently detach from the signature.
///
/// TYPED construction (this crate imports no serialization library): the
/// JSON record path is the research crate's pattern; the composition root
/// may parse JSON and call [`ForecastArtifactProvenance::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastArtifactProvenance {
    /// The provenance-schema id (must be exactly
    /// [`FORECAST_PROVENANCE_SCHEMA_ID`]).
    pub provenance_schema: String,
    /// The artifact reference this record binds (exact; compared against
    /// the artifact being loaded).
    pub artifact_ref: String,
    /// The artifact kind (closed vocabulary).
    pub artifact_kind: ForecastArtifactKind,
    /// The artifact scope (site or portfolio).
    pub artifact_scope: ForecastScope,
    /// The horizon class (closed vocabulary).
    pub horizon: ForecastHorizon,
    /// The forecast version string (non-empty, whitespace-free — the
    /// P3.31 `ForecastVersion` discipline applied to real artifacts).
    pub forecast_version: String,
    /// The digest algorithm, from [`FORECAST_DIGEST_ALGORITHMS`] (closed).
    pub digest_algorithm: String,
    /// The artifact digest as lowercase hex (the verifier compares this
    /// against its own computation over the artifact bytes).
    pub digest_hex: String,
    /// The signer key REFERENCE (scheme-prefixed). Never key material.
    pub signer_key_reference: String,
    /// Signing time in milliseconds of the SIGNER's clock (recorded data;
    /// the verifier never derives it).
    pub signed_at_millis: u64,
    /// The input-data reference — LINKED provenance, REQUIRED (the
    /// telemetry/weather/price data the forecast was produced from).
    pub input_data_ref: String,
    /// The producing-model reference — LINKED provenance, REQUIRED (the
    /// model registry's reference, P15.06; for statistical baselines the
    /// frozen baseline record's reference).
    pub model_ref: String,
    /// The frozen evaluation-set reference — LINKED provenance, REQUIRED
    /// (the P15.05 eval-set version id; the record names the set the
    /// producing model is evaluated against, EMPTY results and all).
    pub eval_set_ref: String,
}

/// Why a provenance record was refused. Every variant is a DISTINCT named
/// error. Error text never echoes a rejected signer-key-reference VALUE
/// (it may be key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastProvenanceError {
    /// `provenance_schema` is not [`FORECAST_PROVENANCE_SCHEMA_ID`].
    SchemaUnsupported { found: String },
    /// A reference/scalar field is empty or overlong.
    FieldInvalid { field: &'static str, reason: String },
    /// The forecast version contained whitespace (the P3.31 rule).
    ForecastVersionHasWhitespace,
    /// `digest_algorithm` is not in [`FORECAST_DIGEST_ALGORITHMS`].
    DigestAlgorithmUnknown { value: String },
    /// `digest_hex` is not a lowercase hex string of plausible length.
    DigestHexInvalid { reason: String },
    /// `signer_key_reference` does not name a scheme-prefixed location
    /// (likely inline key material). Deliberately does NOT echo the value.
    SignerKeyReferenceInvalid { reason: String },
    /// The scope reference (site or portfolio id) was empty.
    EmptyScopeRef,
}

impl ForecastProvenanceError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ForecastProvenanceError::SchemaUnsupported { .. } => {
                "FORECAST_PROVENANCE_SCHEMA_UNSUPPORTED"
            }
            ForecastProvenanceError::FieldInvalid { .. } => "FORECAST_PROVENANCE_FIELD_INVALID",
            ForecastProvenanceError::ForecastVersionHasWhitespace => {
                "FORECAST_PROVENANCE_VERSION_HAS_WHITESPACE"
            }
            ForecastProvenanceError::DigestAlgorithmUnknown { .. } => {
                "FORECAST_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN"
            }
            ForecastProvenanceError::DigestHexInvalid { .. } => {
                "FORECAST_PROVENANCE_DIGEST_HEX_INVALID"
            }
            ForecastProvenanceError::SignerKeyReferenceInvalid { .. } => {
                "FORECAST_PROVENANCE_SIGNER_KEY_REFERENCE_INVALID"
            }
            ForecastProvenanceError::EmptyScopeRef => "FORECAST_PROVENANCE_EMPTY_SCOPE_REF",
        }
    }
}

impl fmt::Display for ForecastProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForecastProvenanceError::SchemaUnsupported { found } => write!(
                f,
                "unsupported provenance-schema id '{found}' (expected \
                 '{FORECAST_PROVENANCE_SCHEMA_ID}')"
            ),
            ForecastProvenanceError::FieldInvalid { field, reason } => {
                write!(f, "field '{field}' is invalid: {reason}")
            }
            ForecastProvenanceError::ForecastVersionHasWhitespace => write!(
                f,
                "the forecast version must not contain whitespace (the P3.31 version rule \
                 applies to real forecast artifacts)"
            ),
            ForecastProvenanceError::DigestAlgorithmUnknown { value } => write!(
                f,
                "digest algorithm '{value}' is not in the closed vocabulary ({}); unknown \
                 algorithms are refused, never accepted permissively",
                FORECAST_DIGEST_ALGORITHMS.join(", ")
            ),
            ForecastProvenanceError::DigestHexInvalid { reason } => {
                write!(f, "forecast provenance digest is invalid: {reason}")
            }
            ForecastProvenanceError::SignerKeyReferenceInvalid { reason } => write!(
                f,
                "signer key reference is invalid: {reason}"
            ),
            ForecastProvenanceError::EmptyScopeRef => {
                write!(f, "the artifact scope must carry a non-empty site or portfolio id")
            }
        }
    }
}

impl std::error::Error for ForecastProvenanceError {}

impl ForecastArtifactProvenance {
    /// Validates and constructs one provenance record (fail-closed; the
    /// typed shape makes the three LINKED provenance fields mandatory by
    /// construction — this constructor re-validates their content).
    pub fn new(
        artifact_ref: String,
        artifact_kind: ForecastArtifactKind,
        artifact_scope: ForecastScope,
        horizon: ForecastHorizon,
        forecast_version: String,
        digest_algorithm: String,
        digest_hex: String,
        signer_key_reference: String,
        signed_at_millis: u64,
        input_data_ref: String,
        model_ref: String,
        eval_set_ref: String,
    ) -> Result<Self, ForecastProvenanceError> {
        let record = Self {
            provenance_schema: FORECAST_PROVENANCE_SCHEMA_ID.to_string(),
            artifact_ref,
            artifact_kind,
            artifact_scope,
            horizon,
            forecast_version,
            digest_algorithm,
            digest_hex,
            signer_key_reference,
            signed_at_millis,
            input_data_ref,
            model_ref,
            eval_set_ref,
        };
        record.validate()?;
        Ok(record)
    }

    /// Full validation (also usable on a deserialized record): schema id,
    /// refs, version, scope, digest algorithm + hex, signer-key scheme.
    pub fn validate(&self) -> Result<(), ForecastProvenanceError> {
        if self.provenance_schema != FORECAST_PROVENANCE_SCHEMA_ID {
            return Err(ForecastProvenanceError::SchemaUnsupported {
                found: self.provenance_schema.clone(),
            });
        }
        validate_ref(&self.artifact_ref, "artifact-ref")?;
        if !self.artifact_scope.is_valid() {
            return Err(ForecastProvenanceError::EmptyScopeRef);
        }
        if self.forecast_version.trim().is_empty() {
            return Err(ForecastProvenanceError::FieldInvalid {
                field: "forecast-version",
                reason: "must be non-empty".to_string(),
            });
        }
        if self.forecast_version.chars().any(|c| c.is_whitespace()) {
            return Err(ForecastProvenanceError::ForecastVersionHasWhitespace);
        }
        if !FORECAST_DIGEST_ALGORITHMS.contains(&self.digest_algorithm.as_str()) {
            return Err(ForecastProvenanceError::DigestAlgorithmUnknown {
                value: self.digest_algorithm.clone(),
            });
        }
        validate_digest_hex(&self.digest_hex)?;
        let scheme_ok = FORECAST_SIGNER_KEY_REFERENCE_SCHEMES.iter().any(|scheme| {
            self.signer_key_reference
                .strip_prefix(*scheme)
                .map(|rest| !rest.is_empty())
                .unwrap_or(false)
        });
        if !scheme_ok {
            return Err(ForecastProvenanceError::SignerKeyReferenceInvalid {
                reason: "a signer key reference must use the key-store:, env:, or file: \
                         scheme and name a location; forecast provenance records carry key \
                         REFERENCES only and can never carry key material"
                    .to_string(),
            });
        }
        validate_ref(&self.input_data_ref, "input-data-ref")?;
        validate_ref(&self.model_ref, "model-ref")?;
        validate_ref(&self.eval_set_ref, "eval-set-ref")?;
        Ok(())
    }
}

/// A provenance reference/id: non-empty, at most 256 characters.
fn validate_ref(value: &str, field: &'static str) -> Result<(), ForecastProvenanceError> {
    if value.trim().is_empty() || value.len() > MAX_FORECAST_REF_CHARS {
        return Err(ForecastProvenanceError::FieldInvalid {
            field,
            reason: format!(
                "{field} must be a non-empty string of at most {MAX_FORECAST_REF_CHARS} characters"
            ),
        });
    }
    Ok(())
}

/// Lowercase hex, even length, 8..=512 digits (the P8.32 record's shape
/// rule, mirrored; the verifier owns the algorithm-exact length).
fn validate_digest_hex(hex: &str) -> Result<(), ForecastProvenanceError> {
    if hex.len() < 8 || hex.len() > 512 || hex.len() % 2 != 0 {
        return Err(ForecastProvenanceError::DigestHexInvalid {
            reason: "digest must be an even-length lowercase hex string of 8..=512 digits"
                .to_string(),
        });
    }
    if !hex
        .bytes()
        .all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b.is_ascii_hexdigit()))
    {
        return Err(ForecastProvenanceError::DigestHexInvalid {
            reason: "digest must be lowercase hex".to_string(),
        });
    }
    Ok(())
}

// --- The fail-closed digest gate (the P8.32 verifier posture, mirrored) --

/// The outcome of one digest verification. Fail-closed: an implementation
/// that cannot verify reports [`ForecastDigestVerdict::Refused`], never a
/// guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastDigestVerdict {
    /// The artifact bytes match the declared digest under this verifier's
    /// algorithm.
    Matches,
    /// The artifact bytes do NOT match the declared digest.
    Mismatch,
    /// The verifier could not verify at all. Never treated as a pass.
    Refused { reason: String },
}

/// The pluggable digest verifier for forecast artifacts (the P8.32 port,
/// mirrored). This module defines the port; the composition root injects
/// the implementation. The DEFAULT is [`NoForecastVerifier`] (refuses
/// everything); [`UnsafeTestOnlyForecastDigest`] is the labelled test
/// implementation; a production SHA-256 implementation is a DEPLOYMENT
/// REQUIREMENT (module header; decision record).
pub trait ForecastDigestVerifier {
    /// The algorithm name this verifier computes (compared against the
    /// record's `digest_algorithm`; a mismatch is a refusal).
    fn algorithm_name(&self) -> &'static str;
    /// Verify `artifact` against the declared hex digest.
    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> ForecastDigestVerdict;
}

/// The fail-closed DEFAULT verifier: refuses EVERY artifact. The absence
/// of signing infrastructure can never look like success (the P8.32 /
/// P7.14 posture, mirrored).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoForecastVerifier;

impl ForecastDigestVerifier for NoForecastVerifier {
    fn algorithm_name(&self) -> &'static str {
        "none"
    }

    fn verify(&self, _artifact: &[u8], _declared_digest_hex: &str) -> ForecastDigestVerdict {
        ForecastDigestVerdict::Refused {
            reason: "no digest verifier is configured; verification is refused \
                     (fail-closed default; production SHA-256 wiring is a deployment \
                     requirement recorded in the P15.01 decision record)"
                .to_string(),
        }
    }
}

/// The FNV-1a 64-bit hash (the repo's non-cryptographic pattern).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Lowercase hex of a `u64` (16 digits).
fn hex_u64(value: u64) -> String {
    format!("{value:016x}")
}

/// The TEST-ONLY digest verifier: FNV-1a 64 over the artifact bytes.
/// UNSAFE FOR SUPPLY-CHAIN INTEGRITY by construction. Exists ONLY so the
/// verification structure, tamper path, and refusal tags are testable
/// without a crypto dependency. A deployment that wires this in
/// production has wired nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsafeTestOnlyForecastDigest;

impl ForecastDigestVerifier for UnsafeTestOnlyForecastDigest {
    fn algorithm_name(&self) -> &'static str {
        FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST
    }

    fn verify(&self, artifact: &[u8], declared_digest_hex: &str) -> ForecastDigestVerdict {
        let computed = hex_u64(fnv1a64(artifact));
        if computed == declared_digest_hex {
            ForecastDigestVerdict::Matches
        } else {
            ForecastDigestVerdict::Mismatch
        }
    }
}

// --- Refusal reason tags (stable; the tests pin them) --------------------------------

/// No provenance record was supplied for the artifact (the unsigned case).
pub const FORECAST_REFUSAL_UNSIGNED: &str = "FORECAST_ARTIFACT_UNSIGNED";
/// The record's `artifact-ref` does not name the artifact being loaded.
pub const FORECAST_REFUSAL_REF_MISMATCH: &str = "FORECAST_ARTIFACT_REF_MISMATCH";
/// The record's declared algorithm is not the configured verifier's.
pub const FORECAST_REFUSAL_ALGORITHM_MISMATCH: &str = "FORECAST_DIGEST_ALGORITHM_MISMATCH";
/// The record's digest does not match the artifact bytes.
pub const FORECAST_REFUSAL_DIGEST_MISMATCH: &str = "FORECAST_DIGEST_MISMATCH";
/// The configured verifier refused (e.g. the fail-closed [`NoForecastVerifier`]).
pub const FORECAST_REFUSAL_VERIFIER_REFUSED: &str = "FORECAST_VERIFIER_REFUSED";

/// The verification outcome for one forecast artifact against its
/// provenance record. `Accepted` is the ONLY outcome that lets a
/// verification pass; every refusal carries a stable reason tag. A
/// verified artifact is a DATA-INTEGRITY fact — never a dispatch
/// authority (module header; ADR-010 extension).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastProvenanceVerdict {
    Accepted,
    Refused { reason: String },
}

/// Verify one forecast artifact against its provenance record:
///
/// 1. the record must bind the artifact being loaded (exact ref) —
///    otherwise `FORECAST_ARTIFACT_REF_MISMATCH`;
/// 2. the record's declared algorithm must be the configured verifier's —
///    otherwise `FORECAST_DIGEST_ALGORITHM_MISMATCH`;
/// 3. the verifier must report [`ForecastDigestVerdict::Matches`] — a
///    mismatch is `FORECAST_DIGEST_MISMATCH`, a verifier refusal is
///    `FORECAST_VERIFIER_REFUSED`.
///
/// Pure: no clock, no audit, no state — the composition root's
/// publication gate adds the side effects.
pub fn verify_forecast_artifact(
    record: &ForecastArtifactProvenance,
    expected_artifact_ref: &str,
    artifact: &[u8],
    verifier: &dyn ForecastDigestVerifier,
) -> ForecastProvenanceVerdict {
    if record.artifact_ref != expected_artifact_ref {
        return ForecastProvenanceVerdict::Refused {
            reason: format!(
                "{FORECAST_REFUSAL_REF_MISMATCH}: the provenance record binds artifact '{}', \
                 not the artifact being loaded ('{}')",
                record.artifact_ref, expected_artifact_ref
            ),
        };
    }
    if record.digest_algorithm != verifier.algorithm_name() {
        return ForecastProvenanceVerdict::Refused {
            reason: format!(
                "{FORECAST_REFUSAL_ALGORITHM_MISMATCH}: the record declares '{}' but the \
                 configured verifier computes '{}'",
                record.digest_algorithm,
                verifier.algorithm_name()
            ),
        };
    }
    match verifier.verify(artifact, &record.digest_hex) {
        ForecastDigestVerdict::Matches => ForecastProvenanceVerdict::Accepted,
        ForecastDigestVerdict::Mismatch => ForecastProvenanceVerdict::Refused {
            reason: format!(
                "{FORECAST_REFUSAL_DIGEST_MISMATCH}: the artifact bytes do not match the \
                 record's declared digest under '{}' — a tampered artifact is refused, \
                 never trusted",
                record.digest_algorithm
            ),
        },
        ForecastDigestVerdict::Refused { reason } => ForecastProvenanceVerdict::Refused {
            reason: format!("{FORECAST_REFUSAL_VERIFIER_REFUSED}: {reason}"),
        },
    }
}

// --- The producer permission gate ------------------------------------------------------

/// Why a forecast-producer permission declaration was refused. Named,
/// matchable; error text never echoes a never-granted VALUE back in a
/// command-shaped context (the refusal names the class, not a grant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastPermissionError {
    /// A permission outside the closed producer vocabulary was requested.
    PermissionOutsideProducerVocabulary { requested: String },
    /// A never-granted permission was requested (structurally impossible
    /// through any manifest, profile, or broker call — mirrored verbatim
    /// from the kernel's set).
    NeverGranted { requested: String },
}

impl ForecastPermissionError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ForecastPermissionError::PermissionOutsideProducerVocabulary { .. } => {
                "FORECAST_PERMISSION_OUTSIDE_PRODUCER_VOCABULARY"
            }
            ForecastPermissionError::NeverGranted { .. } => "FORECAST_PERMISSION_NEVER_GRANTED",
        }
    }
}

impl fmt::Display for ForecastPermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForecastPermissionError::PermissionOutsideProducerVocabulary { requested } => {
                write!(
                    f,
                    "permission '{requested}' is outside the forecast-producer vocabulary \
                     ({}) — a forecast producer has no dispatch reach (ADR-010 extension)",
                    FORECAST_PRODUCER_PERMISSIONS.join(", ")
                )
            }
            ForecastPermissionError::NeverGranted { requested } => write!(
                f,
                "permission '{requested}' is NEVER granted to any plugin — not grantable \
                 through any manifest, profile, or broker call (the kernel's closed set)"
            ),
        }
    }
}

impl std::error::Error for ForecastPermissionError {}

/// Validates one forecast producer's requested permission list (fail-
/// closed, first refusal wins; the never-granted check runs FIRST so a
/// never-granted request is never mislabeled as merely out-of-vocabulary).
pub fn validate_forecast_producer_permissions(
    requested: &[&str],
) -> Result<(), ForecastPermissionError> {
    for permission in requested {
        if NEVER_GRANTED_PERMISSIONS_MIRROR.contains(permission) {
            return Err(ForecastPermissionError::NeverGranted {
                requested: (*permission).to_string(),
            });
        }
        if !FORECAST_PRODUCER_PERMISSIONS.contains(permission) {
            return Err(ForecastPermissionError::PermissionOutsideProducerVocabulary {
                requested: (*permission).to_string(),
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the rules above — the closed vocabularies, the scheme
// rules, the refusal tags — or a literal copied from this module's own
// constants. The one numeric digest expectation is DEFINITIONAL: FNV-1a 64
// over ZERO bytes is the offset basis 0xcbf29ce484222325. Nothing is
// fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const ARTIFACT_REF: &str = "artifact://forecasts/load/site-a/v1";

    fn scope() -> ForecastScope {
        ForecastScope::PerSite {
            site_id: "site-a".to_string(),
        }
    }

    /// A well-formed record (hand-built; the digest hex is filled in per
    /// test). Every field is one of the closed vocabulary members.
    fn provenance(digest_hex: &str) -> ForecastArtifactProvenance {
        ForecastArtifactProvenance::new(
            ARTIFACT_REF.to_string(),
            ForecastArtifactKind::Load,
            scope(),
            ForecastHorizon::DayAhead,
            "v1".to_string(),
            FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST.to_string(),
            digest_hex.to_string(),
            "key-store://release/forecast-signing".to_string(),
            1_000,
            "telemetry://fixture/site-a/history".to_string(),
            "model://registry/baseline-persistence-v1".to_string(),
            "forecast-eval-set-v1".to_string(),
        )
        .expect("the fixture record is valid")
    }

    /// A record with the digest computed over `artifact` by the module's
    /// own test-grade primitive (self-consistent; tamper tests mutate the
    /// bytes, not the record).
    fn parsed_record(artifact: &[u8]) -> ForecastArtifactProvenance {
        provenance(&hex_u64(fnv1a64(artifact)))
    }

    #[test]
    fn the_test_grade_digest_of_empty_input_is_the_fnv_offset_basis() {
        // DEFINITIONAL: FNV-1a 64 processes no bytes for empty input, so
        // the digest IS the offset basis.
        let verdict = UnsafeTestOnlyForecastDigest.verify(&[], "cbf29ce484222325");
        assert_eq!(verdict, ForecastDigestVerdict::Matches);
        let verdict = UnsafeTestOnlyForecastDigest.verify(&[], "0000000000000000");
        assert_eq!(verdict, ForecastDigestVerdict::Mismatch);
    }

    #[test]
    fn a_well_formed_record_validates_with_linked_provenance() {
        let record = parsed_record(b"forecast-artifact-bytes");
        assert_eq!(record.provenance_schema, FORECAST_PROVENANCE_SCHEMA_ID);
        assert_eq!(record.artifact_kind, ForecastArtifactKind::Load);
        assert_eq!(record.horizon, ForecastHorizon::DayAhead);
        assert_eq!(record.artifact_scope.ref_string(), "site:site-a");
        assert_eq!(record.input_data_ref, "telemetry://fixture/site-a/history");
        assert_eq!(record.model_ref, "model://registry/baseline-persistence-v1");
        assert_eq!(record.eval_set_ref, "forecast-eval-set-v1");
        assert_eq!(record.signer_key_reference, "key-store://release/forecast-signing");
    }

    #[test]
    fn a_wrong_schema_id_is_refused_never_parsed_permissively() {
        let mut record = parsed_record(b"forecast-artifact-bytes");
        record.provenance_schema = "flexcore-model-artifact-provenance-v1".to_string();
        let err = record.validate().expect_err("wrong envelope refused");
        assert_eq!(err.name(), "FORECAST_PROVENANCE_SCHEMA_UNSUPPORTED");
    }

    #[test]
    fn an_inline_signer_key_reference_is_refused_and_the_value_never_echoed() {
        let mut record = parsed_record(b"forecast-artifact-bytes");
        record.signer_key_reference = "RAWKEY-0123456789abcdef".to_string();
        let err = record.validate().expect_err("inline key refused");
        assert_eq!(err.name(), "FORECAST_PROVENANCE_SIGNER_KEY_REFERENCE_INVALID");
        let text = format!("{err}");
        assert!(
            !text.contains("RAWKEY"),
            "a rejected key reference VALUE is never echoed back (it may be key material)"
        );
    }

    #[test]
    fn an_unknown_digest_algorithm_is_refused() {
        let mut record = parsed_record(b"forecast-artifact-bytes");
        record.digest_algorithm = "md5".to_string();
        let err = record.validate().expect_err("unknown algorithm refused");
        assert_eq!(err.name(), "FORECAST_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN");
    }

    #[test]
    fn a_whitespace_forecast_version_is_refused() {
        let mut record = parsed_record(b"forecast-artifact-bytes");
        record.forecast_version = "v1 with space".to_string();
        let err = record.validate().expect_err("whitespace version refused");
        assert_eq!(err.name(), "FORECAST_PROVENANCE_VERSION_HAS_WHITESPACE");
    }

    #[test]
    fn a_missing_linked_provenance_ref_is_refused() {
        // Dropping any of the three LINKED provenance references must
        // refuse: provenance is never stored apart from the signature.
        for (field, value) in [
            ("input-data-ref", ""),
            ("model-ref", "  "),
            ("eval-set-ref", ""),
        ] {
            let mut record = parsed_record(b"forecast-artifact-bytes");
            match field {
                "input-data-ref" => record.input_data_ref = value.to_string(),
                "model-ref" => record.model_ref = value.to_string(),
                _ => record.eval_set_ref = value.to_string(),
            }
            let err = record.validate().expect_err("missing linked ref refused");
            assert_eq!(err.name(), "FORECAST_PROVENANCE_FIELD_INVALID");
            assert!(format!("{err}").contains(field));
        }
    }

    #[test]
    fn an_empty_scope_id_is_refused() {
        let record = ForecastArtifactProvenance::new(
            ARTIFACT_REF.to_string(),
            ForecastArtifactKind::PvGeneration,
            ForecastScope::PerPortfolio {
                portfolio_id: "  ".to_string(),
            },
            ForecastHorizon::IntraDay,
            "v1".to_string(),
            FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST.to_string(),
            "cbf29ce484222325".to_string(),
            "env:FORECAST_SIGNER".to_string(),
            1_000,
            "weather://fixture/input".to_string(),
            "model://registry/x".to_string(),
            "forecast-eval-set-v1".to_string(),
        )
        .expect_err("empty portfolio id refused");
        assert_eq!(err.name(), "FORECAST_PROVENANCE_EMPTY_SCOPE_REF");
    }

    #[test]
    fn the_default_verifier_refuses_everything_fail_closed() {
        // A record that would verify under the test-grade verifier STILL
        // fails under the default: the absence of signing infrastructure
        // can never look like success.
        let record = parsed_record(b"forecast-artifact-bytes");
        let verdict = verify_forecast_artifact(
            &record,
            ARTIFACT_REF,
            b"forecast-artifact-bytes",
            &NoForecastVerifier,
        );
        match &verdict {
            ForecastProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(FORECAST_REFUSAL_VERIFIER_REFUSED));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn tampered_bytes_are_refused_with_the_digest_mismatch_tag() {
        let record = parsed_record(b"forecast-artifact-bytes");
        let tampered = b"forecast-artifact-byteZ"; // one byte flipped
        let verdict = verify_forecast_artifact(
            &record,
            ARTIFACT_REF,
            tampered,
            &UnsafeTestOnlyForecastDigest,
        );
        match &verdict {
            ForecastProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(FORECAST_REFUSAL_DIGEST_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_record_bound_to_a_different_artifact_is_refused() {
        let record = parsed_record(b"forecast-artifact-bytes");
        let verdict = verify_forecast_artifact(
            &record,
            "artifact://forecasts/OTHER.v1",
            b"forecast-artifact-bytes",
            &UnsafeTestOnlyForecastDigest,
        );
        match &verdict {
            ForecastProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(FORECAST_REFUSAL_REF_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_algorithm_mismatch_between_record_and_verifier_is_refused() {
        // A record declaring sha-256 (accepted IN the record vocabulary)
        // against the test-grade verifier: the verifier owns the shape
        // match, so this is the ALGORITHM_MISMATCH refusal.
        let mut record = parsed_record(b"forecast-artifact-bytes");
        record.digest_hex = format!("{:064x}", 0u64); // 64 hex digits
        record.digest_algorithm = FORECAST_DIGEST_ALGORITHM_SHA256.to_string();
        record.validate().expect("sha-256 is in the vocabulary");
        let verdict = verify_forecast_artifact(
            &record,
            ARTIFACT_REF,
            b"forecast-artifact-bytes",
            &UnsafeTestOnlyForecastDigest,
        );
        match &verdict {
            ForecastProvenanceVerdict::Refused { reason } => {
                assert!(reason.starts_with(FORECAST_REFUSAL_ALGORITHM_MISMATCH));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn closed_vocabularies_are_the_recorded_strings() {
        assert_eq!(ForecastArtifactKind::Load.wire_name(), "load");
        assert_eq!(ForecastArtifactKind::PvGeneration.wire_name(), "pv_generation");
        assert_eq!(ForecastArtifactKind::Price.wire_name(), "price");
        assert_eq!(ForecastArtifactKind::Load.unit(), "MW");
        assert_eq!(ForecastArtifactKind::PvGeneration.unit(), "MW");
        assert!(ForecastArtifactKind::Price.unit().contains("declared per series"));
        assert_eq!(ForecastHorizon::DayAhead.wire_name(), "day_ahead");
        assert_eq!(ForecastHorizon::IntraDay.wire_name(), "intra_day");
        assert_eq!(
            FORECAST_PROVENANCE_SCHEMA_ID,
            "flexcore-forecast-artifact-provenance-v1"
        );
        assert!(FORECASTING_ARCHITECTURE_STATEMENT.contains("never commands"));
        assert!(FORECASTING_ARCHITECTURE_STATEMENT.contains("EMPTY"));
        assert!(KERNEL_PERMISSION_ALIGNMENT_SEAM.contains("BY CITATION"));
    }

    #[test]
    fn the_producer_permission_vocabulary_has_no_dispatch_member() {
        // The ADR-010 extension, structurally: the forecast producer's
        // closed permission vocabulary contains telemetry.read and
        // forecast.publish and NOTHING dispatch-shaped.
        assert_eq!(FORECAST_PRODUCER_PERMISSIONS, ["telemetry.read", "forecast.publish"]);
        for permission in FORECAST_PRODUCER_PERMISSIONS {
            assert!(!permission.contains("dispatch"));
            assert!(!permission.contains("secret"));
            assert!(!permission.contains("network"));
        }
    }

    #[test]
    fn the_never_granted_mirror_is_byte_identical_to_the_recorded_set() {
        // VERBATIM mirror of crates/flexcore-kernel/src/manifest.rs
        // ::NEVER_GRANTED_PERMISSIONS — this literal is the pin; a change
        // on either side must be a deliberate recorded seam update.
        assert_eq!(
            NEVER_GRANTED_PERMISSIONS_MIRROR,
            ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"]
        );
    }

    #[test]
    fn the_producer_gate_accepts_the_manifest_vocabulary_and_refuses_the_rest() {
        // Exactly the forecast-worker manifest's permission set passes.
        validate_forecast_producer_permissions(&["telemetry.read", "forecast.publish"])
            .expect("the manifest permission set is the producer vocabulary");
        // A dispatch permission is NEVER_GRANTED (it is in the kernel set
        // first; the never-granted check runs before the vocabulary check).
        let err = validate_forecast_producer_permissions(&["dispatch.issue"])
            .expect_err("dispatch never granted");
        assert_eq!(err.name(), "FORECAST_PERMISSION_NEVER_GRANTED");
        // A secret read is never granted.
        let err = validate_forecast_producer_permissions(&["secret.read"])
            .expect_err("secret read never granted");
        assert_eq!(err.name(), "FORECAST_PERMISSION_NEVER_GRANTED");
        // A merely-unknown permission is out-of-vocabulary.
        let err = validate_forecast_producer_permissions(&["schedule.propose"])
            .expect_err("schedule propose is not a forecast-producer permission");
        assert_eq!(
            err.name(),
            "FORECAST_PERMISSION_OUTSIDE_PRODUCER_VOCABULARY"
        );
        // An empty request is vacuously valid (a producer with no
        // permissions can hold none).
        validate_forecast_producer_permissions(&[]).expect("empty request valid");
    }
}
