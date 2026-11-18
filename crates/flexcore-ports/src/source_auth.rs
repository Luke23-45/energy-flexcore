// Patch B (P2.07) — Cryptographic source authentication: the signed
// envelope port.
//
// A telemetry frame is accepted as genuinely from its claimed source only
// when it arrives inside a signed envelope: an Ed25519 signature over a
// canonical byte layout, a SHA-256 digest of the inner payload, a
// strictly increasing per-source sequence, and an event time inside a
// bounded window around the receiver's clock. The signature key is looked
// up by (`tenant_id`, `source_id`, `key_id`) in the signing-key registry
// (the adapter owns that lookup; this crate names only the contract).
//
// This module is a PORT: it defines the envelope shape, the canonical
// signing bytes, the error vocabulary, and the authenticator trait. It
// implements no cryptography and no I/O, and it depends on nothing
// outside `flexcore-contracts` and `flexcore-domain` (plus std) — in
// particular it has no serialization library and no base64/hex codec, so
// it cannot parse or emit the wire format itself.
//
// # Normative wire format (JSON object)
//
// | Field | Type | Rule |
// |---|---|---|
// | `version` | u8 | must be 1 ([`ENVELOPE_VERSION`]) |
// | `key_id` | string | the signing-key identifier |
// | `source_id` | string | the source identity (same vocabulary as the `TelemetrySource` port's provider declaration) |
// | `tenant_id` | string | hyphenated UUID text |
// | `sequence` | u64 | strictly increasing per (tenant, source); starts at 1 |
// | `event_time_unix_millis` | i64 | milliseconds since the Unix epoch |
// | `payload_digest` | string | 64 lowercase hex chars = SHA-256 of the raw inner payload bytes |
// | `payload` | string | base64, STANDARD alphabet with padding, of the raw inner payload bytes |
// | `signature` | string | 128 lowercase hex chars = the Ed25519 signature over [`canonical_signing_bytes`] |
//
// # Deliberate decision: the JSON codec lives in exactly two places
//
// Because this crate cannot depend on a serialization library or a base64
// codec, and the contracts crate cannot depend on base64 either, the JSON
// codec is implemented twice, once per side, and both sides implement the
// field list above and both use [`canonical_signing_bytes`] for the
// security-critical exactness:
//
// - the simulator producer (the `TelemetrySource` provider) wraps a frame
//   in an envelope when a signing key is configured;
// - the database-adapter consumer parses an envelope, verifies it, and
//   returns the inner payload.
//
// Either side changing a field name, a hex/base64 alphabet, or the
// canonical layout breaks the other side by construction (a signature
// made over one layout never verifies over another); there is no third
// codec to drift. The two codecs are tested against each other by the
// sibling test worker's round-trip tests, not by this module.
//
// # Canonical signing bytes (byte-exact)
//
// [`canonical_signing_bytes`] concatenates, in order:
//
// 1. [`ENVELOPE_DOMAIN`] (`flexcore-telemetry-v1`);
// 2. `version` as 1 byte;
// 3. `key_id` as u32 little-endian length + bytes;
// 4. `source_id` as u32 little-endian length + bytes;
// 5. `tenant_id` as 16 raw UUID bytes;
// 6. `sequence` as 8 bytes little-endian;
// 7. `event_time_unix_millis` as 8 bytes little-endian;
// 8. `payload_digest` as 32 raw bytes.
//
// # What this patch does NOT decide
//
// Decision D6 (the open source-authentication decision) stays OPEN: this
// module implements the envelope mechanism; it does not close D6, and no
// claim in this module should be read as closing it.
//
// # Scope and control status
//
// Simulation-only (the entire 0.x line): envelope authentication is a
// receive-side integrity check on simulation telemetry. It grants no
// control authority, performs no device writes, and carries no
// protection-grade functionality.

use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::TenantContext;

/// The domain-separation string for the envelope signature. Every
/// signature is over bytes that start with this string, so a signature
/// made for any other purpose can never verify as a telemetry envelope.
pub const ENVELOPE_DOMAIN: &[u8] = b"flexcore-telemetry-v1";

/// The only accepted envelope version.
pub const ENVELOPE_VERSION: u8 = 1;

/// The default time-window half-width (seconds) for the event-time check:
/// an envelope's `event_time_unix_millis` must lie within ±300 s of the
/// receiver's clock. Deployment configuration may narrow or widen it; the
/// window is never infinite.
pub const DEFAULT_AUTH_WINDOW_SECS: u64 = 300;

/// One signed envelope: the wire object, decoded. The `payload` is the
/// raw inner frame bytes (what the `TelemetrySource` port's `decode` runs
/// over); the `signature` is the Ed25519 signature over
/// [`canonical_signing_bytes`] for these fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEnvelope {
    /// Must be [`ENVELOPE_VERSION`].
    pub version: u8,
    /// The signing-key identifier.
    pub key_id: String,
    /// The claimed source identity.
    pub source_id: String,
    /// The tenant the frame belongs to.
    pub tenant_id: TenantId,
    /// Strictly increasing per (tenant, source); starts at 1.
    pub sequence: u64,
    /// Milliseconds since the Unix epoch.
    pub event_time_unix_millis: i64,
    /// SHA-256 of `payload`.
    pub payload_digest: [u8; 32],
    /// The raw inner frame bytes.
    pub payload: Vec<u8>,
    /// The Ed25519 signature (64 bytes) over the canonical signing bytes.
    pub signature: [u8; 64],
}

/// Builds the byte-exact input to the Ed25519 signature. The layout is
/// fixed by the module docs and must never change without a version bump:
/// domain, version byte, length-prefixed (u32 little-endian length +
/// bytes) `key_id`, length-prefixed `source_id`, `tenant_id` as 16 raw
/// UUID bytes, `sequence` as 8 bytes little-endian,
/// `event_time_unix_millis` as 8 bytes little-endian, `payload_digest` as
/// 32 raw bytes.
pub fn canonical_signing_bytes(
    version: u8,
    key_id: &str,
    source_id: &str,
    tenant_id: &TenantId,
    sequence: u64,
    event_time_unix_millis: i64,
    payload_digest: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        ENVELOPE_DOMAIN.len()
            + 1
            + 4 + key_id.len()
            + 4 + source_id.len()
            + 16
            + 8
            + 8
            + 32,
    );
    out.extend_from_slice(ENVELOPE_DOMAIN);
    out.push(version);
    push_length_prefixed(&mut out, key_id.as_bytes());
    push_length_prefixed(&mut out, source_id.as_bytes());
    out.extend_from_slice(&tenant_id.uuid().to_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&event_time_unix_millis.to_le_bytes());
    out.extend_from_slice(payload_digest);
    out
}

/// Appends `bytes` as u32 little-endian length + bytes.
fn push_length_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len())
        .expect("a key or source identifier longer than u32::MAX bytes cannot be signed");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Why envelope authentication rejected a frame. Every variant maps to
/// exactly one stable quarantine reason via [`SourceAuthError::quarantine_reason`];
/// the reason string is the whole classification the pipeline (and any
/// metric label) may carry — no key material, no payload bytes, and no
/// free-text detail ever enters it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAuthError {
    /// The bytes are not an envelope at all, or a field is the wrong
    /// shape: malformed JSON, wrong version, a hex field of the wrong
    /// length or alphabet, bad base64, or an unparsable tenant UUID.
    /// The detail string is diagnostic only; it never enters the
    /// quarantine reason (which must stay stable).
    MalformedEnvelope(String),
    /// No key is enrolled for (`tenant_id`, `source_id`, `key_id`), or
    /// the current term is not ACTIVE, or its validity window does not
    /// cover now. Revoked, expired, and unknown are deliberately the
    /// same error: the caller must not be able to distinguish them.
    UnknownSourceOrKey,
    /// The signature does not verify over the canonical signing bytes
    /// for the enrolled key.
    BadSignature,
    /// SHA-256 of the payload does not equal the envelope's
    /// `payload_digest`.
    BadDigest,
    /// `sequence` is not strictly greater than the last accepted
    /// sequence for (tenant, source): a replay or a stale frame.
    ReplayOrStaleSequence,
    /// `event_time_unix_millis` lies outside ±window of the receiver's
    /// clock.
    OutsideTimeWindow,
}

impl std::fmt::Display for SourceAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // The detail is a parse diagnostic (field names, lengths);
            // it never carries key material or payload bytes.
            SourceAuthError::MalformedEnvelope(detail) => {
                write!(f, "malformed envelope: {detail}")
            }
            SourceAuthError::UnknownSourceOrKey => {
                write!(f, "unknown source or key")
            }
            SourceAuthError::BadSignature => {
                write!(f, "bad signature")
            }
            SourceAuthError::BadDigest => {
                write!(f, "bad payload digest")
            }
            SourceAuthError::ReplayOrStaleSequence => {
                write!(f, "replay or stale sequence")
            }
            SourceAuthError::OutsideTimeWindow => {
                write!(f, "event time outside the authentication window")
            }
        }
    }
}

impl std::error::Error for SourceAuthError {}

impl SourceAuthError {
    /// The stable quarantine reason for this rejection (stage
    /// `1-receive`, rule `source-auth`). The `MalformedEnvelope`
    /// detail string is deliberately NOT included: the reason must be
    /// stable across frames so it can be counted and matched on.
    pub fn quarantine_reason(&self) -> String {
        match self {
            SourceAuthError::MalformedEnvelope(_) => {
                "1-receive: source-auth: malformed-envelope".to_string()
            }
            SourceAuthError::UnknownSourceOrKey => {
                "1-receive: source-auth: unknown-source-or-key".to_string()
            }
            SourceAuthError::BadSignature => {
                "1-receive: source-auth: bad-signature".to_string()
            }
            SourceAuthError::BadDigest => {
                "1-receive: source-auth: bad-digest".to_string()
            }
            SourceAuthError::ReplayOrStaleSequence => {
                "1-receive: source-auth: replay-or-stale-sequence".to_string()
            }
            SourceAuthError::OutsideTimeWindow => {
                "1-receive: source-auth: outside-time-window".to_string()
            }
        }
    }
}

/// A frame that passed envelope authentication: the verified identity
/// plus the raw inner payload the pipeline decodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEnvelope {
    /// The signing-key identifier that verified.
    pub key_id: String,
    /// The verified source identity.
    pub source_id: String,
    /// The verified tenant.
    pub tenant_id: TenantId,
    /// The envelope sequence (for the watermark advance).
    pub sequence: u64,
    /// The envelope event time, milliseconds since the Unix epoch.
    pub event_time_unix_millis: i64,
    /// `true` when the frame arrived inside a real envelope (the
    /// production path). Test doubles may return `false` with
    /// `payload` equal to the raw frame bytes, so downstream-stage
    /// tests keep working with raw frames; the pipeline skips the
    /// sequence-watermark advance entirely when this is `false`.
    pub was_enveloped: bool,
    /// The raw inner frame bytes to decode.
    pub payload: Vec<u8>,
}

/// The source-authentication port: takes RAW frame bytes, parses the
/// envelope, verifies it, and returns the inner payload. Implementations
/// are sync (the P1.07 rule); an adapter that needs I/O for the key
/// lookup bridges onto its own runtime.
pub trait SourceAuthenticator: Send + Sync {
    /// Authenticates one raw frame under the tenant scope `ctx`.
    /// `Err` means the frame is rejected: the caller quarantines it
    /// under the error's [`SourceAuthError::quarantine_reason`] and
    /// stores nothing.
    fn authenticate(
        &self,
        ctx: &TenantContext,
        payload: &[u8],
    ) -> Result<VerifiedEnvelope, SourceAuthError>;
}

/// How the production authenticator treats keys.
///
/// - `Enforced`: only keys enrolled in the signing-key registry verify;
///   the `dev` key identifier is rejected as unknown. This is the
///   default and the only mode for any deployment past local
///   development.
/// - `DevKey`: the `dev` key identifier verifies against an ephemeral
///   in-process keypair generated at startup (never written to disk or
///   configuration). Local development only; the worker logs a warning
///   on every start in this mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAuthMode {
    Enforced,
    DevKey,
}

impl SourceAuthMode {
    /// Parses the `FLEXCORE_SOURCE_AUTH_MODE` value. Accepts exactly
    /// `"enforced"` and `"dev-key"`; anything else is `None` (the
    /// caller fails fast).
    pub fn from_env_str(s: &str) -> Option<Self> {
        match s {
            "enforced" => Some(SourceAuthMode::Enforced),
            "dev-key" => Some(SourceAuthMode::DevKey),
            _ => None,
        }
    }
}
