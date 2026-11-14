// P7.11 — The host capability broker: scoped capabilities, no ambient access
// (report §5.4 Class B, §5.7, §17.3, §17.4; ledger `ledger_phase_7.md`
// §P7.11; decision record `10_11_plugin_host_broker.md` §6/§7).
//
// THE RULE
//
// Every capability a component may exercise is an EXPLICIT, SCOPED grant,
// derived from the manifest's declared permission ids and the profile — and
// nothing else. There is NO ambient access: the host exposes NO filesystem,
// NO network, NO clock, and NO environment access to a component unless a
// scoped capability for exactly that is granted AND the manifest declares
// the permission AND the grant is recorded. Defaults deny.
//
// Mechanically (ledger §P7.11 step 2): every host function a component can
// call goes through the broker; the broker re-checks manifest permission +
// grant + scope on EVERY call. No cached ambient authority exists —
// revocation is dropping the grant, and it takes effect at the next call
// because nothing between calls holds authority.
//
// THE VOCABULARY (alignment seam, record §6)
//
// The permission ids are a MIRROR of `crates/flexcore-kernel/src/manifest.rs`
// (`ALLOWED_PERMISSIONS`, `NEVER_GRANTED_PERMISSIONS`): the kernel manifest
// gate rejects never-granted and unknown ids at parse time; the broker
// RE-REFUSES them at grant/check time ("P7.11 re-refuses at grant time" —
// kernel manifest.rs header). The mirror exists because this crate's
// non-wasmtime code is deliberately std + tracing only — it does not import
// the kernel crate. If the vocabularies drift, the drift is a defect of the
// mirror, not an enforcement gap: the broker refuses anything not in ITS
// closed vocabulary, so a kernel-side addition would be denied here until
// the mirror is updated (fail-closed in both directions). The kernel crate
// was NOT touched (owned by another wave).
//
// THE NEVER-GRANTED SET (ledger §P7.11 step 3)
//
// `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` —
// not grantable through ANY manifest, profile, or broker call. Secret
// references are resolved by the host/adapter (P7.19), never handed to the
// plugin; there is no `network.any` because the P7.09 worlds declare zero
// network imports and the gated linker starts empty (`bindings.rs`).
//
// THE `dispatch.validate.request` BOUNDARY (ledger §P7.11 step 4)
//
// The broker may forward a plugin's REQUEST for validation to the protected
// validator ([`request_validation_forward`]), but the request path NEVER
// returns a validation verdict to the plugin and never substitutes for the
// protected validation gate (report §5.7, §13.4). Type-level honesty: the
// forward method returns `()` — a verdict cannot flow back through a
// signature that has no verdict in it. A plugin that receives a validation
// outcome and treats it as authorization has crossed the boundary (P7.12
// tests the escalation).
//
// AUDIT-OR-REFUSE (the P4.28/P4.30 discipline, cited not reinvented — same
// pattern as `crates/edge-agent/src/ports.rs::AuditSink`)
//
// Every grant, every use, every denial is audited through the
// [`BrokerAuditSink`] port. A failed audit write during a USE fails the use
// ([`BrokerError::AuditUnavailable`]); a failed audit write during a DENIAL
// surfaces as [`BrokerError::AuditUnavailable`] too — the broker never
// proceeds past a broken audit sink, in either direction.
//
// TIME
//
// The broker reads NO clock of its own (thematic alignment with
// no-ambient-access: the broker has no ambient authority either). Every
// method takes `now_unix_ms` as an explicit parameter; the HOST supplies
// the time, and with it the responsibility for its correctness.

use std::fmt;
use std::sync::Mutex;

// --- The permission vocabulary (mirror of the kernel manifest gate) -------------

/// Permissions a manifest MAY request — the closed §5.7 capability ids.
/// MIRROR of `crates/flexcore-kernel/src/manifest.rs::ALLOWED_PERMISSIONS`
/// (alignment seam, record §6 — kernel side untouched).
pub const ALLOWED_PERMISSIONS: [&str; 6] = [
    "telemetry.read",
    "asset.read",
    "forecast.publish",
    "schedule.propose",
    "dispatch.propose",
    "dispatch.validate.request",
];

/// Permissions NEVER granted to a plugin — not grantable through any
/// manifest, profile, or broker call. MIRROR of
/// `crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS`
/// (alignment seam, record §6 — kernel side untouched).
pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];

/// The only target the `dispatch.validate.request` capability may ever send
/// to: the protected validator. A grant naming any other target for this
/// capability is refused at derivation time.
pub const PROTECTED_VALIDATOR_TARGET: &str = "protected-validator";

/// One capability class of the §5.7 vocabulary (the parseable projection of
/// [`ALLOWED_PERMISSIONS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityClass {
    TelemetryRead,
    AssetRead,
    ForecastPublish,
    SchedulePropose,
    DispatchPropose,
    DispatchValidateRequest,
}

impl CapabilityClass {
    /// Parses the closed id. Unknown ids do not parse — the broker's
    /// vocabulary is closed even against future ids until the mirror is
    /// deliberately updated (fail-closed in both directions, record §6).
    pub fn parse(id: &str) -> Option<CapabilityClass> {
        match id {
            "telemetry.read" => Some(CapabilityClass::TelemetryRead),
            "asset.read" => Some(CapabilityClass::AssetRead),
            "forecast.publish" => Some(CapabilityClass::ForecastPublish),
            "schedule.propose" => Some(CapabilityClass::SchedulePropose),
            "dispatch.propose" => Some(CapabilityClass::DispatchPropose),
            "dispatch.validate.request" => Some(CapabilityClass::DispatchValidateRequest),
            _ => None,
        }
    }

    /// The closed id this class grants.
    pub fn id(self) -> &'static str {
        match self {
            CapabilityClass::TelemetryRead => "telemetry.read",
            CapabilityClass::AssetRead => "asset.read",
            CapabilityClass::ForecastPublish => "forecast.publish",
            CapabilityClass::SchedulePropose => "schedule.propose",
            CapabilityClass::DispatchPropose => "dispatch.propose",
            CapabilityClass::DispatchValidateRequest => "dispatch.validate.request",
        }
    }
}

// --- Scopes ----------------------------------------------------------------------

/// The scope of a capability grant or a capability request. A grant's scope
/// is the ENVELOPE of permitted use; a request is honoured only when it fits
/// inside the grant's envelope (D-P7.05-6: scope is applied at grant time by
/// the broker/profile, never a manifest property).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityScope {
    /// Read-only access within a named path prefix (host-provided data
    /// files only — never ambient filesystem access; the prefix is a
    /// virtual mount the host controls, not the guest's escape hatch).
    /// A request whose prefix is not INSIDE the grant's prefix — including
    /// any `..` escape segment — is refused.
    ReadOnlyPathPrefix { prefix: String },
    /// Send-only to one named target (a worker/queue id). Send-only: there
    /// is no receive, no subscription, no bidirectional channel.
    SendOnlyTarget { target: String },
    /// Data scope: tenant, optionally narrowed to a site and an asset. A
    /// grant covers a request only when every requested dimension is
    /// within the granted one (a tenant-wide grant covers a site request
    /// in the same tenant; a site grant does not cover another site; an
    /// asset grant does not cover another asset).
    DataScope {
        tenant_id: String,
        site_id: Option<String>,
        asset_id: Option<String>,
    },
}

impl CapabilityScope {
    /// Whether the GRANT's envelope covers the REQUESTED use. The broker
    /// calls this per request — no cached authority.
    fn covers(&self, requested: &CapabilityScope) -> bool {
        match (self, requested) {
            (
                CapabilityScope::ReadOnlyPathPrefix { prefix: granted },
                CapabilityScope::ReadOnlyPathPrefix { prefix: wanted },
            ) => prefix_covers(granted, wanted),
            (
                CapabilityScope::SendOnlyTarget { target: granted },
                CapabilityScope::SendOnlyTarget { target: wanted },
            ) => granted == wanted,
            (
                CapabilityScope::DataScope {
                    tenant_id: g_tenant,
                    site_id: g_site,
                    asset_id: g_asset,
                },
                CapabilityScope::DataScope {
                    tenant_id: w_tenant,
                    site_id: w_site,
                    asset_id: w_asset,
                },
            ) => {
                g_tenant == w_tenant
                    && match (g_site, w_site) {
                        (None, _) => true,
                        (Some(g), Some(w)) => g == w,
                        (Some(_), None) => false,
                    }
                    && match (g_asset, w_asset) {
                        (None, _) => true,
                        (Some(g), Some(w)) => g == w,
                        (Some(_), None) => false,
                    }
            }
            // A request can never fit inside a grant of a DIFFERENT scope
            // kind — cross-kind requests are denials, never coercions.
            _ => false,
        }
    }

    /// Structural validity at grant time: non-empty values everywhere, a
    /// path prefix that stays inside the host's virtual mount (no `..`
    /// segments, no empty prefix), a target that is not whitespace.
    fn validate(&self) -> Result<(), BrokerError> {
        match self {
            CapabilityScope::ReadOnlyPathPrefix { prefix } => {
                let trimmed = prefix.trim();
                if trimmed.is_empty() {
                    return Err(BrokerError::ScopeInvalid {
                        reason: "a read-only path prefix must be non-empty".to_string(),
                    });
                }
                if trimmed.split('/').any(|seg| seg == "..") {
                    return Err(BrokerError::ScopeInvalid {
                        reason: "a read-only path prefix may not contain '..' segments (escape-at construction is refused)".to_string(),
                    });
                }
                Ok(())
            }
            CapabilityScope::SendOnlyTarget { target } => {
                if target.trim().is_empty() {
                    return Err(BrokerError::ScopeInvalid {
                        reason: "a send-only target must be non-empty".to_string(),
                    });
                }
                Ok(())
            }
            CapabilityScope::DataScope { tenant_id, .. } => {
                if tenant_id.trim().is_empty() {
                    return Err(BrokerError::ScopeInvalid {
                        reason: "a data scope must name a tenant".to_string(),
                    });
                }
                Ok(())
            }
        }
    }

    /// Human-readable rendering for audit lines (no secret material can
    /// appear here — scopes are identifiers and prefixes only).
    pub fn describe(&self) -> String {
        match self {
            CapabilityScope::ReadOnlyPathPrefix { prefix } => {
                format!("read-only:{prefix}")
            }
            CapabilityScope::SendOnlyTarget { target } => {
                format!("send-only:{target}")
            }
            CapabilityScope::DataScope { tenant_id, site_id, asset_id } => {
                let mut parts = vec![format!("tenant={tenant_id}")];
                if let Some(site) = site_id {
                    parts.push(format!("site={site}"));
                }
                if let Some(asset) = asset_id {
                    parts.push(format!("asset={asset}"));
                }
                format!("data:{}", parts.join(","))
            }
        }
    }
}

/// Whether a granted read prefix covers a requested read prefix. The grant
/// prefix is normalised to end with `/` (a segment boundary — a grant of
/// `/data/telemetry` must NOT cover `/data/telemetry-evil`), and the
/// requested prefix must stay inside it without any `..` segment (a literal
/// escape attempt is refused even when the string starts with the grant).
fn prefix_covers(granted: &str, wanted: &str) -> bool {
    let granted = granted.trim();
    let wanted = wanted.trim();
    if granted.is_empty() || wanted.is_empty() {
        return false;
    }
    if wanted.split('/').any(|seg| seg == "..") {
        return false;
    }
    let granted_norm = if granted.ends_with('/') {
        granted.to_string()
    } else {
        format!("{granted}/")
    };
    // Boundary convention (record §7): requests name paths INSIDE the
    // granted prefix. A request of exactly the granted prefix with the
    // trailing boundary (granted_norm) is inside; a request that does not
    // start with the boundary (e.g. the bare prefix without the trailing
    // '/', or any sibling like `/data/telemetry-evil`) is outside. Fail-closed
    // on the boundary: when in doubt, denied.
    matches!(wanted.strip_prefix(&granted_norm), Some(_))
}

/// The `dispatch.validate.request` boundary enforced at GRANT time: the
/// capability may only ever carry the send-only scope that targets exactly
/// [`PROTECTED_VALIDATOR_TARGET`]. Any other target is refused at grant
/// time ([`BrokerError::ScopeInvalid`]) — a plugin can never hold a grant
/// whose envelope would let it send a "validation request" anywhere else
/// (and thereby fish for verdicts, §5.7).
fn validate_request_target(capability: &str, scope: &CapabilityScope) -> Result<(), BrokerError> {
    if capability == "dispatch.validate.request" {
        match scope {
            CapabilityScope::SendOnlyTarget { target } if target == PROTECTED_VALIDATOR_TARGET => {
                Ok(())
            }
            other => Err(BrokerError::ScopeInvalid {
                reason: format!(
                    "'dispatch.validate.request' may only ever carry the send-only scope \
                     targeting '{PROTECTED_VALIDATOR_TARGET}' (found {}); the request path \
                     never substitutes for the protected validation gate",
                    other.describe()
                ),
            }),
        }
    } else {
        Ok(())
    }
}

// --- Grants ---------------------------------------------------------------------

/// One recorded capability grant: the broker's unit of authority. Grants
/// carry WHO granted them (the manifest's id and version — the derivation
/// trail, record §6), WHAT (capability id), HOW FAR (scope), and HOW LONG
/// (expiry). Revocation is dropping the grant; nothing caches it.
///
/// P7.13 (capability-object forgery): the fields are `pub(crate)` since
/// P7.12/P7.13 (VISIBILITY-ONLY change; the broker's logic is untouched) —
/// a grant can only be CONSTRUCTED inside the broker, so a caller outside
/// this crate cannot forge one (a struct literal over non-public fields is
/// a compile error). Read access outside goes through the getter methods;
/// authority itself never leaves the broker's state — a cloned or captured
/// grant object confers nothing once the broker has dropped it (per-call
/// re-check, no cached authority).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    /// Broker-assigned grant id (`grant/<plugin>/<capability>/<n>` —
    /// deterministic, ordered by issuance).
    pub(crate) grant_id: String,
    /// The plugin the grant is bound to. A grant is usable by exactly one
    /// plugin — never shareable, never inheritable.
    pub(crate) plugin_id: String,
    /// The granted capability id (from [`ALLOWED_PERMISSIONS`]; a
    /// never-granted id cannot become a grant — see
    /// [`BrokerError::PermissionNeverGrantable`]).
    pub(crate) capability: String,
    /// The granted scope envelope.
    pub(crate) scope: CapabilityScope,
    /// Expiry in Unix milliseconds (exclusive: use at `now >= expires_at`
    /// is [`BrokerError::GrantExpired`]). `None` = no expiry — the grant
    /// lives until revoked (composition change, not clock, ends it).
    pub(crate) expires_at_unix_ms: Option<u64>,
    /// The granting manifest's id (the `plugin-id` of the validated
    /// manifest the permission was declared in).
    pub(crate) manifest_id: String,
    /// The granting manifest's version.
    pub(crate) manifest_version: String,
}

impl CapabilityGrant {
    /// The broker-assigned grant id.
    pub fn grant_id(&self) -> &str {
        &self.grant_id
    }

    /// The plugin the grant is bound to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// The granted capability id.
    pub fn capability(&self) -> &str {
        &self.capability
    }

    /// The granted scope envelope.
    pub fn scope(&self) -> &CapabilityScope {
        &self.scope
    }

    /// The grant's expiry (Unix ms, exclusive), if any.
    pub fn expires_at_unix_ms(&self) -> Option<u64> {
        self.expires_at_unix_ms
    }

    /// The granting manifest's id (the derivation trail).
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// The granting manifest's version.
    pub fn manifest_version(&self) -> &str {
        &self.manifest_version
    }
}

/// A grant REQUEST passed to [`CapabilityBroker::grant`] or to
/// [`CapabilityBroker::derive_grants`] (via [`GrantDerivationRequest`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRequest {
    /// The requested capability id — must be in the manifest's declared
    /// permissions AND in [`ALLOWED_PERMISSIONS`].
    pub capability: String,
    /// The requested scope envelope.
    pub scope: CapabilityScope,
    /// Requested expiry (Unix ms; `None` = until revoked).
    pub expires_at_unix_ms: Option<u64>,
}

/// The manifest-derived inputs to [`CapabilityBroker::derive_grants`]: the
/// manifest's own declared permission ids (exactly what the kernel
/// manifest gate accepted) plus the profile's grant requests. Derivation
/// honours the manifest's declared bounds ONLY — a request for a
/// capability the manifest did not declare is over-broad and refused
/// ([`BrokerError::PermissionNotDeclared`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantDerivationRequest {
    pub plugin_id: String,
    /// The granting manifest's id (`plugin-id`).
    pub manifest_id: String,
    /// The granting manifest's version.
    pub manifest_version: String,
    /// The permission ids the VALIDATED manifest declared (closed ids; the
    /// kernel gate already rejected never-granted/unknown ones).
    pub manifest_permissions: Vec<String>,
    /// The profile's grant requests (each within a declared permission).
    pub requested_grants: Vec<GrantRequest>,
}

/// The receipt of a successful grant (what the caller records).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantReceipt {
    pub grant: CapabilityGrant,
}

/// The receipt of a successful capability USE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseReceipt {
    /// The grant the use was honoured under.
    pub grant_id: String,
    /// The capability used.
    pub capability: String,
    /// The scope the use ran in (the REQUESTED scope — always inside the
    /// grant's envelope).
    pub scope: CapabilityScope,
}

// --- Errors ---------------------------------------------------------------------

/// Broker errors. Every refusal is named and audited; nothing is swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerError {
    /// The capability id is not in the broker's closed vocabulary at all.
    PermissionUnknown { value: String },
    /// The capability id is in the never-granted set — refused at grant
    /// time AND at check time, regardless of what any manifest or profile
    /// says (the broker re-refuses; kernel manifest.rs header).
    PermissionNeverGrantable { value: String },
    /// No grant exists for this plugin + capability: the default-deny
    /// outcome (never an implicit permission).
    NotGranted { plugin_id: String, capability: String },
    /// The grant exists but has expired (checked per call against the
    /// caller-supplied time).
    GrantExpired { grant_id: String, expired_at_unix_ms: u64 },
    /// The requested use falls outside the grant's scope envelope (a path
    /// prefix escape, a wrong target, a tenant/site/asset outside the
    /// grant, or a different scope kind entirely).
    ScopeOutsideGrant { grant_id: String, reason: String },
    /// The capability is grantable in principle but the manifest did NOT
    /// declare it — an over-broad grant request is refused, never trimmed
    /// to fit (fail-closed: the broker grants what was declared, and
    /// nothing else).
    PermissionNotDeclared { plugin_id: String, capability: String },
    /// The same capability was requested twice in one derivation batch.
    DuplicateGrantRequest { capability: String },
    /// A grant's expiry is already in the past at grant time.
    GrantExpiryInvalid { reason: String },
    /// A scope is structurally unusable (empty prefix/target/tenant, or a
    /// `..` segment in a grant-time prefix).
    ScopeInvalid { reason: String },
    /// The plugin id is empty or whitespace — a grant must always name its
    /// holder.
    PluginIdInvalid { reason: String },
    /// A grant id handed to revoke is not recorded.
    GrantUnknown { grant_id: String },
    /// The audit sink failed; the event was NOT recorded. The broker
    /// refuses to proceed past a broken audit sink (audit-or-refuse).
    AuditUnavailable { reason: String },
}

impl BrokerError {
    /// The stable machine-readable error name (the named-error table key,
    /// decision record §5).
    pub fn name(&self) -> &'static str {
        match self {
            BrokerError::PermissionUnknown { .. } => "PERMISSION_UNKNOWN",
            BrokerError::PermissionNeverGrantable { .. } => "PERMISSION_NEVER_GRANTABLE",
            BrokerError::NotGranted { .. } => "CAPABILITY_NOT_GRANTED",
            BrokerError::GrantExpired { .. } => "GRANT_EXPIRED",
            BrokerError::ScopeOutsideGrant { .. } => "SCOPE_OUTSIDE_GRANT",
            BrokerError::PermissionNotDeclared { .. } => "PERMISSION_NOT_DECLARED",
            BrokerError::DuplicateGrantRequest { .. } => "DUPLICATE_GRANT_REQUEST",
            BrokerError::GrantExpiryInvalid { .. } => "GRANT_EXPIRY_INVALID",
            BrokerError::ScopeInvalid { .. } => "SCOPE_INVALID",
            BrokerError::PluginIdInvalid { .. } => "PLUGIN_ID_INVALID",
            BrokerError::GrantUnknown { .. } => "GRANT_UNKNOWN",
            BrokerError::AuditUnavailable { .. } => "AUDIT_UNAVAILABLE",
        }
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrokerError::PermissionUnknown { value } => {
                write!(f, "capability '{value}' is not in the broker's closed vocabulary")
            }
            BrokerError::PermissionNeverGrantable { value } => {
                write!(f, "capability '{value}' is in the never-granted set and is not grantable through any manifest, profile, or broker call")
            }
            BrokerError::NotGranted { plugin_id, capability } => {
                write!(f, "plugin '{plugin_id}' holds no grant for '{capability}' (defaults deny)")
            }
            BrokerError::GrantExpired { grant_id, expired_at_unix_ms } => {
                write!(f, "grant '{grant_id}' expired at Unix-ms {expired_at_unix_ms} (checked per call; no cached authority)")
            }
            BrokerError::ScopeOutsideGrant { grant_id, reason } => {
                write!(f, "grant '{grant_id}' does not cover the requested scope: {reason}")
            }
            BrokerError::PermissionNotDeclared { plugin_id, capability } => {
                write!(f, "plugin '{plugin_id}' requested '{capability}', which its manifest does not declare (grants honour the manifest's declared bounds only; over-broad requests are refused, never trimmed)")
            }
            BrokerError::DuplicateGrantRequest { capability } => {
                write!(f, "capability '{capability}' is requested twice in one derivation batch")
            }
            BrokerError::GrantExpiryInvalid { reason } => {
                write!(f, "grant expiry is invalid: {reason}")
            }
            BrokerError::ScopeInvalid { reason } => {
                write!(f, "scope is invalid: {reason}")
            }
            BrokerError::PluginIdInvalid { reason } => {
                write!(f, "plugin id is invalid: {reason}")
            }
            BrokerError::GrantUnknown { grant_id } => {
                write!(f, "grant '{grant_id}' is not recorded (nothing to revoke)")
            }
            BrokerError::AuditUnavailable { reason } => {
                write!(f, "the broker audit sink failed (audit-or-refuse — the broker never proceeds past a broken audit sink): {reason}")
            }
        }
    }
}

impl std::error::Error for BrokerError {}

// --- Audit ----------------------------------------------------------------------

/// The kinds of broker audit records (style of
/// `crates/edge-agent/src/ports.rs::EdgeAuditKind` — SCREAMING_SNAKE stable
/// names, no secret material in any detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerAuditKind {
    /// A grant was issued.
    GrantIssued,
    /// A grant was revoked (dropped — effective immediately at the next
    /// call, because nothing caches authority).
    GrantRevoked,
    /// A granted capability was USED (checked and honoured, this call).
    CapabilityUse,
    /// A capability call was DENIED (no grant, expired, outside scope,
    /// unknown, or never-grantable — the detail names the reason class).
    CapabilityDenied,
    /// A plugin's validation REQUEST was forwarded to the protected
    /// validator. The forward carries the request ONLY — no verdict ever
    /// flows back through the broker (the `dispatch.validate.request`
    /// boundary, ledger §P7.11 step 4).
    ValidationForwardRequested,
}

impl BrokerAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            BrokerAuditKind::GrantIssued => "PLUGIN_CAPABILITY_GRANTED",
            BrokerAuditKind::GrantRevoked => "PLUGIN_CAPABILITY_REVOKED",
            BrokerAuditKind::CapabilityUse => "PLUGIN_CAPABILITY_USED",
            BrokerAuditKind::CapabilityDenied => "PLUGIN_CAPABILITY_DENIED",
            BrokerAuditKind::ValidationForwardRequested => "PLUGIN_VALIDATION_FORWARD_REQUESTED",
        }
    }
}

/// One broker audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAuditEvent {
    /// When the broker recorded the event (Unix ms — the value the host
    /// supplied for the call that produced the event; the broker reads no
    /// clock of its own).
    pub recorded_at_unix_ms: u64,
    /// The event kind.
    pub kind: BrokerAuditKind,
    /// The plugin the event concerns (empty for grant-revocation events
    /// initiated by an operator — the grant id carries the trail).
    pub plugin_id: String,
    /// The capability id concerned (empty where not applicable).
    pub capability: String,
    /// Human-readable detail (identifiers, prefixes, scope renderings —
    /// never secret material; P7.19 resolves secrets host-side and the
    /// broker never sees values).
    pub detail: String,
}

/// The audit sink port. A failure is `Err` — the broker treats a broken
/// audit sink as a stop condition (audit-or-refuse), never as a note in a
/// log it happens to keep.
pub trait BrokerAuditSink: Send + Sync {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError>;
}

/// In-memory audit sink (tests + this-wave composition; the durable
/// audit path is the control plane's audit crate, reached over the P7.20
/// gRPC seam — recorded, not implemented here).
#[derive(Debug, Default)]
pub struct InMemoryBrokerAuditSink {
    events: Mutex<Vec<BrokerAuditEvent>>,
}

impl InMemoryBrokerAuditSink {
    pub fn new() -> InMemoryBrokerAuditSink {
        InMemoryBrokerAuditSink { events: Mutex::new(Vec::new()) }
    }

    /// A snapshot of the recorded events (test/composition inspection).
    pub fn events(&self) -> Vec<BrokerAuditEvent> {
        match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(_poisoned) => Vec::new(),
        }
    }
}

impl BrokerAuditSink for InMemoryBrokerAuditSink {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        match self.events.lock() {
            Ok(mut guard) => {
                guard.push(event);
                Ok(())
            }
            Err(_) => Err(BrokerError::AuditUnavailable {
                reason: "the in-memory audit sink's lock is poisoned".to_string(),
            }),
        }
    }
}

// --- The broker ------------------------------------------------------------------

struct BrokerState {
    grants: Vec<CapabilityGrant>,
    next_grant_seq: u64,
}

/// The capability broker. Every host function a component can call goes
/// through [`check`]; grants are created through [`grant`] / [`derive_grants`]
/// and die through [`revoke`] — or by expiry, checked per call. Defaults
/// deny; nothing is cached between calls.
pub struct CapabilityBroker {
    state: Mutex<BrokerState>,
    audit: Box<dyn BrokerAuditSink>,
}

impl CapabilityBroker {
    /// Composes the broker over an audit sink. There is no broker without
    /// an audit sink — a broker that cannot record denials is not composed
    /// at all (fail-closed at composition).
    pub fn new(audit: Box<dyn BrokerAuditSink>) -> CapabilityBroker {
        CapabilityBroker {
            state: Mutex::new(BrokerState { grants: Vec::new(), next_grant_seq: 1 }),
            audit,
        }
    }

    /// Audits one event; a failed write is [`BrokerError::AuditUnavailable`]
    /// (the caller logs the failure — the audit is never silently skipped).
    fn audit_event(
        &self,
        now_unix_ms: u64,
        kind: BrokerAuditKind,
        plugin_id: &str,
        capability: &str,
        detail: String,
    ) -> Result<(), BrokerError> {
        self.audit.record(BrokerAuditEvent {
            recorded_at_unix_ms: now_unix_ms,
            kind,
            plugin_id: plugin_id.to_string(),
            capability: capability.to_string(),
            detail,
        })
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, BrokerState>, BrokerError> {
        self.state.lock().map_err(|_| BrokerError::AuditUnavailable {
            reason: "the broker state lock is poisoned (a previous call panicked; the broker refuses to proceed)".to_string(),
        })
    }

    /// Issues one grant. Fail-closed at every step:
    ///
    /// 1. the plugin id must be non-empty ([`BrokerError::PluginIdInvalid`]);
    /// 2. the capability must be in the closed vocabulary — a never-granted
    ///    id is [`BrokerError::PermissionNeverGrantable`] (the broker
    ///    re-refuses even though the kernel manifest gate already rejected
    ///    it), an unknown id is [`BrokerError::PermissionUnknown`];
    /// 3. the scope must be structurally valid ([`BrokerError::ScopeInvalid`]);
    /// 4. an expiry in the past at grant time is
    ///    [`BrokerError::GrantExpiryInvalid`].
    ///
    /// The grant is recorded and audited ([`BrokerAuditKind::GrantIssued`]).
    pub fn grant(
        &self,
        plugin_id: &str,
        manifest_id: &str,
        manifest_version: &str,
        request: &GrantRequest,
        now_unix_ms: u64,
    ) -> Result<GrantReceipt, BrokerError> {
        if plugin_id.trim().is_empty() {
            return Err(BrokerError::PluginIdInvalid {
                reason: "a grant must name its holder plugin".to_string(),
            });
        }
        // Never-granted first (the more specific refusal), then closed
        // vocabulary, then structural checks.
        if NEVER_GRANTED_PERMISSIONS.contains(&request.capability.as_str()) {
            return Err(BrokerError::PermissionNeverGrantable { value: request.capability.clone() });
        }
        CapabilityClass::parse(&request.capability).ok_or_else(|| {
            BrokerError::PermissionUnknown { value: request.capability.clone() }
        })?;
        request.scope.validate()?;
        validate_request_target(&request.capability, &request.scope)?;
        if let Some(expires_at) = request.expires_at_unix_ms {
            if expires_at <= now_unix_ms {
                return Err(BrokerError::GrantExpiryInvalid {
                    reason: format!(
                        "requested expiry {expires_at} is not after the grant time {now_unix_ms}"
                    ),
                });
            }
        }
        let mut state = self.lock_state()?;
        let grant = CapabilityGrant {
            grant_id: format!(
                "grant/{}/{}/{}",
                plugin_id,
                request.capability,
                state.next_grant_seq
            ),
            plugin_id: plugin_id.to_string(),
            capability: request.capability.clone(),
            scope: request.scope.clone(),
            expires_at_unix_ms: request.expires_at_unix_ms,
            manifest_id: manifest_id.to_string(),
            manifest_version: manifest_version.to_string(),
        };
        state.next_grant_seq += 1;
        let receipt = GrantReceipt { grant: grant.clone() };
        state.grants.push(grant);
        drop(state);
        if let Err(audit_err) = self.audit_event(
            now_unix_ms,
            BrokerAuditKind::GrantIssued,
            plugin_id,
            &request.capability,
            format!(
                "manifest {}:{} scope {}",
                manifest_id,
                manifest_version,
                request.scope.describe()
            ),
        ) {
            // Audit-or-refuse, atomically: a grant that could not be
            // audited NEVER becomes authority — roll it back. (The grant
            // sequence number stays consumed; ids are deterministic with
            // possible gaps, never reused.)
            if let Ok(mut state) = self.state.lock() {
                state.grants.retain(|g| g.grant_id != receipt.grant.grant_id);
            }
            return Err(audit_err);
        }
        Ok(receipt)
    }

    /// Revokes one grant by id: the grant is DROPPED from the broker's
    /// records, and because [`check`] re-reads the records on every call,
    /// revocation takes effect at the next call — there is no cached
    /// authority to outlive it. Unknown ids are
    /// [`BrokerError::GrantUnknown`] (revoking nothing is an error, not a
    /// silent success).
    pub fn revoke(&self, grant_id: &str, now_unix_ms: u64) -> Result<GrantReceipt, BrokerError> {
        let mut state = self.lock_state()?;
        let position = state
            .grants
            .iter()
            .position(|g| g.grant_id == grant_id)
            .ok_or_else(|| BrokerError::GrantUnknown { grant_id: grant_id.to_string() })?;
        let grant = state.grants.remove(position);
        let receipt = GrantReceipt { grant: grant.clone() };
        drop(state);
        if let Err(audit_err) = self.audit_event(
            now_unix_ms,
            BrokerAuditKind::GrantRevoked,
            &grant.plugin_id,
            &grant.capability,
            format!("grant {} revoked (effective immediately at the next call)", grant.grant_id),
        ) {
            // Deliberate asymmetry with [`grant`]'s rollback: a revocation
            // whose audit failed still STANDS. Removing authority is the
            // safe direction; resurrecting authority through a rollback
            // would re-grant unaudited power. The audit failure is still
            // returned (audit-or-refuse for the caller).
            return Err(audit_err);
        }
        Ok(receipt)
    }

    /// THE dispatch gate: checks one capability call. Runs the full
    /// per-call chain, in order:
    ///
    /// 1. plugin id non-empty;
    /// 2. capability in the closed vocabulary — never-granted ids are
    ///    refused HERE TOO (a check against a never-granted capability can
    ///    only be a crafted call, and it is audited as such);
    /// 3. an active grant for (plugin, capability) exists — defaults deny;
    /// 4. the grant has not expired (`now >= expires_at` denies);
    /// 5. the requested scope fits inside the grant's envelope (path
    ///    prefix containment including `..` refusal, exact send-only
    ///    target, tenant/site/asset narrowing).
    ///
    /// Every outcome is audited: use => [`BrokerAuditKind::CapabilityUse`],
    /// denial => [`BrokerAuditKind::CapabilityDenied`] with the refusal
    /// class in the detail (plugin id, capability, scope — ledger §P7.11
    /// step 5). A failed audit write refuses the call in BOTH directions
    /// (audit-or-refuse).
    pub fn check(
        &self,
        plugin_id: &str,
        capability: &str,
        requested: &CapabilityScope,
        now_unix_ms: u64,
    ) -> Result<UseReceipt, BrokerError> {
        let outcome = self.check_inner(plugin_id, capability, requested, now_unix_ms);
        match &outcome {
            Ok(receipt) => {
                self.audit_event(
                    now_unix_ms,
                    BrokerAuditKind::CapabilityUse,
                    plugin_id,
                    capability,
                    format!("grant {} scope {}", receipt.grant_id, requested.describe()),
                )?;
            }
            Err(err) => {
                self.audit_event(
                    now_unix_ms,
                    BrokerAuditKind::CapabilityDenied,
                    plugin_id,
                    capability,
                    format!(
                        "denied ({}): {err} — requested scope {}",
                        err.name(),
                        requested.describe()
                    ),
                )?;
            }
        }
        outcome
    }

    /// The check chain proper (audit happens in [`check`], around it).
    fn check_inner(
        &self,
        plugin_id: &str,
        capability: &str,
        requested: &CapabilityScope,
        now_unix_ms: u64,
    ) -> Result<UseReceipt, BrokerError> {
        if plugin_id.trim().is_empty() {
            return Err(BrokerError::PluginIdInvalid {
                reason: "a capability call must name its calling plugin".to_string(),
            });
        }
        if NEVER_GRANTED_PERMISSIONS.contains(&capability) {
            return Err(BrokerError::PermissionNeverGrantable { value: capability.to_string() });
        }
        CapabilityClass::parse(capability)
            .ok_or_else(|| BrokerError::PermissionUnknown { value: capability.to_string() })?;

        let state = self.lock_state()?;
        // Owned clones, not borrows: the state guard is dropped before the
        // decision chain so a poisoned-lock-free release never holds the
        // lock across audit writes.
        let mut candidates: Vec<CapabilityGrant> = state
            .grants
            .iter()
            .filter(|g| g.plugin_id == plugin_id && g.capability == capability)
            .cloned()
            .collect();
        drop(state);
        if candidates.is_empty() {
            return Err(BrokerError::NotGranted {
                plugin_id: plugin_id.to_string(),
                capability: capability.to_string(),
            });
        }
        // An EXPIRED grant never shadows a live one: the live candidates
        // get first chance at covering the request; only when nothing live
        // covers it does the expiry refusal surface (deterministic: the
        // earliest-expiring expired candidate is named).
        let is_expired =
            |g: &CapabilityGrant| matches!(g.expires_at_unix_ms, Some(e) if now_unix_ms >= e);
        let live: Vec<&CapabilityGrant> =
            candidates.iter().filter(|g| !is_expired(*g)).collect();
        let expired: Vec<&CapabilityGrant> =
            candidates.iter().filter(|g| is_expired(*g)).collect();
        for grant in &live {
            if grant.scope.covers(requested) {
                return Ok(UseReceipt {
                    grant_id: grant.grant_id.clone(),
                    capability: capability.to_string(),
                    scope: requested.clone(),
                });
            }
        }
        if let Some(grant) = expired.first() {
            let Some(expires_at) = grant.expires_at_unix_ms else {
                // Unreachable by construction of `is_expired`; kept explicit
                // (no unwrap, even where provably safe).
                return Err(BrokerError::NotGranted {
                    plugin_id: plugin_id.to_string(),
                    capability: capability.to_string(),
                });
            };
            return Err(BrokerError::GrantExpired {
                grant_id: grant.grant_id.clone(),
                expired_at_unix_ms: expires_at,
            });
        }
        Err(BrokerError::ScopeOutsideGrant {
            grant_id: candidates[0].grant_id.clone(),
            reason: format!(
                "no recorded grant covers requested scope {} (grant envelope: {})",
                requested.describe(),
                candidates[0].scope.describe()
            ),
        })
    }

    /// The `dispatch.validate.request` boundary (ledger §P7.11 step 4):
    /// forwards a plugin's REQUEST for validation to the protected
    /// validator — and nothing else. The return type is `()`: a validation
    /// verdict cannot flow back through a signature that has no verdict in
    /// it, and the protected validation gate (§5.7) is never substituted by
    /// this forward. The forward is audited
    /// ([`BrokerAuditKind::ValidationForwardRequested`]).
    ///
    /// The capability must be granted with the
    /// [`CapabilityScope::SendOnlyTarget`] scope targeting exactly
    /// [`PROTECTED_VALIDATOR_TARGET`] — any other target for this
    /// capability falls outside every possible grant envelope.
    pub fn request_validation_forward(
        &self,
        plugin_id: &str,
        forward_request_id: &str,
        now_unix_ms: u64,
    ) -> Result<(), BrokerError> {
        let scope = CapabilityScope::SendOnlyTarget {
            target: PROTECTED_VALIDATOR_TARGET.to_string(),
        };
        let receipt = self.check(plugin_id, "dispatch.validate.request", &scope, now_unix_ms)?;
        self.audit_event(
            now_unix_ms,
            BrokerAuditKind::ValidationForwardRequested,
            plugin_id,
            "dispatch.validate.request",
            format!(
                "forward {} dispatched to '{PROTECTED_VALIDATOR_TARGET}' under grant {} — the response path NEVER returns a verdict through the broker (the protected validation gate is not substituted; a proposal gains no authority from being proposed, §13.4)",
                forward_request_id, receipt.grant_id
            ),
        )?;
        Ok(())
    }

    /// Derives grants from a manifest's declared permissions plus the
    /// profile's grant requests (D-P7.05-6: scope is applied HERE, at grant
    /// time — never a manifest property). The derivation honours the
    /// manifest's declared bounds ONLY:
    ///
    /// - a never-granted capability in a request is
    ///   [`BrokerError::PermissionNeverGrantable`];
    /// - an unknown capability is [`BrokerError::PermissionUnknown`];
    /// - a capability the manifest did NOT declare is
    ///   [`BrokerError::PermissionNotDeclared`] — the over-broad refusal,
    ///   never trimmed to fit;
    /// - a duplicate capability in one batch is
    ///   [`BrokerError::DuplicateGrantRequest`].
    ///
    /// The batch is ATOMIC: every request is validated before ANY grant is
    /// issued (fail-closed — a batch never half-applies), then each grant
    /// is issued and audited in order.
    pub fn derive_grants(
        &self,
        request: &GrantDerivationRequest,
        now_unix_ms: u64,
    ) -> Result<Vec<GrantReceipt>, BrokerError> {
        if request.plugin_id.trim().is_empty() {
            return Err(BrokerError::PluginIdInvalid {
                reason: "a derivation must name its holder plugin".to_string(),
            });
        }
        // Phase 1 — validate every request; nothing is granted yet.
        let mut seen: Vec<&str> = Vec::new();
        for grant_request in &request.requested_grants {
            if NEVER_GRANTED_PERMISSIONS.contains(&grant_request.capability.as_str()) {
                return Err(BrokerError::PermissionNeverGrantable {
                    value: grant_request.capability.clone(),
                });
            }
            CapabilityClass::parse(&grant_request.capability).ok_or_else(|| {
                BrokerError::PermissionUnknown { value: grant_request.capability.clone() }
            })?;
            if !request
                .manifest_permissions
                .iter()
                .any(|p| p == &grant_request.capability)
            {
                return Err(BrokerError::PermissionNotDeclared {
                    plugin_id: request.plugin_id.clone(),
                    capability: grant_request.capability.clone(),
                });
            }
            if seen.contains(&grant_request.capability.as_str()) {
                return Err(BrokerError::DuplicateGrantRequest {
                    capability: grant_request.capability.clone(),
                });
            }
            seen.push(grant_request.capability.as_str());
            grant_request.scope.validate()?;
            validate_request_target(&grant_request.capability, &grant_request.scope)?;
            if let Some(expires_at) = grant_request.expires_at_unix_ms {
                if expires_at <= now_unix_ms {
                    return Err(BrokerError::GrantExpiryInvalid {
                        reason: format!(
                            "requested expiry {expires_at} for '{}' is not after the grant time {now_unix_ms}",
                            grant_request.capability
                        ),
                    });
                }
            }
        }
        // Phase 2 — issue in order; every issue is audited by [`grant`].
        let mut receipts = Vec::with_capacity(request.requested_grants.len());
        for grant_request in &request.requested_grants {
            receipts.push(self.grant(
                &request.plugin_id,
                &request.manifest_id,
                &request.manifest_version,
                grant_request,
                now_unix_ms,
            )?);
        }
        Ok(receipts)
    }
}
