// P7.39 — Cross-component isolation and data-level tenant filtering on the
// plugin path (report §17.4 cross-tenant leakage / compromised plugin,
// §5.7, §16.2; ledger `ledger_phase_7.md` P7.39; decision record
// `ledger/phase-07-plugin-sdk/37_38_39_invariants_revocation_tenancy.md`).
//
// THE RULE (report §16.2, verbatim intent): "Separate tenant context from
// untrusted request body fields; derive it from authenticated claims/
// authorized context" — and "Audit cross-tenant access denials". On the
// plugin path this becomes: the tenant scope of EVERY platform→plugin
// request is HOST-STAMPED, derived from the platform-authenticated
// context (the `EdgeSessionScope` pattern of
// `apps/control-plane/src/routes_edge.rs`: the scope is bound to the
// identity by the SERVER, never read from the caller's payload), and it
// TRAVELS WITH the request so data-level filtering is checked at the
// enforcement point — not only at the call edge.
//
// STRUCTURAL ENFORCEMENT (this module):
//
// - [`TenantScope`]: tenant id + optional site narrowing. Its fields are
//   private and [`TenantScope::from_authenticated`] is the ONLY public
//   constructor — a scope cannot be assembled from plugin-supplied data
//   any more than the domain's `TenantContext` can be assembled from
//   unverified claims (`crates/flexcore-domain/src/tenant_context.rs`,
//   the P4.29 precedent). A plugin cannot widen its own scope: the scope
//   is stamped by the host, and a plugin-echoed scope that differs is the
//   named refusal [`TenantScopeError::ScopeEchoMismatch`].
// - [`ScopedRequestEnvelope`]: a WRAPPER over the P7.36
//   [`PluginRequestEnvelope`] that composes (does not rewrite) it and
//   additionally binds the host-stamped scope into the request identity:
//   the wrapper's digest covers the scope's canonical form PLUS the
//   inner envelope's canonical form, so a scope swap or an inner mutation
//   after wrapping is the named refusal
//   [`TenantScopeError::ScopeBindingMismatch`] (deterministic — same
//   inputs, same digest; the FNV-1a 64 test-grade digest honesty of
//   P7.36/P7.14 applies unchanged: a production digest is a DEPLOYMENT
//   REQUIREMENT).
// - Data-level filtering: the scope travels with the request, and the
//   enforcement point cross-checks the REQUESTED data scope against the
//   envelope's scope ([`enforce_scope`], audited in
//   [`enforce_scoped_request`]) — a request for data outside the
//   host-stamped envelope is [`TenantScopeError::ScopeOutsideEnvelope`]
//   (stable name `TENANT_SCOPE_MISMATCH`), refused and audited, even if
//   some grant or call edge would have allowed it. Narrowing is
//   preserved: a site request inside a tenant-wide envelope, or an equal
//   site, passes; anything wider or different is refused.
//
// THE RLS RELATIONSHIP (recorded honestly; §16.2 / [R25]; ADR-007):
// plugins NEVER touch database tables directly (ADR-007: the PostgreSQL
// store is the platform's transactional source of truth; plugins have no
// database capability in the §5.7 vocabulary at all). All plugin data
// access flows through the platform's tenant-scoped services, where the
// application authorization layer and PostgreSQL Row-Level Security stay
// EXACTLY as §16.2 specifies them — the platform's defence in depth, not
// the SDK's. The SDK adds the ENVELOPE-LEVEL guarantee on top: the scope
// is bound to the request identity, travels with it, and is cross-checked
// at enforcement. Nothing in this module weakens, replaces, or re-implements
// RLS.
//
// The host-side cross-check (the P7.11 broker's `DataScope` narrowing
// cross-checked against the envelope scope) lives in
// `apps/plugin-host/src/tenant_gate.rs` as a documented MIRROR — the host
// crate depends on no flexcore crate (the recorded alignment seam, same
// as the permission-vocabulary mirror in its `broker.rs`).
//
// Audit: every enforcement refusal is audited through the in-module
// [`TenantScopeAuditSink`] port with timestamps from the injected
// [`Clock`] (`crate::lifecycle::Clock`) — "Audit cross-tenant access
// denials" (§16.2). Determinism: canonical forms are fixed-format
// strings; no `unwrap`/`expect` on untrusted input anywhere in `src/`.

use std::collections::BTreeSet;
use std::fmt;

use crate::lifecycle::Clock;
use crate::plugin_path_protection::{PluginPathError, PluginRequestEnvelope};

// --- Honesty statements -------------------------------------------------------------

/// The §16.2 rule as applied to the plugin path (pinned by tests).
pub const TENANT_SCOPE_STATEMENT: &str = "the plugin-path tenant scope is HOST-STAMPED \
     from the platform-authenticated context — never from any plugin-supplied field. A \
     plugin cannot widen its own scope: a plugin-echoed scope that differs from the \
     host-stamped one is a named refusal, and a data request outside the host-stamped \
     envelope is refused and audited at enforcement (report §16.2).";

/// The recorded RLS relationship (§16.2 / [R25]; ADR-007 — pinned by tests).
pub const RLS_RELATIONSHIP_STATEMENT: &str = "plugins never touch tables directly (ADR-007: \
     no authoritative state); all plugin data access flows through the platform's \
     tenant-scoped services, where application authorization and PostgreSQL Row-Level \
     Security remain the platform's defence in depth (§16.2, [R25]). The SDK adds the \
     envelope-level guarantee only: the host-stamped scope is bound into the request \
     identity, travels with the request, and is cross-checked at enforcement. RLS is not \
     re-implemented, weakened, or replaced here.";

// --- TenantScope ---------------------------------------------------------------------

/// The host-stamped tenant scope: tenant id + optional site narrowing.
/// Fields are private; [`TenantScope::from_authenticated`] is the ONLY
/// public constructor (the authenticated-claims-only rule — report §16.2;
/// the `TenantContext`/`EdgeSessionScope` pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantScope {
    tenant_id: String,
    site_id: Option<String>,
}

impl TenantScope {
    /// The ONLY construction path: from the platform-authenticated context.
    /// The tenant is REQUIRED and non-empty; a site narrowing, when
    /// present, must be non-empty. There is no way to build a scope from
    /// request data — that is the point (module header).
    pub fn from_authenticated(
        tenant_id: &str,
        site_id: Option<&str>,
    ) -> Result<TenantScope, TenantScopeError> {
        if tenant_id.trim().is_empty() {
            return Err(TenantScopeError::ScopeInvalid {
                reason: "the tenant id is empty — the scope is derived from the \
                         authenticated context and must name a tenant"
                    .to_string(),
            });
        }
        if tenant_id.len() > 256 {
            return Err(TenantScopeError::ScopeInvalid {
                reason: "the tenant id exceeds 256 characters".to_string(),
            });
        }
        let site_id = match site_id {
            None => None,
            Some(site) => {
                if site.trim().is_empty() {
                    return Err(TenantScopeError::ScopeInvalid {
                        reason: "a site narrowing must be a non-empty string (use None, not \
                                 an empty string, for a tenant-wide scope)"
                            .to_string(),
                    });
                }
                if site.len() > 256 {
                    return Err(TenantScopeError::ScopeInvalid {
                        reason: "the site id exceeds 256 characters".to_string(),
                    });
                }
                Some(site.to_string())
            }
        };
        Ok(TenantScope { tenant_id: tenant_id.to_string(), site_id })
    }

    /// The tenant the scope is stamped with.
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// The site narrowing, if any (`None` = tenant-wide).
    pub fn site_id(&self) -> Option<&str> {
        self.site_id.as_deref()
    }

    /// Whether this (envelope) scope COVERS `requested` — the narrowing
    /// predicate used at enforcement:
    ///
    /// - the tenant must match exactly (cross-tenant is never covered);
    /// - a tenant-wide envelope (`site_id() == None`) covers any site
    ///   request in the same tenant;
    /// - a site-narrowed envelope covers only the SAME site (a request
    ///   for a different site, or an UN-narrowed request under a narrowed
    ///   envelope, is not covered — the broker's `DataScope::covers`
    ///   narrowing rule, mirrored).
    pub fn covers(&self, requested: &TenantScope) -> bool {
        if self.tenant_id != requested.tenant_id {
            return false;
        }
        match (&self.site_id, &requested.site_id) {
            (None, _) => true,
            (Some(g), Some(w)) => g == w,
            (Some(_), None) => false,
        }
    }

    /// The deterministic canonical form (fixed field order; the wrapper
    /// digest binds this string).
    pub fn canonical(&self) -> String {
        match &self.site_id {
            Some(site) => format!("tenant:{}|site:{}", self.tenant_id, site),
            None => format!("tenant:{}|site:none", self.tenant_id),
        }
    }
}

impl fmt::Display for TenantScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

// --- Errors --------------------------------------------------------------------------

/// Every tenant-scope refusal is a DISTINCT named error
/// ([`TenantScopeError::name`]) — never a warning, never a silent
/// acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantScopeError {
    /// A scope is structurally unusable (empty tenant, empty site).
    ScopeInvalid { reason: String },
    /// The inner envelope failed its own P7.36 self-validity check.
    EnvelopeInvalid { source: PluginPathError },
    /// The wrapper's scope-binding digest does not recompute — the scope
    /// was swapped or the inner envelope mutated after wrapping.
    ScopeBindingMismatch,
    /// A plugin-echoed scope differs from the host-stamped one — the
    /// cannot-widen-your-own-scope refusal.
    ScopeEchoMismatch { stamped: String, echoed: String },
    /// The requested data scope is outside the host-stamped envelope —
    /// the `TENANT_SCOPE_MISMATCH` refusal at enforcement.
    ScopeOutsideEnvelope { envelope: String, requested: String },
}

impl TenantScopeError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            TenantScopeError::ScopeInvalid { .. } => "TENANT_SCOPE_INVALID",
            TenantScopeError::EnvelopeInvalid { .. } => "TENANT_SCOPE_ENVELOPE_INVALID",
            TenantScopeError::ScopeBindingMismatch => "TENANT_SCOPE_BINDING_MISMATCH",
            TenantScopeError::ScopeEchoMismatch { .. } => "TENANT_SCOPE_ECHO_MISMATCH",
            TenantScopeError::ScopeOutsideEnvelope { .. } => "TENANT_SCOPE_MISMATCH",
        }
    }
}

impl fmt::Display for TenantScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TenantScopeError::ScopeInvalid { reason } => {
                write!(f, "the tenant scope is invalid: {reason}")
            }
            TenantScopeError::EnvelopeInvalid { source } => {
                write!(f, "the inner plugin-path envelope is invalid ({}): {source}", source.name())
            }
            TenantScopeError::ScopeBindingMismatch => write!(
                f,
                "the scoped envelope's scope binding does not recompute — the scope was \
                 swapped or the inner envelope mutated after wrapping; refused"
            ),
            TenantScopeError::ScopeEchoMismatch { stamped, echoed } => write!(
                f,
                "the plugin echoed scope '{echoed}' but the host-stamped scope is \
                 '{stamped}' — a plugin cannot widen its own scope (report §16.2); refused"
            ),
            TenantScopeError::ScopeOutsideEnvelope { envelope, requested } => write!(
                f,
                "the requested data scope '{requested}' is outside the host-stamped envelope \
                 '{envelope}' (TENANT_SCOPE_MISMATCH — data-level filtering, refused and \
                 audited)"
            ),
        }
    }
}

impl std::error::Error for TenantScopeError {}

// --- The scoped envelope (the P7.36 wrapper) -------------------------------------------

/// The FNV-1a 64-bit hash (the repo's non-cryptographic integrity-checksum
/// pattern — P4.28/P6.14/P7.14/P7.36; deterministic, dependency-free,
/// explicitly NOT a security primitive; a production digest is a
/// DEPLOYMENT REQUIREMENT, same honesty as the rest of the kernel).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A HOST-STAMPED platform→plugin request envelope: the P7.36
/// [`PluginRequestEnvelope`] COMPOSED with (not rewritten to carry) the
/// host-stamped [`TenantScope`], plus a scope-binding digest over the
/// scope's canonical form and the inner envelope's canonical form.
///
/// Why a wrapper (the recorded choice): the P7.36 type is closed and its
/// logic is proven — this module composes it and binds the scope WITHOUT
/// rewriting any existing enforcement, so the four P7.36 checks run
/// unchanged (`inner()`) and the scope adds a fifth, independent binding.
///
/// Determinism: same scope + same inner envelope → same binding digest
/// (the canonical strings are fixed-format and the digest is FNV-1a 64
/// over their concatenation). The scope binding is MUTATION-SENSITIVE:
/// mutating the inner envelope's public fields after wrapping breaks the
/// binding ([`ScopedRequestEnvelope::verify_binding`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedRequestEnvelope {
    inner: PluginRequestEnvelope,
    scope: TenantScope,
    scope_digest_hex: String,
}

impl ScopedRequestEnvelope {
    /// Wraps an inner envelope with the host-stamped scope. The inner
    /// envelope must already be self-valid (P7.36 check 1 — the wrapper
    /// re-runs it via the digest binding: an invalid inner envelope is
    /// [`TenantScopeError::EnvelopeInvalid`]).
    pub fn new(
        inner: PluginRequestEnvelope,
        scope: TenantScope,
    ) -> Result<ScopedRequestEnvelope, TenantScopeError> {
        // The inner envelope's own digest binds its fields; recompute it
        // to refuse a malformed/tampered inner envelope at wrap time.
        inner
            .verify_digest()
            .map_err(|source| TenantScopeError::EnvelopeInvalid { source })?;
        let scope_digest_hex = format!(
            "{:016x}",
            fnv1a64(scope_binding_input(&scope, &inner).as_bytes())
        );
        Ok(ScopedRequestEnvelope { inner, scope, scope_digest_hex })
    }

    /// The inner P7.36 envelope (the four plugin-path checks run on it
    /// unchanged — composition, not replacement).
    pub fn inner(&self) -> &PluginRequestEnvelope {
        &self.inner
    }

    /// The host-stamped scope (stamped by the platform; never by a
    /// plugin).
    pub fn scope(&self) -> &TenantScope {
        &self.scope
    }

    /// The recorded scope-binding digest (16 lowercase hex digits).
    pub fn scope_digest_hex(&self) -> &str {
        &self.scope_digest_hex
    }

    /// Recomputes the scope binding over the CURRENT scope and inner
    /// envelope and compares it with the recorded one — the fifth check:
    /// a swapped scope or a mutated inner envelope is
    /// [`TenantScopeError::ScopeBindingMismatch`].
    pub fn verify_binding(&self) -> Result<(), TenantScopeError> {
        let recomputed = format!(
            "{:016x}",
            fnv1a64(scope_binding_input(&self.scope, &self.inner).as_bytes())
        );
        if recomputed == self.scope_digest_hex {
            Ok(())
        } else {
            Err(TenantScopeError::ScopeBindingMismatch)
        }
    }

    /// Verifies a plugin-ECHOED scope: it must match the host-stamped
    /// scope EXACTLY (tenant AND site). A plugin echoing a wider or
    /// different scope is the named cannot-widen refusal
    /// ([`TenantScopeError::ScopeEchoMismatch`]).
    pub fn check_echo(&self, echoed: &TenantScope) -> Result<(), TenantScopeError> {
        if *echoed == self.scope {
            Ok(())
        } else {
            Err(TenantScopeError::ScopeEchoMismatch {
                stamped: self.scope.canonical(),
                echoed: echoed.canonical(),
            })
        }
    }
}

/// The scope-binding input: fixed field order, the scope's canonical form
/// plus the inner envelope's canonical form (deterministic and
/// mutation-sensitive).
fn scope_binding_input(scope: &TenantScope, inner: &PluginRequestEnvelope) -> String {
    format!("scope:{}|inner:{}", scope.canonical(), inner.canonical_string())
}

// --- Enforcement -----------------------------------------------------------------------

/// Pure enforcement: the REQUESTED data scope must be covered by the
/// envelope's host-stamped scope (narrowing preserved; anything wider or
/// different is [`TenantScopeError::ScopeOutsideEnvelope`] — stable name
/// `TENANT_SCOPE_MISMATCH`).
pub fn enforce_scope(
    scoped: &ScopedRequestEnvelope,
    requested: &TenantScope,
) -> Result<(), TenantScopeError> {
    if scoped.scope().covers(requested) {
        Ok(())
    } else {
        Err(TenantScopeError::ScopeOutsideEnvelope {
            envelope: scoped.scope().canonical(),
            requested: requested.canonical(),
        })
    }
}

// --- The audited enforcement wrapper ---------------------------------------------------

/// The kinds of tenant-scope audit records (the SCREAMING_SNAKE stable
/// style; "Audit cross-tenant access denials", §16.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantScopeAuditKind {
    /// A scoped request passed enforcement (scope + binding + echo all
    /// verified).
    ScopeAdmitted,
    /// A scoped request was refused (mismatch, echo, binding, invalid —
    /// the detail names the refusal class).
    ScopeRefused,
}

impl TenantScopeAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            TenantScopeAuditKind::ScopeAdmitted => "PLUGIN_TENANT_SCOPE_ADMITTED",
            TenantScopeAuditKind::ScopeRefused => "PLUGIN_TENANT_SCOPE_REFUSED",
        }
    }
}

/// One tenant-scope audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantScopeAuditRecord {
    /// When the event was recorded (injected-clock milliseconds).
    pub timestamp_millis: u64,
    pub kind: TenantScopeAuditKind,
    /// The plugin the request targeted (the inner envelope's plugin id).
    pub plugin_id: String,
    /// Human-readable detail (scope renderings and refusal names — no
    /// payload data, no secret material).
    pub detail: String,
}

/// The tenant-scope audit sink port (kernel-local, the lifecycle-port
/// style). Implementations MUST be append-only (§5.1 audit integrity).
pub trait TenantScopeAuditSink {
    fn record(&self, record: TenantScopeAuditRecord);
}

/// An append-only in-memory [`TenantScopeAuditSink`] — the deterministic
/// collector used by tests and in-process compositions.
#[derive(Debug, Default)]
pub struct MemoryTenantScopeAuditSink {
    records: std::cell::RefCell<Vec<TenantScopeAuditRecord>>,
}

impl MemoryTenantScopeAuditSink {
    pub fn new() -> MemoryTenantScopeAuditSink {
        MemoryTenantScopeAuditSink { records: std::cell::RefCell::new(Vec::new()) }
    }

    /// The records in emission order (append-only; the borrow is brief
    /// and single-threaded).
    pub fn records(&self) -> std::cell::Ref<Vec<TenantScopeAuditRecord>> {
        self.records.borrow()
    }
}

impl TenantScopeAuditSink for MemoryTenantScopeAuditSink {
    fn record(&self, record: TenantScopeAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}

/// The audited enforcement path for one scoped request. Order (each step
/// is a distinct named refusal; every refusal is audited):
///
/// 1. the scope binding ([`ScopedRequestEnvelope::verify_binding`]);
/// 2. the data-level scope check ([`enforce_scope`]);
/// 3. the plugin-echoed scope, when the plugin supplies one
///    ([`ScopedRequestEnvelope::check_echo`]) — `None` for request classes
///    that carry no scope echo.
///
/// On `Ok` the admission is audited too ([`TenantScopeAuditKind::
/// ScopeAdmitted`]) with the exact enforced scope. Timestamps come only
/// from the injected [`Clock`].
pub fn enforce_scoped_request(
    scoped: &ScopedRequestEnvelope,
    requested: &TenantScope,
    echoed: Option<&TenantScope>,
    clock: &dyn Clock,
    audit: &dyn TenantScopeAuditSink,
) -> Result<(), TenantScopeError> {
    let outcome = enforce_scoped_request_inner(scoped, requested, echoed);
    match &outcome {
        Ok(()) => audit.record(TenantScopeAuditRecord {
            timestamp_millis: clock.now_millis(),
            kind: TenantScopeAuditKind::ScopeAdmitted,
            plugin_id: scoped.inner().plugin_id.clone(),
            detail: format!(
                "scope {} admitted within host-stamped envelope {} (binding verified)",
                requested.canonical(),
                scoped.scope().canonical()
            ),
        }),
        Err(err) => audit.record(TenantScopeAuditRecord {
            timestamp_millis: clock.now_millis(),
            kind: TenantScopeAuditKind::ScopeRefused,
            plugin_id: scoped.inner().plugin_id.clone(),
            detail: format!("refused ({}): {err}", err.name()),
        }),
    }
    outcome
}

fn enforce_scoped_request_inner(
    scoped: &ScopedRequestEnvelope,
    requested: &TenantScope,
    echoed: Option<&TenantScope>,
) -> Result<(), TenantScopeError> {
    scoped.verify_binding()?;
    enforce_scope(scoped, requested)?;
    if let Some(echoed) = echoed {
        scoped.check_echo(echoed)?;
    }
    Ok(())
}

// --- The closed refusal-name table ----------------------------------------------------

/// All stable refusal/admission names of this module (sorted; the
/// pairwise-distinct property is pinned by tests, the house style).
pub const TENANT_SCOPE_ERROR_NAMES: [&str; 5] = [
    "TENANT_SCOPE_BINDING_MISMATCH",
    "TENANT_SCOPE_ECHO_MISMATCH",
    "TENANT_SCOPE_ENVELOPE_INVALID",
    "TENANT_SCOPE_INVALID",
    "TENANT_SCOPE_MISMATCH",
];

/// The refusal names taken from the enum itself (sorted, deduplicated via
/// `BTreeSet`) — the test-side consistency check against
/// [`TENANT_SCOPE_ERROR_NAMES`], so a variant added without updating the
/// table is a named test failure.
pub fn error_names_from_enum() -> BTreeSet<&'static str> {
    let names: [&str; 5] = [
        TenantScopeError::ScopeInvalid { reason: String::new() }.name(),
        TenantScopeError::EnvelopeInvalid {
            source: PluginPathError::RequestEnvelopeInvalid { reason: String::new() },
        }
        .name(),
        TenantScopeError::ScopeBindingMismatch.name(),
        TenantScopeError::ScopeEchoMismatch {
            stamped: String::new(),
            echoed: String::new(),
        }
        .name(),
        TenantScopeError::ScopeOutsideEnvelope {
            envelope: String::new(),
            requested: String::new(),
        }
        .name(),
    ];
    names.into_iter().collect()
}
