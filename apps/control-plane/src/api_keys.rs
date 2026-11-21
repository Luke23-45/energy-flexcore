// P13.08 / FIM-F08.02 — per-tenant API KEYS: issuance, rotation with a
//             grace window, last-used evidence, revocation, and the
//             refusal taxonomy for the legacy/simple integration tier.
//
// Implemented under FIM-F08 (final implementation milestone; the FIM
// standing decisions live in
// `docs/releases/implementation_ledger/README.md`). Contract source: the
// P13.08 decision record
// `docs/phases-11-20/phase-13-enterprise-identity-integration/records/
// 08_p13_08_api_keys_webhook_secret_format.md` — the key-id/secret format
// (§2: `fkid_<10 lowercase base32>` public id; `fk_<43 unpadded base64url>`
// secret, 32 bytes of CSPRNG entropy, shown ONCE), the digest-at-rest
// storage rule (§2: "the plaintext never persists in the database"), the
// scope rule (a STRICT SUBSET of the typed `Action` vocabulary; the
// never-granted names structurally excluded), the rotation overlap, and
// the revocation semantics are consumed BY CITATION, never re-decided.
// The webhook-secret FORMAT of record 08 §4 belongs to
// `crate::webhook_signing` (`parse_webhook_secret`); this module defines
// NO second secret format — only the `fk_` API-key literal P13.08 §2 names.
//
// **WRITTEN-UNRUN.** This module has never been compiled or executed: no
// key is issued, no digest is stored, no rotation has ever been executed,
// no last-used evidence has ever been written, no API call has ever
// authenticated with one. The first human build/test is the gate (program
// conventions §1). LIVE CALLER INTEROP (a real integration authenticating
// with an issued key) IS UNPROVEN — **HUMAN LAB GATE**.
//
// Design points (each is a recorded decision of this implementation):
//
// 1. SECRET POSTURE (hard; FIM acceptance criterion 5, the P11.16
//    posture): the store holds ONLY a keyed VERIFIER — the SHA-256 hex
//    digest of the full `fk_` secret string — plus the key id, scopes,
//    state, and evidence fields. The RAW KEY is returned ONCE at issuance
//    and ONCE at each rotation, never stored retrievable (record 08 §6.4:
//    a lost key is rotated, not recovered). No record type in this module
//    carries raw key material; no error ever echoes a presented key.
//
// 2. WIRE FORM (the bearer grammar; record 08 defines the credential
//    shape, not the transport form — this is the recorded implementation
//    decision): the caller presents the bare secret literal
//    `fk_<43 unpadded base64url characters>` as its bearer value. The
//    store looks the key up BY DIGEST (the verifier is the lookup key);
//    the public `fkid_` id is administrative (audit records, the admin
//    UI) and never required on the wire. The `fk_` prefix is parsed
//    strictly (exact prefix, exactly 43 unpadded-base64url characters) —
//    anything else is a named [`ApiKeyError::MalformedKey`] refusal, and
//    the presented value is never echoed.
//
// 3. ROTATION WITH GRACE (record 08 §2: rotate = issue a new secret for
//    the SAME key id; the old secret is `superseded` with an overlap
//    window): the old digest stays VERIFYING until the configured grace
//    window closes, so in-flight callers do not drop; after the window
//    the old secret refuses with the named
//    [`ApiKeyError::SupersededPastGrace`]. The grace window is
//    configuration ([`ApiKeyConfig::rotation_grace_seconds`]) with the
//    record's registered 24-hour default — a REGISTERED RECOMMENDATION
//    (X-8), never a tuned claim. Rotation bumps nothing else: revocation
//    is the immediate path.
//
// 4. LAST-USED EVIDENCE (record 08 §2): on every SUCCESSFUL
//    authentication the store records the unix-millis timestamp, the
//    subject path the caller presented, and the `ALLOW` result on the
//    key's evidence fields (the monotonically increasing use counter of
//    record 08 §2 is carried as the evidence counter). Denial evidence is
//    an audit-record fact (the append-only trail), not this store's.
//
// 5. SCOPE RULE — a strict subset of the EXISTING
//    `flexcore-authz::action::Action` vocabulary. The never-granted
//    permission set stays byte-identical: `dispatch.authorize`,
//    `dispatch.issue`, `secret.read`, `network.any` are not on the
//    `Action` surface, so no scope string can ever grant them
//    (structurally excluded; `Action::parse` → `None` → named
//    [`ApiKeyError::InvalidScope`] at issuance/rotation).
//
// 6. PERSISTENCE — the store-trait + in-memory-default pattern (like
//    `MemoryWebhookDeliveryStore`): [`ApiKeyStore`] is the port;
//    [`MemoryApiKeyStore`] is the in-memory reference implementation. A
//    Postgres-backed key store is a RECORDED FAIL-CLOSED SEAM (not built
//    in this wave; migration work is another program's): until a real
//    adapter store exists the composition binds the in-memory store and
//    every issued key is lost on restart — fail-closed because an empty
//    store authenticates nothing.
//
// 7. ENTROPY — key material is 32 bytes from the OS CSPRNG (`rand`'s
//    `OsRng`, the standard OS entropy source; record 08 §6.3 records
//    CSPRNG availability as a deployment requirement). Rendering is the
//    unpadded base64url rule the webhook-secret format already defines
//    (32 bytes → 43 characters); the encoder here is the strict inverse
//    of that rule, pinned by a round-trip test through
//    `crate::webhook_signing`'s base64url value table semantics.
//
// 8. ERROR VOCABULARY — the closed [`ApiKeyError`] names every refusal
//    class; the composition maps it onto the EXISTING
//    [`crate::auth::AuthError`] (`InvalidCredentials`) through the
//    provided `From` impl, the same mapping the machine-identity tier
//    uses. No error ever echoes a presented key.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use rand::rngs::OsRng;
use rand::RngCore;

use crate::auth::AuthError;
use crate::webhook_signing::{hex_encode_digest, sha256};

/// The public key-id prefix (record 08 §2: `fkid_<10 lowercase base32
/// chars>`; public, travels in audit records, safe to log).
pub const KEY_ID_PREFIX: &str = "fkid_";
/// The secret-literal prefix (record 08 §2: `fk_<43 unpadded base64url
/// chars>`; EXACT, case-sensitive).
pub const KEY_SECRET_PREFIX: &str = "fk_";
/// The base64url character count of the secret payload (32 bytes → 43
/// unpadded base64url characters; the record 08 §2 rendering rule).
pub const KEY_SECRET_BASE64URL_CHARS: usize = 43;
/// The key-id payload character count (record 08 §2: exactly 10 lowercase
/// base32 characters).
pub const KEY_ID_PAYLOAD_CHARS: usize = 10;
/// The lowercase base32 alphabet (RFC 4648 base32, lowercased — the
/// record 08 §2 rendering rule for the public id).
const BASE32_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// The registered rotation-grace default in seconds (record 08 §2: 24 h
/// overlap — a REGISTERED RECOMMENDATION, X-8 discipline; changing it is
/// a recorded amendment to the P13.08 decision record).
pub const ROTATION_GRACE_SECONDS_DEFAULT: i64 = 86_400;

// --- Errors -------------------------------------------------------------------

/// Errors of the API-key service. Closed vocabulary; every variant names
/// its refusal class and NEVER echoes a presented key (it may be a live
/// credential).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyError {
    /// The record argument was malformed (`field` names the problem).
    /// Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A requested scope string is not on the closed `Action` surface —
    /// which structurally includes the never-granted names. Nothing was
    /// written.
    InvalidScope {
        /// The REJECTED scope's index in the submitted list (never the
        /// value itself).
        index: usize,
    },
    /// The presented bearer value was not a well-formed `fk_` literal
    /// (exact prefix, exactly 43 unpadded-base64url characters). The
    /// presented value is never echoed.
    MalformedKey,
    /// The presented key is well-formed but matches no registered key
    /// (including revoked-and-purged and foreign-tenant keys — a digest
    /// lookup cannot leak which).
    UnknownKey,
    /// The key's state is `revoked` (immediate revocation, record 08 §2:
    /// "there is no 'works until its next refresh' window for a revoked
    /// key").
    KeyRevoked,
    /// The presented key is the SUPERSEDED secret of a rotation and the
    /// configured grace window has closed. In-flight callers rotate now.
    SupersededPastGrace,
    /// The authenticated key attempted an action outside its registered
    /// scope subset.
    ScopeNotGranted {
        /// The canonical action id that was refused (a parseable action on
        /// the closed surface — never the never-granted names, which
        /// cannot reach this variant).
        action: &'static str,
    },
    /// The store backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl ApiKeyError {
    /// The stable machine-readable refusal name (records, metrics).
    pub fn name(&self) -> &'static str {
        match self {
            ApiKeyError::InvalidRecord { .. } => "API_KEY_INVALID_RECORD",
            ApiKeyError::InvalidScope { .. } => "API_KEY_INVALID_SCOPE",
            ApiKeyError::MalformedKey => "API_KEY_MALFORMED_KEY",
            ApiKeyError::UnknownKey => "API_KEY_UNKNOWN_KEY",
            ApiKeyError::KeyRevoked => "API_KEY_REVOKED",
            ApiKeyError::SupersededPastGrace => "API_KEY_SUPERSEDED_PAST_GRACE",
            ApiKeyError::ScopeNotGranted { .. } => "API_KEY_SCOPE_NOT_GRANTED",
            ApiKeyError::Backend { .. } => "API_KEY_BACKEND",
        }
    }
}

impl std::fmt::Display for ApiKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiKeyError::InvalidRecord { field } => {
                write!(f, "the api-key record is invalid: '{field}' is missing or malformed")
            }
            ApiKeyError::InvalidScope { index } => {
                write!(
                    f,
                    "requested scope at index {index} is not on the closed action surface \
                     (never-granted names are structurally excluded)"
                )
            }
            ApiKeyError::MalformedKey => {
                write!(f, "the presented credential is not a well-formed API-key literal")
            }
            ApiKeyError::UnknownKey => {
                write!(f, "the presented credential does not match a registered API key")
            }
            ApiKeyError::KeyRevoked => write!(f, "the API key is revoked (immediate)"),
            ApiKeyError::SupersededPastGrace => write!(
                f,
                "the presented credential is a superseded key whose rotation grace window \
                 has closed"
            ),
            ApiKeyError::ScopeNotGranted { action } => {
                write!(f, "the key's scope subset does not include '{action}'")
            }
            ApiKeyError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ApiKeyError {}

/// The composition mapping onto the EXISTING resolver-error vocabulary
/// (the same posture the machine-identity tier carries): every API-key
/// refusal collapses to the reasoned 401 (`AuthError::InvalidCredentials`)
/// the `IdentityResolver` seam already carries; the named class stays on
/// THIS type for records and metrics.
impl From<ApiKeyError> for AuthError {
    fn from(_err: ApiKeyError) -> Self {
        AuthError::InvalidCredentials
    }
}

// --- Records ------------------------------------------------------------------

/// The last-use evidence row (record 08 §2's evidence fields). Carries NO
/// credential material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastUsedEvidence {
    /// Unix millis of the successful authentication.
    pub last_used_at_unix_millis: i64,
    /// The subject path the caller presented at that authentication.
    pub last_used_subject_path: String,
    /// The authentication result (`ALLOW` — this row is written on
    /// success only; denial evidence is the audit trail's).
    pub last_used_result: String,
}

/// One registered API key (record 08 §2's tenant-scoped key row). The type
/// carries NO raw key material: only digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRecord {
    /// The public key id (`fkid_<10 lowercase base32>`; safe to log).
    pub key_id: String,
    /// The owning tenant (P0.16 rule 1).
    pub tenant_id: TenantId,
    /// The scope subset, as canonical action ids (each a parseable
    /// `Action`; issuance/rotation refuse anything else).
    pub scopes: Vec<String>,
    /// The key's state (the closed [`KEY_STATES`] vocabulary).
    pub state: String,
    /// The SHA-256 hex digest of the CURRENT `fk_` secret string — the
    /// VERIFIER and the lookup key. The raw secret is never stored
    /// retrievable.
    pub secret_verifier_sha256_hex: String,
    /// The SHA-256 hex digest of the PREVIOUS (superseded) secret while a
    /// rotation grace window is open; `None` otherwise. The window CLOSES
    /// at `superseded_until_unix_millis`.
    pub superseded_verifier_sha256_hex: Option<String>,
    /// Unix millis when the superseded secret's grace window closes; `None`
    /// when no superseded secret is live.
    pub superseded_until_unix_millis: Option<i64>,
    /// The secret's display fingerprint: the first 8 characters of the
    /// current secret's payload (record 08 §4's display rule, consumed by
    /// citation for this credential class — an admin UI renders id +
    /// fingerprint, never the secret).
    pub fingerprint: String,
    /// The last-use evidence, when the key has authenticated at least once.
    pub last_used: Option<LastUsedEvidence>,
    /// The use counter (record 08 §2: monotonically increasing).
    pub use_count: u64,
    /// Unix millis of issuance.
    pub created_at_unix_millis: i64,
    /// Unix millis of the most recent rotation; `None` if never rotated.
    pub rotated_at_unix_millis: Option<i64>,
}

/// The closed key-state vocabulary.
pub const KEY_STATES: [&str; 3] = ["ACTIVE", "SUPERSEDED_GRACE", "REVOKED"];

impl ApiKeyRecord {
    /// Validates the non-blank/state/digest-shape rules (the store and the
    /// service both call it — one authority).
    pub fn validate(&self) -> Result<(), ApiKeyError> {
        if self.key_id.trim().is_empty() {
            return Err(ApiKeyError::InvalidRecord { field: "key_id" });
        }
        if !KEY_STATES.contains(&self.state.as_str()) {
            return Err(ApiKeyError::InvalidRecord { field: "state" });
        }
        for (field, digest) in [
            ("secret_verifier_sha256_hex", Some(self.secret_verifier_sha256_hex.as_str())),
            ("superseded_verifier_sha256_hex", self.superseded_verifier_sha256_hex.as_deref()),
        ] {
            if let Some(digest) = digest {
                if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(ApiKeyError::InvalidRecord { field });
                }
            }
        }
        if self.superseded_verifier_sha256_hex.is_some()
            != self.superseded_until_unix_millis.is_some()
        {
            return Err(ApiKeyError::InvalidRecord {
                field: "superseded_verifier_sha256_hex",
            });
        }
        Ok(())
    }
}

/// The resolved principal of a successful API-key authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyPrincipal {
    /// The public key id.
    pub key_id: String,
    /// The owning tenant — FROM THE STORE (a key authenticates its own
    /// tenant; the tenant is never taken from the request).
    pub tenant_id: TenantId,
    /// The granted actions (the key's scope subset, parsed against the
    /// closed surface).
    pub scopes: Vec<Action>,
}

impl ApiKeyPrincipal {
    /// The scope-grant check for an API-key call: allow only when the
    /// action is IN the key's registered subset. Deny-by-default; the
    /// refusal is the named [`ApiKeyError::ScopeNotGranted`].
    pub fn allows(&self, action: Action) -> Result<(), ApiKeyError> {
        if self.scopes.contains(&action) {
            Ok(())
        } else {
            Err(ApiKeyError::ScopeNotGranted { action: action.id() })
        }
    }
}

// --- The store port -----------------------------------------------------------

/// The API-key store port. Persistence follows the store-trait +
/// in-memory-default pattern: [`MemoryApiKeyStore`] is the reference
/// implementation; a Postgres-backed store is a recorded fail-closed seam
/// (module header, point 6).
pub trait ApiKeyStore: Send + Sync {
    /// Inserts a NEW key row. A duplicate key id or a duplicate live
    /// verifier is a named failure — issuance is explicit.
    fn put(&self, record: ApiKeyRecord) -> Result<(), ApiKeyError>;

    /// Replaces the stored row (rotation/revocation/evidence updates).
    fn update(&self, record: ApiKeyRecord) -> Result<(), ApiKeyError>;

    /// The key row with the given id IN THE CALLER'S TENANT scope; a
    /// foreign-tenant key reads as absent (P0.16 rule 5).
    fn get(
        &self,
        tenant_id: &TenantId,
        key_id: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError>;

    /// The key row whose CURRENT verifier digest is `verifier_hex`, across
    /// tenants (authentication happens BEFORE a tenant is known — the
    /// tenant comes from the row). `None` when no live key matches.
    fn get_by_verifier(
        &self,
        verifier_hex: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError>;

    /// The key row whose SUPERSEDED verifier digest is `verifier_hex`,
    /// across tenants (the grace-window lookup). `None` when none matches.
    fn get_by_superseded_verifier(
        &self,
        verifier_hex: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError>;
}

/// The in-memory [`ApiKeyStore`] (the reference implementation and the
/// test path). Ordering: `BTreeMap` keyed by key id (deterministic —
/// conventions §6.6 kernel patterns). The map VALUES are records of
/// digests and evidence only — no raw key material ever enters the store
/// (pinned by test). `Clone` is a HANDLE clone (the map lives behind an
/// `Arc`), so the service's boxed port object and a test's inspection
/// handle share the same store — the repo's shared-fake pattern.
#[derive(Clone, Default)]
pub struct MemoryApiKeyStore {
    keys: Arc<Mutex<BTreeMap<String, ApiKeyRecord>>>,
}

impl MemoryApiKeyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ApiKeyStore for MemoryApiKeyStore {
    fn put(&self, record: ApiKeyRecord) -> Result<(), ApiKeyError> {
        record.validate()?;
        let mut keys = self.keys.lock().expect("poisoned mutex in store fake");
        if keys.contains_key(&record.key_id) {
            return Err(ApiKeyError::Backend {
                code: "E_KEY_EXISTS".to_string(),
                message: format!(
                    "an API key with id {} is already registered; issuance is explicit, \
                     never an upsert",
                    record.key_id
                ),
            });
        }
        keys.insert(record.key_id.clone(), record);
        Ok(())
    }

    fn update(&self, record: ApiKeyRecord) -> Result<(), ApiKeyError> {
        record.validate()?;
        let mut keys = self.keys.lock().expect("poisoned mutex in store fake");
        if !keys.contains_key(&record.key_id) {
            return Err(ApiKeyError::Backend {
                code: "E_KEY_MISSING".to_string(),
                message: format!(
                    "no API key with id {} is registered; update is explicit",
                    record.key_id
                ),
            });
        }
        keys.insert(record.key_id.clone(), record);
        Ok(())
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        key_id: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError> {
        let keys = self.keys.lock().expect("poisoned mutex in store fake");
        Ok(keys.get(key_id).and_then(|record| {
            (record.tenant_id == *tenant_id).then(|| record.clone())
        }))
    }

    fn get_by_verifier(
        &self,
        verifier_hex: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError> {
        let keys = self.keys.lock().expect("poisoned mutex in store fake");
        Ok(keys
            .values()
            .find(|record| record.secret_verifier_sha256_hex == verifier_hex)
            .cloned())
    }

    fn get_by_superseded_verifier(
        &self,
        verifier_hex: &str,
    ) -> Result<Option<ApiKeyRecord>, ApiKeyError> {
        let keys = self.keys.lock().expect("poisoned mutex in store fake");
        Ok(keys
            .values()
            .find(|record| {
                record.superseded_verifier_sha256_hex.as_deref() == Some(verifier_hex)
            })
            .cloned())
    }
}

// --- Literal rendering (the record 08 §2 format) -------------------------------

/// Encodes `bytes` as UNPADDED base64url (the record 08 §2 rendering rule;
/// 32 bytes → 43 characters). The strict inverse of the base64url value
/// table the webhook-secret parser consumes in
/// `crate::webhook_signing`.
pub fn base64url_encode_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 8 + 5) / 6);
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    for byte in bytes {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            let value = ((accumulator >> bits) & 0x3F) as usize;
            out.push(ALPHABET[value] as char);
        }
    }
    // The final partial group (fewer than 6 bits left) emits its low bits
    // zero-padded — exactly the unpadded-base64url rendering whose decode
    // discards the trailing remainder (the recorded non-rejected trailing
    // bits in `crate::webhook_signing`).
    if bits > 0 {
        let value = ((accumulator << (6 - bits)) & 0x3F) as usize;
        out.push(ALPHABET[value] as char);
    }
    out
}

/// The strict structural check of a presented `fk_` secret literal: exact
/// prefix, exactly [`KEY_SECRET_BASE64URL_CHARS`] characters of the
/// unpadded base64url alphabet. No decoding happens here — the verifier
/// is computed over the FULL literal (the digest-at-rest rule digests the
/// secret string as presented).
fn parse_secret_literal(presented: &str) -> Result<&str, ApiKeyError> {
    let payload = presented.strip_prefix(KEY_SECRET_PREFIX).ok_or(ApiKeyError::MalformedKey)?;
    if payload.len() != KEY_SECRET_BASE64URL_CHARS {
        return Err(ApiKeyError::MalformedKey);
    }
    if payload.bytes().any(|b| {
        !(b.is_ascii_uppercase()
            || b.is_ascii_lowercase()
            || b.is_ascii_digit()
            || b == b'-'
            || b == b'_')
    }) {
        return Err(ApiKeyError::MalformedKey);
    }
    Ok(presented)
}

/// The SHA-256 hex verifier of a full secret literal.
fn secret_verifier(literal: &str) -> String {
    hex_encode_digest(&sha256(literal.as_bytes()))
}

/// Generates a fresh public key id (`fkid_` + 10 lowercase base32 chars)
/// and secret literal (`fk_` + 43 unpadded base64url chars) from the OS
/// CSPRNG. The secret literal is returned as part of [`IssuedApiKey`] —
/// shown ONCE, never stored (module header point 1).
pub fn generate_key_material() -> (String, String) {
    // The id payload: 10 base32 chars ← 50 bits ← 7 bytes (30 bits used of
    // the 56; the remainder is discarded entropy — a registered
    // implementation detail of the record 08 §2 shape).
    let mut id_bytes = [0u8; 7];
    OsRng.fill_bytes(&mut id_bytes);
    let mut id_payload = String::with_capacity(KEY_ID_PAYLOAD_CHARS);
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    for byte in id_bytes {
        accumulator = (accumulator << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 && id_payload.len() < KEY_ID_PAYLOAD_CHARS {
            bits -= 5;
            let value = ((accumulator >> bits) & 0x1F) as usize;
            id_payload.push(BASE32_ALPHABET[value] as char);
        }
    }
    let mut secret_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut secret_bytes);
    let secret_payload = base64url_encode_no_pad(&secret_bytes);
    (
        format!("{KEY_ID_PREFIX}{id_payload}"),
        format!("{KEY_SECRET_PREFIX}{secret_payload}"),
    )
}

/// The ONE-TIME issuance (and rotation) response: the raw secret is here
/// and NOWHERE else in the module's outputs (module header point 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedApiKey {
    /// The public key id (safe to keep, log, audit).
    pub key_id: String,
    /// The RAW secret literal — displayed ONCE to the tenant admin, never
    /// stored retrievable (P13.08 §6.4: a lost key is rotated, not
    /// recovered).
    pub secret_literal: String,
    /// The display fingerprint (the first 8 characters of the secret's
    /// payload) — the form every subsequent listing renders.
    pub fingerprint: String,
    /// The key's scope subset, as canonical action ids.
    pub scopes: Vec<String>,
}

// --- The service ---------------------------------------------------------------

/// The API-key service configuration (one deployment). The default is a
/// REGISTERED RECOMMENDATION (X-8), never a tuned value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyConfig {
    /// The rotation grace window in seconds: how long a superseded secret
    /// keeps verifying after a rotation. DEFAULT
    /// [`ROTATION_GRACE_SECONDS_DEFAULT`] (24 h — record 08 §2).
    pub rotation_grace_seconds: i64,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self { rotation_grace_seconds: ROTATION_GRACE_SECONDS_DEFAULT }
    }
}

/// The per-tenant API-key service: issuance, rotation, authentication with
/// evidence, revocation — over the injected store and clock.
pub struct ApiKeyService {
    config: ApiKeyConfig,
    store: Box<dyn ApiKeyStore>,
    clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl ApiKeyService {
    /// Composes the service over the injected store and clock. Nothing is
    /// read from a database here — the store arrives from the composition
    /// root (in-memory default; a Postgres-backed store is the recorded
    /// fail-closed seam).
    pub fn new(
        config: ApiKeyConfig,
        store: Box<dyn ApiKeyStore>,
        clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self { config, store, clock_millis }
    }

    /// Validates every requested scope string against the closed `Action`
    /// surface (the same rule the machine-identity tier carries).
    fn validate_scopes(scopes: &[String]) -> Result<(), ApiKeyError> {
        for (index, scope) in scopes.iter().enumerate() {
            if Action::parse(scope).is_none() {
                return Err(ApiKeyError::InvalidScope { index });
            }
        }
        let mut seen: Vec<&str> = Vec::with_capacity(scopes.len());
        for (index, scope) in scopes.iter().enumerate() {
            if seen.contains(&scope.as_str()) {
                return Err(ApiKeyError::InvalidScope { index });
            }
            seen.push(scope.as_str());
        }
        Ok(())
    }

    /// Issues a NEW API key for the tenant: generates the id + secret from
    /// the OS CSPRNG, verifies the scope subset, stores the VERIFIER (and
    /// fingerprint — never the raw secret), and returns the raw secret
    /// ONCE (module header point 1).
    pub fn issue_key(
        &self,
        tenant_id: TenantId,
        scopes: Vec<String>,
    ) -> Result<IssuedApiKey, ApiKeyError> {
        Self::validate_scopes(&scopes)?;
        let (key_id, secret_literal) = generate_key_material();
        let payload = secret_literal
            .strip_prefix(KEY_SECRET_PREFIX)
            .expect("the generated literal carries the prefix");
        let record = ApiKeyRecord {
            key_id: key_id.clone(),
            tenant_id,
            // Clone (FIM2-R1-001 of the FIM final analysis — the
            // use-after-move shape `rotate_key` already avoids with its own
            // clone): the scope list is ALSO the issuance response's field,
            // so the record literal must not move the parameter.
            scopes: scopes.clone(),
            state: "ACTIVE".to_string(),
            secret_verifier_sha256_hex: secret_verifier(&secret_literal),
            superseded_verifier_sha256_hex: None,
            superseded_until_unix_millis: None,
            fingerprint: payload.chars().take(8).collect(),
            last_used: None,
            use_count: 0,
            created_at_unix_millis: (self.clock_millis)(),
            rotated_at_unix_millis: None,
        };
        record.validate()?;
        self.store.put(record)?;
        Ok(IssuedApiKey {
            key_id,
            secret_literal,
            fingerprint: payload.chars().take(8).collect(),
            scopes: scopes,
        })
    }

    /// Rotates a key (record 08 §2): a NEW secret is generated for the
    /// SAME key id, the OLD digest moves to the superseded slot with the
    /// configured grace window, and the new secret is returned ONCE.
    /// In-flight callers holding the old secret keep authenticating until
    /// the window closes (then [`ApiKeyError::SupersededPastGrace`]).
    pub fn rotate_key(
        &self,
        tenant_id: &TenantId,
        key_id: &str,
    ) -> Result<IssuedApiKey, ApiKeyError> {
        let record = self
            .store
            .get(tenant_id, key_id)?
            .ok_or(ApiKeyError::UnknownKey)?;
        if record.state == "REVOKED" {
            return Err(ApiKeyError::KeyRevoked);
        }
        let (_new_id_hint, secret_literal) = generate_key_material();
        let payload = secret_literal
            .strip_prefix(KEY_SECRET_PREFIX)
            .expect("the generated literal carries the prefix");
        let now = (self.clock_millis)();
        let scopes = record.scopes.clone();
        let updated = ApiKeyRecord {
            state: "ACTIVE".to_string(),
            secret_verifier_sha256_hex: secret_verifier(&secret_literal),
            superseded_verifier_sha256_hex: Some(record.secret_verifier_sha256_hex.clone()),
            superseded_until_unix_millis: Some(
                now + self.config.rotation_grace_seconds * 1_000,
            ),
            fingerprint: payload.chars().take(8).collect(),
            rotated_at_unix_millis: Some(now),
            // The key id, tenant, scopes, evidence, and counter are
            // unchanged by a rotation: same principal, new credential.
            ..record
        };
        updated.validate()?;
        self.store.update(updated)?;
        Ok(IssuedApiKey {
            key_id: key_id.to_string(),
            secret_literal,
            fingerprint: payload.chars().take(8).collect(),
            scopes,
        })
    }

    /// Authenticates a presented secret literal (the wire form, module
    /// header point 2): strict literal parse → current-verifier lookup →
    /// superseded-verifier lookup within the grace window → state check.
    /// On SUCCESS the last-used evidence (timestamp + subject path +
    /// `ALLOW`) is recorded on the key row and the resolved
    /// [`ApiKeyPrincipal`] is returned. On failure NOTHING is written to
    /// the key row (denial evidence is the audit trail's).
    pub fn authenticate(
        &self,
        presented_literal: &str,
        subject_path: &str,
    ) -> Result<ApiKeyPrincipal, ApiKeyError> {
        let literal = parse_secret_literal(presented_literal)?;
        let verifier = secret_verifier(literal);
        // 1. The current-verifier lookup (across tenants — the tenant
        //    comes from the row, never from the request).
        let record = self.store.get_by_verifier(&verifier)?;
        let (record, is_superseded) = match record {
            Some(record) => (record, false),
            None => {
                // 2. The superseded-verifier lookup (the grace window).
                match self.store.get_by_superseded_verifier(&verifier)? {
                    Some(record) => (record, true),
                    None => return Err(ApiKeyError::UnknownKey),
                }
            }
        };
        if record.state == "REVOKED" {
            return Err(ApiKeyError::KeyRevoked);
        }
        if is_superseded {
            let now = (self.clock_millis)();
            let closes = record
                .superseded_until_unix_millis
                .expect("a superseded lookup hit carries a grace deadline");
            if now > closes {
                return Err(ApiKeyError::SupersededPastGrace);
            }
        }
        // 3. Scope decode: a stored row carrying an unparseable scope id
        //    is a stored-record defect — fail closed.
        let mut scopes = Vec::with_capacity(record.scopes.len());
        for scope in &record.scopes {
            scopes.push(Action::parse(scope).ok_or(ApiKeyError::Backend {
                code: "E_KEY_SCOPE_DECODE".to_string(),
                message: "a registered scope id is not on the closed action surface".to_string(),
            })?);
        }
        // 4. Last-used evidence (timestamp + subject + ALLOW), written on
        //    SUCCESS only (module header point 4).
        let now = (self.clock_millis)();
        let mut updated = record.clone();
        updated.use_count += 1;
        updated.last_used = Some(LastUsedEvidence {
            last_used_at_unix_millis: now,
            last_used_subject_path: subject_path.to_string(),
            last_used_result: "ALLOW".to_string(),
        });
        self.store.update(updated)?;

        Ok(ApiKeyPrincipal {
            key_id: record.key_id,
            tenant_id: record.tenant_id,
            scopes,
        })
    }

    /// Revokes a key IMMEDIATELY (record 08 §2): state → `REVOKED`; the
    /// next authentication attempt with either secret refuses
    /// ([`ApiKeyError::KeyRevoked`] — "no works-until-refresh window").
    ///
    /// The SUPERSEDED verifier slot is deliberately RETAINED (FIM2-R1-006
    /// of the FIM final analysis): a rotated-away digest stays in the row
    /// so the OLD secret still RESOLVES through the superseded lookup and
    /// refuses at the STATE check (`KeyRevoked`). Clearing the slot would
    /// leave the old secret matching no verifier at all — an `UnknownKey`
    /// refusal that contradicts the recorded "either secret" semantics and
    /// the pinned tests. Retention widens nothing: the state check runs
    /// BEFORE the grace check for BOTH lookup paths, so a revoked key
    /// authenticates nothing regardless of any open grace window.
    pub fn revoke_key(&self, tenant_id: &TenantId, key_id: &str) -> Result<(), ApiKeyError> {
        let record = self
            .store
            .get(tenant_id, key_id)?
            .ok_or(ApiKeyError::UnknownKey)?;
        // Only the state flips; the superseded slot (when a rotation grace
        // window was open) is retained — see the doc comment.
        let updated = ApiKeyRecord {
            state: "REVOKED".to_string(),
            ..record
        };
        updated.validate()?;
        self.store.update(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
    const OTHER_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";

    fn tenant(uuid_text: &str) -> TenantId {
        TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
    }

    /// Boxes a HANDLE clone of the store (the service owns its port
    /// object; the test keeps its inspection handle on the same map).
    fn service(store: &MemoryApiKeyStore) -> ApiKeyService {
        ApiKeyService::new(
            ApiKeyConfig::default(),
            Box::new(store.clone()),
            Box::new(|| 1_700_000_000_000),
        )
    }

    /// A fixed 32-byte payload rendered by the module's own encoder, for
    /// deterministic literal-shape tests.
    fn fixed_literal(tag: u8) -> String {
        format!("{KEY_SECRET_PREFIX}{}", base64url_encode_no_pad(&[tag; 32]))
    }

    #[test]
    fn the_encoder_renders_the_exact_unpadded_base64url_shape() {
        // 32 bytes → exactly 43 characters (the record 08 §2 rule).
        let rendered = base64url_encode_no_pad(&[0x42; 32]);
        assert_eq!(rendered.len(), 43);
        assert!(rendered.bytes().all(|b| {
            b.is_ascii_uppercase()
                || b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'-'
                || b == b'_'
        }));
        // The known unpadded-base64url rendering of 32 zero bytes is 43
        // 'A' characters (the same hand-derived vector the webhook-secret
        // parser tests anchor).
        assert_eq!(base64url_encode_no_pad(&[0u8; 32]), "A".repeat(43));
        // The empty input renders to the empty string.
        assert_eq!(base64url_encode_no_pad(&[]), "");
    }

    #[test]
    fn issued_keys_authenticate_with_last_used_evidence_recorded() {
        let store = MemoryApiKeyStore::new();
        let svc = service(&store);
        let issued = svc
            .issue_key(
                tenant(TENANT_UUID),
                vec![Action::TelemetryRead.id().to_string()],
            )
            .expect("issued");
        // The literal is exactly the record 08 §2 shape.
        assert!(issued.secret_literal.starts_with(KEY_SECRET_PREFIX));
        assert_eq!(issued.secret_literal.len(), KEY_SECRET_PREFIX.len() + KEY_SECRET_BASE64URL_CHARS);
        assert_eq!(issued.fingerprint.len(), 8);
        let principal = svc
            .authenticate(&issued.secret_literal, "/api/v1/telemetry")
            .expect("authenticates");
        assert_eq!(principal.tenant_id, tenant(TENANT_UUID));
        principal.allows(Action::TelemetryRead).expect("granted");
        // The evidence is on the row (timestamp + subject + ALLOW).
        let row = store
            .get(&tenant(TENANT_UUID), &issued.key_id)
            .expect("read")
            .expect("stored");
        let evidence = row.last_used.expect("evidence recorded on success");
        assert_eq!(evidence.last_used_at_unix_millis, 1_700_000_000_000);
        assert_eq!(evidence.last_used_subject_path, "/api/v1/telemetry");
        assert_eq!(evidence.last_used_result, "ALLOW");
        assert_eq!(row.use_count, 1);
    }

    #[test]
    fn rotation_keeps_the_old_key_authenticating_until_the_grace_closes() {
        let store = MemoryApiKeyStore::new();
        // A shared, controllable clock (the injected-clock discipline —
        // the service never reads a wall clock in decision logic).
        let clock = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(1_700_000_000_000));
        let clock_handle = clock.clone();
        let svc = ApiKeyService::new(
            ApiKeyConfig { rotation_grace_seconds: 3_600 },
            Box::new(store),
            Box::new(move || clock_handle.load(std::sync::atomic::Ordering::SeqCst)),
        );
        let issued = svc
            .issue_key(
                tenant(TENANT_UUID),
                vec![Action::TelemetryRead.id().to_string()],
            )
            .expect("issued");
        let rotated = svc.rotate_key(&tenant(TENANT_UUID), &issued.key_id).expect("rotated");
        assert_ne!(rotated.secret_literal, issued.secret_literal);
        // The NEW secret authenticates; the OLD secret still authenticates
        // inside the grace window (in-flight callers do not drop).
        svc.authenticate(&rotated.secret_literal, "/api/v1/rotated").expect("new key");
        svc.authenticate(&issued.secret_literal, "/api/v1/in-flight").expect("old key in grace");
        // Past the grace window the old secret refuses with the named
        // error (jump well past the one-hour window).
        clock.fetch_add(3_600 * 1_000 * 2, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            svc.authenticate(&issued.secret_literal, "/api/v1/late"),
            Err(ApiKeyError::SupersededPastGrace),
        );
        // And the NEW secret still authenticates after the grace closed.
        svc.authenticate(&rotated.secret_literal, "/api/v1/after-grace")
            .expect("new key unaffected");
    }

    #[test]
    fn revocation_is_immediate_for_both_secrets() {
        let store = MemoryApiKeyStore::new();
        let svc = service(&store);
        let issued = svc
            .issue_key(
                tenant(TENANT_UUID),
                vec![Action::TelemetryRead.id().to_string()],
            )
            .expect("issued");
        let rotated = svc.rotate_key(&tenant(TENANT_UUID), &issued.key_id).expect("rotated");
        svc.revoke_key(&tenant(TENANT_UUID), &issued.key_id).expect("revoked");
        assert_eq!(
            svc.authenticate(&rotated.secret_literal, "/p"),
            Err(ApiKeyError::KeyRevoked),
            "the current secret refuses immediately",
        );
        // The OLD (superseded) secret refuses as KeyRevoked too — through
        // the RETAINED superseded slot: the lookup hits, the state check
        // precedes the grace check, and the refusal is the named revoked
        // class, never UnknownKey (FIM2-R1-006 of the FIM final analysis;
        // clearing the slot on revoke was the recorded defect).
        assert_eq!(
            svc.authenticate(&issued.secret_literal, "/p"),
            Err(ApiKeyError::KeyRevoked),
            "the rotated-away secret refuses immediately too",
        );
        // The retention itself, on the row: the superseded pair stays
        // Some/Some (the record's own agreement rule kept satisfied), the
        // old digest still in the superseded slot, the state flipped.
        let row = store
            .get(&tenant(TENANT_UUID), &issued.key_id)
            .expect("read")
            .expect("stored");
        assert_eq!(row.state, "REVOKED");
        assert_eq!(
            row.superseded_verifier_sha256_hex.as_deref(),
            Some(secret_verifier(&issued.secret_literal).as_str()),
            "the superseded slot retains the OLD digest after revocation",
        );
        assert!(row.superseded_until_unix_millis.is_some());
    }

    #[test]
    fn malformed_and_unknown_literals_are_named_refusals() {
        let store = MemoryApiKeyStore::new();
        let svc = service(&store);
        let issued = svc
            .issue_key(
                tenant(TENANT_UUID),
                vec![Action::TelemetryRead.id().to_string()],
            )
            .expect("issued");
        assert_eq!(svc.authenticate("", "/p"), Err(ApiKeyError::MalformedKey));
        assert_eq!(svc.authenticate("not-a-key", "/p"), Err(ApiKeyError::MalformedKey));
        // Wrong prefix casing is refused (the prefix is EXACT).
        let upper = issued.secret_literal.to_uppercase();
        assert_eq!(svc.authenticate(&upper, "/p"), Err(ApiKeyError::MalformedKey));
        // A truncated payload is refused.
        assert_eq!(
            svc.authenticate(&issued.secret_literal[..issued.secret_literal.len() - 1], "/p"),
            Err(ApiKeyError::MalformedKey),
        );
        // A well-formed literal that matches nothing is UnknownKey — and
        // the refusal NEVER echoes the presented value.
        let stranger = fixed_literal(0x11);
        assert_eq!(svc.authenticate(&stranger, "/p"), Err(ApiKeyError::UnknownKey));
        let refused = svc.authenticate(&stranger, "/p").expect_err("refused");
        assert!(!refused.to_string().contains(&stranger));
    }

    #[test]
    fn the_never_granted_scope_names_are_structurally_refused_at_issuance() {
        let store = MemoryApiKeyStore::new();
        let svc = service(&store);
        assert_eq!(
            svc.issue_key(tenant(TENANT_UUID), vec!["dispatch.issue".to_string()]),
            Err(ApiKeyError::InvalidScope { index: 0 }),
        );
        assert_eq!(
            svc.issue_key(
                tenant(TENANT_UUID),
                vec![
                    Action::TelemetryRead.id().to_string(),
                    "network.any".to_string(),
                ]
            ),
            Err(ApiKeyError::InvalidScope { index: 1 }),
        );
        assert!(store
            .get_by_verifier("")
            .expect("read")
            .is_none(), "nothing was written for a refused issuance");
    }

    #[test]
    fn the_store_holds_only_verifiers_never_raw_key_material() {
        // FIM-F08.05 (the secret-posture pin, API-key half): every stored
        // record's Debug — the only surface a dump would render — carries
        // digests and fingerprints, never a `fk_` literal.
        let store = MemoryApiKeyStore::new();
        let svc = service(&store);
        let issued = svc
            .issue_key(
                tenant(TENANT_UUID),
                vec![Action::TelemetryRead.id().to_string()],
            )
            .expect("issued");
        svc.authenticate(&issued.secret_literal, "/p").expect("authenticates");
        let keys = store.keys.lock().expect("store lock");
        assert_eq!(keys.len(), 1);
        for record in keys.values() {
            let debugged = format!("{record:?}");
            assert!(
                !debugged.contains(&issued.secret_literal),
                "the raw secret never appears in a stored record representation"
            );
            assert!(
                !debugged.contains("fk_"),
                "no `fk_` literal (raw or otherwise) appears in a stored record"
            );
        }
    }

    #[test]
    fn generated_material_carries_the_record_08_shapes() {
        for _ in 0..8 {
            let (key_id, secret_literal) = generate_key_material();
            assert!(key_id.starts_with(KEY_ID_PREFIX));
            assert_eq!(key_id.len(), KEY_ID_PREFIX.len() + KEY_ID_PAYLOAD_CHARS);
            assert!(key_id[KEY_ID_PREFIX.len()..]
                .bytes()
                .all(|b| BASE32_ALPHABET.contains(&b)));
            assert!(secret_literal.starts_with(KEY_SECRET_PREFIX));
            assert_eq!(
                secret_literal.len(),
                KEY_SECRET_PREFIX.len() + KEY_SECRET_BASE64URL_CHARS
            );
        }
    }

    #[test]
    fn error_names_are_pinned() {
        assert_eq!(ApiKeyError::MalformedKey.name(), "API_KEY_MALFORMED_KEY");
        assert_eq!(ApiKeyError::UnknownKey.name(), "API_KEY_UNKNOWN_KEY");
        assert_eq!(ApiKeyError::KeyRevoked.name(), "API_KEY_REVOKED");
        assert_eq!(ApiKeyError::SupersededPastGrace.name(), "API_KEY_SUPERSEDED_PAST_GRACE");
        // The composition mapping onto the existing resolver vocabulary.
        let mapped: AuthError = ApiKeyError::UnknownKey.into();
        assert_eq!(mapped, AuthError::InvalidCredentials);
    }
}
