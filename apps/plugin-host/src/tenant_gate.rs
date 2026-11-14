// P7.39 — The host-side tenant-scope cross-check: the data-level half of
// the plugin path's tenant isolation (report §17.4 cross-tenant leakage,
// §5.7, §16.2; ledger `ledger_phase_7.md` P7.39; decision record
// `ledger/phase-07-plugin-sdk/37_38_39_invariants_revocation_tenancy.md`).
//
// THE RULE
//
// The P7.11 broker narrows capability scopes per call (the GRANT
// envelope: what the profile permitted, derived from the manifest). The
// P7.39 rule adds the HOST-STAMPED ENVELOPE: the tenant scope derived
// from the platform-authenticated context travels with the request, and
// the enforcement point cross-checks the requested data scope against it
// — a request is admitted only when it fits inside BOTH the broker grant
// AND the host-stamped envelope. Mismatch is the named refusal
// [`TenantGateError::TenantScopeMismatch`] (stable name
// `TENANT_SCOPE_MISMATCH`), refused and audited (§16.2: "Audit
// cross-tenant access denials"). A misconfigured or over-broad GRANT can
// never widen the host-stamped envelope — defence in depth, two
// independent narrowing layers.
//
// THE MIRROR (the recorded alignment seam, the broker.rs pattern)
//
// This crate depends on NO other flexcore crate (crate lib.rs header;
// `tools/check_dependency_rule.py` classifies it COMPOSITION), so the
// kernel's `TenantScope` type is mirrored here as [`HostTenantScope`] —
// the same fields, the same authenticated-construction-only rule, the
// same narrowing predicate, values read 2026-11-13 from
// `crates/flexcore-kernel/src/tenant_scope.rs` (kernel side untouched).
// The mirror is closed: the gate refuses anything its own type does not
// express (fail-closed in both directions). If the two vocabularies
// drift, the drift is a defect of the mirror, not an enforcement gap.
//
// THE RLS RELATIONSHIP (§16.2 / [R25]; ADR-007 — recorded, unchanged):
// plugins never touch tables directly; data access flows through the
// platform's tenant-scoped services, where application authorization and
// PostgreSQL RLS remain the platform's defence in depth. The SDK adds
// the envelope-level guarantee only. See
// `crates/flexcore-kernel/src/tenant_scope.rs` for the full statement.
//
// THE WORKER-TENANCY MODEL (ledger P7.39 step 3 — recorded): v0.5.0 runs
// ONE out-of-process gRPC worker per plugin instance, launched by the
// host that instantiated it; no worker process is shared across tenants.
// The tenant scope travels in the request envelope, the worker receives
// only tenant-narrowed data, and the applicable case for the tests is
// the per-tenant worker — the shared-worker case does not exist in this
// kernel version ([`WORKER_TENANCY_STATEMENT`]).
//
// AUDIT-OR-REFUSE (the broker's discipline): every admission and every
// refusal is audited through [`TenantGateAuditSink`]; a failed audit
// write refuses the check in BOTH directions (an admission that could
// not be audited is rolled back to a refusal — no unaudited data
// crossing). The gate reads NO clock of its own: every method takes
// `now_unix_ms` as an explicit parameter, exactly like the broker.

use std::fmt;
use std::sync::Mutex;

use crate::broker::{BrokerError, CapabilityBroker, CapabilityScope, UseReceipt};

// --- Statements ---------------------------------------------------------------------

/// The host-stamped-envelope statement (pinned by tests).
pub const TENANT_GATE_STATEMENT: &str = "the host-stamped tenant envelope is derived from \
     the platform-authenticated context, travels with the request, and is cross-checked at \
     enforcement: a data request must fit inside BOTH the broker's grant envelope AND the \
     host-stamped scope. A misconfigured grant can never widen the host-stamped envelope. \
     Mismatch is TENANT_SCOPE_MISMATCH — refused and audited (report §16.2).";

/// The worker-tenancy model (ledger P7.39 step 3 — recorded, not tested
/// against a shared worker because none exists in this kernel version).
pub const WORKER_TENANCY_STATEMENT: &str = "v0.5.0 worker model: one out-of-process gRPC \
     worker per plugin instance, launched by the host that instantiated it — NO worker \
     process is shared across tenants. The tenant scope travels in the request envelope; \
     the worker receives only tenant-narrowed data; the tested case is the per-tenant \
     worker. A shared multi-tenant worker would be a new architecture with its own \
     decision record.";

/// The recorded RLS relationship (§16.2 / [R25]; ADR-007 — the same
/// statement the kernel module records; restated here so the host-side
/// gate carries its own boundary).
pub const RLS_RELATIONSHIP_STATEMENT: &str = "plugins never touch tables directly (ADR-007: \
     no authoritative state); all plugin data access flows through the platform's \
     tenant-scoped services, where application authorization and PostgreSQL Row-Level \
     Security remain the platform's defence in depth (§16.2, [R25]). This gate adds the \
     envelope-level cross-check only.";

// --- The mirrored scope --------------------------------------------------------------

/// The host-side mirror of the kernel's `TenantScope` (private fields;
/// [`HostTenantScope::from_authenticated`] is the ONLY constructor — the
/// authenticated-claims-only rule, report §16.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTenantScope {
    tenant_id: String,
    site_id: Option<String>,
}

impl HostTenantScope {
    /// The ONLY construction path: from the platform-authenticated
    /// context. The tenant is required and non-empty; a site narrowing,
    /// when present, must be non-empty.
    pub fn from_authenticated(
        tenant_id: &str,
        site_id: Option<&str>,
    ) -> Result<HostTenantScope, TenantGateError> {
        if tenant_id.trim().is_empty() {
            return Err(TenantGateError::ScopeInvalid {
                reason: "the tenant id is empty — the scope is derived from the \
                         authenticated context and must name a tenant"
                    .to_string(),
            });
        }
        if tenant_id.len() > 256 {
            return Err(TenantGateError::ScopeInvalid {
                reason: "the tenant id exceeds 256 characters".to_string(),
            });
        }
        let site_id = match site_id {
            None => None,
            Some(site) => {
                if site.trim().is_empty() {
                    return Err(TenantGateError::ScopeInvalid {
                        reason: "a site narrowing must be a non-empty string (use None, not \
                                 an empty string, for a tenant-wide scope)"
                            .to_string(),
                    });
                }
                if site.len() > 256 {
                    return Err(TenantGateError::ScopeInvalid {
                        reason: "the site id exceeds 256 characters".to_string(),
                    });
                }
                Some(site.to_string())
            }
        };
        Ok(HostTenantScope { tenant_id: tenant_id.to_string(), site_id })
    }

    /// The tenant the scope is stamped with.
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// The site narrowing, if any (`None` = tenant-wide).
    pub fn site_id(&self) -> Option<&str> {
        self.site_id.as_deref()
    }

    /// The narrowing predicate (the kernel `TenantScope::covers` mirror):
    /// tenant must match exactly; a tenant-wide envelope covers any site
    /// request; a site-narrowed envelope covers only the SAME site.
    pub fn covers(&self, requested: &HostTenantScope) -> bool {
        if self.tenant_id != requested.tenant_id {
            return false;
        }
        match (&self.site_id, &requested.site_id) {
            (None, _) => true,
            (Some(g), Some(w)) => g == w,
            (Some(_), None) => false,
        }
    }

    /// The deterministic canonical form (audit rendering).
    pub fn canonical(&self) -> String {
        match &self.site_id {
            Some(site) => format!("tenant:{}|site:{}", self.tenant_id, site),
            None => format!("tenant:{}|site:none", self.tenant_id),
        }
    }

    /// Extracts the scope of a broker `DataScope` request (the other
    /// scope kinds are not data requests and are refused by
    /// [`TenantScopeGate::check_data_request`] before this is called).
    fn from_data_scope(scope: &CapabilityScope) -> Option<HostTenantScope> {
        match scope {
            CapabilityScope::DataScope { tenant_id, site_id, .. } => {
                Some(HostTenantScope {
                    tenant_id: tenant_id.clone(),
                    site_id: site_id.clone(),
                })
            }
            _ => None,
        }
    }
}

impl fmt::Display for HostTenantScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

// --- Errors --------------------------------------------------------------------------

/// Tenant-gate errors. Every refusal is named and audited; nothing is
/// swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantGateError {
    /// A scope is structurally unusable (empty tenant, empty site).
    ScopeInvalid { reason: String },
    /// The requested scope is not a broker `DataScope` — only data
    /// requests carry a tenant scope; anything else through the data gate
    /// is refused, never coerced.
    ScopeKindInvalid { found: String },
    /// THE cross-check refusal: the requested data scope is outside the
    /// host-stamped envelope (stable name `TENANT_SCOPE_MISMATCH`).
    TenantScopeMismatch { envelope: String, requested: String },
    /// The broker refused the call (no grant, expired, outside the GRANT
    /// envelope); the broker's own named error is carried.
    BrokerRefusal { error: BrokerError },
    /// The gate audit sink failed; the event was NOT recorded. The gate
    /// refuses to proceed past a broken audit sink, in BOTH directions
    /// (audit-or-refuse).
    AuditUnavailable { reason: String },
}

impl TenantGateError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            TenantGateError::ScopeInvalid { .. } => "TENANT_SCOPE_INVALID",
            TenantGateError::ScopeKindInvalid { .. } => "TENANT_SCOPE_KIND_INVALID",
            TenantGateError::TenantScopeMismatch { .. } => "TENANT_SCOPE_MISMATCH",
            TenantGateError::BrokerRefusal { error } => error.name(),
            TenantGateError::AuditUnavailable { .. } => "AUDIT_UNAVAILABLE",
        }
    }
}

impl fmt::Display for TenantGateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TenantGateError::ScopeInvalid { reason } => {
                write!(f, "the tenant scope is invalid: {reason}")
            }
            TenantGateError::ScopeKindInvalid { found } => write!(
                f,
                "only a broker DataScope request may pass the tenant data gate (found a \
                 '{found}' scope); non-data requests are refused here, never coerced"
            ),
            TenantGateError::TenantScopeMismatch { envelope, requested } => write!(
                f,
                "the requested data scope '{requested}' is outside the host-stamped envelope \
                 '{envelope}' (TENANT_SCOPE_MISMATCH — the data-level tenant check; refused \
                 and audited, report §16.2)"
            ),
            TenantGateError::BrokerRefusal { error } => {
                write!(f, "the capability broker refused the call: {error}")
            }
            TenantGateError::AuditUnavailable { reason } => write!(
                f,
                "the tenant-gate audit sink failed (audit-or-refuse — the gate never proceeds \
                 past a broken audit sink): {reason}"
            ),
        }
    }
}

impl std::error::Error for TenantGateError {}

// --- Audit ---------------------------------------------------------------------------

/// The kinds of tenant-gate audit records (the broker's audit-kind style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantGateAuditKind {
    /// A scoped data request was admitted (inside BOTH envelopes).
    ScopeAdmitted,
    /// A scoped data request was refused (mismatch, wrong kind, broker
    /// refusal — the detail names the refusal class).
    ScopeRefused,
}

impl TenantGateAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            TenantGateAuditKind::ScopeAdmitted => "PLUGIN_TENANT_GATE_ADMITTED",
            TenantGateAuditKind::ScopeRefused => "PLUGIN_TENANT_GATE_DENIED",
        }
    }
}

/// One tenant-gate audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantGateAuditEvent {
    /// When the gate recorded the event (Unix ms — the value the caller
    /// supplied; the gate reads no clock of its own).
    pub recorded_at_unix_ms: u64,
    pub kind: TenantGateAuditKind,
    /// The plugin the request concerns.
    pub plugin_id: String,
    /// The capability id concerned.
    pub capability: String,
    /// Human-readable detail (scope renderings, refusal names — no
    /// payload data, no secret material).
    pub detail: String,
}

/// The tenant-gate audit sink port (the broker's audit-port style). A
/// failure is `Err` — the gate treats a broken sink as a stop condition
/// in BOTH directions (audit-or-refuse).
pub trait TenantGateAuditSink: Send + Sync {
    fn record(&self, event: TenantGateAuditEvent) -> Result<(), TenantGateError>;
}

/// In-memory tenant-gate audit sink (tests + this-wave composition).
#[derive(Debug, Default)]
pub struct InMemoryTenantGateAuditSink {
    events: Mutex<Vec<TenantGateAuditEvent>>,
}

impl InMemoryTenantGateAuditSink {
    pub fn new() -> InMemoryTenantGateAuditSink {
        InMemoryTenantGateAuditSink { events: Mutex::new(Vec::new()) }
    }

    /// A snapshot of the recorded events (test/composition inspection).
    pub fn events(&self) -> Vec<TenantGateAuditEvent> {
        match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(_poisoned) => Vec::new(),
        }
    }
}

impl TenantGateAuditSink for InMemoryTenantGateAuditSink {
    fn record(&self, event: TenantGateAuditEvent) -> Result<(), TenantGateError> {
        match self.events.lock() {
            Ok(mut guard) => {
                guard.push(event);
                Ok(())
            }
            Err(_) => Err(TenantGateError::AuditUnavailable {
                reason: "the in-memory tenant-gate audit sink's lock is poisoned".to_string(),
            }),
        }
    }
}

/// Shared-sink composition (the enforcement gate's `Arc` pattern).
impl TenantGateAuditSink for std::sync::Arc<InMemoryTenantGateAuditSink> {
    fn record(&self, event: TenantGateAuditEvent) -> Result<(), TenantGateError> {
        (**self).record(event)
    }
}

// --- The gate ------------------------------------------------------------------------

/// The tenant-scope cross-check gate: composes the P7.11 broker's
/// per-call narrowing with the host-stamped envelope check. Order of the
/// two layers is FIXED (envelope FIRST, then grant): a request outside
/// the host-stamped envelope is refused before the broker is consulted,
/// so an over-broad or misconfigured GRANT can never widen what the
/// authenticated context stamped (defence in depth, two independent
/// narrowing layers).
pub struct TenantScopeGate {
    audit: Box<dyn TenantGateAuditSink>,
}

impl TenantScopeGate {
    /// Composes the gate over an audit sink. There is no gate without an
    /// audit sink (a gate that cannot record denials is not composed at
    /// all — fail-closed at composition).
    pub fn compose(audit: Box<dyn TenantGateAuditSink>) -> TenantScopeGate {
        TenantScopeGate { audit }
    }

    fn audit_event(
        &self,
        now_unix_ms: u64,
        kind: TenantGateAuditKind,
        plugin_id: &str,
        capability: &str,
        detail: String,
    ) -> Result<(), TenantGateError> {
        self.audit.record(TenantGateAuditEvent {
            recorded_at_unix_ms: now_unix_ms,
            kind,
            plugin_id: plugin_id.to_string(),
            capability: capability.to_string(),
            detail,
        })
    }

    /// THE DATA GATE: checks one data-scoped capability call against BOTH
    /// narrowing layers. Fail-closed, in order:
    ///
    /// 1. the requested scope must be a broker `DataScope` (anything else
    ///    through the data gate is [`TenantGateError::ScopeKindInvalid`],
    ///    never coerced);
    /// 2. the requested scope must fit inside the HOST-STAMPED envelope
    ///    ([`TenantGateError::TenantScopeMismatch`] — `TENANT_SCOPE_
    ///    MISMATCH` — audited; the broker is not even consulted);
    /// 3. the broker's per-call narrowing runs
    ///    ([`CapabilityBroker::check`]) — grant envelope, expiry,
    ///    never-granted re-check; broker refusals surface under their
    ///    broker names (audited);
    /// 4. the admission is audited with the exact enforced scope.
    ///
    /// A failed audit write refuses the call in BOTH directions
    /// (audit-or-refuse): on step 4's failure the admitted use is not
    /// returned (the broker's use has no side effect to roll back — the
    /// receipt simply never reaches the caller, so the use confers
    /// nothing).
    pub fn check_data_request(
        &self,
        broker: &CapabilityBroker,
        plugin_id: &str,
        envelope: &HostTenantScope,
        capability: &str,
        requested: &CapabilityScope,
        now_unix_ms: u64,
    ) -> Result<UseReceipt, TenantGateError> {
        let outcome = self.check_inner(broker, plugin_id, envelope, capability, requested, now_unix_ms);
        match &outcome {
            Ok(receipt) => {
                self.audit_event(
                    now_unix_ms,
                    TenantGateAuditKind::ScopeAdmitted,
                    plugin_id,
                    capability,
                    format!(
                        "requested scope {} admitted inside host-stamped envelope {} under \
                         grant {}",
                        requested.describe(),
                        envelope.canonical(),
                        receipt.grant_id
                    ),
                )?;
            }
            Err(err) => {
                self.audit_event(
                    now_unix_ms,
                    TenantGateAuditKind::ScopeRefused,
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

    fn check_inner(
        &self,
        broker: &CapabilityBroker,
        plugin_id: &str,
        envelope: &HostTenantScope,
        capability: &str,
        requested: &CapabilityScope,
        now_unix_ms: u64,
    ) -> Result<UseReceipt, TenantGateError> {
        // 1. Only data requests carry a tenant scope.
        let requested_scope = match HostTenantScope::from_data_scope(requested) {
            Some(scope) => scope,
            None => {
                return Err(TenantGateError::ScopeKindInvalid {
                    found: requested.describe(),
                });
            }
        };
        // 2. THE CROSS-CHECK (envelope FIRST — a misconfigured grant can
        //    never widen the host-stamped scope).
        if !envelope.covers(&requested_scope) {
            return Err(TenantGateError::TenantScopeMismatch {
                envelope: envelope.canonical(),
                requested: requested_scope.canonical(),
            });
        }
        // 3. The broker's per-call narrowing (grant envelope, expiry,
        //    never-granted re-check) — refusals surface under their
        //    broker names.
        broker
            .check(plugin_id, capability, requested, now_unix_ms)
            .map_err(|error| TenantGateError::BrokerRefusal { error })
    }
}
