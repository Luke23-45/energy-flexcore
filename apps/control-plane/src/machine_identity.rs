// P13.07 / FIM-F08.01 — OAuth2 client-credentials MACHINE IDENTITY: client
//             registration, short-lived JWT issuance, validation, and
//             revocation for grid-operator and market-adapter integrations.
//
// Implemented under FIM-F08 (final implementation milestone; the FIM
// standing decisions live in
// `docs/releases/implementation_ledger/README.md`). Contract source: the
// P13.07 decision record
// `docs/phases-11-20/phase-13-enterprise-identity-integration/records/
// 07_p13_07_machine_identity.md` — the flow shape (RFC 6749 §4.4
// client-credentials; RFC 9068-shaped access token), the scope-subset rule
// (a strict subset of the EXISTING `flexcore-authz::action::Action`
// vocabulary — no new scope space is defined here), and the revocation
// semantics (record §4) are consumed BY CITATION, never re-decided.
//
// **WRITTEN-UNRUN.** This module has never been compiled or executed: no
// client is registered, no secret exists, no token has been issued,
// validated, or revoked, and no machine call has ever been authorized. The
// first human build/test is the gate (program conventions §1). LIVE TOKEN
// INTEROP (a real integration partner presenting these tokens) IS
// UNPROVEN — **HUMAN LAB GATE**.
//
// Design points (each is a recorded decision of this implementation):
//
// 1. TOKEN SHAPE — a JWT access token signed HS256 by a deployment-injected
//    symmetric issuer key (`EncodingKey::from_secret`; the key material
//    lands in the P11.16 secrets posture — Vault DEFAULT / Compose
//    file-per-host floor — NEVER a database column, the same posture as the
//    P13.08 webhook secrets). Claims: `iss` (the FlexCore token endpoint's
//    issuer), `sub` = `integration:<client_id>` (record 01's subject
//    grammar), `aud` (the control-plane API), `exp`/`iat` (the registered
//    15-minute TTL recommendation — a REGISTERED DEFAULT, X-8 discipline,
//    never a tested optimum), `tenant` (the owning tenant as canonical
//    hyphenated UUID text — informational), `scope` (space-separated
//    canonical action ids, the RFC 9068 style), and `epoch` (the client's
//    credential epoch at issuance — see 3). HS256 (not the human path's
//    RS256) is deliberate: machine-identity tokens are minted and consumed
//    INSIDE this service, so the symmetric key never needs a JWKS surface;
//    the human OIDC path (`crate::oidc`) is untouched.
//
// 2. VALIDATION ORDER — decode + signature + `exp` (jsonwebtoken
//    `Validation`, leeway from the config, registered default 60 s like
//    the OIDC resolver) FIRST, then the named `iss`/`aud` refusals
//    (defense in depth on top of the crate's Validation, mirroring
//    `crate::oidc`), THEN the registry check: the client must exist,
//    be enabled, and carry the token's exact credential epoch. The tenant
//    is resolved FROM THE REGISTRY (the client registration), never from
//    the token claim alone (record 01 §4's tenant-binding rule); a token
//    whose `tenant` claim disagrees with the registry is refused
//    (fail closed), never trusted.
//
// 3. REVOCATION — each client carries a monotonically increasing
//    `credential_epoch`. `revoke_client` sets state → disabled AND bumps
//    the epoch, so every token issued before the revocation refuses at
//    validation IMMEDIATELY (the registry check happens before any scope
//    grant is consulted — fail closed). Issued tokens would in any case
//    die at `exp` (the short TTL bounds the exposure window; record 07
//    §4). Secret ROTATION deliberately does NOT bump the epoch: issued
//    tokens stay valid until `exp` (the record 07 rotation overlap —
//    in-flight callers do not drop); only the issuance credential
//    changes.
//
// 4. SECRET POSTURE — a client's registration secret is shown ONCE at
//    registration (and at each rotation); the registry stores ONLY the
//    SHA-256 hex digest of the secret string (the P13.08 digest-at-rest
//    rule, consumed by citation for this credential class) plus a short
//    display prefix. No record type in this module carries raw secret
//    material, and the raw secret is never echoed in an error (errors name
//    the refusal class, never the presented value).
//
// 5. NEVER-GRANTED SET — byte-identical: `dispatch.authorize`,
//    `dispatch.issue`, `secret.read`, `network.any` are NOT on the
//    `Action` surface (`Action::parse` → `None`), so no registration scope
//    string can ever grant them; a registration naming one is refused at
//    registration as `InvalidScope`, and a machine principal's authority
//    is exactly its parsed `Action` set (see [`MachinePrincipal::allows`]).
//    A market adapter reaches dispatch reach ONLY through the command
//    lifecycle (P4.11–P4.13; ADR-010 boundary, record 07 §6.5).
//
// 6. PERSISTENCE — the registry follows the repo's store-trait +
//    in-memory-default pattern (like `MemoryWebhookDeliveryStore`):
//    [`MachineClientStore`] is the port; [`MemoryMachineClientStore`] is
//    the in-memory reference implementation. A Postgres-backed registry is
//    a RECORDED FAIL-CLOSED SEAM (not built in this wave): until a real
//    adapter store exists, the composition binds the in-memory store and
//    every registration is lost on restart — clients must re-register;
//    fail-closed because an empty registry refuses every token.
//
// 7. ERROR VOCABULARY — the closed [`MachineIdentityError`] names every
//    refusal class; the composition maps it onto the EXISTING
//    [`crate::auth::AuthError`] (`InvalidCredentials` — a reasoned 401)
//    through the provided `From` impl, so the resolver seam vocabulary is
//    unchanged. No error ever echoes a presented secret or token.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::auth::AuthError;
use crate::webhook_signing::sha256;

/// The registered token TTL in seconds (record 07 §2: 15 minutes — a
/// REGISTERED RECOMMENDATION per [S-75]'s short-lived-credential posture;
/// X-8 discipline: written before any run could depend on it; changing it
/// is a recorded amendment to the P13.07 decision record).
pub const TOKEN_TTL_SECONDS_DEFAULT: i64 = 900;

/// The registered `exp`/`nbf` validation leeway in seconds (the same
/// registered default the OIDC resolver carries; `crate::oidc`).
pub const VALIDATION_LEEWAY_SECONDS_DEFAULT: u64 = 60;

/// The subject grammar prefix (record 01 §2 / record 07 §2: a
/// client-credentials token's subject is `integration:<client_id>`, which
/// is what distinguishes it from a human token at the resolver surface).
pub const MACHINE_SUBJECT_PREFIX: &str = "integration:";

// --- Errors -------------------------------------------------------------------

/// Errors of the machine-identity service. Closed vocabulary; every
/// variant names its refusal class and NEVER echoes a presented secret or
/// token (they may be live credentials).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineIdentityError {
    /// The presented registration arguments were malformed (`field` names
    /// the problem). Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A requested scope string is not on the closed `Action` surface —
    /// which structurally includes the never-granted names
    /// (`dispatch.authorize`, `dispatch.issue`, `secret.read`,
    /// `network.any`). Registration and scope changes refuse; nothing was
    /// written.
    InvalidScope {
        /// The REJECTED scope's index in the submitted list (never the
        /// value itself).
        index: usize,
    },
    /// The named client does not exist in the presented tenant scope.
    /// Existence never leaks across a tenant boundary (P0.16 rule 5): a
    /// wrong tenant and an unknown client are indistinguishable.
    UnknownClient,
    /// The presented client secret did not match the stored verifier.
    InvalidSecret,
    /// The client is registered but its state is `disabled` (the emergency
    /// kill-switch landed; record 07 §4).
    ClientDisabled,
    /// The token's credential epoch does not match the registry: the
    /// client was revoked (or otherwise epoch-bumped) after the token was
    /// issued. Issued-before-revocation tokens refuse HERE — the registry
    /// check runs before any scope grant is consulted.
    ClientRevoked,
    /// The token failed structural/signature validation (malformed JWT,
    /// bad signature, wrong algorithm).
    InvalidToken,
    /// The token's `exp` has passed (beyond the configured leeway).
    ExpiredToken,
    /// The token's `iss` is not the configured issuer.
    WrongIssuer,
    /// The token's `aud` does not name the configured control-plane API.
    WrongAudience,
    /// The token's `tenant` claim disagrees with the client's registered
    /// tenant — fail closed, never trusted (the tenant comes from the
    /// registry).
    TenantMismatch,
    /// The authenticated principal attempted an action outside its
    /// registered scope subset.
    ScopeNotGranted {
        /// The canonical action id that was refused (a parseable action on
        /// the closed surface — never the never-granted names, which cannot
        /// reach this variant).
        action: &'static str,
    },
    /// The store backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl MachineIdentityError {
    /// The stable machine-readable refusal name (records, metrics).
    pub fn name(&self) -> &'static str {
        match self {
            MachineIdentityError::InvalidRecord { .. } => "MACHINE_IDENTITY_INVALID_RECORD",
            MachineIdentityError::InvalidScope { .. } => "MACHINE_IDENTITY_INVALID_SCOPE",
            MachineIdentityError::UnknownClient => "MACHINE_IDENTITY_UNKNOWN_CLIENT",
            MachineIdentityError::InvalidSecret => "MACHINE_IDENTITY_INVALID_SECRET",
            MachineIdentityError::ClientDisabled => "MACHINE_IDENTITY_CLIENT_DISABLED",
            MachineIdentityError::ClientRevoked => "MACHINE_IDENTITY_CLIENT_REVOKED",
            MachineIdentityError::InvalidToken => "MACHINE_IDENTITY_INVALID_TOKEN",
            MachineIdentityError::ExpiredToken => "MACHINE_IDENTITY_EXPIRED_TOKEN",
            MachineIdentityError::WrongIssuer => "MACHINE_IDENTITY_WRONG_ISSUER",
            MachineIdentityError::WrongAudience => "MACHINE_IDENTITY_WRONG_AUDIENCE",
            MachineIdentityError::TenantMismatch => "MACHINE_IDENTITY_TENANT_MISMATCH",
            MachineIdentityError::ScopeNotGranted { .. } => "MACHINE_IDENTITY_SCOPE_NOT_GRANTED",
            MachineIdentityError::Backend { .. } => "MACHINE_IDENTITY_BACKEND",
        }
    }
}

impl std::fmt::Display for MachineIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MachineIdentityError::InvalidRecord { field } => {
                write!(f, "the machine-client record is invalid: '{field}' is missing or malformed")
            }
            MachineIdentityError::InvalidScope { index } => {
                write!(
                    f,
                    "requested scope at index {index} is not on the closed action surface \
                     (never-granted names are structurally excluded)"
                )
            }
            MachineIdentityError::UnknownClient => {
                write!(f, "the machine client does not exist in this tenant scope")
            }
            MachineIdentityError::InvalidSecret => {
                write!(f, "the presented client secret does not verify")
            }
            MachineIdentityError::ClientDisabled => {
                write!(f, "the machine client is disabled (revocation state)")
            }
            MachineIdentityError::ClientRevoked => {
                write!(f, "the machine client was revoked after this token was issued")
            }
            MachineIdentityError::InvalidToken => {
                write!(f, "the presented token failed signature or structural validation")
            }
            MachineIdentityError::ExpiredToken => write!(f, "the presented token has expired"),
            MachineIdentityError::WrongIssuer => {
                write!(f, "the token issuer is not the configured machine-identity issuer")
            }
            MachineIdentityError::WrongAudience => {
                write!(f, "the token audience does not name this control-plane API")
            }
            MachineIdentityError::TenantMismatch => {
                write!(f, "the token tenant claim disagrees with the client registration")
            }
            MachineIdentityError::ScopeNotGranted { action } => {
                write!(f, "the principal's scope subset does not include '{action}'")
            }
            MachineIdentityError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for MachineIdentityError {}

/// The composition mapping onto the EXISTING resolver-error vocabulary:
/// every machine-identity refusal collapses to the reasoned 401
/// (`AuthError::InvalidCredentials`) the `IdentityResolver` seam already
/// carries. The named class stays on THIS type (for records and metrics);
/// the seam vocabulary is unchanged.
impl From<MachineIdentityError> for AuthError {
    fn from(_err: MachineIdentityError) -> Self {
        AuthError::InvalidCredentials
    }
}

// --- Records ------------------------------------------------------------------

/// One registered machine client (record 07 §4's registration row). The
/// type carries NO raw secret material: only the SHA-256 hex VERIFIER of
/// the secret string and a short display prefix (the first 4 characters
/// after the `fkint_` prefix — the fingerprint form record 08 §4 prescribes
/// for display, consumed by citation for this credential class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineClientRecord {
    /// The client identifier (opaque non-empty text, as registered). Public
    /// — travels in audit records, safe to log.
    pub client_id: String,
    /// The owning tenant (record 07 §2: one client belongs to exactly one
    /// tenant — a client never crosses the tenant boundary).
    pub tenant_id: TenantId,
    /// The allowed scope subset, as canonical action ids (each one a
    /// parseable `Action`; registration refuses anything else).
    pub allowed_scopes: Vec<String>,
    /// The SHA-256 hex digest of the client secret string — the VERIFIER.
    /// The raw secret is shown ONCE at registration/rotation and is never
    /// stored retrievable (P13.08 digest-at-rest, consumed by citation).
    pub secret_verifier_sha256_hex: String,
    /// The secret's display prefix (first 4 characters of the secret's
    /// payload) — the fingerprint an admin UI renders; never the secret.
    pub secret_prefix: String,
    /// Whether the client may authenticate at all (`false` = the emergency
    /// kill-switch has landed; issuance and validation both refuse).
    pub enabled: bool,
    /// The credential epoch: bumped on every revocation; tokens carry the
    /// epoch they were issued under, and a mismatch refuses at validation
    /// (the immediate-revocation rule, record 07 §4).
    pub credential_epoch: u64,
}

impl MachineClientRecord {
    /// Validates the non-blank/shape rules of a registration row (the
    /// store and the service both call it — one authority).
    pub fn validate(&self) -> Result<(), MachineIdentityError> {
        if self.client_id.trim().is_empty() {
            return Err(MachineIdentityError::InvalidRecord { field: "client_id" });
        }
        if self.secret_verifier_sha256_hex.len() != 64
            || !self
                .secret_verifier_sha256_hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(MachineIdentityError::InvalidRecord {
                field: "secret_verifier_sha256_hex",
            });
        }
        Ok(())
    }
}

/// The parsed, registry-verified principal a validated machine token
/// resolves to. The tenant is the REGISTRY's tenant (never taken from the
/// token claim alone); the scope set is the registry's parsed action set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachinePrincipal {
    /// The client identifier (the token's `sub` minus the
    /// [`MACHINE_SUBJECT_PREFIX`]).
    pub client_id: String,
    /// The full subject string as it appears in audit records
    /// (`integration:<client_id>`).
    pub subject: String,
    /// The owning tenant, from the registry.
    pub tenant_id: TenantId,
    /// The granted actions (the registry's scope subset, parsed against
    /// the closed surface).
    pub scopes: Vec<Action>,
}

impl MachinePrincipal {
    /// The scope-grant check for a machine-identity call: allow only when
    /// the action is IN the client's registered subset. Deny-by-default;
    /// the refusal is the named [`MachineIdentityError::ScopeNotGranted`].
    pub fn allows(&self, action: Action) -> Result<(), MachineIdentityError> {
        if self.scopes.contains(&action) {
            Ok(())
        } else {
            Err(MachineIdentityError::ScopeNotGranted { action: action.id() })
        }
    }
}

// --- The registry port --------------------------------------------------------

/// The machine-client registry port. Persistence follows the store-trait +
/// in-memory-default pattern: [`MemoryMachineClientStore`] is the reference
/// implementation; a Postgres-backed registry is a recorded fail-closed
/// seam (module header, point 6).
pub trait MachineClientStore: Send + Sync {
    /// Inserts a registration row (the caller has already validated scope
    /// strings and built the verifier). A duplicate `client_id` is a
    /// named failure — registration is explicit, never an upsert.
    fn put(&self, record: MachineClientRecord) -> Result<(), MachineIdentityError>;

    /// Replaces the stored registration row (revocation and other
    /// administrative state transitions — the emergency kill-switch's
    /// state flip + credential-epoch bump travels through HERE, never
    /// through `put`). An unknown `client_id` is a named failure — update
    /// is explicit, never an implicit insert (the same shape
    /// `ApiKeyStore::update` records; FIM2-R1-002 of the FIM final
    /// analysis added this method because revoking through `put` could
    /// never succeed).
    fn update(&self, record: MachineClientRecord) -> Result<(), MachineIdentityError>;

    /// The registration row for `client_id` IN THE CALLER'S TENANT scope;
    /// a foreign-tenant client reads as absent (P0.16 rule 5).
    fn get(
        &self,
        tenant_id: &TenantId,
        client_id: &str,
    ) -> Result<Option<MachineClientRecord>, MachineIdentityError>;
}

/// The in-memory [`MachineClientStore`] (the reference implementation and
/// the test path). Ordering: `BTreeMap` keyed by client id (deterministic —
/// conventions §6.6 kernel patterns). `Clone` is a HANDLE clone (the map
/// lives behind an `Arc`), so two service instances and a test's
/// inspection handle can share one registry — the repo's shared-fake
/// pattern.
#[derive(Clone, Default)]
pub struct MemoryMachineClientStore {
    clients: Arc<Mutex<BTreeMap<String, MachineClientRecord>>>,
}

impl MemoryMachineClientStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MachineClientStore for MemoryMachineClientStore {
    fn put(&self, record: MachineClientRecord) -> Result<(), MachineIdentityError> {
        record.validate()?;
        let mut clients = self.clients.lock().expect("poisoned mutex in store fake");
        if clients.contains_key(&record.client_id) {
            return Err(MachineIdentityError::Backend {
                code: "E_CLIENT_EXISTS".to_string(),
                message: format!(
                    "a machine client with id {} is already registered; registration is \
                     explicit, never an upsert",
                    record.client_id
                ),
            });
        }
        clients.insert(record.client_id.clone(), record);
        Ok(())
    }

    fn update(&self, record: MachineClientRecord) -> Result<(), MachineIdentityError> {
        record.validate()?;
        let mut clients = self.clients.lock().expect("poisoned mutex in store fake");
        if !clients.contains_key(&record.client_id) {
            return Err(MachineIdentityError::Backend {
                code: "E_CLIENT_MISSING".to_string(),
                message: format!(
                    "no machine client with id {} is registered; update is explicit",
                    record.client_id
                ),
            });
        }
        clients.insert(record.client_id.clone(), record);
        Ok(())
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        client_id: &str,
    ) -> Result<Option<MachineClientRecord>, MachineIdentityError> {
        let clients = self.clients.lock().expect("poisoned mutex in store fake");
        Ok(clients.get(client_id).and_then(|record| {
            // The tenant scopes the lookup like the Postgres RLS filter
            // would: a foreign-tenant client reads as absent (P0.16
            // rule 5).
            (record.tenant_id == *tenant_id).then(|| record.clone())
        }))
    }
}

// --- Claims and tokens --------------------------------------------------------

/// The JWT claims of a machine-identity access token (module header
/// point 1). Serialized into the compact JWS by [`jsonwebtoken::encode`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineTokenClaims {
    /// The issuer: the FlexCore machine-identity token endpoint.
    pub iss: String,
    /// The subject: `integration:<client_id>`.
    pub sub: String,
    /// The audience: this deployment's control-plane API.
    pub aud: String,
    /// Expiry (unix seconds).
    pub exp: i64,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// The owning tenant as canonical hyphenated UUID text. INFORMATIONAL
    /// at validation: the tenant is resolved from the registry (module
    /// header point 2); a claim that disagrees with the registry refuses.
    pub tenant: String,
    /// The granted scope subset, space-separated canonical action ids.
    pub scope: String,
    /// The client's credential epoch at issuance (the revocation check's
    /// fast path; the registry comparison is the authority).
    pub epoch: u64,
}

/// The machine-identity configuration (one deployment). All defaults are
/// REGISTERED RECOMMENDATIONS (X-8), never tuned values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineIdentityConfig {
    /// The issuer (`iss`) the service mints and expects.
    pub issuer: String,
    /// The control-plane API audience (`aud`).
    pub audience: String,
    /// The token TTL in seconds. DEFAULT [`TOKEN_TTL_SECONDS_DEFAULT`]
    /// (15 minutes — record 07 §2's registered recommendation).
    pub token_ttl_seconds: i64,
    /// The `exp` validation leeway in seconds. DEFAULT
    /// [`VALIDATION_LEEWAY_SECONDS_DEFAULT`].
    pub leeway_seconds: u64,
}

impl Default for MachineIdentityConfig {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            audience: String::new(),
            token_ttl_seconds: TOKEN_TTL_SECONDS_DEFAULT,
            leeway_seconds: VALIDATION_LEEWAY_SECONDS_DEFAULT,
        }
    }
}

/// The machine-identity service: registration, issuance, validation,
/// revocation, over the injected registry and clock. Constructed sealed
/// (all fields at construction; no builder creep).
pub struct MachineIdentityService {
    config: MachineIdentityConfig,
    /// The symmetric issuer key (module header point 1): injected from the
    /// P11.16 secrets posture — never stored in any database. `Debug` is
    /// not derived on this type (the material can never be logged).
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    registry: Box<dyn MachineClientStore>,
    clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl MachineIdentityService {
    /// Composes the service over the injected issuer key, registry, and
    /// clock. NOTHING is read from a database here — the key arrives from
    /// the deployment's secrets posture, the registry from the
    /// composition root.
    pub fn new(
        config: MachineIdentityConfig,
        issuer_secret: &[u8],
        registry: Box<dyn MachineClientStore>,
        clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            config,
            encoding_key: EncodingKey::from_secret(issuer_secret),
            decoding_key: DecodingKey::from_secret(issuer_secret),
            registry,
            clock_millis,
        }
    }

    /// The SHA-256 hex verifier of a secret string (the digest-at-rest
    /// rule; the raw secret is never stored).
    fn secret_verifier(secret: &str) -> String {
        crate::webhook_signing::hex_encode_digest(&sha256(secret.as_bytes()))
    }

    /// Validates every requested scope string against the closed `Action`
    /// surface. A scope that does not parse — which structurally includes
    /// every never-granted name (`dispatch.authorize`, `dispatch.issue`,
    /// `secret.read`, `network.any`) — is a named [`InvalidScope`]
    /// refusal. A duplicate scope string is equally refused (a duplicated
    /// grant is a registration defect, not a wider grant).
    fn validate_scopes(scopes: &[String]) -> Result<(), MachineIdentityError> {
        for (index, scope) in scopes.iter().enumerate() {
            if Action::parse(scope).is_none() {
                return Err(MachineIdentityError::InvalidScope { index });
            }
        }
        let mut seen: Vec<&str> = Vec::with_capacity(scopes.len());
        for (index, scope) in scopes.iter().enumerate() {
            if seen.contains(&scope.as_str()) {
                return Err(MachineIdentityError::InvalidScope { index });
            }
            seen.push(scope.as_str());
        }
        Ok(())
    }

    /// Registers a machine client: builds the record, verifies the scope
    /// subset, stores the VERIFIER (never the raw secret), and returns the
    /// raw secret ONCE (module header point 4). The secret is
    /// caller-supplied (generated by the deployment's CSPRNG path — this
    /// module never invents credential material, unlike the API-key tier
    /// which generates its own; a client secret may come from the partner
    /// or the admin UI, and minting it here would create a second secret
    /// source to audit).
    pub fn register_client(
        &self,
        client_id: &str,
        tenant_id: TenantId,
        allowed_scopes: Vec<String>,
        secret: &str,
    ) -> Result<(), MachineIdentityError> {
        Self::validate_scopes(&allowed_scopes)?;
        if secret.trim().is_empty() {
            return Err(MachineIdentityError::InvalidRecord { field: "secret" });
        }
        let record = MachineClientRecord {
            client_id: client_id.to_string(),
            tenant_id,
            allowed_scopes,
            secret_verifier_sha256_hex: Self::secret_verifier(secret),
            secret_prefix: secret.chars().take(4).collect(),
            enabled: true,
            credential_epoch: 0,
        };
        record.validate()?;
        self.registry.put(record)
    }

    /// Issues a client-credentials access token for `client_id` in
    /// `tenant_id`: authenticates the client (existence in the tenant
    /// scope, enabled state, presented secret against the stored
    /// verifier) and mints the short-lived JWT (module header points 1-2).
    pub fn issue_client_credentials_token(
        &self,
        tenant_id: &TenantId,
        client_id: &str,
        presented_secret: &str,
    ) -> Result<String, MachineIdentityError> {
        let record = self
            .registry
            .get(tenant_id, client_id)?
            .ok_or(MachineIdentityError::UnknownClient)?;
        if !record.enabled {
            return Err(MachineIdentityError::ClientDisabled);
        }
        // Constant-time digest comparison: the presented secret's digest
        // must equal the stored verifier.
        let presented_verifier = Self::secret_verifier(presented_secret);
        if !constant_time_eq(presented_verifier.as_bytes(), record.secret_verifier_sha256_hex.as_bytes()) {
            return Err(MachineIdentityError::InvalidSecret);
        }
        let now_seconds = (self.clock_millis)().div_euclid(1_000);
        let claims = MachineTokenClaims {
            iss: self.config.issuer.clone(),
            sub: format!("{MACHINE_SUBJECT_PREFIX}{client_id}"),
            aud: self.config.audience.clone(),
            iat: now_seconds,
            exp: now_seconds + self.config.token_ttl_seconds,
            tenant: tenant_id.uuid().to_hyphenated(),
            scope: record.allowed_scopes.join(" "),
            epoch: record.credential_epoch,
        };
        jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|_| MachineIdentityError::Backend {
                code: "E_TOKEN_ENCODE".to_string(),
                message: "the machine-identity token could not be encoded".to_string(),
            })
    }

    /// Validates a presented machine-identity token and resolves it to a
    /// [`MachinePrincipal`] (module header point 2's order): crate
    /// decode (signature, `exp`, `iss`, `aud` — each refusal named on
    /// THIS surface), then the registry check (existence in scope,
    /// enabled, epoch match, tenant agreement). The tenant and the scope
    /// set both come from the REGISTRY.
    pub fn validate_machine_token(
        &self,
        token: &str,
    ) -> Result<MachinePrincipal, MachineIdentityError> {
        // 1. Decode: signature + the crate's claim validation.
        let header = match jsonwebtoken::decode_header(token) {
            Ok(header) => header,
            Err(_) => return Err(MachineIdentityError::InvalidToken),
        };
        // 1a. The algorithm allow-list: machine-identity tokens are HS256
        //     (module header point 1). Anything else is refused BEFORE any
        //     key material is touched (algorithm-confusion defense).
        if header.alg != Algorithm::HS256 {
            return Err(MachineIdentityError::InvalidToken);
        }
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[self.config.issuer.clone()]);
        validation.set_audience(&[self.config.audience.clone()]);
        validation.leeway = self.config.leeway_seconds;
        validation.validate_exp = true;
        validation.algorithms = vec![Algorithm::HS256];
        let data = match jsonwebtoken::decode::<MachineTokenClaims>(
            token,
            &self.decoding_key,
            &validation,
        ) {
            Ok(data) => data,
            Err(err) => {
                return Err(match err.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        MachineIdentityError::ExpiredToken
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                        MachineIdentityError::WrongIssuer
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                        MachineIdentityError::WrongAudience
                    }
                    _ => MachineIdentityError::InvalidToken,
                })
            }
        };
        let claims = data.claims;

        // 2. Named issuer/audience refusals (defense in depth on top of
        //    the crate's Validation — the same posture `crate::oidc`
        //    carries).
        if claims.iss != self.config.issuer {
            return Err(MachineIdentityError::WrongIssuer);
        }
        if claims.aud != self.config.audience {
            return Err(MachineIdentityError::WrongAudience);
        }

        // 3. The subject must carry the machine grammar.
        let client_id = claims
            .sub
            .strip_prefix(MACHINE_SUBJECT_PREFIX)
            .filter(|id| !id.trim().is_empty())
            .ok_or(MachineIdentityError::InvalidToken)?
            .to_string();

        // 4. The registry check (BEFORE any scope grant is consulted):
        //    existence — the token's tenant claim first scopes the lookup
        //    (a token cannot make us look in the WRONG tenant, but the
        //    refusal must not leak which side is wrong, so the
        //    mismatched-claim case and the unknown-client case both land
        //    on their named errors only after the scope-consistent lookup
        //    attempt); enabled state; epoch match; tenant agreement.
        let claimed_tenant = FlexUuid::parse(&claims.tenant)
            .map(TenantId::new)
            .ok_or(MachineIdentityError::InvalidToken)?;
        let record = match self.registry.get(&claimed_tenant, &client_id)? {
            Some(record) => record,
            None => return Err(MachineIdentityError::UnknownClient),
        };
        if !record.enabled {
            return Err(MachineIdentityError::ClientDisabled);
        }
        if record.credential_epoch != claims.epoch {
            return Err(MachineIdentityError::ClientRevoked);
        }
        if record.tenant_id != claimed_tenant {
            return Err(MachineIdentityError::TenantMismatch);
        }

        // 5. The scope set is the REGISTRY's (never the token claim's):
        //    parse every stored scope id; a registry row carrying an
        //    unparseable scope is a stored-record defect — fail closed.
        let mut scopes = Vec::with_capacity(record.allowed_scopes.len());
        for scope in &record.allowed_scopes {
            scopes.push(Action::parse(scope).ok_or(MachineIdentityError::Backend {
                code: "E_REGISTRY_SCOPE_DECODE".to_string(),
                message: "a registered scope id is not on the closed action surface".to_string(),
            })?);
        }

        Ok(MachinePrincipal {
            subject: claims.sub,
            client_id,
            tenant_id: record.tenant_id,
            scopes,
        })
    }

    /// Revokes a machine client (the emergency kill-switch, record 07
    /// §4): state → disabled AND the credential epoch bumps, so every
    /// token issued before the revocation refuses at validation
    /// IMMEDIATELY (the [`validate_machine_token`] registry check).
    pub fn revoke_client(
        &self,
        tenant_id: &TenantId,
        client_id: &str,
    ) -> Result<(), MachineIdentityError> {
        let record = self
            .registry
            .get(tenant_id, client_id)?
            .ok_or(MachineIdentityError::UnknownClient)?;
        // The update path, NOT `put` (FIM2-R1-002 of the FIM final
        // analysis): `put` is insert-only and refuses an existing
        // `client_id` — routing the kill-switch through it made every
        // revocation fail with `E_CLIENT_EXISTS` and left the epoch
        // unbumped. `update` replaces the registered row in place, so the
        // state flip + epoch bump actually land and every token issued
        // before the revocation refuses at validation immediately.
        self.registry.update(MachineClientRecord {
            enabled: false,
            credential_epoch: record.credential_epoch + 1,
            ..record
        })
    }
}

/// Constant-time equality for verifier comparison: always scans the full
/// length, accumulating the difference (the same posture the webhook
/// signing module's `constant_time_eq` records).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
    const OTHER_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
    const ISSUER_SECRET: &[u8] = b"fim-f08-machine-identity-test-issuer-key";

    fn tenant(uuid_text: &str) -> TenantId {
        TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
    }

    /// The system clock, in unix millis. RECORDED TEST DECISION: the
    /// issuance path stamps `exp` from the injected clock, but the
    /// jsonwebtoken crate validates `exp` against the PROCESS clock at
    /// decode time (the recorded boundary in `crate::oidc`: token expiry
    /// is the JWT library's validated claim). A fixed past clock would
    /// make every issued token expired-at-decode, so the issuance/
    /// validation tests inject the system clock; the expiry margins
    /// (±1 hour against a 15-minute TTL) are far beyond any plausible
    /// execution delay. The non-token tests (registration refusals,
    /// verifier posture) never decode a token and are clock-agnostic.
    fn system_clock_millis() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after the epoch")
            .as_millis() as i64
    }

    fn service(registry: MemoryMachineClientStore) -> MachineIdentityService {
        MachineIdentityService::new(
            MachineIdentityConfig {
                issuer: "https://control-plane.test/token".to_string(),
                audience: "flexcore-api".to_string(),
                ..MachineIdentityConfig::default()
            },
            ISSUER_SECRET,
            Box::new(registry),
            Box::new(system_clock_millis),
        )
    }

    fn register_grid_operator(svc: &MachineIdentityService) {
        svc.register_client(
            "grid-operator-1",
            tenant(TENANT_UUID),
            vec![
                Action::TelemetryRead.id().to_string(),
                Action::SchedulePropose.id().to_string(),
            ],
            "test-only-client-secret-not-a-real-credential",
        )
        .expect("registration");
    }

    #[test]
    fn an_issued_token_validates_to_the_registered_scoped_principal() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        let token = svc
            .issue_client_credentials_token(
                &tenant(TENANT_UUID),
                "grid-operator-1",
                "test-only-client-secret-not-a-real-credential",
            )
            .expect("token issued");
        let principal = svc.validate_machine_token(&token).expect("validates");
        assert_eq!(principal.client_id, "grid-operator-1");
        assert_eq!(principal.subject, "integration:grid-operator-1");
        assert_eq!(principal.tenant_id, tenant(TENANT_UUID));
        assert_eq!(principal.scopes.len(), 2);
        principal.allows(Action::TelemetryRead).expect("granted");
        principal.allows(Action::SchedulePropose).expect("granted");
        assert_eq!(
            principal.allows(Action::CommandApprove),
            Err(MachineIdentityError::ScopeNotGranted { action: "command.approve" }),
        );
    }

    #[test]
    fn a_revoked_client_refuses_immediately_at_validation() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        let token = svc
            .issue_client_credentials_token(
                &tenant(TENANT_UUID),
                "grid-operator-1",
                "test-only-client-secret-not-a-real-credential",
            )
            .expect("token issued");
        svc.revoke_client(&tenant(TENANT_UUID), "grid-operator-1")
            .expect("revoked");
        assert_eq!(
            svc.validate_machine_token(&token),
            Err(MachineIdentityError::ClientRevoked),
            "the pre-revocation token refuses at the registry check"
        );
        // Even a freshly issued token cannot exist: issuance refuses too.
        assert_eq!(
            svc.issue_client_credentials_token(
                &tenant(TENANT_UUID),
                "grid-operator-1",
                "test-only-client-secret-not-a-real-credential",
            ),
            Err(MachineIdentityError::ClientDisabled),
        );
    }

    #[test]
    fn a_wrong_secret_is_refused_at_issuance() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        assert_eq!(
            svc.issue_client_credentials_token(
                &tenant(TENANT_UUID),
                "grid-operator-1",
                "a-different-secret",
            ),
            Err(MachineIdentityError::InvalidSecret),
        );
    }

    #[test]
    fn an_unknown_or_foreign_tenant_client_reads_as_absent() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        assert_eq!(
            svc.issue_client_credentials_token(
                &tenant(OTHER_TENANT_UUID),
                "grid-operator-1",
                "test-only-client-secret-not-a-real-credential",
            ),
            Err(MachineIdentityError::UnknownClient),
            "existence never leaks across the tenant boundary",
        );
    }

    #[test]
    fn the_never_granted_scope_names_are_structurally_refused_at_registration() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        // dispatch.authorize is not on the Action surface (Action::parse →
        // None), so registration refuses it as InvalidScope.
        assert_eq!(
            svc.register_client(
                "overreaching-1",
                tenant(TENANT_UUID),
                vec!["dispatch.authorize".to_string()],
                "test-only-client-secret-not-a-real-credential",
            ),
            Err(MachineIdentityError::InvalidScope { index: 0 }),
        );
        assert_eq!(
            svc.register_client(
                "overreaching-2",
                tenant(TENANT_UUID),
                vec![
                    Action::TelemetryRead.id().to_string(),
                    "secret.read".to_string(),
                ],
                "test-only-client-secret-not-a-real-credential",
            ),
            Err(MachineIdentityError::InvalidScope { index: 1 }),
        );
        // Nothing was written for either refusal.
        assert!(registry
            .get(&tenant(TENANT_UUID), "overreaching-1")
            .expect("read")
            .is_none());
        assert!(registry
            .get(&tenant(TENANT_UUID), "overreaching-2")
            .expect("read")
            .is_none());
    }

    #[test]
    fn the_stored_record_carries_a_verifier_never_the_secret() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        let record = registry
            .get(&tenant(TENANT_UUID), "grid-operator-1")
            .expect("read")
            .expect("registered");
        assert_eq!(record.secret_verifier_sha256_hex.len(), 64);
        assert!(!record
            .secret_verifier_sha256_hex
            .contains("test-only-client-secret"), "the verifier is a digest, not the secret");
        // The Debug of the record names no secret material.
        let debugged = format!("{record:?}");
        assert!(
            !debugged.contains("test-only-client-secret"),
            "the raw secret never appears in any record representation"
        );
    }

    #[test]
    fn error_names_are_pinned_and_errors_never_echo_credentials() {
        let registry = MemoryMachineClientStore::new();
        let svc = service(registry);
        register_grid_operator(&svc);
        let refused = svc
            .issue_client_credentials_token(
                &tenant(TENANT_UUID),
                "grid-operator-1",
                "a-different-secret",
            )
            .expect_err("refused");
        assert_eq!(refused.name(), "MACHINE_IDENTITY_INVALID_SECRET");
        assert!(
            !refused.to_string().contains("a-different-secret"),
            "no refusal echoes the presented secret"
        );
        // The composition mapping onto the existing resolver vocabulary.
        let mapped: AuthError = refused.clone().into();
        assert_eq!(mapped, AuthError::InvalidCredentials);
    }
}
