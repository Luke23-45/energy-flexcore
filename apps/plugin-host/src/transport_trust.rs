// P7.34 — Plugin-host trust boundary: mTLS, service identity, UNTRUSTED HOST
// (report §17.3, §18.2, §5.4 Class B, §8.3, §5.7, §13.4; ledger
// `ledger_phase_7.md` §P7.34; decision record
// `ledger/phase-07-plugin-sdk/33_34_audit_integrity_trust_boundary.md`).
//
// THE RULE
//
// The plugin host is NOT trusted merely because it is a first-party
// process — it executes untrusted code (report §8.3: the WASM host is
// extracted for security isolation). The host is therefore NOT in the TCB
// for authorization decisions: the kernel/enforcement are policy, the
// PLATFORM is the authority. See [`TRUST_POSTURE`] and
// [`COMPROMISED_HOST_BOUND`] (both asserted as documentation by the P7.34
// tests).
//
// WHAT THIS MODULE IMPLEMENTS (config-shaped; no TLS, no PKI, no new deps)
//
// 1. **Service identity, fail-closed**: each plugin-host instance presents
//    a service identity bound to its DEPLOYMENT (the P6.06 edge-enrollment
//    pattern cited, mirrored not imported — this crate depends on no
//    flexcore crate): [`HostServiceIdentity`] + [`HostIdentityStore`] +
//    [`assemble_trust_material`]. Identity MATERIAL (certificates, keys)
//    is a DEPLOYMENT REQUIREMENT — no PKI is implemented here (the
//    `edge-agent/src/enrollment.rs` honesty pattern: `DenyAllCredentialIssuer`
//    is the shipped issuer there; here there is no issuer at all, only
//    REFERENCES to deployment-held material).
// 2. **Mutual verification, fail-closed**: the platform verifies the host
//    (the host's client certificate) and the host verifies the platform
//    (a PINNED platform CA reference from its deployment config — never
//    the ambient system store, mirroring
//    `crates/edge-agent/src/uplink_mtls.rs::assemble_material`). A missing
//    identity, a missing certificate reference, a missing CA pin, a
//    plaintext endpoint, or an unpinned server name each REFUSE
//    composition with a named [`TrustError`] — fail-closed at the door,
//    the P6.07 rules restated for the host channel.
// 3. **The untrusted-host posture**: [`TRUST_POSTURE`] (the
//    layer/decider/compromise table) and [`COMPROMISED_HOST_BOUND`]
//    (what a compromised host can and cannot do), restating the
//    enforcement never-granted set ([`never_granted_bound`], a re-export
//    of `broker::NEVER_GRANTED_PERMISSIONS`) as the compromised-host
//    authorization bound, and the P7.33 boundary statement
//    (`crate::audit_chain::AUDIT_INTEGRITY_BOUNDARY`) as the
//    compromised-host audit bound.
//
// WHAT IS A RECORDED SEAM (NOT implemented — the task's honesty lines)
//
// The gRPC-with-mTLS TRANSPORT ITSELF is config-shaped only. rustls is NOT
// added to plugin-host in this wave; the transport wiring is the
// composition owner's, with the exact dependency/feature list recorded in
// [`TRANSPORT_WIRING_SEAM`] (mirroring how edge-agent added rustls with
// its D4 note). Rotation/revocation SESSION enforcement (ledger P7.34
// steps 3–4: revoked identity gets no session; in-flight work drained)
// is the platform-side session gate — config-shaped here as
// [`RevocationChecking`] and the expiry-monitor/rotation-owner fields,
// enforced at VALIDATION time (a config without revocation checking
// refuses to compose), not at handshake time.
//
// House rules: std only, fail-closed everywhere, named errors with stable
// machine names, no `unwrap`/`expect` on untrusted input, simulation-only.

use std::fmt;
use std::time::Duration;

use crate::broker::NEVER_GRANTED_PERMISSIONS;

// --- The trust-posture table (asserted as documentation by the P7.34 tests) ------

/// One row of the trust-posture table: (boundary layer, who decides, what
/// a compromised host can still do).
pub type TrustPostureRow = (&'static str, &'static str, &'static str);

/// The trust-posture table for the plugin-host ↔ platform boundary. Every
/// row's "decides" column names the component OUTSIDE the host that holds
/// the decision; the "compromised host" column is the residual capability
/// of a host gone hostile — deny service and expose its own inputs, never
/// more.
pub const TRUST_POSTURE: [TrustPostureRow; 6] = [
    (
        "capability grants",
        "the enforcement gate over the closed permission vocabulary (manifest ∩ profile ceiling; broker re-refusal)",
        "cannot grant itself capabilities: the never-granted set is refused at grant AND check time",
    ),
    (
        "authorization authority",
        "the platform: validation, feasibility, and approval are distinct kernel gates (§5.7); a proposal gains no authority from having been proposed (§13.4, ADR-010)",
        "can deny service and expose its own inputs; its outputs are CANDIDATES that enter the protected path unprivileged",
    ),
    (
        "authoritative audit",
        "the platform's append-only audit store (migration 0008 grants + trigger + RLS; the SELECT-only read contract)",
        "cannot forge authoritative audit; it can only withhold or falsify its LOCAL chain, which the P7.33 boundary statement bounds",
    ),
    (
        "control-authority mode",
        "a deployment and per-site setting; neither a plugin nor the host on its behalf can select it",
        "cannot raise the control-authority mode (simulation/shadow stays simulation/shadow)",
    ),
    (
        "session authentication",
        "the platform authenticates each host session over mTLS with the host's service identity (this module, config-shaped)",
        "without a valid, unrevoked identity no session composes (fail-closed); a revoked identity is refused, its in-flight work drained not completed on trust",
    ),
    (
        "result validation",
        "the platform validates every plugin result (schema, §9.2 provenance, tenant scope) before it enters the protected path — a result from the host is UNTRUSTED INPUT",
        "forged, oversized, or wrong-tenant results are rejected and the host reported failed, never acted on",
    ),
];

/// The untrusted-host bound, restated as prose (asserted by the P7.34
/// tests). The host is NOT in the TCB for authorization decisions.
pub const COMPROMISED_HOST_BOUND: &str = concat!(
    "A compromised plugin host can DENY SERVICE and EXPOSE ITS OWN INPUTS. ",
    "It cannot grant itself capabilities: dispatch.authorize, dispatch.issue, ",
    "secret.read, and network.any are in the never-granted set, refused by the ",
    "broker at grant time and at check time regardless of what any manifest, ",
    "profile, or host call says. It cannot forge authoritative audit: the ",
    "platform's append-only audit store holds the authoritative record and the ",
    "local hash chain is tamper-EVIDENT evidence only (P7.33 boundary ",
    "statement). Its every result is untrusted input that the platform ",
    "validates (schema, provenance, tenant scope) before the protected path ",
    "sees it — a proposal gains no authority from having been proposed ",
    "(ADR-010, §13.4)."
);

/// The never-granted set, restated as the compromised-host authorization
/// bound: a re-export of `broker::NEVER_GRANTED_PERMISSIONS` so the trust
/// boundary cites ONE set, not a copy of it.
pub fn never_granted_bound() -> &'static [&'static str; 4] {
    &NEVER_GRANTED_PERMISSIONS
}

/// The recorded transport-wiring seam (NOT implemented in this wave). The
/// composition owner adds, at composition time and through the D4 audit
/// note discipline (the edge-agent rustls precedent):
/// `rustls = { version = "<pin>", default-features = false, features =
/// ["ring", "std", "tls12", "logging"] }` + `rustls-pemfile = { version =
/// "<pin>", default-features = false, features = ["std"] }` (the P6.07
/// pairing; the `ring` provider avoids the aws-lc-rs C toolchain; CARET
/// pins are UNPINNED-REVERIFY until the first human `Cargo.lock`), plus
/// the chosen gRPC stack for the P7.20 seam. Licence/dependency audit
/// (P7.03 discipline) re-verifies each; no pin in this comment is a
/// recorded pin.
pub const TRANSPORT_WIRING_SEAM: &str = concat!(
    "RECORDED SEAM — not implemented in this wave: the gRPC-with-mTLS ",
    "transport for the host ↔ platform channel. The composition owner adds ",
    "at composition time (D4 note REQUIRED, mirroring edge-agent's rustls ",
    "addition): (1) rustls with default-features = false and the ring ",
    "crypto provider feature set (exactly one provider feature), ",
    "(2) rustls-pemfile for PEM parsing of the pinned CA and the host ",
    "certificate/key references, (3) the chosen gRPC stack for the P7.20 ",
    "host↔control-plane seam. Version pins are UNPINNED-REVERIFY until the ",
    "first human lockfile and the P7.03 license audit re-verify them. ",
    "Until then this module ships configuration VALIDATION only: the ",
    "fail-closed material assembly (assemble_trust_material) and the ",
    "pinned policy (TlsMinimum, plaintext refusal, pinned server name and ",
    "CA reference) — no TLS handshake, no socket, no PKI."
);

// --- TLS minimum (the P6.07 mirror; see the module header) -----------------------

/// The pinned minimum protocol version of the host ↔ platform channel.
/// MIRROR of `crates/edge-agent/src/uplink_mtls.rs::TlsMinimum` (the same
/// DEFAULT and the same opt-in floor; this crate cannot import the
/// edge-agent, so the two-token vocabulary is mirrored — the same recorded
/// mirror discipline as the broker's permission vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMinimum {
    /// TLS 1.3 only. THE DEFAULT.
    Tls13,
    /// TLS 1.2 as the floor (opt-in per a site's signed D6 record; never
    /// the default).
    Tls12Floor,
}

impl TlsMinimum {
    pub const DEFAULT: Self = Self::Tls13;

    pub fn as_str(self) -> &'static str {
        match self {
            TlsMinimum::Tls13 => "tls13",
            TlsMinimum::Tls12Floor => "tls12-floor",
        }
    }

    /// Parses the closed two-token vocabulary (unknown tokens refuse).
    pub fn parse(raw: &str) -> Result<Self, TrustError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tls13" => Ok(TlsMinimum::Tls13),
            "tls12-floor" => Ok(TlsMinimum::Tls12Floor),
            other => Err(TrustError::TlsMinimumUnknown {
                value: other.to_string(),
            }),
        }
    }
}

impl fmt::Display for TlsMinimum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Revocation (config-shaped; ledger P7.34 step 4 posture) ----------------------

/// The revocation-checking configuration of the host identity. A host
/// whose config does not configure revocation checking REFUSES to compose
/// (fail-closed: an unverifiable identity is not an identity) — the
/// config-shaped enforcement of the ledger's "a revoked host identity is
/// refused a new session" posture; the SESSION-time refusal itself is the
/// platform-side gate over the recorded transport seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationChecking {
    /// Refused at validation: revocation must be configured.
    Unconfigured,
    /// The deployment's revocation-checking endpoint reference (the
    /// platform-side checker the session gate consults; the reference is
    /// resolved by the composition, never embedded here).
    DeploymentEndpoint { reference: String },
}

// --- Service identity (the P6.06 pattern, mirrored) -------------------------------

/// The plugin-host service identity, bound to its DEPLOYMENT (the P6.06
/// edge-enrollment pattern cited, mirrored not imported). This module
/// carries REFERENCES only — the certificate and key MATERIAL live in the
/// deployment's key source (§18.3) and are a DEPLOYMENT REQUIREMENT; no
/// PKI, no key generation, no key material ever enters this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostServiceIdentity {
    /// The host instance id (unique per running host instance; the session
    /// identity the platform sees).
    pub host_instance_id: String,
    /// The deployment the instance belongs to (the identity is bound to
    /// the deployment, not to a binary or a machine).
    pub deployment_id: String,
    /// Reference to the instance's client certificate (deployment-held
    /// material; short-lived per the §17.3 posture — the LIFETIME policy
    /// lives in [`MtlsTrustConfig::session_credential_ttl`], the material
    /// in the deployment).
    pub certificate_reference: Option<String>,
    /// Reference to the instance's private key (deployment-held, hardware-
    /// backed where the platform offers it; never logged, never in an
    /// audit record).
    pub private_key_reference: Option<String>,
}

impl HostServiceIdentity {
    /// Structural validation (fail-closed): non-empty instance and
    /// deployment ids, and BOTH material references present. A missing
    /// reference is a named error — the host does not compose a trust
    /// material it cannot present or verify with.
    pub fn validate(&self) -> Result<(), TrustError> {
        if self.host_instance_id.trim().is_empty() {
            return Err(TrustError::IdentityIncomplete {
                field: "host_instance_id",
            });
        }
        if self.deployment_id.trim().is_empty() {
            return Err(TrustError::IdentityIncomplete {
                field: "deployment_id",
            });
        }
        let certificate = self
            .certificate_reference
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if certificate.is_empty() {
            return Err(TrustError::IdentityIncomplete {
                field: "certificate_reference",
            });
        }
        let private_key = self
            .private_key_reference
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if private_key.is_empty() {
            return Err(TrustError::IdentityIncomplete {
                field: "private_key_reference",
            });
        }
        Ok(())
    }
}

/// The identity store seam (mirrors
/// `crates/edge-agent/src/uplink_mtls.rs::FilesystemCredentialStoreLoad`):
/// the production store resolves the deployment's identity record; tests
/// supply a scripted one. `None` means "no identity is stored" — a
/// fail-closed refusal at assembly, never an anonymous fallback.
pub trait HostIdentityStore: Send + Sync {
    fn load_identity(&self) -> Result<Option<HostServiceIdentity>, TrustError>;
}

// --- The channel configuration ----------------------------------------------------

/// The host ↔ platform channel configuration, validated fail-closed (the
/// P6.07 `MtlsUplinkConfig` rules, restated for the host channel: the
/// HOST dials the platform; the platform never dials into the host — no
/// inbound listener exists on the host side of this seam).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtlsTrustConfig {
    /// The platform endpoint (`wss://host:port` or `https://host:port`).
    /// Plaintext schemes are REFUSED at validation.
    pub platform_endpoint: String,
    /// The TLS server name the host pins and verifies (config-sourced,
    /// never taken from an untrusted response).
    pub pinned_server_name: String,
    /// Reference to the PINNED platform CA in the host's deployment config
    /// — never the ambient system store (the `uplink_mtls.rs` rule). A
    /// missing/empty reference refuses composition (fail-closed on the
    /// absence of a trust anchor).
    pub pinned_platform_ca_reference: Option<String>,
    /// The pinned minimum protocol version (DEFAULT TLS 1.3).
    pub tls_minimum: TlsMinimum,
    /// The maximum credential lifetime of a host session (short-lived
    /// credentials, §17.3: the session re-authenticates on expiry; must be
    /// positive).
    pub session_credential_ttl: Duration,
    /// The recorded owner of identity issuance and rotation (ledger P7.34
    /// step 1: "record issuance, rotation owner, and expiry monitoring" —
    /// the owner is CONFIGURATION, validated non-empty, never invented by
    /// code).
    pub rotation_owner: String,
    /// Whether expiry monitoring is enabled (the rotation-window warning
    /// posture of the P6.06 `CredentialExpiryMonitor`, mirrored). Disabled
    /// expiry monitoring refuses composition.
    pub expiry_monitor_enabled: bool,
    /// The revocation-checking configuration ([`RevocationChecking`];
    /// `Unconfigured` refuses composition).
    pub revocation_checking: RevocationChecking,
}

impl MtlsTrustConfig {
    /// Validation (fail-closed; every rule is a named error). Order is
    /// deterministic so tests pin the FIRST refusal of a multiply-broken
    /// config.
    pub fn validate(&self) -> Result<(), TrustError> {
        // 1. The endpoint must be an explicit TLS scheme (plaintext
        //    refusal — ledger P6.07 step 3 rule, restated).
        let scheme = self
            .platform_endpoint
            .split("://")
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(scheme.as_str(), "wss" | "https") {
            return Err(TrustError::EndpointRefused {
                endpoint_scheme: if self.platform_endpoint.contains("://") {
                    scheme
                } else {
                    "<no scheme>".to_string()
                },
            });
        }
        // 2. The pinned server name is mandatory.
        if self.pinned_server_name.trim().is_empty() {
            return Err(TrustError::ServerNameMissing);
        }
        // 3. The pinned platform CA reference is mandatory (fail-closed on
        //    the absence of a trust anchor — never "trust the system
        //    store" fallback).
        let ca = self
            .pinned_platform_ca_reference
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if ca.is_empty() {
            return Err(TrustError::CaPinMissing);
        }
        // 4. Short-lived credentials: a non-positive TTL is invalid.
        if self.session_credential_ttl.is_zero() {
            return Err(TrustError::CredentialTtlInvalid {
                reason: "the session credential TTL must be positive (short-lived \
                         credentials, §17.3)"
                    .to_string(),
            });
        }
        // 5. The rotation owner is recorded, not invented.
        if self.rotation_owner.trim().is_empty() {
            return Err(TrustError::RotationOwnerMissing);
        }
        // 6. Expiry monitoring must be enabled.
        if !self.expiry_monitor_enabled {
            return Err(TrustError::ExpiryMonitorDisabled);
        }
        // 7. Revocation checking must be configured.
        match &self.revocation_checking {
            RevocationChecking::Unconfigured => Err(TrustError::RevocationUnconfigured),
            RevocationChecking::DeploymentEndpoint { reference } => {
                if reference.trim().is_empty() {
                    Err(TrustError::RevocationUnconfigured)
                } else {
                    Ok(())
                }
            }
        }
    }
}

// --- The assembled trust material -------------------------------------------------

/// The validated trust material of a host instance: REFERENCES to
/// deployment-held material plus the pinned policy. NO key material, NO
/// certificate bytes, NO CA bytes — the transport seam resolves the
/// references at composition time (the `uplink_mtls.rs::ChannelMaterial`
/// step, which stays the composition owner's because the transport itself
/// is the recorded seam).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTrustMaterial {
    pub host_instance_id: String,
    pub deployment_id: String,
    pub certificate_reference: String,
    pub private_key_reference: String,
    pub pinned_platform_ca_reference: String,
    pub pinned_server_name: String,
    pub platform_endpoint: String,
    pub tls_minimum: TlsMinimum,
    pub session_credential_ttl: Duration,
    pub rotation_owner: String,
    pub revocation_endpoint_reference: String,
}

/// Assembles the host trust material: loads the service identity
/// (fail-closed on absence), validates it (fail-closed on an incomplete
/// identity), and validates the channel config (fail-closed on a missing
/// CA pin, a plaintext endpoint, an unpinned server name, or an
/// unconfigured rotation/revocation posture). Mirrors
/// `crates/edge-agent/src/uplink_mtls.rs::assemble_material` rule for
/// rule; identity-missing and CA-pin-missing are the two fail-closed
/// constructions the P7.34 tests pin.
pub fn assemble_trust_material(
    store: &dyn HostIdentityStore,
    config: &MtlsTrustConfig,
) -> Result<HostTrustMaterial, TrustError> {
    let identity = store.load_identity()?.ok_or_else(|| TrustError::IdentityMissing {
        reason: "fail closed: no host service identity is stored (the host instance has \
                 not been issued one by its deployment); the host does not compose a \
                 trust boundary without an identity"
            .to_string(),
    })?;
    identity.validate()?;
    config.validate()?;
    let (certificate_reference, private_key_reference) = match (
        identity.certificate_reference.as_deref(),
        identity.private_key_reference.as_deref(),
    ) {
        (Some(cert), Some(key)) => (cert.trim().to_string(), key.trim().to_string()),
        // Unreachable after `identity.validate()`; kept explicit (no
        // unwrap, even where provably safe).
        _ => {
            return Err(TrustError::IdentityIncomplete {
                field: "certificate_reference/private_key_reference",
            })
        }
    };
    let pinned_platform_ca_reference = identity_ref_or_empty(
        config
            .pinned_platform_ca_reference
            .as_deref()
            .unwrap_or_default(),
    );
    let revocation_endpoint_reference = match &config.revocation_checking {
        RevocationChecking::DeploymentEndpoint { reference } => reference.trim().to_string(),
        RevocationChecking::Unconfigured => String::new(),
    };
    Ok(HostTrustMaterial {
        host_instance_id: identity.host_instance_id,
        deployment_id: identity.deployment_id,
        certificate_reference,
        private_key_reference,
        pinned_platform_ca_reference,
        pinned_server_name: config.pinned_server_name.clone(),
        platform_endpoint: config.platform_endpoint.clone(),
        tls_minimum: config.tls_minimum,
        session_credential_ttl: config.session_credential_ttl,
        rotation_owner: config.rotation_owner.clone(),
        revocation_endpoint_reference,
    })
}

/// Trims a validated non-empty reference (post-`validate` this cannot
/// produce an empty string; the helper keeps the assembly unwrap-free).
fn identity_ref_or_empty(reference: &str) -> String {
    reference.trim().to_string()
}

// --- Errors -------------------------------------------------------------------------

/// Trust-boundary errors. Every refusal is named with a stable
/// machine-readable name (the named-error table discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustError {
    /// No host service identity is stored — fail-closed (never an
    /// anonymous fallback).
    IdentityMissing { reason: String },
    /// The identity record is structurally incomplete (`field` names the
    /// missing piece).
    IdentityIncomplete { field: &'static str },
    /// The deployment config pins no platform CA — fail-closed (the host
    /// refuses to trust an unpinned platform endpoint).
    CaPinMissing,
    /// The endpoint is plaintext or scheme-less — refused.
    EndpointRefused { endpoint_scheme: String },
    /// The TLS server name is not pinned.
    ServerNameMissing,
    /// The session credential TTL is invalid.
    CredentialTtlInvalid { reason: String },
    /// No rotation owner is recorded (issuance/rotation ownership is
    /// configuration, never invented).
    RotationOwnerMissing,
    /// Expiry monitoring is disabled.
    ExpiryMonitorDisabled,
    /// Revocation checking is unconfigured — fail-closed.
    RevocationUnconfigured,
    /// The TLS-minimum token is unknown.
    TlsMinimumUnknown { value: String },
    /// The identity store itself failed.
    IdentityStoreFailed { reason: String },
}

impl TrustError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            TrustError::IdentityMissing { .. } => "TRUST_IDENTITY_MISSING",
            TrustError::IdentityIncomplete { .. } => "TRUST_IDENTITY_INCOMPLETE",
            TrustError::CaPinMissing => "TRUST_CA_PIN_MISSING",
            TrustError::EndpointRefused { .. } => "TRUST_ENDPOINT_REFUSED",
            TrustError::ServerNameMissing => "TRUST_SERVER_NAME_MISSING",
            TrustError::CredentialTtlInvalid { .. } => "TRUST_CREDENTIAL_TTL_INVALID",
            TrustError::RotationOwnerMissing => "TRUST_ROTATION_OWNER_MISSING",
            TrustError::ExpiryMonitorDisabled => "TRUST_EXPIRY_MONITOR_DISABLED",
            TrustError::RevocationUnconfigured => "TRUST_REVOCATION_UNCONFIGURED",
            TrustError::TlsMinimumUnknown { .. } => "TRUST_TLS_MINIMUM_UNKNOWN",
            TrustError::IdentityStoreFailed { .. } => "TRUST_IDENTITY_STORE_FAILED",
        }
    }
}

impl fmt::Display for TrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustError::IdentityMissing { reason } => {
                write!(f, "no host service identity: {reason}")
            }
            TrustError::IdentityIncomplete { field } => {
                write!(f, "the host service identity is incomplete: '{field}' is missing")
            }
            TrustError::CaPinMissing => {
                write!(
                    f,
                    "fail closed: the deployment config pins no platform CA; the host \
                     refuses to trust an unpinned platform endpoint (the ambient system \
                     store is never a fallback)"
                )
            }
            TrustError::EndpointRefused { endpoint_scheme } => {
                write!(
                    f,
                    "the platform endpoint must be an explicit TLS scheme (wss:// or \
                     https://); got '{endpoint_scheme}'. Plaintext (ws://, http://) and \
                     scheme-less endpoints are REFUSED"
                )
            }
            TrustError::ServerNameMissing => {
                write!(f, "the pinned TLS server name is mandatory")
            }
            TrustError::CredentialTtlInvalid { reason } => {
                write!(f, "the session credential TTL is invalid: {reason}")
            }
            TrustError::RotationOwnerMissing => {
                write!(
                    f,
                    "no rotation owner is recorded (issuance and rotation ownership is \
                     deployment configuration, never invented by code)"
                )
            }
            TrustError::ExpiryMonitorDisabled => {
                write!(
                    f,
                    "expiry monitoring is disabled; the host refuses to compose a trust \
                     boundary that cannot see its own credential expiry"
                )
            }
            TrustError::RevocationUnconfigured => {
                write!(
                    f,
                    "revocation checking is unconfigured; the host refuses to compose a \
                     trust boundary that cannot detect a revoked identity"
                )
            }
            TrustError::TlsMinimumUnknown { value } => {
                write!(
                    f,
                    "unknown TLS minimum '{value}'; expected 'tls13' (default) or \
                     'tls12-floor' (opt-in per a signed site D6 record)"
                )
            }
            TrustError::IdentityStoreFailed { reason } => {
                write!(f, "the host identity store failed: {reason}")
            }
        }
    }
}

impl std::error::Error for TrustError {}
