// P8.31 — Research-worker tenant isolation and scoped data access.
//         Ledger `ledger/phase-08-advanced-coordination/ledger_phase_8.md`
//         §P8.31; report trace §17.4 (cross-tenant leakage), §16.2
//         (multi-tenancy), §5.7; decision record:
//         `ledger/phase-08-advanced-coordination/31_tenant_isolation.md`.
//
// WHAT THIS IS: the RESEARCH-SIDE tenant-scope enforcement — the
// research-worker analogue of the kernel's host-stamped tenant scope
// (`crates/flexcore-kernel/src/tenant_scope.rs`, P7.39). Every research
// worker (ADMM/DMPC, GNN, MARL, ANDES, HELICS, portfolio analytics) is
// scoped to a tenant/site, and a worker CANNOT read or serve another
// tenant's data: training data, model artifacts, and inference inputs are
// all scoped (the ledger's step-1 rule, recorded verbatim in
// [`RESEARCH_WORKER_SCOPING_RULE`]).
//
// THE KERNEL ALIGNMENT (mirror by citation, NO kernel dependency — the
// recorded alignment seam of this crate, see `lib.rs` and `Cargo.toml`):
// this module MIRRORS the type shape and the enforcement rules of
// `crates/flexcore-kernel/src/tenant_scope.rs::TenantScope` —
//
// 1. HOST-STAMPED CONSTRUCTION. The scope's fields are private and
//    [`ResearchWorkerScope::from_authenticated_context`] is the ONLY
//    public constructor — a scope cannot be assembled from worker-supplied
//    request data (the §16.2 rule: "Separate tenant context from untrusted
//    request body fields; derive it from authenticated claims"). The host
//    stamps the scope onto a worker id ([`WorkerScopeStamp::stamp_by_host`])
//    with a scope-binding digest; a worker-echoed scope that differs from
//    the stamped one is the named refusal
//    [`TenantIsolationError::ScopeEchoMismatch`] — a worker cannot widen
//    its own scope.
// 2. ECHO-MISMATCH REFUSAL. [`WorkerScopeStamp::verify_worker_echo`]
//    requires the echoed scope to match the stamped scope EXACTLY
//    (tenant AND site); anything wider or different is refused.
// 3. SCOPED DATA ACCESS. The [`ScopedResearchCatalog`] is the research
//    input surface: telemetry, topology, training data, model artifacts,
//    and scenario artifacts are registered under (tenant, site, kind,
//    record id) keys, and a read is granted ONLY when the worker's
//    host-stamped scope COVERS the requested data scope
//    ([`ResearchWorkerScope::covers`] — the kernel's narrowing rule:
//    tenant exact, tenant-wide envelope covers any site in the tenant,
//    site-narrowed envelope covers only the same site). Every denial is
//    REFUSED and AUDITED (§16.2: "Audit cross-tenant access denials") —
//    the cross-tenant negative-test property the ledger's exit check
//    requires.
// 4. UNAPPROVED CROSS-TENANT MODEL SERVING REFUSED. A model artifact is
//    registered with its OWNING tenant scope; serving it to another
//    tenant requires a RECORDED approval in the append-only
//    [`ModelServingRegister`]. The serving path
//    ([`enforce_model_serving`]) refuses unapproved cross-tenant use with
//    the named refusal [`TenantIsolationError::ModelServingNotApproved`]
//    — the ledger's step-3 rule ("a GNN trained on tenant A's
//    topologies/data must not serve tenant B without a recorded approval").
//    HONEST RESIDUAL RISK (recorded, never hidden): an approval admits
//    cross-tenant SERVING but does not treat WEIGHT LEAKAGE — a model
//    trained on tenant A's data can leak tenant A's information through
//    its weights even when serving to tenant B is approved. That treatment
//    is the training-data/leakage-control obligation of P8.35 and the
//    evaluation leakage controls (P8.02 §4 held-out rules); it is NOT
//    solved by this module, and no claim here says otherwise.
//
// FAIL-CLOSED: unknown worker stamps, unverifiable stamp bindings,
// unknown data records, unknown data kinds, and unknown model artifacts
// are all NAMED refusals — never "treat as in-scope", never a silent pass.
// Scope error names ALIGN with the kernel's stable names where the refusal
// class is the same (`TENANT_SCOPE_MISMATCH` for data outside scope, per
// the kernel `TenantScopeError::ScopeOutsideEnvelope` posture) so audit
// queries span both paths.
//
// DETERMINISM: catalog and register iteration is `BTreeMap`-ordered;
// canonical forms are fixed-format strings; the stamp digest is FNV-1a 64
// (the repo's non-cryptographic integrity-checksum pattern — P7.36/P7.14
// honesty applies unchanged: NOT a security primitive; a production
// digest is a DEPLOYMENT REQUIREMENT). Timestamps come ONLY from the
// injected clock ([`crate::validation::HarnessClock`] — the harness's
// injected-clock port, reused here so the crate has ONE clock vocabulary);
// `SystemTime::now()` appears nowhere in this module.
//
// RLS RELATIONSHIP (recorded honestly; mirrors the kernel module header):
// research workers never touch database tables directly — all platform
// data access flows through the platform's tenant-scoped services where
// application authorization and PostgreSQL Row-Level Security (§16.2,
// [R25]; ADR-007) remain the platform's defence in depth. This module adds
// the WORKER-LEVEL guarantee only: the host-stamped scope is bound to the
// worker identity, travels with every request, and is cross-checked at the
// enforcement point. RLS is not re-implemented, weakened, or replaced.
//
// Control status: research only, simulation/shadow (ENTRY_NOTE boundary 1).
// Nothing here authorizes, issues, approves, or dispatches anything
// (ADR-010); the "approval" of the model-serving register is a
// DATA-ACCESS approval inside the research plane, never a platform
// command authorization. No safety, certification, or field-authority
// claim is made.

use std::collections::BTreeMap;
use std::fmt;

use crate::validation::HarnessClock;

// --- Honesty / seam statements -------------------------------------------------------

/// The §16.2 rule as applied to research workers (pinned by tests).
pub const TENANT_ISOLATION_STATEMENT: &str = "every research worker is scoped to a \
     HOST-STAMPED tenant/site scope derived from the authenticated context — never from \
     worker-supplied request data. A worker cannot widen its own scope (an echoed scope \
     that differs is a named refusal), every data read outside the stamped scope is \
     refused AND audited, and cross-tenant model serving is refused without a recorded \
     approval. No safety, certification, or field-authority claim is made.";

/// The kernel alignment seam (recorded — do not drift silently): this
/// module MIRRORS `crates/flexcore-kernel/src/tenant_scope.rs::TenantScope`
/// (host-stamped construction, echo-mismatch refusal, covers-narrowing,
/// audited enforcement) BY CITATION. This crate does not depend on
/// flexcore-kernel (composition-crate policy, `Cargo.toml`); if the kernel
/// shape changes, the mirror update is deliberate, fail-closed in both
/// directions until then — pinned by tests here.
pub const KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/tenant_scope.rs::TenantScope \
     (from_authenticated host-stamp, ScopeEchoMismatch, covers narrowing, audited \
     enforcement); kernel unchanged; no flexcore-kernel dependency";

/// The per-worker scoping rule (ledger P8.31 step 1, verbatim substance):
/// EVERY research worker family carries the same rule — the scoping rule
/// is per REQUEST, and training data, model artifacts, and inference
/// inputs are ALL scoped. Pinned by tests.
pub const RESEARCH_WORKER_SCOPING_RULE: &str = "each worker request carries a \
     host-stamped tenant/site scope; training data, model artifacts, and inference \
     inputs are all scoped. Families covered (closed set): admm_dmpc, gnn, marl, \
     andes, helics, portfolio_analytics.";

/// The closed worker-family set ([`ResearchWorkerFamily::parse`] accepts
/// nothing else).
pub const WORKER_FAMILIES: [&str; 6] = [
    "admm_dmpc",
    "gnn",
    "marl",
    "andes",
    "helics",
    "portfolio_analytics",
];

// --- ResearchWorkerFamily -------------------------------------------------------------

/// The closed set of research worker families (the ledger's step-1 list).
/// A family outside this set is refused — there is no "generic" worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResearchWorkerFamily {
    /// The ADMM/DMPC coordination provider (P8.07–P8.09).
    AdmmDmpc,
    /// The GNN warm-start/screening worker (P8.10–P8.12).
    Gnn,
    /// The MARL simulation/shadow worker (P8.13–P8.15).
    Marl,
    /// The ANDES dynamic integration worker (P8.16–P8.17).
    Andes,
    /// The HELICS co-simulation worker (P8.18).
    Helics,
    /// The portfolio-analytics reporting worker (P8.22).
    PortfolioAnalytics,
}

impl ResearchWorkerFamily {
    /// The exact family string (closed vocabulary; the as_str/parse
    /// discipline of `validation::ResearchResultKind`).
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchWorkerFamily::AdmmDmpc => "admm_dmpc",
            ResearchWorkerFamily::Gnn => "gnn",
            ResearchWorkerFamily::Marl => "marl",
            ResearchWorkerFamily::Andes => "andes",
            ResearchWorkerFamily::Helics => "helics",
            ResearchWorkerFamily::PortfolioAnalytics => "portfolio_analytics",
        }
    }

    /// Parses the exact family string. The vocabulary is CLOSED.
    pub fn parse(text: &str) -> Option<ResearchWorkerFamily> {
        let family = match text {
            "admm_dmpc" => ResearchWorkerFamily::AdmmDmpc,
            "gnn" => ResearchWorkerFamily::Gnn,
            "marl" => ResearchWorkerFamily::Marl,
            "andes" => ResearchWorkerFamily::Andes,
            "helics" => ResearchWorkerFamily::Helics,
            "portfolio_analytics" => ResearchWorkerFamily::PortfolioAnalytics,
            _ => return None,
        };
        Some(family)
    }
}

// --- ResearchWorkerScope ---------------------------------------------------------------

/// The HOST-STAMPED research-worker tenant scope: tenant id + optional
/// site narrowing. Fields are private;
/// [`ResearchWorkerScope::from_authenticated_context`] is the ONLY public
/// constructor (the kernel `TenantScope::from_authenticated` pattern,
/// mirrored). There is no way to build a scope from request data — that
/// is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchWorkerScope {
    tenant_id: String,
    site_id: Option<String>,
}

/// Hard identity cap mirroring the kernel's 256-character id bounds
/// (validation.rs `MAX_ID_CHARS` posture; the same value).
const MAX_SCOPE_ID_CHARS: usize = 256;

impl ResearchWorkerScope {
    /// The ONLY construction path: from the platform-authenticated
    /// context. The tenant is REQUIRED and non-empty; a site narrowing,
    /// when present, must be non-empty (use `None`, not an empty string,
    /// for a tenant-wide scope).
    pub fn from_authenticated_context(
        tenant_id: &str,
        site_id: Option<&str>,
    ) -> Result<ResearchWorkerScope, TenantIsolationError> {
        if tenant_id.trim().is_empty() {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "the tenant id is empty — the scope is derived from the \
                         authenticated context and must name a tenant"
                    .to_string(),
            });
        }
        if tenant_id.len() > MAX_SCOPE_ID_CHARS {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "the tenant id exceeds 256 characters".to_string(),
            });
        }
        let site_id = match site_id {
            None => None,
            Some(site) => {
                if site.trim().is_empty() {
                    return Err(TenantIsolationError::ScopeInvalid {
                        reason: "a site narrowing must be a non-empty string (use None, not \
                                 an empty string, for a tenant-wide scope)"
                            .to_string(),
                    });
                }
                if site.len() > MAX_SCOPE_ID_CHARS {
                    return Err(TenantIsolationError::ScopeInvalid {
                        reason: "the site id exceeds 256 characters".to_string(),
                    });
                }
                Some(site.to_string())
            }
        };
        Ok(ResearchWorkerScope { tenant_id: tenant_id.to_string(), site_id })
    }

    /// The tenant the scope is stamped with.
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// The site narrowing, if any (`None` = tenant-wide).
    pub fn site_id(&self) -> Option<&str> {
        self.site_id.as_deref()
    }

    /// Whether this scope COVERS `requested` — the kernel narrowing rule,
    /// mirrored exactly (`tenant_scope.rs::TenantScope::covers`):
    ///
    /// - the tenant must match exactly (cross-tenant is never covered);
    /// - a tenant-wide envelope (`site_id() == None`) covers any site
    ///   request in the same tenant;
    /// - a site-narrowed envelope covers only the SAME site.
    pub fn covers(&self, requested: &ResearchWorkerScope) -> bool {
        if self.tenant_id != requested.tenant_id {
            return false;
        }
        match (&self.site_id, &requested.site_id) {
            (None, _) => true,
            (Some(g), Some(w)) => g == w,
            (Some(_), None) => false,
        }
    }

    /// The deterministic canonical form (fixed field order; the stamp
    /// digest and the audit details bind this string).
    pub fn canonical(&self) -> String {
        match &self.site_id {
            Some(site) => format!("tenant:{}|site:{}", self.tenant_id, site),
            None => format!("tenant:{}|site:none", self.tenant_id),
        }
    }
}

impl fmt::Display for ResearchWorkerScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

// --- WorkerScopeStamp --------------------------------------------------------------------

/// The FNV-1a 64-bit hash (the repo's non-cryptographic integrity-checksum
/// pattern — P4.28/P6.14/P7.14/P7.36; deterministic, dependency-free,
/// explicitly NOT a security primitive; a production digest is a
/// DEPLOYMENT REQUIREMENT, same honesty as the kernel).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A HOST-STAMPED worker scope binding: the host stamps a (worker id,
/// family, scope) triple at worker registration; the stamp digest covers
/// the triple's canonical form so a scope swap or identity change after
/// stamping is the named refusal
/// ([`TenantIsolationError::WorkerStampBindingMismatch`]).
///
/// Only the host constructs a stamp ([`WorkerScopeStamp::stamp_by_host`]
/// takes the scope from the AUTHENTICATED context); a worker receives the
/// stamp and may echo it, never widen it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerScopeStamp {
    worker_id: String,
    family: ResearchWorkerFamily,
    scope: ResearchWorkerScope,
    stamp_digest_hex: String,
}

impl WorkerScopeStamp {
    /// The host's stamping operation: binds `worker_id` + `family` to the
    /// scope derived from the authenticated context. The worker id must be
    /// non-empty and at most 256 characters.
    pub fn stamp_by_host(
        worker_id: &str,
        family: ResearchWorkerFamily,
        scope: ResearchWorkerScope,
    ) -> Result<WorkerScopeStamp, TenantIsolationError> {
        if worker_id.trim().is_empty() {
            return Err(TenantIsolationError::WorkerStampInvalid {
                reason: "the worker id is empty — a stamp binds an identity".to_string(),
            });
        }
        if worker_id.len() > MAX_SCOPE_ID_CHARS {
            return Err(TenantIsolationError::WorkerStampInvalid {
                reason: "the worker id exceeds 256 characters".to_string(),
            });
        }
        let digest = format!(
            "{:016x}",
            fnv1a64(worker_stamp_input(worker_id, family, &scope).as_bytes())
        );
        Ok(WorkerScopeStamp {
            worker_id: worker_id.to_string(),
            family,
            scope,
            stamp_digest_hex: digest,
        })
    }

    /// The stamped worker id.
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// The stamped worker family.
    pub fn family(&self) -> ResearchWorkerFamily {
        self.family
    }

    /// The host-stamped scope (stamped by the platform; never by a
    /// worker).
    pub fn scope(&self) -> &ResearchWorkerScope {
        &self.scope
    }

    /// The recorded stamp digest (16 lowercase hex digits).
    pub fn stamp_digest_hex(&self) -> &str {
        &self.stamp_digest_hex
    }

    /// Recomputes the stamp binding over the CURRENT fields and compares
    /// it with the recorded digest — the stamp-verification step every
    /// enforcement path runs FIRST (fail-closed: an unverifiable stamp is
    /// a refusal, never a pass).
    pub fn verify_binding(&self) -> Result<(), TenantIsolationError> {
        let recomputed = format!(
            "{:016x}",
            fnv1a64(worker_stamp_input(&self.worker_id, self.family, &self.scope).as_bytes())
        );
        if recomputed == self.stamp_digest_hex {
            Ok(())
        } else {
            Err(TenantIsolationError::WorkerStampBindingMismatch)
        }
    }

    /// Verifies a worker-ECHOED scope: it must match the host-stamped
    /// scope EXACTLY (tenant AND site). A worker echoing a wider or
    /// different scope is the named cannot-widen refusal
    /// ([`TenantIsolationError::ScopeEchoMismatch`]; the kernel
    /// `ScopeEchoMismatch` posture, mirrored).
    pub fn verify_worker_echo(&self, echoed: &ResearchWorkerScope) -> Result<(), TenantIsolationError> {
        if *echoed == self.scope {
            Ok(())
        } else {
            Err(TenantIsolationError::ScopeEchoMismatch {
                stamped: self.scope.canonical(),
                echoed: echoed.canonical(),
            })
        }
    }
}

/// The stamp-binding input: fixed field order — worker id, family string,
/// scope canonical form (deterministic and mutation-sensitive).
fn worker_stamp_input(worker_id: &str, family: ResearchWorkerFamily, scope: &ResearchWorkerScope) -> String {
    format!("worker:{}|family:{}|scope:{}", worker_id, family.as_str(), scope.canonical())
}

// --- The scoped research catalog ---------------------------------------------------------

/// The closed set of research data kinds a worker can request (the ledger:
/// "training data, model artifacts, and inference inputs are all scoped" —
/// telemetry and topology are the inference inputs; scenarios are the
/// co-sim/dynamic artifacts). CLOSED: `parse` accepts nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResearchDataKind {
    /// Telemetry reads (the `telemetry.read`-class inputs).
    Telemetry,
    /// Topology/network-model reads.
    Topology,
    /// Training datasets (P8.35 owns provenance; the scope is enforced
    /// HERE).
    TrainingData,
    /// Model artifacts (the P8.32 signing seam applies at load; the scope
    /// is enforced HERE).
    ModelArtifact,
    /// Scenario/co-simulation artifacts (P8.18/P8.19).
    Scenario,
}

impl ResearchDataKind {
    /// The exact kind string.
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchDataKind::Telemetry => "telemetry",
            ResearchDataKind::Topology => "topology",
            ResearchDataKind::TrainingData => "training_data",
            ResearchDataKind::ModelArtifact => "model_artifact",
            ResearchDataKind::Scenario => "scenario",
        }
    }

    /// Parses the exact kind string. The vocabulary is CLOSED: an unknown
    /// kind is [`TenantIsolationError::DataKindUnknown`], never guessed.
    pub fn parse(text: &str) -> Option<ResearchDataKind> {
        let kind = match text {
            "telemetry" => ResearchDataKind::Telemetry,
            "topology" => ResearchDataKind::Topology,
            "training_data" => ResearchDataKind::TrainingData,
            "model_artifact" => ResearchDataKind::ModelArtifact,
            "scenario" => ResearchDataKind::Scenario,
            _ => return None,
        };
        Some(kind)
    }
}

/// One research data record registered in the catalog (host-side
/// registration; a record lives under exactly one (tenant, site) data
/// scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchDataRecord {
    /// The closed data kind.
    pub kind: ResearchDataKind,
    /// The tenant the data belongs to.
    pub tenant_id: String,
    /// The site the data belongs to (`None` = tenant-wide data).
    pub site_id: Option<String>,
    /// The record id (unique within (tenant, site, kind)).
    pub record_id: String,
    /// A non-secret descriptor (reference, version tag — never payload
    /// data, never a secret value).
    pub descriptor: String,
}

impl ResearchDataRecord {
    /// The catalog's canonical key (BTreeMap key; deterministic fixed
    /// field order). `None` site renders as `site:none` (the scope
    /// canonical discipline).
    pub fn catalog_key(&self) -> String {
        format!(
            "tenant:{}|site:{}|kind:{}|record:{}",
            self.tenant_id,
            self.site_id.as_deref().unwrap_or("none"),
            self.kind.as_str(),
            self.record_id
        )
    }
}

/// The scoped research data catalog: the research input surface. The host
/// registers records ([`ScopedResearchCatalog::register`]); a worker read
/// ([`ScopedResearchCatalog::enforce_scoped_read`]) is granted ONLY when
/// the worker's host-stamped scope covers the record's data scope.
/// Iteration is `BTreeMap`-ordered (deterministic).
#[derive(Debug, Default)]
pub struct ScopedResearchCatalog {
    records: BTreeMap<String, ResearchDataRecord>,
}

impl ScopedResearchCatalog {
    pub fn new() -> ScopedResearchCatalog {
        ScopedResearchCatalog { records: BTreeMap::new() }
    }

    /// Registers one data record (host-side). A duplicate key is refused
    /// (the catalog is deterministic: exactly one record per key), and the
    /// record must be structurally valid (non-empty record id, non-empty
    /// tenant, non-empty descriptor).
    pub fn register(&mut self, record: ResearchDataRecord) -> Result<(), TenantIsolationError> {
        if record.tenant_id.trim().is_empty() {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "a catalog record must name a tenant".to_string(),
            });
        }
        if record.record_id.trim().is_empty() {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "a catalog record must have a non-empty record id".to_string(),
            });
        }
        if record.descriptor.trim().is_empty() {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "a catalog record must have a non-empty descriptor".to_string(),
            });
        }
        let key = record.catalog_key();
        if self.records.contains_key(&key) {
            return Err(TenantIsolationError::DataRecordDuplicate { key });
        }
        self.records.insert(key, record);
        Ok(())
    }

    /// The number of registered records (diagnostics).
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the catalog is empty (diagnostics).
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Pure enforcement: can `stamp` read the record named by
    /// (kind, tenant, site, record_id)? Order (each step a distinct named
    /// refusal; the first failure wins):
    ///
    /// 1. the stamp binding is re-verified (an unverifiable stamp is
    ///    refused before ANY data question is answered);
    /// 2. the record must EXIST (unknown is refused — never "treat as
    ///    in-scope", never "treat as out-of-scope" for an unknown id);
    /// 3. the stamped scope must COVER the record's data scope.
    ///
    /// A narrowing is preserved: a site-scoped read inside a tenant-wide
    /// stamp, or an equal-site read, passes; anything wider or different
    /// is [`TenantIsolationError::DataOutsideScope`] (stable name
    /// `TENANT_SCOPE_MISMATCH` — aligned with the kernel's
    /// `ScopeOutsideEnvelope` name so audit queries span both paths).
    pub fn enforce_scoped_read(
        &self,
        stamp: &WorkerScopeStamp,
        kind: ResearchDataKind,
        tenant_id: &str,
        site_id: Option<&str>,
        record_id: &str,
    ) -> Result<&ResearchDataRecord, TenantIsolationError> {
        stamp.verify_binding()?;
        if tenant_id.trim().is_empty() {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "a data read must name a tenant (fail-closed: an unnamed tenant is \
                         never in-scope)"
                    .to_string(),
            });
        }
        if site_id.map(|s| s.trim().is_empty()).unwrap_or(false) {
            return Err(TenantIsolationError::ScopeInvalid {
                reason: "a site narrowing must be a non-empty string (use None for a \
                         tenant-wide read)"
                    .to_string(),
            });
        }
        let key = format!(
            "tenant:{}|site:{}|kind:{}|record:{}",
            tenant_id,
            site_id.unwrap_or("none"),
            kind.as_str(),
            record_id
        );
        let record = self
            .records
            .get(&key)
            .ok_or_else(|| TenantIsolationError::DataRecordUnknown { key: key.clone() })?;
        let requested = ResearchWorkerScope {
            tenant_id: tenant_id.to_string(),
            site_id: site_id.map(|s| s.to_string()),
        };
        if stamp.scope().covers(&requested) {
            Ok(record)
        } else {
            Err(TenantIsolationError::DataOutsideScope {
                stamped: stamp.scope().canonical(),
                requested: requested.canonical(),
            })
        }
    }

    /// The audited read path ([`enforce_scoped_read`] plus an audit record
    /// for EVERY outcome — §16.2 "Audit cross-tenant access denials":
    /// denials are audited, admissions are audited too). Timestamps come
    /// ONLY from the injected clock.
    pub fn scoped_read_audited(
        &self,
        stamp: &WorkerScopeStamp,
        kind: ResearchDataKind,
        tenant_id: &str,
        site_id: Option<&str>,
        record_id: &str,
        clock: &dyn HarnessClock,
        audit: &dyn TenantIsolationAuditSink,
    ) -> Result<&ResearchDataRecord, TenantIsolationError> {
        let outcome = self.enforce_scoped_read(stamp, kind, tenant_id, site_id, record_id);
        let (audit_kind, detail) = match &outcome {
            Ok(record) => (
                TenantIsolationAuditKind::ScopeReadAdmitted,
                format!(
                    "worker '{}' ({}) read {} within stamped scope {} (binding verified)",
                    stamp.worker_id(),
                    stamp.family().as_str(),
                    record.catalog_key(),
                    stamp.scope().canonical()
                ),
            ),
            Err(err) => (
                TenantIsolationAuditKind::ScopeReadRefused,
                format!(
                    "worker '{}' ({}) read refused ({}): {err}",
                    stamp.worker_id(),
                    stamp.family().as_str(),
                    err.name()
                ),
            ),
        };
        audit.record(TenantIsolationAuditRecord {
            timestamp_millis: clock.now_millis(),
            kind: audit_kind,
            worker_id: stamp.worker_id().to_string(),
            detail,
        });
        outcome
    }
}

// --- The model-serving register (cross-tenant transfer control) ----------

/// One model-artifact registration: the artifact's OWNER scope (the
/// tenant/site whose data trained it — the P8.35 provenance record is the
/// training-data reference; this registration carries only the REFERENCE,
/// never the record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactRegistration {
    /// The artifact reference (the P8.32 signature binds to this
    /// reference; signing itself is P8.32's obligation, not this module's).
    pub artifact_ref: String,
    /// The owning worker family (which research line produced it).
    pub owner_family: ResearchWorkerFamily,
    /// The owner scope (the tenant/site whose data trained the model).
    pub owner_scope: ResearchWorkerScope,
    /// The training-data reference (P8.35 owns the full record; carried
    /// here as the reference only).
    pub training_data_ref: String,
}

/// One RECORDED cross-tenant serving approval (append-only; the ledger's
/// "without a recorded approval" is enforced structurally — there is no
/// approval that is not a record here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelServingApproval {
    /// The artifact reference the approval covers.
    pub artifact_ref: String,
    /// The serving scope granted (the tenant/site that may receive it).
    pub serving_scope: ResearchWorkerScope,
    /// Why the approval was granted (decision record reference — REQUIRED
    /// non-empty; an unattributed approval is refused, never recorded).
    pub reason: String,
    /// Who granted it (REQUIRED non-empty).
    pub actor: String,
    /// When (injected-clock millis).
    pub approved_at_millis: i64,
}

impl ModelServingApproval {
    /// The register's canonical approval key (BTreeMap key;
    /// deterministic).
    pub fn approval_key(&self) -> String {
        format!(
            "artifact:{}|serve:{}",
            self.artifact_ref,
            self.serving_scope.canonical()
        )
    }
}

/// The append-only model-serving register. There is NO revoke and NO
/// edit: an approval withdrawal is a NEW record type and a new policy
/// decision (the kernel `UNREVOCATION_SEAM` honesty, mirrored — the only
/// direction implemented here is the safe one: approvals can be ADDED,
/// never removed, so a recorded approval cannot silently disappear; the
/// containment path for a bad approval is revoking the WORKER stamp /
/// artifact, P8.32's and the host's obligation).
#[derive(Debug, Default)]
pub struct ModelServingRegister {
    artifacts: BTreeMap<String, ModelArtifactRegistration>,
    approvals: BTreeMap<String, ModelServingApproval>,
    append_order: Vec<ModelServingApproval>,
}

impl ModelServingRegister {
    pub fn new() -> ModelServingRegister {
        ModelServingRegister {
            artifacts: BTreeMap::new(),
            approvals: BTreeMap::new(),
            append_order: Vec::new(),
        }
    }

    /// Registers a model artifact with its OWNER scope (host-side). A
    /// duplicate artifact reference is refused.
    pub fn register_artifact(
        &mut self,
        registration: ModelArtifactRegistration,
    ) -> Result<(), TenantIsolationError> {
        if registration.artifact_ref.trim().is_empty() {
            return Err(TenantIsolationError::ModelArtifactInvalid {
                reason: "the artifact reference is empty".to_string(),
            });
        }
        if registration.training_data_ref.trim().is_empty() {
            return Err(TenantIsolationError::ModelArtifactInvalid {
                reason: "the training-data reference is empty (provenance is never \
                         optional — P8.35)"
                    .to_string(),
            });
        }
        if self.artifacts.contains_key(&registration.artifact_ref) {
            return Err(TenantIsolationError::ModelArtifactDuplicate {
                artifact_ref: registration.artifact_ref.clone(),
            });
        }
        self.artifacts.insert(registration.artifact_ref.clone(), registration);
        Ok(())
    }

    /// The registration for an artifact reference, if any.
    pub fn artifact(&self, artifact_ref: &str) -> Option<&ModelArtifactRegistration> {
        self.artifacts.get(artifact_ref)
    }

    /// Records one cross-tenant serving approval (APPEND-ONLY; a
    /// duplicate (artifact, serving scope) approval is refused — exactly
    /// one record per key). The reason and actor are REQUIRED. This call
    /// is the ONLY way an approval exists; there is no approval flag a
    /// caller could set inline.
    pub fn record_approval(
        &mut self,
        approval: ModelServingApproval,
        clock: &dyn HarnessClock,
        audit: &dyn TenantIsolationAuditSink,
    ) -> Result<(), TenantIsolationError> {
        let outcome = self.record_approval_inner(approval, clock);
        match &outcome {
            Ok(key) => audit.record(TenantIsolationAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: TenantIsolationAuditKind::ServingApprovalRecorded,
                worker_id: "(host)".to_string(),
                detail: format!(
                    "cross-tenant serving approval recorded: {key} — reason: {}",
                    self.approvals.get(key).map(|a| a.reason.as_str()).unwrap_or("")
                ),
            }),
            Err(err) => audit.record(TenantIsolationAuditRecord {
                timestamp_millis: clock.now_millis(),
                kind: TenantIsolationAuditKind::ServingApprovalRefused,
                worker_id: "(host)".to_string(),
                detail: format!("approval refused ({}): {err}", err.name()),
            }),
        }
        outcome.map(|_| ())
    }

    fn record_approval_inner(
        &mut self,
        mut approval: ModelServingApproval,
        clock: &dyn HarnessClock,
    ) -> Result<String, TenantIsolationError> {
        if approval.artifact_ref.trim().is_empty() {
            return Err(TenantIsolationError::ModelArtifactInvalid {
                reason: "the approval's artifact reference is empty".to_string(),
            });
        }
        if !self.artifacts.contains_key(&approval.artifact_ref) {
            return Err(TenantIsolationError::ModelArtifactUnknown {
                artifact_ref: approval.artifact_ref.clone(),
            });
        }
        if approval.reason.trim().is_empty() {
            return Err(TenantIsolationError::ApprovalReasonRequired);
        }
        if approval.actor.trim().is_empty() {
            return Err(TenantIsolationError::ApprovalActorRequired);
        }
        approval.approved_at_millis = clock.now_millis();
        let key = approval.approval_key();
        if self.approvals.contains_key(&key) {
            return Err(TenantIsolationError::ApprovalAlreadyRecorded { key: key.clone() });
        }
        self.approvals.insert(key.clone(), approval.clone());
        self.append_order.push(approval);
        Ok(key)
    }

    /// The approvals in append order (audit/replay view).
    pub fn approvals_in_append_order(&self) -> &[ModelServingApproval] {
        &self.append_order
    }

    /// Pure serving enforcement: may `stamp` serve `artifact_ref` to
    /// `serving_scope`? Order (each step a distinct named refusal):
    ///
    /// 1. the stamp binding is re-verified;
    /// 2. the artifact must be registered (unknown is refused);
    /// 3. the worker's stamped scope must cover the REQUESTED serving
    ///    scope (a worker cannot serve into a scope it does not hold);
    /// 4. if the serving scope is WITHIN the artifact's owner scope, the
    ///    serving is in-tenant and admitted;
    /// 5. otherwise (cross-tenant, or a different site under a narrowed
    ///    owner scope) a RECORDED approval for exactly
    ///    (artifact, serving scope) is REQUIRED — no approval is
    ///    [`TenantIsolationError::ModelServingNotApproved`].
    pub fn enforce_model_serving(
        &self,
        stamp: &WorkerScopeStamp,
        artifact_ref: &str,
        serving_scope: &ResearchWorkerScope,
    ) -> Result<(), TenantIsolationError> {
        stamp.verify_binding()?;
        let registration = self
            .artifacts
            .get(artifact_ref)
            .ok_or_else(|| TenantIsolationError::ModelArtifactUnknown {
                artifact_ref: artifact_ref.to_string(),
            })?;
        if !stamp.scope().covers(serving_scope) {
            return Err(TenantIsolationError::DataOutsideScope {
                stamped: stamp.scope().canonical(),
                requested: serving_scope.canonical(),
            });
        }
        if registration.owner_scope.covers(serving_scope) {
            return Ok(());
        }
        let key = format!(
            "artifact:{}|serve:{}",
            artifact_ref,
            serving_scope.canonical()
        );
        if self.approvals.contains_key(&key) {
            Ok(())
        } else {
            Err(TenantIsolationError::ModelServingNotApproved {
                artifact_ref: artifact_ref.to_string(),
                owner: registration.owner_scope.canonical(),
                requested: serving_scope.canonical(),
            })
        }
    }

    /// The audited serving path ([`enforce_model_serving`] plus an audit
    /// record for EVERY outcome).
    pub fn model_serving_audited(
        &self,
        stamp: &WorkerScopeStamp,
        artifact_ref: &str,
        serving_scope: &ResearchWorkerScope,
        clock: &dyn HarnessClock,
        audit: &dyn TenantIsolationAuditSink,
    ) -> Result<(), TenantIsolationError> {
        let outcome = self.enforce_model_serving(stamp, artifact_ref, serving_scope);
        let (audit_kind, detail) = match &outcome {
            Ok(()) => (
                TenantIsolationAuditKind::ModelServingAdmitted,
                format!(
                    "worker '{}' ({}) may serve artifact '{artifact_ref}' to {} (owner {})",
                    stamp.worker_id(),
                    stamp.family().as_str(),
                    serving_scope.canonical(),
                    self.artifact(artifact_ref)
                        .map(|r| r.owner_scope.canonical())
                        .unwrap_or("(unknown)")
                ),
            ),
            Err(err) => (
                TenantIsolationAuditKind::ModelServingRefused,
                format!(
                    "worker '{}' ({}) serving of '{artifact_ref}' to {} refused ({}): {err}",
                    stamp.worker_id(),
                    stamp.family().as_str(),
                    serving_scope.canonical(),
                    err.name()
                ),
            ),
        };
        audit.record(TenantIsolationAuditRecord {
            timestamp_millis: clock.now_millis(),
            kind: audit_kind,
            worker_id: stamp.worker_id().to_string(),
            detail,
        });
        outcome
    }
}

// --- Errors --------------------------------------------------------------------------

/// Every tenant-isolation refusal is a DISTINCT named error
/// ([`TenantIsolationError::name`]) — never a warning, never a silent
/// acceptance, never a bare boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantIsolationError {
    /// A scope or record is structurally unusable (empty tenant, empty
    /// site string, empty record id).
    ScopeInvalid { reason: String },
    /// A worker stamp could not be created (empty/oversized worker id).
    WorkerStampInvalid { reason: String },
    /// The stamp's binding digest does not recompute — the stamp was
    /// forged or mutated after stamping.
    WorkerStampBindingMismatch,
    /// A worker-echoed scope differs from the host-stamped one — the
    /// cannot-widen-your-own-scope refusal.
    ScopeEchoMismatch { stamped: String, echoed: String },
    /// The requested data is outside the host-stamped scope — the
    /// TENANT_SCOPE_MISMATCH refusal (name-aligned with the kernel's
    /// `ScopeOutsideEnvelope`).
    DataOutsideScope { stamped: String, requested: String },
    /// The requested data record does not exist in the catalog (unknown
    /// is never in-scope — fail-closed).
    DataRecordUnknown { key: String },
    /// A registration attempted to reuse an existing catalog key.
    DataRecordDuplicate { key: String },
    /// The data kind is not in the closed vocabulary (never guessed).
    DataKindUnknown { attempted: String },
    /// A model artifact registration is structurally invalid.
    ModelArtifactInvalid { reason: String },
    /// The artifact reference is not registered (unknown is never
    /// servable — fail-closed).
    ModelArtifactUnknown { artifact_ref: String },
    /// The artifact reference is already registered.
    ModelArtifactDuplicate { artifact_ref: String },
    /// Cross-tenant (or cross-site) model serving without a RECORDED
    /// approval — the ledger's step-3 refusal.
    ModelServingNotApproved { artifact_ref: String, owner: String, requested: String },
    /// A serving approval requires a non-empty reason.
    ApprovalReasonRequired,
    /// A serving approval requires a non-empty actor.
    ApprovalActorRequired,
    /// The (artifact, serving scope) approval is already recorded (the
    /// register is append-only; exactly one record per key).
    ApprovalAlreadyRecorded { key: String },
}

impl TenantIsolationError {
    /// The stable machine-readable error name (the named-error table key;
    /// pinned by tests).
    pub fn name(&self) -> &'static str {
        match self {
            TenantIsolationError::ScopeInvalid { .. } => "RESEARCH_SCOPE_INVALID",
            TenantIsolationError::WorkerStampInvalid { .. } => "RESEARCH_WORKER_STAMP_INVALID",
            TenantIsolationError::WorkerStampBindingMismatch => {
                "RESEARCH_WORKER_STAMP_BINDING_MISMATCH"
            }
            TenantIsolationError::ScopeEchoMismatch { .. } => "TENANT_SCOPE_ECHO_MISMATCH",
            TenantIsolationError::DataOutsideScope { .. } => "TENANT_SCOPE_MISMATCH",
            TenantIsolationError::DataRecordUnknown { .. } => "RESEARCH_DATA_RECORD_UNKNOWN",
            TenantIsolationError::DataRecordDuplicate { .. } => "RESEARCH_DATA_RECORD_DUPLICATE",
            TenantIsolationError::DataKindUnknown { .. } => "RESEARCH_DATA_KIND_UNKNOWN",
            TenantIsolationError::ModelArtifactInvalid { .. } => "RESEARCH_MODEL_ARTIFACT_INVALID",
            TenantIsolationError::ModelArtifactUnknown { .. } => "RESEARCH_MODEL_ARTIFACT_UNKNOWN",
            TenantIsolationError::ModelArtifactDuplicate { .. } => "RESEARCH_MODEL_ARTIFACT_DUPLICATE",
            TenantIsolationError::ModelServingNotApproved { .. } => {
                "RESEARCH_MODEL_SERVING_NOT_APPROVED"
            }
            TenantIsolationError::ApprovalReasonRequired => "RESEARCH_APPROVAL_REASON_REQUIRED",
            TenantIsolationError::ApprovalActorRequired => "RESEARCH_APPROVAL_ACTOR_REQUIRED",
            TenantIsolationError::ApprovalAlreadyRecorded { .. } => {
                "RESEARCH_APPROVAL_ALREADY_RECORDED"
            }
        }
    }
}

impl fmt::Display for TenantIsolationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TenantIsolationError::ScopeInvalid { reason } => {
                write!(f, "the research worker scope is invalid: {reason}")
            }
            TenantIsolationError::WorkerStampInvalid { reason } => {
                write!(f, "the worker scope stamp is invalid: {reason}")
            }
            TenantIsolationError::WorkerStampBindingMismatch => write!(
                f,
                "the worker stamp binding does not recompute — the stamp was forged or \
                 mutated after stamping; refused"
            ),
            TenantIsolationError::ScopeEchoMismatch { stamped, echoed } => write!(
                f,
                "the worker echoed scope '{echoed}' but the host-stamped scope is \
                 '{stamped}' — a worker cannot widen its own scope (report §16.2); refused"
            ),
            TenantIsolationError::DataOutsideScope { stamped, requested } => write!(
                f,
                "the requested data scope '{requested}' is outside the host-stamped worker \
                 scope '{stamped}' (TENANT_SCOPE_MISMATCH — cross-tenant access denied and \
                 audited, report §17.4/§16.2)"
            ),
            TenantIsolationError::DataRecordUnknown { key } => write!(
                f,
                "the requested data record '{key}' is not in the catalog; unknown is never \
                 in-scope (fail-closed)"
            ),
            TenantIsolationError::DataRecordDuplicate { key } => write!(
                f,
                "a catalog record with key '{key}' is already registered (exactly one \
                 record per key)"
            ),
            TenantIsolationError::DataKindUnknown { attempted } => write!(
                f,
                "data kind {attempted:?} is not in the closed research data vocabulary; \
                 refused"
            ),
            TenantIsolationError::ModelArtifactInvalid { reason } => {
                write!(f, "the model artifact registration is invalid: {reason}")
            }
            TenantIsolationError::ModelArtifactUnknown { artifact_ref } => write!(
                f,
                "the model artifact '{artifact_ref}' is not registered; unknown is never \
                 servable (fail-closed)"
            ),
            TenantIsolationError::ModelArtifactDuplicate { artifact_ref } => write!(
                f,
                "the model artifact '{artifact_ref}' is already registered"
            ),
            TenantIsolationError::ModelServingNotApproved { artifact_ref, owner, requested } => {
                write!(
                    f,
                    "serving model artifact '{artifact_ref}' (owner {owner}) to {requested} \
                     has NO recorded approval — cross-tenant model transfer is refused \
                     (ledger P8.31 step 3)"
                )
            }
            TenantIsolationError::ApprovalReasonRequired => write!(
                f,
                "a cross-tenant serving approval requires a non-empty reason (an \
                 unattributed approval is never recorded)"
            ),
            TenantIsolationError::ApprovalActorRequired => write!(
                f,
                "a cross-tenant serving approval requires a non-empty actor"
            ),
            TenantIsolationError::ApprovalAlreadyRecorded { key } => write!(
                f,
                "the serving approval '{key}' is already recorded (the register is \
                 append-only; exactly one record per key)"
            ),
        }
    }
}

impl std::error::Error for TenantIsolationError {}

// --- The audit port ---------------------------------------------------------------------

/// The kinds of tenant-isolation audit records (SCREAMING_SNAKE stable
/// style; §16.2 "Audit cross-tenant access denials" — and admissions are
/// audited too, so the audit trail shows every scoped access).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantIsolationAuditKind {
    /// A scoped data read was admitted (scope + binding verified).
    ScopeReadAdmitted,
    /// A scoped data read was REFUSED (cross-tenant, unknown record,
    /// binding mismatch — the detail names the refusal class).
    ScopeReadRefused,
    /// A model serving was admitted (in-tenant or approved).
    ModelServingAdmitted,
    /// A model serving was REFUSED (unapproved cross-tenant, unknown
    /// artifact — the detail names the refusal class).
    ModelServingRefused,
    /// A cross-tenant serving approval was recorded.
    ServingApprovalRecorded,
    /// A serving approval was refused (duplicate, missing reason/actor,
    /// unknown artifact).
    ServingApprovalRefused,
}

impl TenantIsolationAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            TenantIsolationAuditKind::ScopeReadAdmitted => "RESEARCH_SCOPE_READ_ADMITTED",
            TenantIsolationAuditKind::ScopeReadRefused => "RESEARCH_SCOPE_READ_REFUSED",
            TenantIsolationAuditKind::ModelServingAdmitted => "RESEARCH_MODEL_SERVING_ADMITTED",
            TenantIsolationAuditKind::ModelServingRefused => "RESEARCH_MODEL_SERVING_REFUSED",
            TenantIsolationAuditKind::ServingApprovalRecorded => {
                "RESEARCH_MODEL_SERVING_APPROVAL_RECORDED"
            }
            TenantIsolationAuditKind::ServingApprovalRefused => {
                "RESEARCH_MODEL_SERVING_APPROVAL_REFUSED"
            }
        }
    }
}

/// One tenant-isolation audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantIsolationAuditRecord {
    /// When the event was recorded (injected-clock millis).
    pub timestamp_millis: i64,
    pub kind: TenantIsolationAuditKind,
    /// The worker the event concerns (or "(host)" for host operations).
    pub worker_id: String,
    /// Human-readable detail (scope renderings and refusal names — no
    /// payload data, no secret material).
    pub detail: String,
}

/// The tenant-isolation audit sink port (crate-local, the kernel
/// `TenantScopeAuditSink` pattern mirrored). Implementations MUST be
/// append-only (§5.1 audit integrity).
pub trait TenantIsolationAuditSink {
    fn record(&self, record: TenantIsolationAuditRecord);
}

/// An append-only in-memory [`TenantIsolationAuditSink`] — the
/// deterministic collector used by tests and in-process compositions.
#[derive(Debug, Default)]
pub struct MemoryTenantIsolationAuditSink {
    records: std::cell::RefCell<Vec<TenantIsolationAuditRecord>>,
}

impl MemoryTenantIsolationAuditSink {
    pub fn new() -> MemoryTenantIsolationAuditSink {
        MemoryTenantIsolationAuditSink { records: std::cell::RefCell::new(Vec::new()) }
    }

    /// The records in emission order (append-only; the borrow is brief
    /// and single-threaded).
    pub fn records(&self) -> std::cell::Ref<Vec<TenantIsolationAuditRecord>> {
        self.records.borrow()
    }
}

impl TenantIsolationAuditSink for MemoryTenantIsolationAuditSink {
    fn record(&self, record: TenantIsolationAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}

// --- The closed error-name table -----------------------------------------------------------

/// All stable error names of this module (sorted; the pairwise-distinct
/// property is pinned by tests, the house style).
pub const TENANT_ISOLATION_ERROR_NAMES: [&str; 15] = [
    "RESEARCH_APPROVAL_ACTOR_REQUIRED",
    "RESEARCH_APPROVAL_ALREADY_RECORDED",
    "RESEARCH_APPROVAL_REASON_REQUIRED",
    "RESEARCH_DATA_KIND_UNKNOWN",
    "RESEARCH_DATA_RECORD_DUPLICATE",
    "RESEARCH_DATA_RECORD_UNKNOWN",
    "RESEARCH_MODEL_ARTIFACT_DUPLICATE",
    "RESEARCH_MODEL_ARTIFACT_INVALID",
    "RESEARCH_MODEL_ARTIFACT_UNKNOWN",
    "RESEARCH_MODEL_SERVING_NOT_APPROVED",
    "RESEARCH_SCOPE_INVALID",
    "RESEARCH_WORKER_STAMP_BINDING_MISMATCH",
    "RESEARCH_WORKER_STAMP_INVALID",
    "TENANT_SCOPE_ECHO_MISMATCH",
    "TENANT_SCOPE_MISMATCH",
];

/// The error names taken from the enum itself (sorted, deduplicated via
/// `BTreeSet`) — the test-side consistency check against
/// [`TENANT_ISOLATION_ERROR_NAMES`], so a variant added without updating
/// the table is a named test failure.
pub fn error_names_from_enum() -> std::collections::BTreeSet<&'static str> {
    let names: [&str; 15] = [
        TenantIsolationError::ScopeInvalid { reason: String::new() }.name(),
        TenantIsolationError::WorkerStampInvalid { reason: String::new() }.name(),
        TenantIsolationError::WorkerStampBindingMismatch.name(),
        TenantIsolationError::ScopeEchoMismatch {
            stamped: String::new(),
            echoed: String::new(),
        }
        .name(),
        TenantIsolationError::DataOutsideScope {
            stamped: String::new(),
            requested: String::new(),
        }
        .name(),
        TenantIsolationError::DataRecordUnknown { key: String::new() }.name(),
        TenantIsolationError::DataRecordDuplicate { key: String::new() }.name(),
        TenantIsolationError::DataKindUnknown { attempted: String::new() }.name(),
        TenantIsolationError::ModelArtifactInvalid { reason: String::new() }.name(),
        TenantIsolationError::ModelArtifactUnknown { artifact_ref: String::new() }.name(),
        TenantIsolationError::ModelArtifactDuplicate { artifact_ref: String::new() }.name(),
        TenantIsolationError::ModelServingNotApproved {
            artifact_ref: String::new(),
            owner: String::new(),
            requested: String::new(),
        }
        .name(),
        TenantIsolationError::ApprovalReasonRequired.name(),
        TenantIsolationError::ApprovalActorRequired.name(),
        TenantIsolationError::ApprovalAlreadyRecorded { key: String::new() }.name(),
    ];
    names.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the rules above — scope covers matrix, catalog keys,
// refusal names — or a literal copied from this module's own constants.
// Nothing is fabricated.)
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

    /// The host stamps: tenant A site s1 (narrowed), tenant A tenant-wide,
    /// tenant B tenant-wide. Hand-built fixtures; each test owns its own.
    fn scope_a_s1() -> ResearchWorkerScope {
        ResearchWorkerScope::from_authenticated_context("tenant-a", Some("s1"))
            .expect("tenant-a/s1 is a valid scope")
    }

    fn scope_a_wide() -> ResearchWorkerScope {
        ResearchWorkerScope::from_authenticated_context("tenant-a", None)
            .expect("tenant-a wide is a valid scope")
    }

    fn scope_b_wide() -> ResearchWorkerScope {
        ResearchWorkerScope::from_authenticated_context("tenant-b", None)
            .expect("tenant-b wide is a valid scope")
    }

    fn stamp(worker_id: &str, family: ResearchWorkerFamily, scope: ResearchWorkerScope) -> WorkerScopeStamp {
        WorkerScopeStamp::stamp_by_host(worker_id, family, scope).expect("the stamp stamps")
    }

    fn catalog() -> ScopedResearchCatalog {
        let mut catalog = ScopedResearchCatalog::new();
        // Hand-computed keys (the catalog_key format):
        //   tenant:tenant-a|site:s1|kind:telemetry|record:ckt-1
        //   tenant:tenant-a|site:none|kind:training_data|record:ds-1
        //   tenant:tenant-b|site:none|kind:telemetry|record:ckt-9
        catalog
            .register(ResearchDataRecord {
                kind: ResearchDataKind::Telemetry,
                tenant_id: "tenant-a".to_string(),
                site_id: Some("s1".to_string()),
                record_id: "ckt-1".to_string(),
                descriptor: "feeder telemetry window".to_string(),
            })
            .expect("registers");
        catalog
            .register(ResearchDataRecord {
                kind: ResearchDataKind::TrainingData,
                tenant_id: "tenant-a".to_string(),
                site_id: None,
                record_id: "ds-1".to_string(),
                descriptor: "training dataset ref".to_string(),
            })
            .expect("registers");
        catalog
            .register(ResearchDataRecord {
                kind: ResearchDataKind::Telemetry,
                tenant_id: "tenant-b".to_string(),
                site_id: None,
                record_id: "ckt-9".to_string(),
                descriptor: "other tenant telemetry".to_string(),
            })
            .expect("registers");
        catalog
    }

    // --- statements and closed vocabularies --------------------------------

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(TENANT_ISOLATION_STATEMENT.contains("HOST-STAMPED"));
        assert!(TENANT_ISOLATION_STATEMENT.contains("cannot widen its own scope"));
        assert!(TENANT_ISOLATION_STATEMENT.contains("No safety, certification, or field-authority claim"));
        assert!(KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM.contains("tenant_scope.rs::TenantScope"));
        assert!(KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM.contains("kernel unchanged"));
        assert!(RESEARCH_WORKER_SCOPING_RULE.contains("training data, model artifacts, and inference inputs are all scoped"));
        assert!(RESEARCH_WORKER_SCOPING_RULE.contains("admm_dmpc"));
        assert!(RESEARCH_WORKER_SCOPING_RULE.contains("portfolio_analytics"));
    }

    #[test]
    fn worker_family_vocabulary_is_closed_and_round_trips() {
        for text in WORKER_FAMILIES {
            let family = ResearchWorkerFamily::parse(text).expect("parses");
            assert_eq!(family.as_str(), text);
        }
        for intruder in ["dispatch", "operator_console", "control_plane", ""] {
            assert_eq!(ResearchWorkerFamily::parse(intruder), None);
        }
    }

    #[test]
    fn data_kind_vocabulary_is_closed_and_round_trips() {
        let expected: [(&str, ResearchDataKind); 5] = [
            ("telemetry", ResearchDataKind::Telemetry),
            ("topology", ResearchDataKind::Topology),
            ("training_data", ResearchDataKind::TrainingData),
            ("model_artifact", ResearchDataKind::ModelArtifact),
            ("scenario", ResearchDataKind::Scenario),
        ];
        for (text, kind) in expected {
            assert_eq!(kind.as_str(), text);
            assert_eq!(ResearchDataKind::parse(text), Some(kind));
        }
        for intruder in ["authorization", "payload", ""] {
            assert_eq!(ResearchDataKind::parse(intruder), None);
        }
    }

    #[test]
    fn every_error_has_its_known_answer_name_and_the_table_is_complete() {
        // The table is sorted and pairwise distinct.
        let table = TENANT_ISOLATION_ERROR_NAMES;
        let mut sorted = table;
        sorted.sort_unstable();
        assert_eq!(table, sorted);
        for pair in table.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
        // The enum produces EXACTLY the table (a variant added without
        // updating the table is a named test failure).
        let from_enum = error_names_from_enum();
        let from_table: std::collections::BTreeSet<&'static str> = table.into_iter().collect();
        assert_eq!(from_enum, from_table);
        // Known answers for the alignment-critical names.
        assert_eq!(
            TenantIsolationError::DataOutsideScope {
                stamped: String::new(),
                requested: String::new(),
            }
            .name(),
            "TENANT_SCOPE_MISMATCH"
        );
        assert_eq!(
            TenantIsolationError::ScopeEchoMismatch {
                stamped: String::new(),
                echoed: String::new(),
            }
            .name(),
            "TENANT_SCOPE_ECHO_MISMATCH"
        );
        assert_eq!(
            TenantIsolationError::ModelServingNotApproved {
                artifact_ref: String::new(),
                owner: String::new(),
                requested: String::new(),
            }
            .name(),
            "RESEARCH_MODEL_SERVING_NOT_APPROVED"
        );
    }

    // --- scope construction: host-stamped only ------------------------------

    #[test]
    fn scope_construction_refuses_empty_and_invalid_parts() {
        let cases: [(&str, Option<&str>, bool); 5] = [
            ("tenant-a", None, true),
            ("tenant-a", Some("s1"), true),
            ("", None, false),            // empty tenant
            ("   ", None, false),         // blank tenant
            ("tenant-a", Some("  "), false), // blank site
        ];
        for (tenant, site, ok) in cases {
            let outcome = ResearchWorkerScope::from_authenticated_context(tenant, site);
            assert_eq!(outcome.is_ok(), ok, "tenant={tenant:?} site={site:?}");
            if let Err(err) = outcome {
                assert_eq!(err.name(), "RESEARCH_SCOPE_INVALID");
            }
        }
    }

    #[test]
    fn scope_canonical_forms_are_known_answers() {
        assert_eq!(scope_a_s1().canonical(), "tenant:tenant-a|site:s1");
        assert_eq!(scope_a_wide().canonical(), "tenant:tenant-a|site:none");
    }

    #[test]
    fn the_covers_matrix_is_the_kernel_narrowing_rule_hand_computed() {
        // (envelope, requested, covered?) — hand-computed from the rule:
        // tenant exact; tenant-wide covers any site in the tenant;
        // site-narrowed covers only the same site; a narrowed envelope
        // never covers an un-narrowed request.
        let cases: [(ResearchWorkerScope, ResearchWorkerScope, bool); 7] = [
            // tenant-wide envelope covers site request, same tenant.
            (scope_a_wide(), scope_a_s1(), true),
            // tenant-wide covers tenant-wide, same tenant.
            (scope_a_wide(), scope_a_wide(), true),
            // narrowed covers the SAME site.
            (scope_a_s1(), scope_a_s1(), true),
            // narrowed does NOT cover a different site (build s2).
            (scope_a_s1(), ResearchWorkerScope::from_authenticated_context("tenant-a", Some("s2")).expect("s2 valid"), false),
            // narrowed does NOT cover an un-narrowed request.
            (scope_a_s1(), scope_a_wide(), false),
            // cross-tenant is NEVER covered, even tenant-wide.
            (scope_a_wide(), scope_b_wide(), false),
            (scope_a_s1(), scope_b_wide(), false),
        ];
        for (envelope, requested, covered) in cases {
            assert_eq!(
                envelope.covers(&requested),
                covered,
                "envelope={} requested={}",
                envelope.canonical(),
                requested.canonical()
            );
        }
    }

    // --- worker stamps: host-stamped, binding-verified, echo-checked -------

    #[test]
    fn stamps_are_deterministic_and_binding_verified() {
        let first = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_s1());
        let second = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_s1());
        // Determinism: same inputs → same digest (16 lowercase hex digits).
        assert_eq!(first.stamp_digest_hex(), second.stamp_digest_hex());
        assert_eq!(first.stamp_digest_hex().len(), 16);
        assert!(first.verify_binding().is_ok());
        // Different inputs → different digest (a scope swap or identity
        // change cannot keep the binding).
        let other_scope = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_wide());
        assert_ne!(first.stamp_digest_hex(), other_scope.stamp_digest_hex());
        let other_id = stamp("worker/gnn-2", ResearchWorkerFamily::Gnn, scope_a_s1());
        assert_ne!(first.stamp_digest_hex(), other_id.stamp_digest_hex());
        let other_family = stamp("worker/gnn-1", ResearchWorkerFamily::Marl, scope_a_s1());
        assert_ne!(first.stamp_digest_hex(), other_family.stamp_digest_hex());
    }

    #[test]
    fn stamp_construction_refuses_an_empty_worker_id() {
        let err = WorkerScopeStamp::stamp_by_host("", ResearchWorkerFamily::Gnn, scope_a_s1())
            .expect_err("an empty worker id is refused");
        assert_eq!(err.name(), "RESEARCH_WORKER_STAMP_INVALID");
    }

    #[test]
    fn echoed_scope_that_differs_is_the_named_refusal() {
        let stamped = stamp("worker/marl-1", ResearchWorkerFamily::Marl, scope_a_s1());
        // Exact echo passes.
        stamped.verify_worker_echo(&scope_a_s1()).expect("exact echo passes");
        // A WIDER echo (tenant-wide) is refused.
        let err = stamped
            .verify_worker_echo(&scope_a_wide())
            .expect_err("a widened echo is refused");
        assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
        assert_eq!(
            err,
            TenantIsolationError::ScopeEchoMismatch {
                stamped: "tenant:tenant-a|site:s1".to_string(),
                echoed: "tenant:tenant-a|site:none".to_string(),
            }
        );
        // A cross-tenant echo is refused.
        let err = stamped
            .verify_worker_echo(&scope_b_wide())
            .expect_err("a cross-tenant echo is refused");
        assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    }

    // --- scoped data access: the cross-tenant negative suite ----------------

    #[test]
    fn cross_tenant_read_is_refused_and_audited() {
        let catalog = catalog();
        let worker_a = stamp("worker/admm-1", ResearchWorkerFamily::AdmmDmpc, scope_a_s1());
        let audit = MemoryTenantIsolationAuditSink::new();
        let clock = FixedClock { now: 5_000 };

        // Worker A (tenant-a, s1) reads tenant-b telemetry: REFUSED.
        let err = catalog
            .scoped_read_audited(
                &worker_a,
                ResearchDataKind::Telemetry,
                "tenant-b",
                None,
                "ckt-9",
                &clock,
                &audit,
            )
            .expect_err("cross-tenant read is denied");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");
        assert_eq!(
            err,
            TenantIsolationError::DataOutsideScope {
                stamped: "tenant:tenant-a|site:s1".to_string(),
                requested: "tenant:tenant-b|site:none".to_string(),
            }
        );
        // The denial is AUDITED (§16.2).
        let records = audit.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, TenantIsolationAuditKind::ScopeReadRefused);
        assert_eq!(records[0].kind.name(), "RESEARCH_SCOPE_READ_REFUSED");
        assert_eq!(records[0].timestamp_millis, 5_000);
        assert!(records[0].detail.contains("TENANT_SCOPE_MISMATCH"));
    }

    #[test]
    fn in_scope_reads_pass_and_unknown_records_fail_closed() {
        let catalog = catalog();
        let worker_a_wide = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_wide());
        let audit = MemoryTenantIsolationAuditSink::new();
        let clock = FixedClock { now: 6_000 };

        // Tenant-wide worker A reads the tenant-wide training dataset:
        // admitted (narrowing preserved).
        let record = catalog
            .scoped_read_audited(
                &worker_a_wide,
                ResearchDataKind::TrainingData,
                "tenant-a",
                None,
                "ds-1",
                &clock,
                &audit,
            )
            .expect("in-scope read admitted");
        assert_eq!(record.descriptor, "training dataset ref");

        // Tenant-wide worker A reads tenant-a/s1 telemetry: admitted
        // (tenant-wide envelope covers any site in the tenant).
        catalog
            .scoped_read_audited(
                &worker_a_wide,
                ResearchDataKind::Telemetry,
                "tenant-a",
                Some("s1"),
                "ckt-1",
                &clock,
                &audit,
            )
            .expect("site read under a tenant-wide stamp admitted");

        // A site-NARROWED worker cannot read an un-narrowed (tenant-wide)
        // record: refused.
        let worker_a_s1 = stamp("worker/gnn-2", ResearchWorkerFamily::Gnn, scope_a_s1());
        let err = catalog
            .enforce_scoped_read(
                &worker_a_s1,
                ResearchDataKind::TrainingData,
                "tenant-a",
                None,
                "ds-1",
            )
            .expect_err("a narrowed stamp does not cover a tenant-wide record");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");

        // An UNKNOWN record id fails closed (never "treat as in-scope").
        let err = catalog
            .enforce_scoped_read(
                &worker_a_wide,
                ResearchDataKind::Telemetry,
                "tenant-a",
                Some("s1"),
                "no-such-record",
            )
            .expect_err("unknown record refused");
        assert_eq!(err.name(), "RESEARCH_DATA_RECORD_UNKNOWN");

        // Audit trail: 2 admissions, 0 refusals (the refusals above used
        // the non-audited path by design).
        let records = audit.records();
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|r| r.kind == TenantIsolationAuditKind::ScopeReadAdmitted));
    }

    #[test]
    fn catalog_keys_are_known_answers_and_duplicates_are_refused() {
        let mut catalog = ScopedResearchCatalog::new();
        let record = ResearchDataRecord {
            kind: ResearchDataKind::Scenario,
            tenant_id: "tenant-a".to_string(),
            site_id: Some("s1".to_string()),
            record_id: "kundur-2area".to_string(),
            descriptor: "dynamic scenario artifact".to_string(),
        };
        // Hand-computed from the catalog_key format.
        assert_eq!(
            record.catalog_key(),
            "tenant:tenant-a|site:s1|kind:scenario|record:kundur-2area"
        );
        catalog.register(record.clone()).expect("first registration");
        assert_eq!(catalog.len(), 1);
        let err = catalog.register(record).expect_err("duplicate key refused");
        assert_eq!(err.name(), "RESEARCH_DATA_RECORD_DUPLICATE");
        assert_eq!(catalog.len(), 1, "the register stays exactly-one-per-key");
    }

    // --- model transfer: unapproved cross-tenant serving refused ------------

    #[test]
    fn unapproved_cross_tenant_model_serving_is_refused_and_audited() {
        let mut register = ModelServingRegister::new();
        register
            .register_artifact(ModelArtifactRegistration {
                artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                owner_family: ResearchWorkerFamily::Gnn,
                owner_scope: scope_a_wide(),
                training_data_ref: "training-data://tenant-a/ds-1".to_string(),
            })
            .expect("registers");
        let worker_a = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_s1());
        let audit = MemoryTenantIsolationAuditSink::new();
        let clock = FixedClock { now: 7_000 };

        // Tenant B asks for the model trained on tenant A's data:
        // REFUSED — no recorded approval exists.
        let err = register
            .model_serving_audited(&worker_a, "artifact://models/gnn-warmstart.v1", &scope_b_wide(), &clock, &audit)
            .expect_err("unapproved cross-tenant serving is refused");
        assert_eq!(err.name(), "RESEARCH_MODEL_SERVING_NOT_APPROVED");
        assert_eq!(
            err,
            TenantIsolationError::ModelServingNotApproved {
                artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                owner: "tenant:tenant-a|site:none".to_string(),
                requested: "tenant:tenant-b|site:none".to_string(),
            }
        );
        let records = audit.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, TenantIsolationAuditKind::ModelServingRefused);
        assert!(records[0].detail.contains("RESEARCH_MODEL_SERVING_NOT_APPROVED"));
    }

    #[test]
    fn recorded_approval_admits_cross_tenant_serving_and_unknown_artifacts_fail_closed() {
        let mut register = ModelServingRegister::new();
        register
            .register_artifact(ModelArtifactRegistration {
                artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                owner_family: ResearchWorkerFamily::Gnn,
                owner_scope: scope_a_wide(),
                training_data_ref: "training-data://tenant-a/ds-1".to_string(),
            })
            .expect("registers");
        let worker_a = stamp("worker/gnn-1", ResearchWorkerFamily::Gnn, scope_a_wide());
        let audit = MemoryTenantIsolationAuditSink::new();
        let clock = FixedClock { now: 8_000 };

        // Without an approval, tenant-b serving is refused.
        assert_eq!(
            register
                .enforce_model_serving(&worker_a, "artifact://models/gnn-warmstart.v1", &scope_b_wide())
                .expect_err("no approval yet")
                .name(),
            "RESEARCH_MODEL_SERVING_NOT_APPROVED"
        );

        // The host records the approval (reason + actor REQUIRED).
        register
            .record_approval(
                ModelServingApproval {
                    artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                    serving_scope: scope_b_wide(),
                    reason: "decision/2026-11-14 cross-tenant benchmark exchange".to_string(),
                    actor: "product".to_string(),
                    approved_at_millis: 0, // overwritten by the injected clock
                },
                &clock,
                &audit,
            )
            .expect("the approval is recorded");
        // The timestamp came from the INJECTED clock, not from the caller.
        assert_eq!(register.approvals_in_append_order()[0].approved_at_millis, 8_000);

        // Now the serving is admitted — and audited as admitted.
        register
            .model_serving_audited(&worker_a, "artifact://models/gnn-warmstart.v1", &scope_b_wide(), &clock, &audit)
            .expect("recorded approval admits the serving");
        let records = audit.records();
        assert_eq!(records.len(), 3); // approval recorded + 1 refusal + 1 admission
        assert_eq!(records[0].kind, TenantIsolationAuditKind::ServingApprovalRecorded);
        assert_eq!(records[1].kind, TenantIsolationAuditKind::ModelServingRefused);
        assert_eq!(records[2].kind, TenantIsolationAuditKind::ModelServingAdmitted);

        // A duplicate approval is refused (append-only, one per key).
        let err = register
            .record_approval(
                ModelServingApproval {
                    artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                    serving_scope: scope_b_wide(),
                    reason: "again".to_string(),
                    actor: "product".to_string(),
                    approved_at_millis: 0,
                },
                &clock,
                &audit,
            )
            .expect_err("duplicate approval refused");
        assert_eq!(err.name(), "RESEARCH_APPROVAL_ALREADY_RECORDED");

        // An approval for an UNKNOWN artifact is refused (fail-closed).
        let err = register
            .record_approval(
                ModelServingApproval {
                    artifact_ref: "artifact://models/not-registered".to_string(),
                    serving_scope: scope_b_wide(),
                    reason: "r".to_string(),
                    actor: "a".to_string(),
                    approved_at_millis: 0,
                },
                &clock,
                &audit,
            )
            .expect_err("unknown artifact approval refused");
        assert_eq!(err.name(), "RESEARCH_MODEL_ARTIFACT_UNKNOWN");

        // In-tenant serving needs NO approval (owner scope covers it).
        register
            .enforce_model_serving(&worker_a, "artifact://models/gnn-warmstart.v1", &scope_a_s1())
            .expect("in-tenant serving admitted without approval");

        // An unknown artifact at the serving path fails closed.
        let err = register
            .enforce_model_serving(&worker_a, "artifact://models/unknown", &scope_a_s1())
            .expect_err("unknown artifact refused");
        assert_eq!(err.name(), "RESEARCH_MODEL_ARTIFACT_UNKNOWN");
    }

    #[test]
    fn approvals_require_reason_and_actor_and_stamp_coverage() {
        let mut register = ModelServingRegister::new();
        register
            .register_artifact(ModelArtifactRegistration {
                artifact_ref: "artifact://models/m.v1".to_string(),
                owner_family: ResearchWorkerFamily::Marl,
                owner_scope: scope_a_wide(),
                training_data_ref: "training-data://tenant-a/ds-1".to_string(),
            })
            .expect("registers");
        let audit = MemoryTenantIsolationAuditSink::new();
        let clock = FixedClock { now: 9_000 };
        let base = ModelServingApproval {
            artifact_ref: "artifact://models/m.v1".to_string(),
            serving_scope: scope_b_wide(),
            reason: "r".to_string(),
            actor: "a".to_string(),
            approved_at_millis: 0,
        };
        // Empty reason refused.
        let mut no_reason = base.clone();
        no_reason.reason = "  ".to_string();
        assert_eq!(
            register
                .record_approval(no_reason, &clock, &audit)
                .expect_err("reason required")
                .name(),
            "RESEARCH_APPROVAL_REASON_REQUIRED"
        );
        // Empty actor refused.
        let mut no_actor = base.clone();
        no_actor.actor = String::new();
        assert_eq!(
            register
                .record_approval(no_actor, &clock, &audit)
                .expect_err("actor required")
                .name(),
            "RESEARCH_APPROVAL_ACTOR_REQUIRED"
        );
        // A worker whose stamped scope does NOT cover the serving scope
        // cannot serve even WITH an approval (the worker cannot serve into
        // a scope it does not hold).
        register
            .record_approval(base, &clock, &audit)
            .expect("the approval records");
        let worker_narrow = stamp("worker/gnn-3", ResearchWorkerFamily::Gnn, scope_a_s1());
        let err = register
            .enforce_model_serving(&worker_narrow, "artifact://models/m.v1", &scope_b_wide())
            .expect_err("the worker's stamp does not cover tenant-b");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");
    }

    #[test]
    fn the_residual_weight_leakage_risk_is_recorded_not_hidden() {
        // The module header records the honest residual: approval admits
        // SERVING, it does not treat WEIGHT LEAKAGE. The statement text
        // carries the boundary so no prose can imply otherwise.
        assert!(TENANT_ISOLATION_STATEMENT.contains("recorded approval"));
        assert!(
            !TENANT_ISOLATION_STATEMENT.contains("prevents weight leakage"),
            "no claim that the approval treats weight leakage"
        );
    }
}
