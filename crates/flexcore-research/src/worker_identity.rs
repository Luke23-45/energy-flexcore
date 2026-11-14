// P8.33 — Research-worker identity, mTLS, and secret handling.
//         Ledger `ledger/phase-08-advanced-coordination/ledger_phase_8.md`
//         §P8.33; report trace §17.3 (mTLS, short-lived credentials,
//         secrets manager; no secrets in plugin config or source control),
//         §5.4 Class C; decision record:
//         `ledger/phase-08-advanced-coordination/
//         33_identity_mtls_secrets.md`.
//
// WHAT THIS IS: the research-worker identity and transport-trust posture —
// the research-side counterpart of the plugin-host's P7.34 trust boundary
// (`apps/plugin-host/src/transport_trust.rs`, mirrored BY CITATION — this
// crate depends on no flexcore crate), plus the worker-side secret-
// handling rule (the kernel manifest's secret-REFERENCE rule mirrored).
//
// THE THREE PARTS (the ledger's steps):
//
// 1. SERVICE IDENTITY, FAIL-CLOSED (ledger step 1). Each research worker
//    presents a service identity bound to its DEPLOYMENT (the P6.06
//    edge-enrollment / P7.34 host-identity pattern, mirrored):
//    [`ResearchWorkerIdentity`] + [`WorkerIdentityStore`] +
//    [`assemble_worker_trust_material`]. Identity MATERIAL (certificates,
//    keys) is a DEPLOYMENT REQUIREMENT — no PKI is implemented here; the
//    module carries REFERENCES to deployment-held material only. The
//    worker family is the CLOSED P8.31 vocabulary
//    ([`crate::tenant_isolation::ResearchWorkerFamily`]) — there is no
//    "generic" worker identity. Issuance/rotation ownership is recorded
//    CONFIGURATION (validated non-empty), never invented by code.
// 2. MUTUAL TRANSPORT TRUST, CONFIG-SHAPED (ledger step 1, the §17.3
//    posture). [`WorkerMtlsTransportConfig`] validates the worker ↔
//    platform channel rule-for-rule against the plugin-host mirror: an
//    explicit-TLS endpoint (plaintext REFUSED), a pinned server name, a
//    PINNED platform CA reference (never the ambient system store),
//    short-lived session credentials (positive TTL), a recorded rotation
//    owner, expiry monitoring enabled, and revocation checking configured
//    — each missing piece is a named [`WorkerIdentityError`] and the
//    composition REFUSES. The gRPC-with-mTLS TRANSPORT ITSELF is the
//    recorded seam ([`RESEARCH_TRANSPORT_WIRING_SEAM`]): rustls is NOT
//    added to this crate in this wave; the wiring is the composition
//    owner's, exactly as recorded for the plugin host.
// 3. SECRETS AS REFERENCES ONLY (ledger step 2). Every credential need —
//    dataset access, model-store access, simulator access — is declared
//    as a scheme-prefixed REFERENCE ([`WorkerSecretReference`], closed
//    vocabulary `env:` / `file:` / `secret-store:`, the kernel manifest
//    rule mirrored) resolved BY THE HOST at composition time. There is no
//    field anywhere in these types that can carry a secret VALUE. The
//    worker-config scan ([`scan_worker_config`]) rejects any config key in
//    the closed forbidden vocabulary ([`FORBIDDEN_CONFIG_SECRET_KEYS`]) at
//    ANY depth — a literal-secret config is REFUSED at load, and the
//    attempt is audited (ledger step 3). A reference without a scheme
//    prefix is rejected as likely inline secret material (the value is
//    never echoed back — it may be a secret).
//
// WHAT IS A RECORDED SEAM (NOT implemented — the honesty lines):
//
// - The gRPC binding of the research workers is the Phase 7 follow-up
//   recorded in [`PHASE7_GRPC_BINDING_FOLLOWUP`] (carried from
//   `25_security_review.md` finding S-1): the research workers today use
//   the JSON-document transport discipline; the P7.34 transport-identity
//   wave covers them only once they bind the servicer at their activation
//   wave. This module is the CONFIG-SHAPED posture those workers adopt
//   then — never an invented parallel contract.
// - The SESSION-time enforcement of rotation/revocation (a revoked
//   identity gets no NEW session; in-flight work drained) is the
//   platform-side session gate over the recorded transport seam. Here the
//   revocation is enforced at VALIDATION/admission time
//   ([`verify_worker_admission`]) — a config whose revocation checking is
//   unconfigured refuses to compose, and a revoked worker id is refused at
//   the admission door (fail-closed), mirroring the P7.34 posture.
//
// HONEST RESIDUAL (recorded, never hidden): the config scan refuses the
// forbidden KEY vocabulary at any depth; it CANNOT detect a secret hidden
// under an innocuous key name. The recorded mitigation is structural: the
// only sanctioned way to name a credential need is a [`WorkerSecretReference`]
// (scheme-prefixed, host-resolved), the closed config schema the workers
// ship with declares those needs, and the audit trail shows every
// refused attempt. No statement here claims the scan proves absence of
// secrets.
//
// House rules: std only, fail-closed everywhere, named errors with stable
// machine names, closed key sets, `BTreeMap` registers, injected clock
// ([`crate::validation::HarnessClock`]) wherever time appears, no
// `unwrap`/`expect` on untrusted input, simulation-only (ADR-010: nothing
// here authorizes, issues, approves, or dispatches anything).

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde_json::Value;

use crate::tenant_isolation::ResearchWorkerFamily;
use crate::validation::HarnessClock;

// --- Honesty / seam statements -------------------------------------------------------

/// The P8.33 honesty statement (recorded so tests and reviewers can pin
/// what IS and IS NOT claimed — the P4.28/P7.34 statement pattern).
pub const WORKER_IDENTITY_STATEMENT: &str = "each research worker carries a DEPLOYMENT-BOUND \
     service identity and a config-shaped mTLS trust posture: identity and credential \
     MATERIAL are deployment-held (no PKI here), the channel config refuses plaintext, \
     unpinned server names, absent CA pins, long-lived credentials, unowned rotation, \
     disabled expiry monitoring, and unconfigured revocation — each a named refusal. \
     Credentials travel as scheme-prefixed REFERENCES (env:, file:, secret-store:) \
     resolved by the host — never as values in worker config, environment, or training \
     scripts. No TLS handshake, no socket, no gRPC binding exists in this crate; the \
     transport wiring is a recorded seam. No certification or security claim is made.";

/// The plugin-host alignment seam (recorded — do not drift silently): this
/// module MIRRORS `apps/plugin-host/src/transport_trust.rs` (P7.34 —
/// service identity, MtlsTrustConfig validation rule-for-rule, config-
/// shaped revocation checking, the recorded rustls wiring seam) BY
/// CITATION. This crate does not depend on the plugin host; if that shape
/// changes, the mirror update is deliberate, fail-closed in both
/// directions until then.
pub const PLUGIN_HOST_TRUST_ALIGNMENT_SEAM: &str =
    "mirror of apps/plugin-host/src/transport_trust.rs (P7.34 HostServiceIdentity, \
     MtlsTrustConfig validate, RevocationChecking, TRANSPORT_WIRING_SEAM) BY CITATION; \
     plugin-host unchanged; no dependency";

/// The kernel secret-reference alignment seam (recorded): the secret
/// REFERENCE rule mirrors `crates/flexcore-kernel/src/manifest.rs`
/// (P7.05/§17.3 — scheme-prefixed references, closed key sets, values
/// never echoed on rejection) BY CITATION; the kernel is unchanged.
pub const KERNEL_SECRET_SCHEME_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/manifest.rs SECRET_REFERENCE_SCHEMES + \
     parse_secrets (references only, closed schemes, no value echo) BY CITATION; \
     kernel unchanged; no dependency";

/// The recorded transport-wiring seam (NOT implemented in this wave; the
/// plugin-host [`TRANSPORT_WIRING_SEAM`] wording, restated for the
/// research-worker channel). The composition owner adds, at composition
/// time and through the D4 audit-note discipline (the edge-agent rustls
/// precedent): rustls with default-features = false and the ring crypto
/// provider feature set (exactly one provider feature), rustls-pemfile for
/// PEM parsing of the pinned CA and the worker certificate/key references,
/// and the chosen gRPC stack for the Phase 7 worker seam. Version pins are
/// UNPINNED-REVERIFY until the first human lockfile and the P7.03/P8.03
/// license audit re-verify them. Until then this module ships
/// configuration VALIDATION only — no TLS handshake, no socket, no PKI,
/// nothing installed.
pub const RESEARCH_TRANSPORT_WIRING_SEAM: &str = concat!(
    "RECORDED SEAM — not implemented in this wave: the gRPC-with-mTLS ",
    "transport for the research-worker ↔ platform channel. The composition ",
    "owner adds at composition time (D4 note REQUIRED): (1) rustls with ",
    "default-features = false and the ring crypto provider feature set ",
    "(exactly one provider feature), (2) rustls-pemfile for PEM parsing of ",
    "the pinned CA and the worker certificate/key references, (3) the ",
    "chosen gRPC stack for the Phase 7 worker seam. Version pins are ",
    "UNPINNED-REVERIFY until the first human lockfile and the license ",
    "audit re-verify them. Until then this module ships configuration ",
    "VALIDATION only: the fail-closed material assembly ",
    "(assemble_worker_trust_material) and the pinned policy (TlsMinimum, ",
    "plaintext refusal, pinned server name and CA reference) — no TLS ",
    "handshake, no socket, no PKI, nothing installed."
);

/// The Phase 7 follow-up: the gRPC binding activation seam of the research
/// workers (carried from `25_security_review.md` finding S-1, word for
/// word in substance). The trust-relevant consequence is restated so no
/// reader assumes this module already covers the live channel.
pub const PHASE7_GRPC_BINDING_FOLLOWUP: &str = "PHASE 7 FOLLOW-UP — the Phase 7 worker \
     template's gRPC binding is not wired into the research workers (JSON-document \
     transport discipline instead; buf generation remains the activation step). The \
     P7.34 transport-identity wave (mTLS, untrusted-host posture) does not cover the \
     research workers until they bind the servicer at their activation wave. This module \
     records the CONFIG-SHAPED posture they adopt then; it does not wire the binding, \
     and never claims the workers are mTLS-covered today.";

/// The secret-handling rule (ledger step 2, verbatim substance; pinned by
/// tests).
pub const WORKER_SECRET_HANDLING_RULE: &str = "all credential needs (dataset access, \
     model-store access, simulator access, and every other credential need) are declared \
     as scheme-prefixed secret REFERENCES (env:, file:, secret-store:) resolved BY THE \
     HOST at composition time — never as values in worker config, environment variables, \
     or training scripts. The closed reference vocabulary and the closed forbidden-key \
     scan are what make a literal-secret config a named refusal at load, audited.";

// --- Closed vocabularies --------------------------------------------------------------

/// The only accepted secret-reference schemes (the kernel manifest rule,
/// mirrored — §17.3: the secrets manager is the `secret-store:` scheme's
/// deployment realization).
pub const SECRET_REFERENCE_SCHEMES: [&str; 3] = ["env:", "file:", "secret-store:"];

/// The closed forbidden-key vocabulary of the worker-config scan. A config
/// key whose normalized form (lowercased, `-` folded to `_`) CONTAINS any
/// of these entries is a refusal at ANY depth — the config-shaped refusal
/// of a literal-secret config (ledger step 3). The scan's honest residual
/// (it cannot catch a secret under an innocuous key name) is recorded in
/// the module header and pinned by a test.
pub const FORBIDDEN_CONFIG_SECRET_KEYS: [&str; 15] = [
    "access_token",
    "api_key",
    "apikey",
    "bearer",
    "client_secret",
    "credential",
    "passwd",
    "password",
    "passphrase",
    "private_key",
    "secret",
    "session_key",
    "signing_key",
    "token",
    "secret_value",
];

/// Maximum reference/name/id length (the crate's 256-character id bound
/// posture; the kernel manifest secret-name bound mirrored).
pub const MAX_IDENTITY_REF_CHARS: usize = 256;

// --- Service identity (the P7.34 pattern, mirrored) -----------------------------------

/// The research-worker service identity, bound to its DEPLOYMENT. This
/// module carries REFERENCES only — the certificate and key MATERIAL live
/// in the deployment's key source (§18.3) and are a DEPLOYMENT
/// REQUIREMENT; no PKI, no key generation, no key material ever enters
/// this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchWorkerIdentity {
    /// The worker instance id (unique per running worker; the admission
    /// identity the platform sees).
    pub worker_id: String,
    /// The closed worker family (the P8.31 vocabulary — no "generic"
    /// worker identity exists).
    pub family: ResearchWorkerFamily,
    /// The deployment the instance belongs to (the identity is bound to
    /// the deployment, not to a binary or a machine).
    pub deployment_id: String,
    /// Reference to the instance's client certificate (deployment-held
    /// material; short-lived per the §17.3 posture — the LIFETIME policy
    /// lives in [`WorkerMtlsTransportConfig::session_credential_ttl`], the
    /// material in the deployment).
    pub certificate_reference: Option<String>,
    /// Reference to the instance's private key (deployment-held, hardware-
    /// backed where the platform offers it; never logged, never in an
    /// audit record).
    pub private_key_reference: Option<String>,
}

impl ResearchWorkerIdentity {
    /// Structural validation (fail-closed): non-empty worker and
    /// deployment ids within bounds, and BOTH material references
    /// present. A missing reference is a named error — the worker does not
    /// compose a trust material it cannot present or verify with.
    pub fn validate(&self) -> Result<(), WorkerIdentityError> {
        if self.worker_id.trim().is_empty() {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "worker_id",
            });
        }
        if self.worker_id.len() > MAX_IDENTITY_REF_CHARS {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "worker_id",
            });
        }
        if self.deployment_id.trim().is_empty() {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "deployment_id",
            });
        }
        if self.deployment_id.len() > MAX_IDENTITY_REF_CHARS {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "deployment_id",
            });
        }
        let certificate = self
            .certificate_reference
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if certificate.is_empty() {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "certificate_reference",
            });
        }
        let private_key = self
            .private_key_reference
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if private_key.is_empty() {
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "private_key_reference",
            });
        }
        Ok(())
    }
}

/// The identity store seam (mirrors
/// `apps/plugin-host/src/transport_trust.rs::HostIdentityStore`): the
/// production store resolves the deployment's identity record; tests
/// supply a scripted one. `None` means "no identity is stored" — a
/// fail-closed refusal at assembly, never an anonymous fallback.
pub trait WorkerIdentityStore: Send + Sync {
    fn load_identity(&self) -> Result<Option<ResearchWorkerIdentity>, WorkerIdentityError>;
}

// --- The TLS minimum (the P6.07/P7.34 mirror) -----------------------------------------

/// The pinned minimum protocol version of the worker ↔ platform channel.
/// MIRROR of `apps/plugin-host/src/transport_trust.rs::TlsMinimum` (the
/// same DEFAULT and the same opt-in floor; this crate cannot import the
/// plugin host, so the two-token vocabulary is mirrored — the same
/// recorded mirror discipline as the kernel's permission vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerTlsMinimum {
    /// TLS 1.3 only. THE DEFAULT.
    Tls13,
    /// TLS 1.2 as the floor (opt-in per a site's signed D6 record; never
    /// the default).
    Tls12Floor,
}

impl WorkerTlsMinimum {
    pub const DEFAULT: Self = Self::Tls13;

    pub fn as_str(self) -> &'static str {
        match self {
            WorkerTlsMinimum::Tls13 => "tls13",
            WorkerTlsMinimum::Tls12Floor => "tls12-floor",
        }
    }

    /// Parses the closed two-token vocabulary (unknown tokens refuse).
    pub fn parse(raw: &str) -> Result<Self, WorkerIdentityError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tls13" => Ok(WorkerTlsMinimum::Tls13),
            "tls12-floor" => Ok(WorkerTlsMinimum::Tls12Floor),
            other => Err(WorkerIdentityError::TlsMinimumUnknown {
                value: other.to_string(),
            }),
        }
    }
}

impl fmt::Display for WorkerTlsMinimum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Revocation checking (config-shaped; the P7.34 posture) ----------------------------

/// The revocation-checking configuration of the worker identity. A worker
/// whose config does not configure revocation checking REFUSES to compose
/// (fail-closed: an unverifiable identity is not an identity) — the
/// config-shaped enforcement of the ledger's "a worker presenting a
/// revoked identity is refused" posture; the SESSION-time refusal itself
/// is the platform-side gate over the recorded transport seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerRevocationChecking {
    /// Refused at validation: revocation must be configured.
    Unconfigured,
    /// The deployment's revocation-checking endpoint reference (the
    /// platform-side checker the session gate consults; the reference is
    /// resolved by the composition, never embedded here).
    DeploymentEndpoint { reference: String },
}

// --- The channel configuration ----------------------------------------------------------

/// The worker ↔ platform channel configuration, validated fail-closed
/// (the P7.34 `MtlsTrustConfig` rules, restated for the research-worker
/// channel: the WORKER dials the platform; the platform never dials into
/// the worker — no inbound listener exists on the worker side of this
/// seam, the Class C worker posture).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerMtlsTransportConfig {
    /// The platform endpoint (`wss://host:port` or `https://host:port`).
    /// Plaintext schemes are REFUSED at validation.
    pub platform_endpoint: String,
    /// The TLS server name the worker pins and verifies (config-sourced,
    /// never taken from an untrusted response).
    pub pinned_server_name: String,
    /// Reference to the PINNED platform CA in the worker's deployment
    /// config — never the ambient system store. A missing/empty reference
    /// refuses composition (fail-closed on the absence of a trust anchor).
    pub pinned_platform_ca_reference: Option<String>,
    /// The pinned minimum protocol version (DEFAULT TLS 1.3).
    pub tls_minimum: WorkerTlsMinimum,
    /// The maximum credential lifetime of a worker session (short-lived
    /// credentials, §17.3: the session re-authenticates on expiry; must be
    /// positive).
    pub session_credential_ttl: Duration,
    /// The recorded owner of identity issuance and rotation (ledger P8.33
    /// step 1: "record issuance and rotation owner" — the owner is
    /// CONFIGURATION, validated non-empty, never invented by code).
    pub rotation_owner: String,
    /// Whether expiry monitoring is enabled (the rotation-window warning
    /// posture of the P6.06 `CredentialExpiryMonitor`, mirrored). Disabled
    /// expiry monitoring refuses composition.
    pub expiry_monitor_enabled: bool,
    /// The revocation-checking configuration ([`WorkerRevocationChecking`];
    /// `Unconfigured` refuses composition).
    pub revocation_checking: WorkerRevocationChecking,
}

impl WorkerMtlsTransportConfig {
    /// Validation (fail-closed; every rule is a named error). Order is
    /// deterministic (the mirror order) so tests pin the FIRST refusal of
    /// a multiply-broken config.
    pub fn validate(&self) -> Result<(), WorkerIdentityError> {
        // 1. The endpoint must be an explicit TLS scheme (plaintext
        //    refusal — the P7.34 rule, restated).
        let scheme = self
            .platform_endpoint
            .split("://")
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(scheme.as_str(), "wss" | "https") {
            return Err(WorkerIdentityError::EndpointRefused {
                endpoint_scheme: if self.platform_endpoint.contains("://") {
                    scheme
                } else {
                    "<no scheme>".to_string()
                },
            });
        }
        // 2. The pinned server name is mandatory.
        if self.pinned_server_name.trim().is_empty() {
            return Err(WorkerIdentityError::ServerNameMissing);
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
            return Err(WorkerIdentityError::CaPinMissing);
        }
        // 4. Short-lived credentials: a zero TTL is invalid.
        if self.session_credential_ttl.is_zero() {
            return Err(WorkerIdentityError::CredentialTtlInvalid {
                reason: "the session credential TTL must be positive (short-lived \
                         credentials, §17.3)"
                    .to_string(),
            });
        }
        // 5. The rotation owner is recorded, not invented.
        if self.rotation_owner.trim().is_empty() {
            return Err(WorkerIdentityError::RotationOwnerMissing);
        }
        // 6. Expiry monitoring must be enabled.
        if !self.expiry_monitor_enabled {
            return Err(WorkerIdentityError::ExpiryMonitorDisabled);
        }
        // 7. Revocation checking must be configured.
        match &self.revocation_checking {
            WorkerRevocationChecking::Unconfigured => Err(WorkerIdentityError::RevocationUnconfigured),
            WorkerRevocationChecking::DeploymentEndpoint { reference } => {
                if reference.trim().is_empty() {
                    Err(WorkerIdentityError::RevocationUnconfigured)
                } else {
                    Ok(())
                }
            }
        }
    }
}

// --- The assembled trust material --------------------------------------------------------

/// The validated trust material of a worker instance: REFERENCES to
/// deployment-held material plus the pinned policy. NO key material, NO
/// certificate bytes, NO CA bytes — the transport seam resolves the
/// references at composition time (the recorded
/// [`RESEARCH_TRANSPORT_WIRING_SEAM`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerTrustMaterial {
    pub worker_id: String,
    pub family: ResearchWorkerFamily,
    pub deployment_id: String,
    pub certificate_reference: String,
    pub private_key_reference: String,
    pub pinned_platform_ca_reference: String,
    pub pinned_server_name: String,
    pub platform_endpoint: String,
    pub tls_minimum: WorkerTlsMinimum,
    pub session_credential_ttl: Duration,
    pub rotation_owner: String,
    pub revocation_endpoint_reference: String,
}

/// Assembles the worker trust material: loads the service identity
/// (fail-closed on absence), validates it (fail-closed on an incomplete
/// identity), and validates the channel config (fail-closed on a missing
/// CA pin, a plaintext endpoint, an unpinned server name, or an
/// unconfigured rotation/revocation posture). Mirrors
/// `apps/plugin-host/src/transport_trust.rs::assemble_trust_material`
/// rule for rule; identity-missing and CA-pin-missing are the two
/// fail-closed constructions the tests pin.
pub fn assemble_worker_trust_material(
    store: &dyn WorkerIdentityStore,
    config: &WorkerMtlsTransportConfig,
) -> Result<WorkerTrustMaterial, WorkerIdentityError> {
    let identity = store.load_identity()?.ok_or_else(|| WorkerIdentityError::IdentityMissing {
        reason: "fail closed: no research-worker service identity is stored (the worker \
                 instance has not been issued one by its deployment); the worker does not \
                 compose a trust boundary without an identity"
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
            return Err(WorkerIdentityError::IdentityIncomplete {
                field: "certificate_reference/private_key_reference",
            })
        }
    };
    let pinned_platform_ca_reference = config
        .pinned_platform_ca_reference
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let revocation_endpoint_reference = match &config.revocation_checking {
        WorkerRevocationChecking::DeploymentEndpoint { reference } => reference.trim().to_string(),
        WorkerRevocationChecking::Unconfigured => String::new(),
    };
    Ok(WorkerTrustMaterial {
        worker_id: identity.worker_id,
        family: identity.family,
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

// --- The append-only worker-identity revocation register (ledger step 3) -----------------

/// One worker-identity revocation record (append-only). The reason and
/// actor are REQUIRED — an unattributed revocation is refused, never
/// recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerIdentityRevocation {
    /// The revoked worker instance id.
    pub worker_id: String,
    /// Why the identity was revoked (decision-record reference — REQUIRED).
    pub reason: String,
    /// Who revoked it (REQUIRED — the recorded revocation owner acts here).
    pub actor: String,
    /// When (injected-clock millis; set by the register, never by the caller).
    pub revoked_at_millis: i64,
}

/// The append-only worker-identity revocation register (ledger step 3's
/// "a worker presenting a revoked identity is refused"). There is NO
/// un-revoke and NO edit: the only direction implemented is the safe one —
/// a revocation cannot silently disappear (the kernel `UNREVOCATION_SEAM`
/// honesty, mirrored). Re-issue of a rotated identity is a NEW worker
/// instance id, never an edit of this register.
#[derive(Debug, Default)]
pub struct WorkerIdentityRevocationRegister {
    revoked: BTreeMap<String, WorkerIdentityRevocation>,
    append_order: Vec<WorkerIdentityRevocation>,
}

impl WorkerIdentityRevocationRegister {
    pub fn new() -> WorkerIdentityRevocationRegister {
        WorkerIdentityRevocationRegister {
            revoked: BTreeMap::new(),
            append_order: Vec::new(),
        }
    }

    /// Records one revocation (host-side). The reason and actor are
    /// REQUIRED; a duplicate worker id is refused. Both the recording and
    /// the refusal are audited.
    pub fn revoke(
        &mut self,
        worker_id: &str,
        reason: &str,
        actor: &str,
        clock: &dyn HarnessClock,
        audit: &dyn WorkerIdentityAuditSink,
    ) -> Result<(), WorkerIdentityError> {
        let outcome = self.revoke_inner(worker_id, reason, actor, clock);
        match &outcome {
            Ok(()) => audit.record(WorkerIdentityAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: WorkerIdentityAuditKind::RevocationRecorded,
                worker_id: worker_id.to_string(),
                detail: format!("worker identity revoked (by '{actor}'): {reason}"),
            }),
            Err(err) => audit.record(WorkerIdentityAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: WorkerIdentityAuditKind::RevocationRefused,
                worker_id: worker_id.to_string(),
                detail: format!("revocation refused ({}): {err}", err.name()),
            }),
        }
        outcome
    }

    fn revoke_inner(
        &mut self,
        worker_id: &str,
        reason: &str,
        actor: &str,
        clock: &dyn HarnessClock,
    ) -> Result<(), WorkerIdentityError> {
        if worker_id.trim().is_empty() {
            return Err(WorkerIdentityError::RevocationInvalid {
                reason: "the worker id is empty — a revocation binds an identity".to_string(),
            });
        }
        if reason.trim().is_empty() {
            return Err(WorkerIdentityError::RevocationInvalid {
                reason: "a revocation requires a non-empty reason (an unattributed \
                         revocation is never recorded)"
                    .to_string(),
            });
        }
        if actor.trim().is_empty() {
            return Err(WorkerIdentityError::RevocationInvalid {
                reason: "a revocation requires a non-empty actor".to_string(),
            });
        }
        if self.revoked.contains_key(worker_id) {
            return Err(WorkerIdentityError::RevocationAlreadyRecorded {
                worker_id: worker_id.to_string(),
            });
        }
        let record = WorkerIdentityRevocation {
            worker_id: worker_id.to_string(),
            reason: reason.to_string(),
            actor: actor.to_string(),
            revoked_at_millis: clock.now_millis(),
        };
        self.revoked.insert(worker_id.to_string(), record.clone());
        self.append_order.push(record);
        Ok(())
    }

    /// Whether the worker id is revoked.
    pub fn is_revoked(&self, worker_id: &str) -> bool {
        self.revoked.contains_key(worker_id)
    }

    /// The revocations in append order (audit/replay view).
    pub fn revocations_in_append_order(&self) -> &[WorkerIdentityRevocation] {
        &self.append_order
    }
}

// --- The admission door (fail-closed composition) -----------------------------------------

/// The worker admission door (the composition-root entry point; ledger
/// step 3's "a worker presenting a revoked identity is refused"). Order is
/// deterministic:
///
/// 1. the trust material is assembled (missing/incomplete identity,
///    broken channel config — each a named refusal);
/// 2. the assembled identity is checked against the revocation register
///    (a revoked worker is refused EVEN with a perfect config).
///
/// EVERY outcome is audited with a timestamp from the injected clock. The
/// SESSION-time revocation enforcement (in-flight work drained, no new
/// session on expiry) remains the platform-side gate over the recorded
/// transport seam — this is the ADMISSION-time enforcement, and no
/// statement here claims session-time coverage.
pub fn verify_worker_admission(
    store: &dyn WorkerIdentityStore,
    config: &WorkerMtlsTransportConfig,
    revocations: &WorkerIdentityRevocationRegister,
    clock: &dyn HarnessClock,
    audit: &dyn WorkerIdentityAuditSink,
) -> Result<WorkerTrustMaterial, WorkerIdentityError> {
    let outcome = verify_worker_admission_inner(store, config, revocations, clock);
    let (kind, worker_id, detail) = match &outcome {
        Ok(material) => (
            WorkerIdentityAuditKind::AdmissionAccepted,
            material.worker_id.clone(),
            format!(
                "worker admission accepted ({} in deployment '{}'; tls minimum {}; \
                 rotation owner '{}'; revocation endpoint configured)",
                material.family.as_str(),
                material.deployment_id,
                material.tls_minimum,
                material.rotation_owner,
            ),
        ),
        Err(err) => {
            // The worker id is best-effort here (the identity may be the
            // thing that failed); the store's record, if loadable, names
            // it. No secret material ever reaches this string.
            let worker_id = store
                .load_identity()
                .ok()
                .and_then(|identity| identity)
                .map(|identity| identity.worker_id)
                .unwrap_or_else(|| "(unknown)".to_string());
            (
                WorkerIdentityAuditKind::AdmissionRefused,
                worker_id,
                format!("worker admission refused ({}): {err}", err.name()),
            )
        }
    };
    audit.record(WorkerIdentityAuditRecord {
        timestamp_millis: clock.now_millis(),
        kind,
        worker_id,
        detail,
    });
    outcome
}

fn verify_worker_admission_inner(
    store: &dyn WorkerIdentityStore,
    config: &WorkerMtlsTransportConfig,
    revocations: &WorkerIdentityRevocationRegister,
    _clock: &dyn HarnessClock,
) -> Result<WorkerTrustMaterial, WorkerIdentityError> {
    let material = assemble_worker_trust_material(store, config)?;
    if revocations.is_revoked(&material.worker_id) {
        return Err(WorkerIdentityError::IdentityRevoked {
            worker_id: material.worker_id.clone(),
        });
    }
    Ok(material)
}

// --- Secret handling: references only (ledger step 2) --------------------------------------

/// A secret REFERENCE: a name and a scheme-prefixed location. There is no
/// field anywhere in the worker-identity types that can carry a secret
/// VALUE — the object keys are closed to `{name, reference}` and the
/// reference must name a location via one of [`SECRET_REFERENCE_SCHEMES`]
/// (the kernel manifest rule, mirrored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSecretReference {
    pub name: String,
    pub reference: String,
}

/// The credential-need names a research worker may declare (closed
/// vocabulary; the ledger's enumerated needs plus the artifact/simulator
/// access the workers actually use). A need outside this set is refused —
/// there is no "other" bucket a secret could hide in.
pub const WORKER_CREDENTIAL_NEEDS: [&str; 5] = [
    "dataset-access",
    "model-store-access",
    "simulator-access",
    "artifact-store-access",
    "telemetry-access",
];

/// Parses the worker's declared secret references from their JSON form
/// (an array of `{name, reference}` objects). Fail-closed rules, each a
/// named error:
///
/// - the value must be an array of objects with EXACTLY the keys
///   `{name, reference}` (the closed key set — no field a secret value
///   could hide in);
/// - `name` must be a non-empty member of [`WORKER_CREDENTIAL_NEEDS`] and
///   at most 256 characters, declared exactly once;
/// - `reference` must use one of [`SECRET_REFERENCE_SCHEMES`] and name a
///   location — a scheme-less reference is rejected as LIKELY INLINE
///   SECRET MATERIAL, and the rejected VALUE is never echoed back.
pub fn parse_secret_declarations(value: &Value) -> Result<Vec<WorkerSecretReference>, WorkerIdentityError> {
    let items = match value {
        Value::Array(items) => items,
        _ => {
            return Err(WorkerIdentityError::SecretDeclarationInvalid {
                reason: format!(
                    "the secret declarations must be a JSON array of {{name, reference}} \
                     objects, found {}",
                    json_type_name(value)
                ),
            })
        }
    };
    let mut out: Vec<WorkerSecretReference> = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let location = format!("secrets[{idx}]");
        let obj = match item {
            Value::Object(map) => map,
            _ => {
                return Err(WorkerIdentityError::SecretDeclarationInvalid {
                    reason: format!(
                        "{location} must be an object with keys {{name, reference}}, found {}",
                        json_type_name(item)
                    ),
                })
            }
        };
        const ALLOWED_SECRET_KEYS: [&str; 2] = ["name", "reference"];
        reject_unknown_fields(obj, &ALLOWED_SECRET_KEYS, &location)?;
        let name = required_string(obj, "name", &location)?;
        let reference = required_string(obj, "reference", &location)?;
        if name.is_empty() || name.len() > MAX_IDENTITY_REF_CHARS {
            return Err(WorkerIdentityError::SecretReferenceInvalid {
                name,
                reason: "secret names are non-empty strings of at most 256 characters"
                    .to_string(),
            });
        }
        if !WORKER_CREDENTIAL_NEEDS.contains(&name.as_str()) {
            return Err(WorkerIdentityError::SecretReferenceInvalid {
                name,
                reason: format!(
                    "secret names are the closed credential-need vocabulary ({}); there is \
                     no 'other' bucket a secret could hide in",
                    WORKER_CREDENTIAL_NEEDS.join(", ")
                ),
            });
        }
        // Reference, not value: must name a location through one of the
        // accepted schemes. The rejected VALUE is never echoed back.
        let scheme_ok = SECRET_REFERENCE_SCHEMES.iter().any(|scheme| {
            reference
                .strip_prefix(*scheme)
                .map(|rest| !rest.is_empty())
                .unwrap_or(false)
        });
        if !scheme_ok {
            return Err(WorkerIdentityError::SecretReferenceInvalid {
                name,
                reason: "a secret reference must use the env:, file:, or secret-store: \
                         scheme and name a location; worker secrets are references only \
                         and can never carry values (a scheme-less reference is treated \
                         as likely inline secret material and refused)"
                    .to_string(),
            });
        }
        if out.iter().any(|s| s.name == name) {
            return Err(WorkerIdentityError::SecretNameDuplicate { name });
        }
        out.push(WorkerSecretReference { name, reference });
    }
    Ok(out)
}

// --- The worker-config secret-literal scan (ledger step 3) -----------------------------------

/// The SANCTIONED secret-declaration container keys. These keys are
/// exempt from the forbidden-vocabulary check (the kernel manifest
/// legitimately declares a `secrets` array of scheme-prefixed
/// REFERENCES; the workers' declared credential needs travel under one of
/// these keys) — but their CHILDREN are still scanned, and the
/// declarations themselves are validated by [`parse_secret_declarations`]
/// (closed `{name, reference}` keys, closed schemes), so the exemption
/// opens no value channel.
pub const SANCTIONED_DECLARATION_KEYS: [&str; 2] = ["secrets", "secret_references"];

/// Normalizes a config key for the scan: lowercased, `-` folded to `_`
/// (so `db-password`, `DB_PASSWORD`, and `db_password` are one shape).
fn normalized_key(key: &str) -> String {
    key.to_lowercase().replace('-', "_")
}

/// Whether a config key (after normalization) is in the closed forbidden
/// vocabulary (substring containment — `my_password_field` is refused too).
fn key_is_forbidden(normalized: &str) -> bool {
    FORBIDDEN_CONFIG_SECRET_KEYS
        .iter()
        .any(|forbidden| normalized.contains(forbidden))
}

/// Whether a config key is one of the sanctioned declaration containers
/// (its key name is forbidden-shaped but its content is the sanctioned
/// reference path).
fn key_is_sanctioned(normalized: &str) -> bool {
    SANCTIONED_DECLARATION_KEYS.contains(&normalized)
}

/// The worker-config secret-literal scan (ledger step 3's "a worker config
/// containing a literal secret value is rejected at load"): a recursive
/// walk over the config JSON that refuses any object key whose normalized
/// form contains a [`FORBIDDEN_CONFIG_SECRET_KEYS`] entry, at ANY depth
/// (the worker suites' whole-tree scan discipline, mirrored). ALL
/// violations are collected and sorted by (location, key) so the reported
/// FIRST violation is deterministic.
///
/// HONEST RESIDUAL (recorded in the module header, pinned by a test): the
/// scan refuses the forbidden KEY vocabulary; it cannot detect a secret
/// hidden under an innocuous key name. The structural mitigation is the
/// [`WorkerSecretReference`] declaration path plus the closed config
/// schema the workers ship with.
pub fn scan_worker_config(config: &Value) -> Result<(), WorkerIdentityError> {
    let mut violations: Vec<(String, String)> = Vec::new();
    match config {
        Value::Object(_) => walk_config_for_secret_keys(config, "$", &mut violations),
        _ => {
            return Err(WorkerIdentityError::ConfigStructureInvalid {
                reason: format!("the worker config must be a JSON object, found {}", json_type_name(config)),
            })
        }
    }
    if violations.is_empty() {
        return Ok(());
    }
    violations.sort();
    let (location, key) = violations.remove(0);
    Err(WorkerIdentityError::ConfigSecretKeyRefused { location, key })
}

fn walk_config_for_secret_keys(value: &Value, path: &str, violations: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = normalized_key(key);
                if key_is_forbidden(&normalized) && !key_is_sanctioned(&normalized) {
                    violations.push((path.to_string(), key.clone()));
                }
                let child_path = format!("{path}.{key}");
                walk_config_for_secret_keys(child, &child_path, violations);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{idx}]");
                walk_config_for_secret_keys(child, &child_path, violations);
            }
        }
        // Scalars carry no keys; nothing to scan. NOTE: a secret VALUE
        // under an innocuous key is not detectable here — the recorded
        // residual (module header).
        _ => {}
    }
}

/// The audited scan path ([`scan_worker_config`] plus an audit record for
/// EVERY outcome — the ledger's "rejected at load and the attempt audited").
/// The audit detail carries the refused KEY NAME and its location — never
/// any config VALUE (a value under a refused key is exactly the secret
/// material the scan exists to keep out of logs).
pub fn scan_worker_config_audited(
    config: &Value,
    clock: &dyn HarnessClock,
    audit: &dyn WorkerIdentityAuditSink,
) -> Result<(), WorkerIdentityError> {
    let outcome = scan_worker_config(config);
    let (kind, detail) = match &outcome {
        Ok(()) => (
            WorkerIdentityAuditKind::SecretConfigAccepted,
            "worker config scanned: no forbidden credential key present".to_string(),
        ),
        Err(err) => match err {
            WorkerIdentityError::ConfigSecretKeyRefused { ref location, ref key } => (
                WorkerIdentityAuditKind::SecretConfigRefused,
                // The KEY NAME is diagnostic; the VALUE under it is never
                // included (it is the secret the scan keeps out of logs).
                format!(
                    "worker config refused ({WORKER_CONFIG_REFUSAL_TAG}): forbidden \
                     credential key '{key}' at {location}"
                ),
            ),
            _ => (
                WorkerIdentityAuditKind::SecretConfigRefused,
                format!("worker config refused ({}): {err}", err.name()),
            ),
        },
    };
    audit.record(WorkerIdentityAuditRecord {
        timestamp_millis: clock.now_millis(),
        kind,
        worker_id: "(config-scan)".to_string(),
        detail,
    });
    outcome
}

/// The stable refusal tag the scan rides (the load-site wiring at the
/// with-framework activation wave surfaces this tag to the worker's
/// structured refusal channel).
pub const WORKER_CONFIG_REFUSAL_TAG: &str = "WORKER_CONFIG_SECRET_KEY_REFUSED";

// --- Errors ---------------------------------------------------------------------------

/// Every worker-identity refusal is a DISTINCT named error
/// ([`WorkerIdentityError::name`]) — never a warning, never a silent
/// acceptance, never a bare boolean. Error text never echoes a rejected
/// secret-reference VALUE or any config VALUE (they may be secret
/// material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerIdentityError {
    /// No worker service identity is stored — fail-closed (never an
    /// anonymous fallback).
    IdentityMissing { reason: String },
    /// The identity record is structurally incomplete (`field` names the
    /// missing piece).
    IdentityIncomplete { field: &'static str },
    /// The identity store itself failed.
    IdentityStoreFailed { reason: String },
    /// The platform endpoint is plaintext or scheme-less — refused.
    EndpointRefused { endpoint_scheme: String },
    /// The TLS server name is not pinned.
    ServerNameMissing,
    /// The deployment config pins no platform CA — fail-closed (the worker
    /// refuses to trust an unpinned platform endpoint).
    CaPinMissing,
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
    /// The worker identity is in the append-only revocation register —
    /// the ledger's step-3 refusal.
    IdentityRevoked { worker_id: String },
    /// A revocation record is structurally invalid (empty worker id,
    /// reason, or actor).
    RevocationInvalid { reason: String },
    /// The worker identity is already in the revocation register (the
    /// register is append-only; exactly one record per worker id).
    RevocationAlreadyRecorded { worker_id: String },
    /// The secret-declaration shape is invalid (not an array of
    /// `{name, reference}` objects).
    SecretDeclarationInvalid { reason: String },
    /// A secret reference does not name a scheme-prefixed location (likely
    /// inline secret material) or the name is out of vocabulary. The
    /// rejected VALUE is deliberately NOT echoed.
    SecretReferenceInvalid { name: String, reason: String },
    /// Two secret references share one name.
    SecretNameDuplicate { name: String },
    /// A config key in the closed forbidden credential vocabulary was
    /// found at the named location (the literal-secret refusal). The key
    /// NAME is carried for diagnosis; the VALUE under it is never carried.
    ConfigSecretKeyRefused { location: String, key: String },
    /// The scanned config is not a JSON object.
    ConfigStructureInvalid { reason: String },
}

impl WorkerIdentityError {
    /// The stable machine-readable error name (the named-error table key;
    /// pinned by tests).
    pub fn name(&self) -> &'static str {
        match self {
            WorkerIdentityError::IdentityMissing { .. } => "WORKER_IDENTITY_MISSING",
            WorkerIdentityError::IdentityIncomplete { .. } => "WORKER_IDENTITY_INCOMPLETE",
            WorkerIdentityError::IdentityStoreFailed { .. } => "WORKER_IDENTITY_STORE_FAILED",
            WorkerIdentityError::EndpointRefused { .. } => "WORKER_IDENTITY_ENDPOINT_REFUSED",
            WorkerIdentityError::ServerNameMissing => "WORKER_IDENTITY_SERVER_NAME_MISSING",
            WorkerIdentityError::CaPinMissing => "WORKER_IDENTITY_CA_PIN_MISSING",
            WorkerIdentityError::CredentialTtlInvalid { .. } => {
                "WORKER_IDENTITY_CREDENTIAL_TTL_INVALID"
            }
            WorkerIdentityError::RotationOwnerMissing => "WORKER_IDENTITY_ROTATION_OWNER_MISSING",
            WorkerIdentityError::ExpiryMonitorDisabled => "WORKER_IDENTITY_EXPIRY_MONITOR_DISABLED",
            WorkerIdentityError::RevocationUnconfigured => "WORKER_IDENTITY_REVOCATION_UNCONFIGURED",
            WorkerIdentityError::TlsMinimumUnknown { .. } => "WORKER_IDENTITY_TLS_MINIMUM_UNKNOWN",
            WorkerIdentityError::IdentityRevoked { .. } => "WORKER_IDENTITY_REVOKED",
            WorkerIdentityError::RevocationInvalid { .. } => "WORKER_IDENTITY_REVOCATION_INVALID",
            WorkerIdentityError::RevocationAlreadyRecorded { .. } => {
                "WORKER_IDENTITY_REVOCATION_ALREADY_RECORDED"
            }
            WorkerIdentityError::SecretDeclarationInvalid { .. } => {
                "WORKER_SECRET_DECLARATION_INVALID"
            }
            WorkerIdentityError::SecretReferenceInvalid { .. } => {
                "WORKER_SECRET_REFERENCE_INVALID"
            }
            WorkerIdentityError::SecretNameDuplicate { .. } => "WORKER_SECRET_NAME_DUPLICATE",
            WorkerIdentityError::ConfigSecretKeyRefused { .. } => "WORKER_CONFIG_SECRET_KEY_REFUSED",
            WorkerIdentityError::ConfigStructureInvalid { .. } => "WORKER_CONFIG_STRUCTURE_INVALID",
        }
    }
}

impl fmt::Display for WorkerIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerIdentityError::IdentityMissing { reason } => {
                write!(f, "no research-worker service identity: {reason}")
            }
            WorkerIdentityError::IdentityIncomplete { field } => {
                write!(f, "the worker service identity is incomplete: '{field}' is missing")
            }
            WorkerIdentityError::IdentityStoreFailed { reason } => {
                write!(f, "the worker identity store failed: {reason}")
            }
            WorkerIdentityError::EndpointRefused { endpoint_scheme } => write!(
                f,
                "the platform endpoint must be an explicit TLS scheme (wss:// or \
                 https://); got '{endpoint_scheme}'. Plaintext (ws://, http://) and \
                 scheme-less endpoints are REFUSED"
            ),
            WorkerIdentityError::ServerNameMissing => {
                write!(f, "the pinned TLS server name is mandatory")
            }
            WorkerIdentityError::CaPinMissing => write!(
                f,
                "fail closed: the deployment config pins no platform CA; the worker \
                 refuses to trust an unpinned platform endpoint (the ambient system \
                 store is never a fallback)"
            ),
            WorkerIdentityError::CredentialTtlInvalid { reason } => {
                write!(f, "the session credential TTL is invalid: {reason}")
            }
            WorkerIdentityError::RotationOwnerMissing => write!(
                f,
                "no rotation owner is recorded (issuance and rotation ownership is \
                 deployment configuration, never invented by code)"
            ),
            WorkerIdentityError::ExpiryMonitorDisabled => write!(
                f,
                "expiry monitoring is disabled; the worker refuses to compose a trust \
                 boundary that cannot see its own credential expiry"
            ),
            WorkerIdentityError::RevocationUnconfigured => write!(
                f,
                "revocation checking is unconfigured; the worker refuses to compose a \
                 trust boundary that cannot detect a revoked identity"
            ),
            WorkerIdentityError::TlsMinimumUnknown { value } => write!(
                f,
                "unknown TLS minimum '{value}'; expected 'tls13' (default) or \
                 'tls12-floor' (opt-in per a signed site D6 record)"
            ),
            WorkerIdentityError::IdentityRevoked { worker_id } => write!(
                f,
                "the worker identity '{worker_id}' is REVOKED; a worker presenting a \
                 revoked identity is refused at admission (ledger P8.33 step 3)"
            ),
            WorkerIdentityError::RevocationInvalid { reason } => {
                write!(f, "the worker-identity revocation is invalid: {reason}")
            }
            WorkerIdentityError::RevocationAlreadyRecorded { worker_id } => write!(
                f,
                "the worker identity '{worker_id}' is already revoked (the register is \
                 append-only; exactly one revocation record per worker id)"
            ),
            WorkerIdentityError::SecretDeclarationInvalid { reason } => {
                write!(f, "the secret declaration is invalid: {reason}")
            }
            WorkerIdentityError::SecretReferenceInvalid { name, reason } => {
                // The reference VALUE is deliberately not echoed: a
                // rejected reference may be inline secret material.
                write!(f, "secret '{name}' is not a valid reference: {reason}")
            }
            WorkerIdentityError::SecretNameDuplicate { name } => {
                write!(f, "secret name '{name}' is declared twice")
            }
            WorkerIdentityError::ConfigSecretKeyRefused { location, key } => write!(
                f,
                "worker config refused: a forbidden credential key '{key}' was found at \
                 {location} — credential needs travel as scheme-prefixed secret \
                 REFERENCES resolved by the host, never as values in worker config \
                 (§17.3); the value under the key is never echoed or logged"
            ),
            WorkerIdentityError::ConfigStructureInvalid { reason } => {
                write!(f, "the worker config is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for WorkerIdentityError {}

// --- The audit port ---------------------------------------------------------------------

/// The kinds of worker-identity audit records (SCREAMING_SNAKE stable
/// style; every admission outcome, every revocation outcome, and every
/// config-scan outcome is audited).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerIdentityAuditKind {
    /// A worker admission was accepted (material assembled, identity not
    /// revoked).
    AdmissionAccepted,
    /// A worker admission was REFUSED (missing/incomplete identity, broken
    /// config, revoked identity — the detail names the refusal class).
    AdmissionRefused,
    /// A revocation was recorded (append-only register).
    RevocationRecorded,
    /// A revocation was refused (duplicate, missing reason/actor).
    RevocationRefused,
    /// A worker config passed the secret-literal scan.
    SecretConfigAccepted,
    /// A worker config was REFUSED by the secret-literal scan (forbidden
    /// credential key at a named location — the VALUE is never in the
    /// detail).
    SecretConfigRefused,
}

impl WorkerIdentityAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            WorkerIdentityAuditKind::AdmissionAccepted => "WORKER_IDENTITY_ADMISSION_ACCEPTED",
            WorkerIdentityAuditKind::AdmissionRefused => "WORKER_IDENTITY_ADMISSION_REFUSED",
            WorkerIdentityAuditKind::RevocationRecorded => "WORKER_IDENTITY_REVOCATION_RECORDED",
            WorkerIdentityAuditKind::RevocationRefused => "WORKER_IDENTITY_REVOCATION_REFUSED",
            WorkerIdentityAuditKind::SecretConfigAccepted => "WORKER_SECRET_CONFIG_ACCEPTED",
            WorkerIdentityAuditKind::SecretConfigRefused => "WORKER_SECRET_CONFIG_REFUSED",
        }
    }
}

/// One worker-identity audit record. The `detail` carries identity
/// reference renderings, refusal names, and forbidden KEY NAMES only —
/// never a certificate/key/secret-reference VALUE (the ledger's "no secret
/// appears in worker logs", enforced at the audit boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerIdentityAuditRecord {
    /// When the event was recorded (injected-clock millis).
    pub timestamp_millis: i64,
    pub kind: WorkerIdentityAuditKind,
    /// The worker the event concerns (or "(host)"/"(config-scan)" for
    /// those operations).
    pub worker_id: String,
    /// Human-readable detail (reference renderings and refusal names — no
    /// secret material).
    pub detail: String,
}

/// The worker-identity audit sink port (crate-local, the kernel audit
/// posture mirrored). Implementations MUST be append-only (§5.1 audit
/// integrity).
pub trait WorkerIdentityAuditSink {
    fn record(&self, record: WorkerIdentityAuditRecord);
}

/// An append-only in-memory [`WorkerIdentityAuditSink`] — the deterministic
/// collector used by tests and in-process compositions.
#[derive(Debug, Default)]
pub struct MemoryWorkerIdentityAuditSink {
    records: std::cell::RefCell<Vec<WorkerIdentityAuditRecord>>,
}

impl MemoryWorkerIdentityAuditSink {
    pub fn new() -> MemoryWorkerIdentityAuditSink {
        MemoryWorkerIdentityAuditSink {
            records: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// The records in emission order (append-only; the borrow is brief and
    /// single-threaded).
    pub fn records(&self) -> std::cell::Ref<Vec<WorkerIdentityAuditRecord>> {
        self.records.borrow()
    }
}

impl WorkerIdentityAuditSink for MemoryWorkerIdentityAuditSink {
    fn record(&self, record: WorkerIdentityAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}

// --- Shared parsing helpers (the manifest.rs style, local copy) -------------------

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
) -> Result<String, WorkerIdentityError> {
    match obj.get(field) {
        None => Err(WorkerIdentityError::SecretDeclarationInvalid {
            reason: format!("missing required field '{field}' at {location}"),
        }),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(WorkerIdentityError::SecretDeclarationInvalid {
            reason: format!(
                "field '{field}' at {location} must be a string, found {}",
                json_type_name(other)
            ),
        }),
    }
}

fn reject_unknown_fields(
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    location: &str,
) -> Result<(), WorkerIdentityError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(WorkerIdentityError::SecretDeclarationInvalid {
            reason: format!("unknown field '{field}' at {location} (the closed key set is \
                             what makes a secret VALUE structurally impossible)"),
        }),
    }
}

// --- The closed error-name table ---------------------------------------------------------

/// All stable error names of this module (sorted; the pairwise-distinct
/// property is pinned by tests, the house style).
pub const WORKER_IDENTITY_ERROR_NAMES: [&str; 19] = [
    "WORKER_CONFIG_SECRET_KEY_REFUSED",
    "WORKER_CONFIG_STRUCTURE_INVALID",
    "WORKER_IDENTITY_CA_PIN_MISSING",
    "WORKER_IDENTITY_CREDENTIAL_TTL_INVALID",
    "WORKER_IDENTITY_ENDPOINT_REFUSED",
    "WORKER_IDENTITY_EXPIRY_MONITOR_DISABLED",
    "WORKER_IDENTITY_INCOMPLETE",
    "WORKER_IDENTITY_MISSING",
    "WORKER_IDENTITY_REVOCATION_ALREADY_RECORDED",
    "WORKER_IDENTITY_REVOCATION_INVALID",
    "WORKER_IDENTITY_REVOKED",
    "WORKER_IDENTITY_REVOCATION_UNCONFIGURED",
    "WORKER_IDENTITY_ROTATION_OWNER_MISSING",
    "WORKER_IDENTITY_SERVER_NAME_MISSING",
    "WORKER_IDENTITY_STORE_FAILED",
    "WORKER_IDENTITY_TLS_MINIMUM_UNKNOWN",
    "WORKER_SECRET_DECLARATION_INVALID",
    "WORKER_SECRET_NAME_DUPLICATE",
    "WORKER_SECRET_REFERENCE_INVALID",
];

/// The error names taken from the enum itself (sorted, deduplicated via
/// `BTreeSet`) — the test-side consistency check against
/// [`WORKER_IDENTITY_ERROR_NAMES`], so a variant added without updating
/// the table is a named test failure.
pub fn error_names_from_enum() -> std::collections::BTreeSet<&'static str> {
    let names: [&str; 19] = [
        WorkerIdentityError::IdentityMissing { reason: String::new() }.name(),
        WorkerIdentityError::IdentityIncomplete { field: "worker_id" }.name(),
        WorkerIdentityError::IdentityStoreFailed { reason: String::new() }.name(),
        WorkerIdentityError::EndpointRefused { endpoint_scheme: String::new() }.name(),
        WorkerIdentityError::ServerNameMissing.name(),
        WorkerIdentityError::CaPinMissing.name(),
        WorkerIdentityError::CredentialTtlInvalid { reason: String::new() }.name(),
        WorkerIdentityError::RotationOwnerMissing.name(),
        WorkerIdentityError::ExpiryMonitorDisabled.name(),
        WorkerIdentityError::RevocationUnconfigured.name(),
        WorkerIdentityError::TlsMinimumUnknown { value: String::new() }.name(),
        WorkerIdentityError::IdentityRevoked { worker_id: String::new() }.name(),
        WorkerIdentityError::RevocationInvalid { reason: String::new() }.name(),
        WorkerIdentityError::RevocationAlreadyRecorded { worker_id: String::new() }.name(),
        WorkerIdentityError::SecretDeclarationInvalid { reason: String::new() }.name(),
        WorkerIdentityError::SecretReferenceInvalid {
            name: String::new(),
            reason: String::new(),
        }
        .name(),
        WorkerIdentityError::SecretNameDuplicate { name: String::new() }.name(),
        WorkerIdentityError::ConfigSecretKeyRefused {
            location: String::new(),
            key: String::new(),
        }
        .name(),
        WorkerIdentityError::ConfigStructureInvalid { reason: String::new() }.name(),
    ];
    names.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the rules above — the config-validation order, the
// closed key sets, the scheme rules, the refusal names — or a literal
// copied from this module's own constants. Nothing is fabricated.)
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

    /// A structurally complete identity fixture.
    fn identity(worker_id: &str, family: ResearchWorkerFamily) -> ResearchWorkerIdentity {
        ResearchWorkerIdentity {
            worker_id: worker_id.to_string(),
            family,
            deployment_id: "deployment-lab-1".to_string(),
            certificate_reference: Some("file:///etc/flexcore/workers/worker.crt".to_string()),
            private_key_reference: Some("file:///etc/flexcore/workers/worker.key".to_string()),
        }
    }

    struct FixedStore {
        identity: Option<ResearchWorkerIdentity>,
    }

    impl WorkerIdentityStore for FixedStore {
        fn load_identity(&self) -> Result<Option<ResearchWorkerIdentity>, WorkerIdentityError> {
            Ok(self.identity.clone())
        }
    }

    /// A fully-valid channel config fixture (each test mutates one rule to
    /// pin the FIRST refusal of the deterministic order).
    fn valid_config() -> WorkerMtlsTransportConfig {
        WorkerMtlsTransportConfig {
            platform_endpoint: "https://controlplane.internal:8443".to_string(),
            pinned_server_name: "controlplane.internal".to_string(),
            pinned_platform_ca_reference: Some("file:///etc/flexcore/workers/platform-ca.pem".to_string()),
            tls_minimum: WorkerTlsMinimum::Tls13,
            session_credential_ttl: Duration::from_secs(300),
            rotation_owner: "platform-security".to_string(),
            expiry_monitor_enabled: true,
            revocation_checking: WorkerRevocationChecking::DeploymentEndpoint {
                reference: "file:///etc/flexcore/workers/revocation-endpoint".to_string(),
            },
        }
    }

    #[test]
    fn an_incomplete_identity_names_the_missing_piece() {
        let mut id = identity("worker/gnn-1", ResearchWorkerFamily::Gnn);
        id.certificate_reference = None;
        let err = id.validate().expect_err("missing certificate refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_INCOMPLETE");
        assert!(format!("{err}").contains("certificate_reference"));
        let mut id = identity("worker/gnn-1", ResearchWorkerFamily::Gnn);
        id.private_key_reference = Some("   ".to_string());
        let err = id.validate().expect_err("blank private-key reference refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_INCOMPLETE");
        assert!(format!("{err}").contains("private_key_reference"));
    }

    #[test]
    fn the_channel_config_refuses_each_broken_rule_in_order() {
        // 1. plaintext endpoint first.
        let mut config = valid_config();
        config.platform_endpoint = "http://controlplane.internal:8443".to_string();
        let err = config.validate().expect_err("plaintext refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_ENDPOINT_REFUSED");
        // 2. missing server name.
        let mut config = valid_config();
        config.pinned_server_name = "  ".to_string();
        let err = config.validate().expect_err("unpinned server name refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_SERVER_NAME_MISSING");
        // 3. missing CA pin.
        let mut config = valid_config();
        config.pinned_platform_ca_reference = None;
        let err = config.validate().expect_err("absent CA pin refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_CA_PIN_MISSING");
        // 4. zero TTL.
        let mut config = valid_config();
        config.session_credential_ttl = Duration::from_secs(0);
        let err = config.validate().expect_err("zero TTL refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_CREDENTIAL_TTL_INVALID");
        // 5. missing rotation owner.
        let mut config = valid_config();
        config.rotation_owner = String::new();
        let err = config.validate().expect_err("unowned rotation refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_ROTATION_OWNER_MISSING");
        // 6. disabled expiry monitor.
        let mut config = valid_config();
        config.expiry_monitor_enabled = false;
        let err = config.validate().expect_err("disabled expiry monitor refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_EXPIRY_MONITOR_DISABLED");
        // 7. unconfigured revocation.
        let mut config = valid_config();
        config.revocation_checking = WorkerRevocationChecking::Unconfigured;
        let err = config.validate().expect_err("unconfigured revocation refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_REVOCATION_UNCONFIGURED");
        // Multiply-broken config: the FIRST rule wins (deterministic order).
        let mut config = valid_config();
        config.platform_endpoint = "ws://x".to_string();
        config.pinned_server_name = String::new();
        let err = config.validate().expect_err("first refusal is the endpoint");
        assert_eq!(err.name(), "WORKER_IDENTITY_ENDPOINT_REFUSED");
    }

    #[test]
    fn the_tls_minimum_vocabulary_is_closed() {
        assert_eq!(WorkerTlsMinimum::parse("tls13").expect("parses"), WorkerTlsMinimum::Tls13);
        assert_eq!(
            WorkerTlsMinimum::parse("TLS12-FLOOR").expect("case-insensitive"),
            WorkerTlsMinimum::Tls12Floor
        );
        let err = WorkerTlsMinimum::parse("ssl3").expect_err("unknown refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_TLS_MINIMUM_UNKNOWN");
    }

    #[test]
    fn a_missing_identity_is_fail_closed_never_anonymous() {
        let store = FixedStore { identity: None };
        let err = assemble_worker_trust_material(&store, &valid_config())
            .expect_err("no identity is a refusal");
        assert_eq!(err.name(), "WORKER_IDENTITY_MISSING");
    }

    #[test]
    fn a_valid_identity_assembles_the_trust_material_with_references_only() {
        let store = FixedStore {
            identity: Some(identity("worker/marl-1", ResearchWorkerFamily::Marl)),
        };
        let material = assemble_worker_trust_material(&store, &valid_config()).expect("composes");
        assert_eq!(material.worker_id, "worker/marl-1");
        assert_eq!(material.family, ResearchWorkerFamily::Marl);
        assert_eq!(material.tls_minimum, WorkerTlsMinimum::Tls13);
        assert_eq!(material.session_credential_ttl, Duration::from_secs(300));
        assert_eq!(material.rotation_owner, "platform-security");
        assert_eq!(
            material.pinned_platform_ca_reference,
            "file:///etc/flexcore/workers/platform-ca.pem"
        );
        assert_eq!(
            material.revocation_endpoint_reference,
            "file:///etc/flexcore/workers/revocation-endpoint"
        );
    }

    #[test]
    fn a_revoked_worker_identity_is_refused_at_admission_and_audited() {
        let store = FixedStore {
            identity: Some(identity("worker/gnn-2", ResearchWorkerFamily::Gnn)),
        };
        let config = valid_config();
        let mut register = WorkerIdentityRevocationRegister::new();
        let clock = FixedClock { now: 9_000 };
        let audit = MemoryWorkerIdentityAuditSink::new();
        register
            .revoke("worker/gnn-2", "compromised instance; key rotated", "platform-security", &clock, &audit)
            .expect("the revocation records");
        let err = verify_worker_admission(&store, &config, &register, &clock, &audit)
            .expect_err("a revoked identity is refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_REVOKED");
        // The audit trail: revocation recorded, then the refused admission.
        let records = audit.records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].kind, WorkerIdentityAuditKind::RevocationRecorded);
        assert_eq!(records[1].kind, WorkerIdentityAuditKind::AdmissionRefused);
        assert_eq!(records[1].timestamp_millis, 9_000);
    }

    #[test]
    fn an_unrevoked_worker_admission_is_accepted_and_audited() {
        let store = FixedStore {
            identity: Some(identity("worker/admm-1", ResearchWorkerFamily::AdmmDmpc)),
        };
        let mut register = WorkerIdentityRevocationRegister::new();
        let clock = FixedClock { now: 10_000 };
        let audit = MemoryWorkerIdentityAuditSink::new();
        let material =
            verify_worker_admission(&store, &valid_config(), &register, &clock, &audit)
                .expect("admitted");
        assert_eq!(material.worker_id, "worker/admm-1");
        let records = audit.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, WorkerIdentityAuditKind::AdmissionAccepted);
    }

    #[test]
    fn revocation_requires_reason_and_actor_and_refuses_duplicates() {
        let mut register = WorkerIdentityRevocationRegister::new();
        let clock = FixedClock { now: 11_000 };
        let audit = MemoryWorkerIdentityAuditSink::new();
        let err = register
            .revoke("worker/gnn-3", " ", "security", &clock, &audit)
            .expect_err("empty reason refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_REVOCATION_INVALID");
        let err = register
            .revoke("", "reason", "security", &clock, &audit)
            .expect_err("empty worker id refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_REVOCATION_INVALID");
        register
            .revoke("worker/gnn-3", "instance compromised", "platform-security", &clock, &audit)
            .expect("records");
        let err = register
            .revoke("worker/gnn-3", "again", "platform-security", &clock, &audit)
            .expect_err("duplicate refused");
        assert_eq!(err.name(), "WORKER_IDENTITY_REVOCATION_ALREADY_RECORDED");
        assert_eq!(register.revocations_in_append_order().len(), 1);
    }

    #[test]
    fn secret_declarations_accept_scheme_references_and_refuse_inline_material() {
        let declarations = serde_json::json!([
            { "name": "dataset-access", "reference": "secret-store://research/dataset-credentials" },
            { "name": "model-store-access", "reference": "env:MODEL_STORE_TOKEN_LOCATION" }
        ]);
        let parsed = parse_secret_declarations(&declarations).expect("parses");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "dataset-access");
        assert_eq!(
            parsed[0].reference,
            "secret-store://research/dataset-credentials"
        );

        // A scheme-less reference is LIKELY INLINE SECRET MATERIAL: refused,
        // and the VALUE never echoes back.
        let inline = serde_json::json!([
            { "name": "dataset-access", "reference": "hunter2-super-secret" }
        ]);
        let err = parse_secret_declarations(&inline).expect_err("inline secret refused");
        assert_eq!(err.name(), "WORKER_SECRET_REFERENCE_INVALID");
        let text = format!("{err}");
        assert!(
            !text.contains("hunter2"),
            "a rejected secret-reference VALUE is never echoed back (it may be a secret)"
        );

        // An unknown credential-need name is refused (closed vocabulary).
        let unknown = serde_json::json!([
            { "name": "my-database", "reference": "env:LOCATION" }
        ]);
        let err = parse_secret_declarations(&unknown).expect_err("unknown need refused");
        assert_eq!(err.name(), "WORKER_SECRET_REFERENCE_INVALID");

        // A duplicate name is refused.
        let duplicate = serde_json::json!([
            { "name": "dataset-access", "reference": "env:A" },
            { "name": "dataset-access", "reference": "env:B" }
        ]);
        let err = parse_secret_declarations(&duplicate).expect_err("duplicate name refused");
        assert_eq!(err.name(), "WORKER_SECRET_NAME_DUPLICATE");

        // An unknown key inside the secret object is refused (the closed
        // key set is what makes a secret VALUE structurally impossible).
        let smuggled = serde_json::json!([
            { "name": "dataset-access", "reference": "env:A", "value": "hunter2" }
        ]);
        let err = parse_secret_declarations(&smuggled).expect_err("smuggled value key refused");
        assert_eq!(err.name(), "WORKER_SECRET_DECLARATION_INVALID");
    }

    #[test]
    fn a_literal_secret_config_is_refused_at_any_depth_and_audited() {
        let clock = FixedClock { now: 12_000 };
        let audit = MemoryWorkerIdentityAuditSink::new();
        // A literal secret at the top level...
        let config = serde_json::json!({
            "worker": "gnn",
            "password": "hunter2",
            "horizon_steps": 4
        });
        let err = scan_worker_config_audited(&config, &clock, &audit)
            .expect_err("top-level forbidden key refused");
        assert_eq!(err.name(), "WORKER_CONFIG_SECRET_KEY_REFUSED");
        // ...and nested at depth.
        let nested = serde_json::json!({
            "worker": "gnn",
            "storage": { "options": [ { "api_key": "AKIA..." } ] }
        });
        let err = scan_worker_config(&nested).expect_err("nested forbidden key refused");
        assert_eq!(err.name(), "WORKER_CONFIG_SECRET_KEY_REFUSED");
        // Hyphen/underscore/case folding catches the same key in other shapes.
        let folded = serde_json::json!({ "db-Password": "hunter2" });
        let err = scan_worker_config(&folded).expect_err("folded key refused");
        assert_eq!(err.name(), "WORKER_CONFIG_SECRET_KEY_REFUSED");
        // The audit detail carries the KEY NAME — never the config VALUE.
        let records = audit.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, WorkerIdentityAuditKind::SecretConfigRefused);
        assert!(records[0].detail.contains("password"));
        assert!(!records[0].detail.contains("hunter2"));
        // The error text names the key, never the value.
        assert!(format!("{err}").contains("db-Password"));
        assert!(!format!("{err}").contains("hunter2"));
    }

    #[test]
    fn an_innocuous_config_passes_the_scan_and_the_scan_residual_is_recorded() {
        let clock = FixedClock { now: 13_000 };
        let audit = MemoryWorkerIdentityAuditSink::new();
        // The sanctioned declaration container (`secrets`) is EXEMPT from
        // the forbidden-key check (the kernel manifest declares the same
        // key), but its children are still scanned — and the declarations
        // themselves are validated by `parse_secret_declarations`.
        let config = serde_json::json!({
            "worker": "gnn",
            "topology_scope": "held-out-feeder",
            "secrets": [
                { "name": "dataset-access", "reference": "secret-store://research/dataset-credentials" }
            ]
        });
        scan_worker_config_audited(&config, &clock, &audit).expect("sanctioned config passes");
        let records = audit.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, WorkerIdentityAuditKind::SecretConfigAccepted);
        // But a forbidden key INSIDE the sanctioned container is still
        // refused (the exemption opens no value channel).
        let nested_forbidden = serde_json::json!({
            "secrets": [ { "name": "dataset-access", "token": "hunter2" } ]
        });
        let err = scan_worker_config(&nested_forbidden)
            .expect_err("a forbidden key inside the sanctioned container is refused");
        assert_eq!(err.name(), "WORKER_CONFIG_SECRET_KEY_REFUSED");
        // HONEST RESIDUAL pinned: a secret under an innocuous key name is
        // NOT detectable by the scan — the statement of that residual is a
        // recorded constant, and this test asserts the scan accepts the
        // shape it cannot judge.
        let undetectable = serde_json::json!({ "notes": "the password is hunter2" });
        scan_worker_config(&undetectable).expect("the scan cannot judge innocuous keys");
        assert!(WORKER_IDENTITY_STATEMENT.contains("REFERENCES"));
    }

    #[test]
    fn a_non_object_config_is_a_structured_refusal() {
        let err = scan_worker_config(&serde_json::json!([1, 2, 3]))
            .expect_err("array config refused");
        assert_eq!(err.name(), "WORKER_CONFIG_STRUCTURE_INVALID");
    }

    #[test]
    fn the_error_name_table_equals_the_enum_names() {
        let from_table: std::collections::BTreeSet<&'static str> =
            WORKER_IDENTITY_ERROR_NAMES.into_iter().collect();
        assert_eq!(from_table, error_names_from_enum());
        assert_eq!(WORKER_IDENTITY_ERROR_NAMES.len(), from_table.len());
    }
}
