// P7.12 — The enforcement gate: manifest permissions are ENFORCED, not
// decorative (report §5.4 Class B, §5.7, §9.1, §13.4, §17.5; ledger
// `ledger_phase_7.md` §P7.12; decision record
// `ledger/phase-07-plugin-sdk/12_13_enforcement_negatives.md`).
//
// THE RULE
//
// A component is instantiated ONLY with the capability set derived from
// (manifest permissions ∩ profile ceiling) filtered through the broker's
// scope narrowing — and THIS module is the ONLY constructor path to a
// binding: [`instantiate`] takes an [`EnforcementVerdict`], and the host's
// raw binding helper (`crate::host::bind_component`) is `pub(crate)` since
// P7.12 — no public API instantiates from raw manifest data. There is no
// function anywhere in this crate that takes manifest permissions and
// yields a binding without an enforcement verdict.
//
// A manifest permission the broker cannot scope — an unknown id, a
// never-granted id, an over-broad request (declared but above the profile
// ceiling) — is a REFUSAL BEFORE ANY COMPONENT CODE RUNS: enforcement is
// fail-closed at the door, not a runtime afterthought. The gate composes
// the P7.11 broker and drives
// [`crate::broker::CapabilityBroker::derive_grants`], so every refusal the
// broker can name (unknown, never-granted, not-declared, invalid
// scope/expiry) surfaces here under its broker name.
//
// THE DRIFT-REFUSAL RULE (the recorded alignment seam, record §6)
//
// The permission vocabulary and the resource-field names are documented
// MIRRORS of `crates/flexcore-kernel/src/manifest.rs` (values read
// 2026-11-13; the kernel crate was NOT touched — it is owned by another
// wave). Where the kernel vocabulary and this mirror differ — the recorded
// seam cases are the ledger's `resources.timeoutMs` spelling vs the
// kernel's `timeout-ms`, and the kernel `ResourceBounds` having NO fuel or
// concurrency field — ANY unalignable field is a refusal with the named
// [`EnforcementError::PermissionVocabularyDrift`] error
// (`PERMISSION_VOCABULARY_DRIFT`), never a default interpretation. The
// mirror is closed: a kernel-side addition is denied here until the mirror
// is deliberately updated (fail-closed in both directions; §6 of
// `10_11_plugin_host_broker.md`).
//
// THE FROZEN-CAPABILITY RULE
//
// The enforced capability set is FROZEN AT INSTANTIATION. The verdict owns
// CLONED copies of the granted grants; a later change to the manifest
// request (or to the profile ceiling) cannot expand a verdict that has
// already been issued — re-evaluation produces a NEW verdict, and the old
// one is immutable by construction (no `&mut` path exists; `Clone` copies
// freeze further). The broker's per-call re-checks still govern every USE,
// so revocation and expiry can only TIGHTEN a running instance, never
// widen it. The only path to a different capability set is
// RE-INSTANTIATION through drain-before-replace
// (`crates/flexcore-kernel/src/lifecycle.rs`: `is_legal_transition`,
// `DrainPort`, `DrainPolicy`; ledger §P7.07) — cited, NOT implemented here:
// the lifecycle is kernel-owned and this crate depends on no flexcore crate.
//
// THE CANDIDATE-ONLY RULE (§13.4, ledger §P7.12 step 2)
//
// `schedule.propose` / `dispatch.propose` grants make a plugin's output a
// CANDIDATE: it enters the protected validation → feasibility →
// authorization path and gains no authority from having been proposed. The
// gate enforces this structurally: the vocabulary contains no authorization
// id at all (the never-granted set is re-refused here), and the broker's
// forward path returns `()` — no verdict can flow back through a signature
// that has no verdict in it. Research plugins return proposals, never an
// authorization token.
//
// AUDIT-OR-REFUSE (the host broker's audit port style)
//
// Every enforcement decision — allow WITH the exact granted scope, or
// refuse WITH the named reason — is audited through [`EnforcementAuditSink`].
// A failed audit write refuses the decision in BOTH directions (a verdict
// that could not be audited is never returned; a refusal whose audit failed
// surfaces as [`EnforcementError::AuditUnavailable`]). Like the broker, the
// gate reads NO clock of its own: [`EnforcementGate::evaluate`] takes
// `now_unix_ms` as an explicit parameter and the HOST carries the
// responsibility for its correctness.

use std::fmt;
use std::sync::Mutex;

use crate::broker::{
    BrokerError, CapabilityBroker, CapabilityClass, CapabilityGrant, CapabilityScope,
    GrantDerivationRequest, GrantRequest, NEVER_GRANTED_PERMISSIONS,
};
use crate::host::{bind_component, ComponentArtifact, HostError, WorldId};
use crate::limits::{DeclaredBounds, ResourceLimits};

// --- Vocabulary mirrors (alignment seam; kernel side untouched) -----------------

/// The closed §5.7 permission ids the gate may ever grant. MIRROR of
/// `crates/flexcore-kernel/src/manifest.rs::ALLOWED_PERMISSIONS` — the same
/// values the P7.11 broker mirrors; restated here so the gate refuses
/// anything its OWN vocabulary does not name (the gate never trusts that a
/// caller pre-filtered the ids).
pub const ENFORCEABLE_PERMISSIONS: [&str; 6] = [
    "telemetry.read",
    "asset.read",
    "forecast.publish",
    "schedule.propose",
    "dispatch.propose",
    "dispatch.validate.request",
];

/// The resource-field vocabulary of the kernel manifest schema v1 — the
/// closed key set of the kernel's `resources` object (values read
/// 2026-11-13 from `crates/flexcore-kernel/src/manifest.rs::parse_resources`;
/// the kernel's key list is private, so the mirror is restated here). ANY
/// other field name — including the ledger's `timeoutMs` spelling and any
/// fuel or concurrency field the kernel schema does not carry — is
/// [`EnforcementError::PermissionVocabularyDrift`].
pub const RESOURCE_FIELD_VOCABULARY: [&str; 4] = [
    "max-memory-mb",
    "timeout-ms",
    "max-output-mb",
    "max-calls-per-minute",
];

// --- The instantiation request ---------------------------------------------------

/// The manifest-derived inputs to the enforcement gate: exactly what a
/// VALIDATED manifest (kernel P7.05 gate) plus the composition seam hand to
/// the host. Every field is re-checked here — the gate trusts nothing about
/// the caller, including the claim that the manifest was validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementRequest {
    /// The plugin the instance would belong to (non-empty).
    pub plugin_id: String,
    /// The manifest's `plugin-id` (the derivation trail recorded on grants).
    pub manifest_id: String,
    /// The manifest's version.
    pub manifest_version: String,
    /// The recorded P7.09 world the instance would bind against.
    pub world: WorldId,
    /// The class-B entry-point symbol the manifest declares.
    pub entry_symbol: String,
    /// The permission ids the manifest declares (closed ids; never-granted
    /// and unknown ids are refused HERE, at the door, even though the kernel
    /// manifest gate already rejects them — the gate re-refuses, kernel
    /// manifest.rs header discipline).
    pub manifest_permissions: Vec<String>,
    /// The resource-field names the manifest's `resources` object carried.
    /// Any name outside [`RESOURCE_FIELD_VOCABULARY`] is vocabulary drift
    /// ([`EnforcementError::PermissionVocabularyDrift`]) — never a default
    /// interpretation.
    pub declared_resource_fields: Vec<String>,
    /// The manifest's declared resource bounds (the [`DeclaredBounds`]
    /// mirror of the kernel's `ResourceBounds`), re-validated against the
    /// SAME closed ranges at [`ResourceLimits::from_declared_bounds`].
    pub bounds: DeclaredBounds,
    /// The mandatory deployment-configured fuel budget (there is no default:
    /// an unmetered instance is not constructible — §3 of
    /// `10_11_plugin_host_broker.md`).
    pub fuel_budget: u64,
}

// --- The profile ceiling ---------------------------------------------------------

/// One entry of the deployment profile's ceiling: the WIDEST grant (scope
/// and expiry) the profile permits for one capability id. The gate
/// intersects the manifest's declared permissions with the ceiling — a
/// manifest permission with no ceiling entry is over-broad and REFUSED.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileGrant {
    /// The capability id this ceiling entry permits.
    pub capability: String,
    /// The widest scope the profile permits for the capability. The broker's
    /// scope validation still applies (a `dispatch.validate.request` entry
    /// may carry ONLY the protected-validator target — anything else is
    /// refused by [`crate::broker::CapabilityBroker::derive_grants`]).
    pub scope: CapabilityScope,
    /// The expiry applied to grants derived from this entry (`None` =
    /// until revoked by composition change).
    pub expires_at_unix_ms: Option<u64>,
}

/// The deployment profile's permission ceiling — the `profile` side of the
/// (manifest permissions ∩ profile ceiling) intersection. The ceiling is
/// deployment configuration, never a manifest property and never selectable
/// by a plugin (report §5.7; a plugin can never choose its
/// control-authority mode or widen its profile).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileCeiling {
    entries: Vec<ProfileGrant>,
}

impl ProfileCeiling {
    /// Composes a ceiling from its entries. The ceiling is config INPUT —
    /// it is re-validated by the gate at [`EnforcementGate::evaluate`]
    /// (never-granted or unknown capability ids in a ceiling are refused
    /// there; duplicates are [`EnforcementError::ProfileInvalid`]).
    pub fn new(entries: Vec<ProfileGrant>) -> ProfileCeiling {
        ProfileCeiling { entries }
    }

    /// The EMPTY ceiling: the DEFAULT posture of a deployment that has
    /// composed no profile (DEFAULT decision, decision record §4). Every
    /// manifest permission is over-broad against it — a host without a
    /// profile instantiates capability-less components only, or nothing.
    pub fn empty() -> ProfileCeiling {
        ProfileCeiling { entries: Vec::new() }
    }

    /// The widest scope the ceiling permits for one capability.
    fn entry_for(&self, capability: &str) -> Option<&ProfileGrant> {
        self.entries.iter().find(|e| e.capability == capability)
    }
}

// --- The full input to the door --------------------------------------------------

/// One instantiation request plus the deployment profile's ceiling — the
/// full input to [`EnforcementGate::evaluate`]. The ceiling is a parameter,
/// NOT a field of [`EnforcementRequest`], so the manifest-derived part can
/// be evaluated against different profiles without rebuilding it (and so a
/// ceiling can never be smuggled INTO a manifest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiationInput<'a> {
    /// The manifest-derived request.
    pub request: &'a EnforcementRequest,
    /// The deployment profile's ceiling.
    pub ceiling: &'a ProfileCeiling,
}

// --- The enforcement verdict -----------------------------------------------------

/// The ALLOW decision of the enforcement gate: the frozen capability set +
/// the re-validated resource limits an instance may run under. The verdict
/// is the ONLY key to [`instantiate`].
///
/// Unforgeable by construction: the fields are private and there is no
/// public constructor — the only way to obtain a verdict is
/// [`EnforcementGate::evaluate`]. A caller outside this crate cannot build
/// one (a struct literal over private fields is a compile error), which is
/// the capability-object forgery refusal of ledger §P7.13(c) applied to the
/// enforcement layer's own authority object.
///
/// FROZEN at instantiation (the rule in the module header): the grants are
/// cloned copies; no later manifest or ceiling change can expand an issued
/// verdict. Re-instantiation through drain-before-replace (kernel lifecycle,
/// ledger §P7.07) is the only path to a different capability set — cited,
/// not implemented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementVerdict {
    plugin_id: String,
    manifest_id: String,
    manifest_version: String,
    world: WorldId,
    entry_symbol: String,
    /// The frozen capability grants (cloned from the broker's receipts, in
    /// the manifest's declaration order).
    grants: Vec<CapabilityGrant>,
    limits: ResourceLimits,
    evaluated_at_unix_ms: u64,
}

impl EnforcementVerdict {
    /// The plugin the verdict belongs to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// The granting manifest's id (the derivation trail).
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// The granting manifest's version.
    pub fn manifest_version(&self) -> &str {
        &self.manifest_version
    }

    /// The recorded world the instance binds against.
    pub fn world(&self) -> WorldId {
        self.world
    }

    /// The class-B entry-point symbol.
    pub fn entry_symbol(&self) -> &str {
        &self.entry_symbol
    }

    /// The frozen capability grants (read-only copies; the authority
    /// dynamics live in the broker's per-call re-checks — the frozen set
    /// bounds what the instance CAN hold, the broker state bounds what it
    /// can use RIGHT NOW).
    pub fn grants(&self) -> &[CapabilityGrant] {
        &self.grants
    }

    /// The capability ids in the frozen set, in declaration order.
    pub fn granted_capabilities(&self) -> Vec<String> {
        self.grants.iter().map(|g| g.capability().to_string()).collect()
    }

    /// The frozen scope of one granted capability.
    pub fn granted_scope(&self, capability: &str) -> Option<CapabilityScope> {
        self.grants
            .iter()
            .find(|g| g.capability() == capability)
            .map(|g| g.scope().clone())
    }

    /// The re-validated resource limits the instance runs under (mirrored
    /// from the declared bounds and re-checked — the host enforces the
    /// DECLARED bounds, never the component's belief, ledger §P7.13(e)).
    pub fn limits(&self) -> ResourceLimits {
        self.limits
    }

    /// The host-supplied time of the decision (the gate reads no clock).
    pub fn evaluated_at_unix_ms(&self) -> u64 {
        self.evaluated_at_unix_ms
    }

    /// Audit rendering of the frozen set (identifiers and scope renderings
    /// only — no secret material can appear here).
    pub fn describe_grants(&self) -> String {
        if self.grants.is_empty() {
            return "none (capability-less instance)".to_string();
        }
        self.grants
            .iter()
            .map(|g| format!("{}={}", g.capability(), g.scope().describe()))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

// --- Errors ---------------------------------------------------------------------

/// Enforcement errors. Every refusal is named and audited; nothing is
/// swallowed, and no refusal path returns a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementError {
    /// The plugin id is empty or whitespace — an enforcement decision must
    /// always name its subject.
    PluginIdInvalid { reason: String },
    /// A required identity field of the request (manifest id, manifest
    /// version, entry symbol) is empty — the door does not instantiate
    /// unnamed things.
    RequestInvalid { reason: String },
    /// The capability id is not in the gate's closed vocabulary at all.
    PermissionUnknown { value: String },
    /// The capability id is in the never-granted set — refused at the door
    /// regardless of what any manifest or profile says (the gate re-refuses;
    /// kernel manifest.rs header discipline).
    PermissionNeverGrantable { value: String },
    /// The same permission id is declared twice in one request (the kernel
    /// manifest validator rejects duplicates; the gate re-refuses).
    PermissionDuplicateDeclaration { value: String },
    /// The manifest declares a permission the profile ceiling does not
    /// permit — the over-broad refusal, never trimmed to fit: the gate
    /// grants the intersection, and a permission outside it REFUSES the
    /// instantiation rather than silently dropping the row (fail-closed:
    /// a manifest that asks for more than its profile must not run).
    PermissionAboveProfile { plugin_id: String, capability: String },
    /// A field of the request cannot be aligned with the host's mirror of
    /// the kernel manifest vocabulary (the recorded seam: kernel
    /// `timeout-ms` vs ledger `timeoutMs`; the kernel `ResourceBounds`
    /// carries no fuel/concurrency field). NEVER a default interpretation.
    PermissionVocabularyDrift { field: String, reason: String },
    /// The profile ceiling itself is misconfigured (duplicate entries, or a
    /// capability id the vocabulary does not name).
    ProfileInvalid { reason: String },
    /// A declared bound is outside its closed range or the fuel budget is
    /// missing ([`crate::limits::ResourceLimits::from_declared_bounds`]
    /// re-validation — the resource-bound lie is refused at the door).
    LimitsInvalid { field: String, reason: String },
    /// The broker refused the derivation; the broker's own named error is
    /// carried (PERMISSION_UNKNOWN, PERMISSION_NEVER_GRANTABLE,
    /// PERMISSION_NOT_DECLARED, SCOPE_INVALID, GRANT_EXPIRY_INVALID, ...).
    BrokerRefusal { error: BrokerError },
    /// The binding seam refused (e.g. `BINDINGS_NOT_GENERATED` in the
    /// default build); the host's own named error is carried.
    HostRefusal { error: HostError },
    /// The enforcement audit sink failed; the decision was NOT recorded.
    /// The gate never proceeds past a broken audit sink (audit-or-refuse,
    /// in both directions).
    AuditUnavailable { reason: String },
}

impl EnforcementError {
    /// The stable machine-readable error name (the named-error table key,
    /// decision record §6).
    pub fn name(&self) -> &'static str {
        match self {
            EnforcementError::PluginIdInvalid { .. } => "PLUGIN_ID_INVALID",
            EnforcementError::RequestInvalid { .. } => "INSTANTIATION_REQUEST_INVALID",
            EnforcementError::PermissionUnknown { .. } => "PERMISSION_UNKNOWN",
            EnforcementError::PermissionNeverGrantable { .. } => "PERMISSION_NEVER_GRANTABLE",
            EnforcementError::PermissionDuplicateDeclaration { .. } => {
                "PERMISSION_DUPLICATE_DECLARATION"
            }
            EnforcementError::PermissionAboveProfile { .. } => "PERMISSION_ABOVE_PROFILE",
            EnforcementError::PermissionVocabularyDrift { .. } => "PERMISSION_VOCABULARY_DRIFT",
            EnforcementError::ProfileInvalid { .. } => "PROFILE_INVALID",
            EnforcementError::LimitsInvalid { .. } => "LIMITS_INVALID",
            EnforcementError::BrokerRefusal { error } => error.name(),
            EnforcementError::HostRefusal { error } => error.name(),
            EnforcementError::AuditUnavailable { .. } => "AUDIT_UNAVAILABLE",
        }
    }
}

impl fmt::Display for EnforcementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnforcementError::PluginIdInvalid { reason } => {
                write!(f, "plugin id is invalid: {reason}")
            }
            EnforcementError::RequestInvalid { reason } => {
                write!(f, "instantiation request is invalid: {reason}")
            }
            EnforcementError::PermissionUnknown { value } => {
                write!(f, "capability '{value}' is not in the enforcement vocabulary")
            }
            EnforcementError::PermissionNeverGrantable { value } => {
                write!(f, "capability '{value}' is in the never-granted set and is refused at the door, before any component code runs")
            }
            EnforcementError::PermissionDuplicateDeclaration { value } => {
                write!(f, "permission '{value}' is declared twice in one request")
            }
            EnforcementError::PermissionAboveProfile { plugin_id, capability } => {
                write!(
                    f,
                    "plugin '{plugin_id}' declares '{capability}', which its deployment profile's ceiling does not permit (over-broad requests are refused, never trimmed)"
                )
            }
            EnforcementError::PermissionVocabularyDrift { field, reason } => {
                write!(f, "permission vocabulary drift on field '{field}': {reason}")
            }
            EnforcementError::ProfileInvalid { reason } => {
                write!(f, "profile ceiling is invalid: {reason}")
            }
            EnforcementError::LimitsInvalid { field, reason } => {
                write!(f, "resource limit '{field}' is invalid: {reason}")
            }
            EnforcementError::BrokerRefusal { error } => {
                write!(f, "the capability broker refused the derivation: {error}")
            }
            EnforcementError::HostRefusal { error } => {
                write!(f, "the binding seam refused: {error}")
            }
            EnforcementError::AuditUnavailable { reason } => {
                write!(
                    f,
                    "the enforcement audit sink failed (audit-or-refuse — the gate never proceeds past a broken audit sink): {reason}"
                )
            }
        }
    }
}

impl std::error::Error for EnforcementError {}

// --- Audit ----------------------------------------------------------------------

/// The kinds of enforcement audit records (style of
/// [`crate::broker::BrokerAuditKind`] — SCREAMING_SNAKE stable names, no
/// secret material in any detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementAuditKind {
    /// The gate ALLOWED the instantiation; the detail carries the EXACT
    /// granted scope (capability=scope rendering per grant).
    InstantiationAllowed,
    /// The gate REFUSED the instantiation; the detail names the refusal
    /// class and reason.
    InstantiationRefused,
    /// The frozen-capability rule was applied: the granted set is frozen at
    /// instantiation and can only be replaced (never widened) through
    /// re-instantiation via drain-before-replace.
    CapabilitySetFrozen,
}

impl EnforcementAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            EnforcementAuditKind::InstantiationAllowed => "PLUGIN_INSTANTIATION_ALLOWED",
            EnforcementAuditKind::InstantiationRefused => "PLUGIN_INSTANTIATION_REFUSED",
            EnforcementAuditKind::CapabilitySetFrozen => "PLUGIN_CAPABILITY_SET_FROZEN",
        }
    }
}

/// One enforcement audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementAuditEvent {
    /// When the gate recorded the event (Unix ms — the value the host
    /// supplied for the call; the gate reads no clock of its own).
    pub recorded_at_unix_ms: u64,
    /// The event kind.
    pub kind: EnforcementAuditKind,
    /// The plugin the decision concerns.
    pub plugin_id: String,
    /// The capability id concerned (empty for whole-request decisions such
    /// as vocabulary drift or the freeze record).
    pub capability: String,
    /// Human-readable detail (identifiers, scope renderings, refusal
    /// names — never secret material).
    pub detail: String,
}

/// The enforcement audit sink port (the broker's audit-port style). A
/// failure is `Err` — the gate treats a broken sink as a stop condition in
/// BOTH directions (audit-or-refuse), never as a note in a log.
pub trait EnforcementAuditSink: Send + Sync {
    fn record(&self, event: EnforcementAuditEvent) -> Result<(), EnforcementError>;
}

/// In-memory enforcement audit sink (tests + this-wave composition; the
/// durable audit path is the control plane's audit crate over the P7.20
/// gRPC seam — recorded, not implemented here).
#[derive(Debug, Default)]
pub struct InMemoryEnforcementAuditSink {
    events: Mutex<Vec<EnforcementAuditEvent>>,
}

impl InMemoryEnforcementAuditSink {
    pub fn new() -> InMemoryEnforcementAuditSink {
        InMemoryEnforcementAuditSink { events: Mutex::new(Vec::new()) }
    }

    /// A snapshot of the recorded events (test/composition inspection).
    pub fn events(&self) -> Vec<EnforcementAuditEvent> {
        match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(_poisoned) => Vec::new(),
        }
    }
}

impl EnforcementAuditSink for InMemoryEnforcementAuditSink {
    fn record(&self, event: EnforcementAuditEvent) -> Result<(), EnforcementError> {
        match self.events.lock() {
            Ok(mut guard) => {
                guard.push(event);
                Ok(())
            }
            Err(_) => Err(EnforcementError::AuditUnavailable {
                reason: "the in-memory enforcement audit sink's lock is poisoned".to_string(),
            }),
        }
    }
}

/// Shared-sink composition: the gate owns one `Arc` clone (behind the
/// `Box<dyn EnforcementAuditSink>` it was composed with) and the composition
/// root / test inspects the other. Cloning shares the SAME event log —
/// never a copy taken at clone time.
impl EnforcementAuditSink for std::sync::Arc<InMemoryEnforcementAuditSink> {
    fn record(&self, event: EnforcementAuditEvent) -> Result<(), EnforcementError> {
        (**self).record(event)
    }
}

// --- The gate --------------------------------------------------------------------

/// The enforcement gate: composes the P7.11 capability broker and the
/// enforcement audit sink, and is the ONLY issuer of
/// [`EnforcementVerdict`]s. Every instantiation decision flows through
/// [`evaluate`]; every binding flows through [`instantiate`].
pub struct EnforcementGate {
    broker: CapabilityBroker,
    audit: Box<dyn EnforcementAuditSink>,
}

impl EnforcementGate {
    /// Composes the gate over a broker and an audit sink. There is no gate
    /// without a broker (the scope narrowing IS the broker's derivation)
    /// and none without an audit sink (a gate that cannot record its
    /// decisions is not composed at all — fail-closed at composition).
    pub fn compose(
        broker: CapabilityBroker,
        audit: Box<dyn EnforcementAuditSink>,
    ) -> EnforcementGate {
        EnforcementGate { broker, audit }
    }

    /// The composed broker — the per-call re-check surface for every USE of
    /// the granted capabilities (no cached ambient authority; revocation
    /// and expiry tighten a running instance at the next call).
    pub fn broker(&self) -> &CapabilityBroker {
        &self.broker
    }

    /// Audits one event; a failed write is
    /// [`EnforcementError::AuditUnavailable`].
    fn audit_event(
        &self,
        now_unix_ms: u64,
        kind: EnforcementAuditKind,
        plugin_id: &str,
        capability: &str,
        detail: String,
    ) -> Result<(), EnforcementError> {
        self.audit.record(EnforcementAuditEvent {
            recorded_at_unix_ms: now_unix_ms,
            kind,
            plugin_id: plugin_id.to_string(),
            capability: capability.to_string(),
            detail,
        })
    }

    /// The refuse path: audits [`EnforcementAuditKind::InstantiationRefused`]
    /// with the named reason, then returns the original error. A failed
    /// audit write REPLACES the refusal with
    /// [`EnforcementError::AuditUnavailable`] (audit-or-refuse, the broker's
    /// [`CapabilityBroker::check`] pattern).
    fn refused(
        &self,
        now_unix_ms: u64,
        plugin_id: &str,
        capability: &str,
        error: EnforcementError,
    ) -> EnforcementError {
        let audit_result = self.audit_event(
            now_unix_ms,
            EnforcementAuditKind::InstantiationRefused,
            plugin_id,
            capability,
            format!("refused ({}): {error}", error.name()),
        );
        match audit_result {
            Ok(()) => error,
            Err(audit_error) => audit_error,
        }
    }

    /// Revokes every grant a verdict carries (the rollback when the allow's
    /// own audit write failed). Revocation is the safe direction: authority
    /// is DROPPED, and the revocation stands even if its own audit write
    /// fails (the broker's recorded asymmetry — no rollback of a rollback).
    fn rollback_grants(&self, verdict: &EnforcementVerdict, now_unix_ms: u64) {
        for grant in verdict.grants() {
            let _ = self.broker.revoke(grant.grant_id(), now_unix_ms);
        }
    }

    /// THE DOOR: evaluates one instantiation request against one deployment
    /// profile's ceiling. Fail-closed, in order:
    ///
    /// 1. identity fields non-empty;
    /// 2. vocabulary alignment — every declared resource field must be in
    ///    [`RESOURCE_FIELD_VOCABULARY`] (`timeoutMs`, `fuel-*`,
    ///    `concurrency`, or any other unalignable name is
    ///    `PERMISSION_VOCABULARY_DRIFT`, never a default interpretation);
    /// 3. the profile ceiling is structurally sound (parseable
    ///    capabilities, no never-granted id, no duplicates);
    /// 4. every declared permission is re-checked against the closed
    ///    vocabulary (never-granted and unknown ids refused at the door;
    ///    duplicates too);
    /// 5. the intersection (manifest permissions ∩ profile ceiling) is
    ///    built — a declared permission with no ceiling entry is
    ///    `PERMISSION_ABOVE_PROFILE` (over-broad, refused, never trimmed);
    /// 6. the declared resource bounds are re-validated
    ///    ([`ResourceLimits::from_declared_bounds`]) — the host enforces
    ///    the DECLARED bounds, never the component's belief (validated
    ///    BEFORE the broker derivation so no grant can outlive a refused
    ///    request);
    /// 7. the broker's scope narrowing ([`CapabilityBroker::derive_grants`])
    ///    issues the scoped grants — any broker refusal surfaces under its
    ///    broker name (atomic batch, nothing half-issued);
    /// 8. the decision is audited (allow, with the exact granted scope) and
    ///    the capability set is frozen onto the verdict
    ///    ([`EnforcementAuditKind::CapabilitySetFrozen`]). If the allow's
    ///    own audit write fails, the just-issued grants are REVOKED (the
    ///    safe direction; the revocation stands even if its audit write
    ///    fails) — unaudited authority is not authority, and no grant can
    ///    outlive a refused verdict.
    ///
    /// NO component code has run at any point of this pipeline: every
    /// refusal above happens BEFORE instantiation, at the door.
    pub fn evaluate(
        &self,
        input: &InstantiationInput<'_>,
        now_unix_ms: u64,
    ) -> Result<EnforcementVerdict, EnforcementError> {
        let request = input.request;
        let ceiling = input.ceiling;

        // 1. Identity.
        if request.plugin_id.trim().is_empty() {
            return self.refused(
                now_unix_ms,
                request.plugin_id.as_str(),
                "",
                EnforcementError::PluginIdInvalid {
                    reason: "an enforcement decision must name its subject plugin".to_string(),
                },
            );
        }
        if request.manifest_id.trim().is_empty() || request.manifest_version.trim().is_empty() {
            return self.refused(
                now_unix_ms,
                request.plugin_id.as_str(),
                "",
                EnforcementError::RequestInvalid {
                    reason:
                        "the manifest id and version must be non-empty (the derivation trail)"
                            .to_string(),
                },
            );
        }
        if request.entry_symbol.trim().is_empty() {
            return self.refused(
                now_unix_ms,
                request.plugin_id.as_str(),
                "",
                EnforcementError::RequestInvalid {
                    reason: "the entry-point symbol must be non-empty".to_string(),
                },
            );
        }

        // 2. Vocabulary alignment (the drift-refusal rule). Duplicates are
        //    unalignable shape too — the kernel schema's unknown-field
        //    rejection makes duplicate keys unreachable in a parsed
        //    manifest, so a duplicate here is a caller-shape defect and is
        //    refused, not deduplicated.
        let mut seen_fields: Vec<&str> = Vec::new();
        for field in &request.declared_resource_fields {
            if !RESOURCE_FIELD_VOCABULARY.contains(&field.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    "",
                    EnforcementError::PermissionVocabularyDrift {
                        field: field.clone(),
                        reason: format!(
                            "resource field '{field}' is not in the host's mirror of the kernel \
                             manifest vocabulary (expected one of {RESOURCE_FIELD_VOCABULARY:?}); \
                             unalignable fields are refused, never given a default interpretation"
                        ),
                    },
                );
            }
            if seen_fields.contains(&field.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    "",
                    EnforcementError::PermissionVocabularyDrift {
                        field: field.clone(),
                        reason: format!(
                            "resource field '{field}' appears twice in the declared-field set \
                             (unalignable request shape; the kernel schema cannot express it)"
                        ),
                    },
                );
            }
            seen_fields.push(field.as_str());
        }

        // 3. The profile ceiling is structurally sound.
        let mut seen_ceiling: Vec<&str> = Vec::new();
        for entry in &ceiling.entries {
            if NEVER_GRANTED_PERMISSIONS.contains(&entry.capability.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    entry.capability.as_str(),
                    EnforcementError::PermissionNeverGrantable {
                        value: entry.capability.clone(),
                    },
                );
            }
            if CapabilityClass::parse(&entry.capability).is_none() {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    entry.capability.as_str(),
                    EnforcementError::PermissionUnknown {
                        value: entry.capability.clone(),
                    },
                );
            }
            if seen_ceiling.contains(&entry.capability.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    entry.capability.as_str(),
                    EnforcementError::ProfileInvalid {
                        reason: format!(
                            "capability '{}' appears twice in the profile ceiling",
                            entry.capability
                        ),
                    },
                );
            }
            seen_ceiling.push(entry.capability.as_str());
        }

        // 4. Every declared permission re-checked against the closed
        //    vocabulary (the gate trusts no upstream validation).
        let mut declared: Vec<&str> = Vec::new();
        for permission in &request.manifest_permissions {
            if NEVER_GRANTED_PERMISSIONS.contains(&permission.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    permission.as_str(),
                    EnforcementError::PermissionNeverGrantable {
                        value: permission.clone(),
                    },
                );
            }
            if CapabilityClass::parse(permission).is_none() {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    permission.as_str(),
                    EnforcementError::PermissionUnknown {
                        value: permission.clone(),
                    },
                );
            }
            if declared.contains(&permission.as_str()) {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    permission.as_str(),
                    EnforcementError::PermissionDuplicateDeclaration {
                        value: permission.clone(),
                    },
                );
            }
            declared.push(permission.as_str());
        }

        // 5. The intersection: manifest permissions ∩ profile ceiling. A
        //    declared permission with no ceiling entry REFUSES the request
        //    (over-broad) instead of silently granting the rest.
        let mut grant_requests: Vec<GrantRequest> = Vec::with_capacity(declared.len());
        for permission in &declared {
            let Some(entry) = ceiling.entry_for(permission) else {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    permission,
                    EnforcementError::PermissionAboveProfile {
                        plugin_id: request.plugin_id.clone(),
                        capability: (*permission).to_string(),
                    },
                );
            };
            grant_requests.push(GrantRequest {
                capability: (*permission).to_string(),
                scope: entry.scope.clone(),
                expires_at_unix_ms: entry.expires_at_unix_ms,
            });
        }

        // 6. The declared bounds re-validated (the resource-bound lie is
        //    refused at the door; there is no default fuel budget). This
        //    happens BEFORE the broker derivation: no grant may outlive a
        //    refused request.
        let limits =
            match ResourceLimits::from_declared_bounds(&request.bounds, request.fuel_budget) {
                Ok(limits) => limits,
                Err(HostError::LimitsInvalid { field, reason }) => {
                    return self.refused(
                        now_unix_ms,
                        request.plugin_id.as_str(),
                        "",
                        EnforcementError::LimitsInvalid { field, reason },
                    );
                }
                Err(other) => {
                    return self.refused(
                        now_unix_ms,
                        request.plugin_id.as_str(),
                        "",
                        EnforcementError::HostRefusal { error: other },
                    );
                }
            };

        // 7. The broker's scope narrowing issues the scoped grants. Every
        //    broker refusal (scope invalid, expiry invalid, ...) surfaces
        //    under its broker name — atomic batch, nothing half-issued.
        let derivation = GrantDerivationRequest {
            plugin_id: request.plugin_id.clone(),
            manifest_id: request.manifest_id.clone(),
            manifest_version: request.manifest_version.clone(),
            manifest_permissions: request.manifest_permissions.clone(),
            requested_grants: grant_requests,
        };
        let receipts = match self.broker.derive_grants(&derivation, now_unix_ms) {
            Ok(receipts) => receipts,
            Err(error) => {
                return self.refused(
                    now_unix_ms,
                    request.plugin_id.as_str(),
                    "",
                    EnforcementError::BrokerRefusal { error },
                );
            }
        };

        // 8. Freeze + audit. The verdict owns CLONED grants (frozen at
        //    instantiation); the audit carries the EXACT granted scope. If
        //    the allow's own audit write fails, the just-issued grants are
        //    REVOKED — unaudited authority is not authority (audit-or-
        //    refuse, applied to the gate's own decision; the revocation
        //    stands even if its audit write fails — the broker's recorded
        //    asymmetry, broker.rs `revoke`).
        let verdict = EnforcementVerdict {
            plugin_id: request.plugin_id.clone(),
            manifest_id: request.manifest_id.clone(),
            manifest_version: request.manifest_version.clone(),
            world: request.world,
            entry_symbol: request.entry_symbol.clone(),
            grants: receipts.into_iter().map(|receipt| receipt.grant).collect(),
            limits,
            evaluated_at_unix_ms: now_unix_ms,
        };
        let allow_audit = self.audit_event(
            now_unix_ms,
            EnforcementAuditKind::InstantiationAllowed,
            request.plugin_id.as_str(),
            "",
            format!(
                "manifest {}:{} granted [{}] (world {}, symbol {})",
                request.manifest_id,
                request.manifest_version,
                verdict.describe_grants(),
                request.world,
                request.entry_symbol,
            ),
        );
        if let Err(audit_error) = allow_audit {
            self.rollback_grants(&verdict, now_unix_ms);
            return Err(audit_error);
        }
        let freeze_audit = self.audit_event(
            now_unix_ms,
            EnforcementAuditKind::CapabilitySetFrozen,
            request.plugin_id.as_str(),
            "",
            "capability set frozen at instantiation: a later manifest or profile change cannot \
             expand a running component; the only path to a different set is re-instantiation \
             through drain-before-replace (kernel lifecycle, ledger P7.07)"
                .to_string(),
        );
        if let Err(audit_error) = freeze_audit {
            self.rollback_grants(&verdict, now_unix_ms);
            return Err(audit_error);
        }
        Ok(verdict)
    }
}

// --- The only constructor path to a binding --------------------------------------

/// Instantiates a component artifact under an enforcement verdict — the
/// ONLY public constructor path to a binding in this crate. There is no API
/// that instantiates from raw manifest data: the host's raw binding helper
/// (`crate::host::bind_component`) is `pub(crate)` since P7.12 and reachable
/// ONLY through this function, which requires an [`EnforcementVerdict`] —
/// and a verdict can only come from [`EnforcementGate::evaluate`].
///
/// The default build returns the fail-closed `NoBindings` object (every use
/// is `BINDINGS_NOT_GENERATED`); the real wasmtime instantiation exists only
/// inside the `generated-bindings` feature gate — the enforcement verdict is
/// required in BOTH builds, so no build exists in which raw manifest data
/// reaches a binding.
pub fn instantiate(
    verdict: &EnforcementVerdict,
    artifact: &ComponentArtifact,
) -> Result<Box<dyn crate::bindings::PluginBinding>, EnforcementError> {
    bind_component(artifact, verdict.world, &verdict.entry_symbol, &verdict.limits)
        .map_err(|error| EnforcementError::HostRefusal { error })
}
