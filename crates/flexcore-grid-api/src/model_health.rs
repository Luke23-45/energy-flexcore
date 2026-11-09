// P4.03 — Versioned network model and model-health tracking (report §12.5
// steps 3 and 6, §12.6, §20.1 item 5 "Grid: topology/model version, feeder/
// substation boundaries, voltage/thermal validation, model health"; ledger
// P4.03).
//
// THE DECISIONS, MADE ONCE (full rationale in
// `ledger/phase-04-grid-feasibility-dispatch/03_05_feasibility_gate.md`):
//
// 1. **Model lifecycle states (D-P4.03-1):** `Draft`, `Active`, `Retired`.
//    Allowed transitions: `Draft → Active`, `Draft → Retired`,
//    `Active → Retired`. There are NO transitions out of `Retired` and no
//    re-activation. Only an `Active` model may back a feasibility check.
// 2. **Content hash (D-P4.03-2):** the hash is FNV-1a 64-bit over the
//    `serde_json` serialization of the model IR
//    (`fnv1a64:<16 lowercase hex digits>`), assigned BY this versioning
//    layer at registration — a caller-declared hash is overwritten, never
//    trusted. Every piece of feasibility evidence references THE HASH, so a
//    model change invalidates prior evidence (the P4.12 hook: dispatch
//    commands carry the model version + hash of §14.3).
//    ASSUMPTION (recorded): the hash is deterministic for a given Rust
//    toolchain + serde_json version (ryu shortest-roundtrip float
//    formatting); cross-toolchain hash stability is NOT claimed and is
//    re-verified when the durable store lands.
// 3. **Immutability (ledger step 1):** model versions are immutable. The
//    registry refuses ANY re-registration of an existing
//    `(model_id, version)` pair — a change produces a NEW version, never an
//    in-place edit. `VersionedModel::verify_content_hash` recomputes the
//    hash from the live content and compares it against the recorded
//    `GridModelRef.content_hash`; a mismatch means the artefact was edited
//    after registration and the gate aborts fail-closed (the P4.05 pre-run
//    hook, and the P4.12 evidence-invalidation hook).
// 4. **Model health (D-P4.03-3):** the health check covers exactly the
//    three ledger axes — completeness (structural validation + a non-empty
//    coverage statement), staleness (unverified source-data changes
//    recorded after the model's last verification, plus the configured
//    maximum age since last verification — `age == limit` is NOT stale,
//    the P2.08/P3.13 convention), and coverage (the scope-vs-registry
//    statement: every registry element the caller declares must appear in
//    the model's declared coverage — an uncovered element is a health
//    FINDING, never silently dropped).
// 5. **Fail-closed (ledger step 3; §17.5 invariant 9):** a stale,
//    incomplete, or uncovered model is never used for a new candidate —
//    `assert_usable_for_feasibility` refuses, and the gate (P4.05 step 3)
//    calls it BEFORE any model application. The reject-vs-hold split is
//    P4.08's policy table; this module only refuses.
//
// Storage seam (recorded): this module holds the in-crate, append-only
// version store the gate and the tests consume. The DURABLE store
// (PostgreSQL model-version rows, migration + adapter) is a composition-
// root concern: flexcore-grid-api depends only on flexcore-contracts +
// serde/serde_json/time (the dependency-direction test pins the exact
// set), so the persistence adapter is a later-wave deliverable that must
// re-verify hashes through `model_content_hash` / `verify_content_hash`.
//
// Control status: pure bookkeeping and checks for the simulation-only 0.x
// line. No I/O, no device-write path, no safety or certification claim: a
// healthy model means "fit to evaluate against", never "the real grid".

use crate::port::{GridModelRef, ModelError, VersionedNetworkModel};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recorded statements
// ---------------------------------------------------------------------------

/// The stale-model policy reference (ledger P4.03 step 3; the P4.08 policy
/// table owns the reject-vs-hold split).
pub const MODEL_STALENESS_POLICY: &str = "If the model is stale, incomplete, \
     or its health check fails, feasibility is NOT inferred: the gate refuses \
     the model before applying any candidate (§12.5 final paragraph; §17.5 \
     invariant 9). A topology/capability source change after the model's \
     last verification makes the model stale for NEW candidates. The \
     reject-vs-hold split is the P4.08 policy table.";

/// The lifecycle decision statement (D-P4.03-1).
pub const MODEL_LIFECYCLE_DECISION: &str = "Model lifecycle states, decided \
     once: Draft, Active, Retired. Draft → Active, Draft → Retired, \
     Active → Retired; no transitions out of Retired, no re-activation. \
     Only an Active model may back a feasibility check.";

/// The content-hash statement (D-P4.03-2).
pub const CONTENT_HASH_STATEMENT: &str = "The content hash is FNV-1a 64-bit \
     over the serde_json serialization of the model IR, assigned by the \
     versioning layer at registration and carried on every result and \
     dispatch command (§14.3; P4.12). Evidence references THE HASH: a model \
     change invalidates prior evidence.";

// ---------------------------------------------------------------------------
// Content hash
// ---------------------------------------------------------------------------

/// Computes the content hash of a model IR (D-P4.03-2): FNV-1a 64-bit over
/// the `serde_json` serialization of the model, rendered as
/// `fnv1a64:<16 lowercase hex digits>`. Deterministic for the recorded
/// serialization (see the ASSUMPTION in the module header).
pub fn model_content_hash(model: &VersionedNetworkModel) -> String {
    let json = serde_json::to_string(model).expect("the model IR always serializes");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in json.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// The model lifecycle state (D-P4.03-1). Only [`ModelLifecycleState::Active`]
/// models back a feasibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLifecycleState {
    /// Registered but not yet reviewed/verified against source data. Not
    /// usable for feasibility.
    Draft,
    /// Reviewed and verified: usable for feasibility checks.
    Active,
    /// Retired: never usable again (immutable terminal state).
    Retired,
}

impl ModelLifecycleState {
    /// The canonical state name (records, evidence rows, wire).
    pub const fn as_str(self) -> &'static str {
        match self {
            ModelLifecycleState::Draft => "draft",
            ModelLifecycleState::Active => "active",
            ModelLifecycleState::Retired => "retired",
        }
    }

    /// Whether the transition `self → to` is allowed (D-P4.03-1).
    pub const fn can_transition_to(self, to: ModelLifecycleState) -> bool {
        matches!(
            (self, to),
            (ModelLifecycleState::Draft, ModelLifecycleState::Active)
                | (ModelLifecycleState::Draft, ModelLifecycleState::Retired)
                | (ModelLifecycleState::Active, ModelLifecycleState::Retired)
        )
    }
}

// ---------------------------------------------------------------------------
// Versioned model
// ---------------------------------------------------------------------------

/// The declared coverage statement of one model version (ledger P4.03
/// step 2: "coverage statement (which feeders/equipment are modelled)").
/// The scope-vs-registry health check compares these lists against the
/// registry elements the caller declares; an element missing here is a
/// health FINDING, never silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCoverage {
    /// Feeder identifiers the model covers (e.g. `"feeder-a"`).
    pub covered_feeder_ids: Vec<String>,
    /// Non-feeder element identifiers the model covers (assets, buses,
    /// equipment the portfolio may reference).
    pub covered_element_ids: Vec<String>,
    /// The human-readable coverage statement carried on records.
    pub coverage_statement: String,
}

impl ModelCoverage {
    /// Validates the coverage statement: non-empty statement, non-empty
    /// identifier lists, no blank identifiers. A model may cover zero
    /// feeders structurally, but the health check then reports every
    /// registry element as uncovered (fail-closed).
    pub fn validate(&self) -> Result<(), ModelHealthError> {
        if self.coverage_statement.trim().is_empty() {
            return Err(ModelHealthError::InvalidCoverage {
                field: "coverage_statement",
            });
        }
        for field in ["covered_feeder_ids", "covered_element_ids"] {
            let list = if field == "covered_feeder_ids" {
                &self.covered_feeder_ids
            } else {
                &self.covered_element_ids
            };
            if list.iter().any(|id| id.trim().is_empty()) {
                return Err(ModelHealthError::InvalidCoverage { field });
            }
        }
        Ok(())
    }

    /// Whether the element id is inside the declared coverage.
    pub fn covers_element(&self, element_id: &str) -> bool {
        self.covered_feeder_ids.iter().any(|id| id == element_id)
            || self.covered_element_ids.iter().any(|id| id == element_id)
    }
}

/// One immutable, versioned, hashed model version with its lifecycle state,
/// coverage statement, and verification timestamps (ledger P4.03 step 1).
///
/// Constructed exclusively through [`VersionedModel::register`], which
/// assigns the content hash — the versioning layer owns the hash, never the
/// caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedModel {
    /// The model IR (identity + version + the ASSIGNED content hash inside
    /// [`VersionedNetworkModel::model_ref`]).
    pub model: VersionedNetworkModel,
    /// The lifecycle state (D-P4.03-1).
    pub lifecycle: ModelLifecycleState,
    /// The declared coverage statement.
    pub coverage: ModelCoverage,
    /// UTC millis when this version was registered (§10.3 UTC convention).
    pub registered_at_millis: i64,
    /// UTC millis of the last verification against source data. Staleness
    /// is measured from this instant; a source change after it makes the
    /// model stale until re-verified.
    pub last_verified_at_millis: i64,
}

impl VersionedModel {
    /// Registers a model version: validates the IR structurally, validates
    /// the coverage statement, and ASSIGNS the content hash (overwriting
    /// any caller-supplied hash — the versioning layer owns the hash,
    /// D-P4.03-2). `now_millis` stamps registration and last-verification.
    pub fn register(
        mut model: VersionedNetworkModel,
        lifecycle: ModelLifecycleState,
        coverage: ModelCoverage,
        now_millis: i64,
    ) -> Result<Self, ModelHealthError> {
        coverage.validate()?;
        if now_millis < 0 {
            return Err(ModelHealthError::InvalidTimestamp);
        }
        // The versioning layer assigns the hash FIRST (any caller-supplied
        // hash is overwritten), then the structural validation runs on the
        // completed identity — a model cannot register without a hash.
        let hash = model_content_hash(&model);
        model.model_ref.content_hash = hash;
        model.validate().map_err(ModelHealthError::MalformedModel)?;
        Ok(Self {
            model,
            lifecycle,
            coverage,
            registered_at_millis: now_millis,
            last_verified_at_millis: now_millis,
        })
    }

    /// The model reference (identity + version + assigned hash).
    pub fn model_ref(&self) -> &GridModelRef {
        &self.model.model_ref
    }

    /// Recomputes the content hash from the LIVE model content and compares
    /// it against the recorded hash. `false` means the artefact was edited
    /// after registration — prior evidence citing this hash is invalidated
    /// (the P4.12 hook) and the gate aborts fail-closed (the P4.05 pre-run
    /// hook). This is the check that makes "a silently edited model" unable
    /// to back a run.
    pub fn verify_content_hash(&self) -> bool {
        model_content_hash(&self.model) == self.model.model_ref.content_hash
    }
}

// ---------------------------------------------------------------------------
// The immutable version registry
// ---------------------------------------------------------------------------

/// One recorded change to the source data the models are derived from
/// (topology change, capability change, feeder reconfiguration — ledger
/// P4.03 step 2 staleness: "max age since last verification against source
/// data"; the §12.5/§17.5 inv. 9 rule: a source change after model version
/// X means the gate must not use X for new candidates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceChangeRecord {
    /// Stable change identifier (records name it).
    pub change_id: String,
    /// UTC millis of the change (§10.3).
    pub at_millis: i64,
    /// What changed (audit text; e.g. "topology: line_2_3 reconfigured").
    pub description: String,
}

/// Errors of the model-health layer. Every variant is a named, structured
/// condition — never a bare error.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelHealthError {
    /// The model IR failed structural validation.
    MalformedModel(ModelError),
    /// The coverage statement is invalid (`field` names the problem).
    InvalidCoverage { field: &'static str },
    /// A timestamp argument was negative or otherwise unusable.
    InvalidTimestamp,
    /// A source-change record is incomplete (`field` names the problem).
    InvalidSourceChange { field: &'static str },
    /// The `(model_id, version)` pair already exists: versions are
    /// immutable, a change must be a NEW version (ledger step 1).
    VersionAlreadyExists { model_id: String, version: String },
    /// The `(model_id, version)` pair does not exist.
    UnknownVersion { model_id: String, version: String },
    /// The lifecycle transition is not allowed (D-P4.03-1).
    IllegalTransition {
        from: ModelLifecycleState,
        to: ModelLifecycleState,
    },
    /// The model is not usable for feasibility (the fail-closed refusal;
    /// every finding is listed).
    NotUsable { findings: Vec<HealthFinding> },
}

impl std::fmt::Display for ModelHealthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelHealthError::MalformedModel(error) => {
                write!(f, "model IR failed structural validation: {error}")
            }
            ModelHealthError::InvalidCoverage { field } => {
                write!(f, "model coverage statement is invalid: field '{field}'")
            }
            ModelHealthError::InvalidTimestamp => {
                write!(f, "timestamp argument is invalid (negative)")
            }
            ModelHealthError::InvalidSourceChange { field } => {
                write!(f, "source-change record is invalid: field '{field}' is empty")
            }
            ModelHealthError::VersionAlreadyExists { model_id, version } => write!(
                f,
                "model '{model_id}' version '{version}' already exists; versions \
                 are immutable — a change must be a NEW version"
            ),
            ModelHealthError::UnknownVersion { model_id, version } => {
                write!(f, "model '{model_id}' version '{version}' does not exist")
            }
            ModelHealthError::IllegalTransition { from, to } => write!(
                f,
                "illegal lifecycle transition {} → {} (D-P4.03-1)",
                from.as_str(),
                to.as_str()
            ),
            ModelHealthError::NotUsable { findings } => {
                write!(f, "model is not usable for feasibility: {} finding(s): ", findings.len())?;
                for (index, finding) in findings.iter().enumerate() {
                    if index > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{finding}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ModelHealthError {}

/// The in-crate, append-only model-version registry (ledger P4.03 step 1:
/// "store models immutably (new version on change, never in-place edits)").
///
/// - `register_version` refuses ANY re-registration of an existing
///   `(model_id, version)` pair.
/// - Lifecycle transitions go through [`ModelVersionRegistry::transition`]
///   and are checked against D-P4.03-1.
/// - `record_source_change` appends the source-data change records the
///   staleness check consumes.
///
/// The durable (PostgreSQL) store is a composition-root seam recorded in
/// the module header; this registry is the gate-facing contract.
#[derive(Debug, Clone, Default)]
pub struct ModelVersionRegistry {
    versions: Vec<VersionedModel>,
    source_changes: Vec<SourceChangeRecord>,
}

impl ModelVersionRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a version (append-only). Any duplicate
    /// `(model_id, version)` is refused — a change is a NEW version.
    pub fn register_version(&mut self, versioned: VersionedModel) -> Result<(), ModelHealthError> {
        let model_id = versioned.model.model_ref.model_id.as_str();
        let version = versioned.model.model_ref.version.as_str();
        if self.get(model_id, version).is_some() {
            return Err(ModelHealthError::VersionAlreadyExists {
                model_id: model_id.to_string(),
                version: version.to_string(),
            });
        }
        self.versions.push(versioned);
        Ok(())
    }

    /// Applies a lifecycle transition to the named version (checked against
    /// D-P4.03-1). The version's content is untouched.
    pub fn transition(
        &mut self,
        model_id: &str,
        version: &str,
        to: ModelLifecycleState,
    ) -> Result<(), ModelHealthError> {
        let versioned = self
            .versions
            .iter_mut()
            .find(|v| {
                v.model.model_ref.model_id == model_id && v.model.model_ref.version == version
            })
            .ok_or_else(|| ModelHealthError::UnknownVersion {
                model_id: model_id.to_string(),
                version: version.to_string(),
            })?;
        if !versioned.lifecycle.can_transition_to(to) {
            return Err(ModelHealthError::IllegalTransition {
                from: versioned.lifecycle,
                to,
            });
        }
        versioned.lifecycle = to;
        Ok(())
    }

    /// Records one source-data change (topology/capability/equipment) that
    /// models derived from this source must be re-verified against.
    pub fn record_source_change(
        &mut self,
        change_id: String,
        at_millis: i64,
        description: String,
    ) -> Result<(), ModelHealthError> {
        if change_id.trim().is_empty() {
            return Err(ModelHealthError::InvalidSourceChange { field: "change_id" });
        }
        if description.trim().is_empty() {
            return Err(ModelHealthError::InvalidSourceChange {
                field: "description",
            });
        }
        if at_millis < 0 {
            return Err(ModelHealthError::InvalidTimestamp);
        }
        self.source_changes.push(SourceChangeRecord {
            change_id,
            at_millis,
            description,
        });
        Ok(())
    }

    /// The named version, if present.
    pub fn get(&self, model_id: &str, version: &str) -> Option<&VersionedModel> {
        self.versions.iter().find(|v| {
            v.model.model_ref.model_id == model_id && v.model.model_ref.version == version
        })
    }

    /// The most recently registered `Active` version of the model, if any.
    pub fn latest_active(&self, model_id: &str) -> Option<&VersionedModel> {
        self.versions
            .iter()
            .filter(|v| v.model.model_ref.model_id == model_id)
            .filter(|v| v.lifecycle == ModelLifecycleState::Active)
            .next_back()
    }

    /// The recorded source changes (the staleness input).
    pub fn source_changes(&self) -> &[SourceChangeRecord] {
        &self.source_changes
    }

    /// All registered versions (insertion order).
    pub fn versions(&self) -> &[VersionedModel] {
        &self.versions
    }
}

// ---------------------------------------------------------------------------
// The health check
// ---------------------------------------------------------------------------

/// The kind of a registry element the coverage check compares against the
/// model's declared coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryElementKind {
    /// A feeder: must appear in `covered_feeder_ids`.
    Feeder,
    /// An asset, bus, or equipment element: must appear in
    /// `covered_element_ids`.
    Element,
}

/// One registry element the scope-vs-registry coverage check runs over
/// (what the portfolio contains, as declared by the caller — the registry
/// projection, injected: flexcore-grid-api holds no Phase 2 dependency).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryElement {
    /// The element identifier (feeder / asset / bus id as declared).
    pub element_id: String,
    /// Which coverage list the element must appear in.
    pub kind: RegistryElementKind,
}

/// The model-health configuration. The age bound is CONFIGURATION with a
/// recorded default (the D-P3.13-3 pattern); the default here is
/// ASSUMPTION-labelled: 24 hours since last verification against source
/// data, pending the power-systems review of P4.08's policy table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelHealthConfig {
    /// Maximum age (millis) since `last_verified_at_millis` before the
    /// model is stale. Convention: `age == limit` is NOT stale;
    /// `age > limit` is (the P2.08/P3.13 convention).
    pub max_age_since_verification_millis: i64,
}

impl Default for ModelHealthConfig {
    fn default() -> Self {
        // ASSUMPTION: a 24-hour verification bound, recorded as the default
        // until the P4.08 policy review confirms or changes it.
        Self {
            max_age_since_verification_millis: 86_400_000,
        }
    }
}

/// One health finding. EVERY finding blocks feasibility (fail-closed);
/// findings are reported ALL AT ONCE — the first problem never hides the
/// rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "finding")]
pub enum HealthFinding {
    /// The model is not `Active` (D-P4.03-1).
    NotActive { state: String },
    /// The model IR failed structural validation (completeness axis).
    StructurallyIncomplete { detail: String },
    /// The coverage statement is missing or empty (completeness axis).
    CoverageStatementMissing,
    /// The model is stale: source changes are unverified and/or the
    /// verification age bound is exceeded (staleness axis).
    Stale {
        unverified_changes: Vec<String>,
        age_millis: Option<i64>,
        max_age_millis: Option<i64>,
    },
    /// A registry element the portfolio contains is NOT inside the model's
    /// declared coverage (coverage axis). Never silently dropped.
    UncoveredElement { element_id: String, kind: String },
    /// The live content hash no longer matches the recorded hash: the model
    /// artefact was edited after registration (the evidence-invalidation
    /// condition; checked by `verify_content_hash`).
    ContentHashMismatch { recorded: String, computed: String },
}

impl std::fmt::Display for HealthFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthFinding::NotActive { state } => {
                write!(f, "model lifecycle state is '{state}'; only 'active' models may back a feasibility check")
            }
            HealthFinding::StructurallyIncomplete { detail } => {
                write!(f, "model is structurally incomplete: {detail}")
            }
            HealthFinding::CoverageStatementMissing => {
                write!(f, "the model carries no coverage statement")
            }
            HealthFinding::Stale {
                unverified_changes,
                age_millis,
                max_age_millis,
            } => {
                write!(f, "model is stale:")?;
                if !unverified_changes.is_empty() {
                    write!(
                        f,
                        " unverified source changes [{}]",
                        unverified_changes.join(", ")
                    )?;
                }
                if let (Some(age), Some(max)) = (age_millis, max_age_millis) {
                    write!(
                        f,
                        " age since last verification {age} ms exceeds the bound {max} ms"
                    )?;
                }
                Ok(())
            }
            HealthFinding::UncoveredElement { element_id, kind } => {
                write!(
                    f,
                    "registry element '{element_id}' (kind {kind}) is outside the model's \
                     declared coverage — an uncovered element is a finding, never dropped"
                )
            }
            HealthFinding::ContentHashMismatch { recorded, computed } => write!(
                f,
                "content hash mismatch: recorded '{recorded}', computed from live content \
                 '{computed}' — the model was edited after registration and prior evidence \
                 citing this hash is invalidated"
            ),
        }
    }
}

/// The model-health report (the P4.05 gate consumes
/// [`assert_usable_for_feasibility`] over this).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelHealthReport {
    /// The model identity + version + assigned hash (evidence references
    /// THE HASH).
    pub model_ref: GridModelRef,
    /// The lifecycle state at check time.
    pub lifecycle: ModelLifecycleState,
    /// UTC millis of the check.
    pub checked_at_millis: i64,
    /// Whether the live content matched the recorded hash at check time.
    pub content_hash_verified: bool,
    /// ALL findings (empty = usable).
    pub findings: Vec<HealthFinding>,
}

impl ModelHealthReport {
    /// Whether the model may back a feasibility check: NO findings.
    pub fn is_usable(&self) -> bool {
        self.findings.is_empty()
    }
}

/// The model-health check (ledger P4.03 step 2): completeness, staleness,
/// and coverage, reported all at once. Pure — no I/O.
///
/// NOTE: the content-hash verification is deliberately NOT part of this
/// check (the ledger axes are completeness/staleness/coverage); it is the
/// separate P4.05 pre-run hook (`VersionedModel::verify_content_hash`) so
/// the two fail-closed conditions carry distinct, attributable evidence.
pub fn check_model_health(
    versioned: &VersionedModel,
    registry: &ModelVersionRegistry,
    portfolio_elements: &[RegistryElement],
    config: &ModelHealthConfig,
    now_millis: i64,
) -> ModelHealthReport {
    let mut findings = Vec::new();

    // Lifecycle (D-P4.03-1).
    if versioned.lifecycle != ModelLifecycleState::Active {
        findings.push(HealthFinding::NotActive {
            state: versioned.lifecycle.as_str().to_string(),
        });
    }

    // Completeness axis: structural validation + a non-empty coverage
    // statement.
    if let Err(error) = versioned.model.validate() {
        findings.push(HealthFinding::StructurallyIncomplete {
            detail: error.to_string(),
        });
    }
    if versioned.coverage.coverage_statement.trim().is_empty() {
        findings.push(HealthFinding::CoverageStatementMissing);
    }

    // Staleness axis: (a) any source change recorded after the model's
    // last verification is unverified; (b) the age bound. The change set
    // is the registry's own record, so "registry changes" here means
    // source-data changes, not registration order.
    let unverified: Vec<String> = registry
        .source_changes()
        .iter()
        .filter(|change| change.at_millis > versioned.last_verified_at_millis)
        .map(|change| change.change_id.clone())
        .collect();
    let age = now_millis - versioned.last_verified_at_millis;
    let age_exceeded = age > config.max_age_since_verification_millis;
    if !unverified.is_empty() || age_exceeded {
        findings.push(HealthFinding::Stale {
            unverified_changes: unverified,
            age_millis: age_exceeded.then_some(age),
            max_age_millis: age_exceeded.then_some(config.max_age_since_verification_millis),
        });
    }

    // Coverage axis: every registry element the caller declares must be
    // inside the model's declared coverage. An uncovered element is a
    // FINDING, never silently dropped.
    for element in portfolio_elements {
        let covered = match element.kind {
            RegistryElementKind::Feeder => versioned
                .coverage
                .covered_feeder_ids
                .iter()
                .any(|id| *id == element.element_id),
            RegistryElementKind::Element => versioned
                .coverage
                .covered_element_ids
                .iter()
                .any(|id| *id == element.element_id),
        };
        if !covered {
            findings.push(HealthFinding::UncoveredElement {
                element_id: element.element_id.clone(),
                kind: match element.kind {
                    RegistryElementKind::Feeder => "feeder".to_string(),
                    RegistryElementKind::Element => "element".to_string(),
                },
            });
        }
    }

    ModelHealthReport {
        model_ref: versioned.model.model_ref.clone(),
        lifecycle: versioned.lifecycle,
        checked_at_millis: now_millis,
        content_hash_verified: versioned.verify_content_hash(),
        findings,
    }
}

/// The fail-closed gate the feasibility pipeline calls BEFORE applying any
/// candidate to the model (ledger P4.03 step 3; §17.5 invariant 9). A model
/// with ANY finding is refused; the reject-vs-hold split is P4.08's.
pub fn assert_usable_for_feasibility(
    report: &ModelHealthReport,
) -> Result<(), ModelHealthError> {
    if report.is_usable() {
        Ok(())
    } else {
        Err(ModelHealthError::NotUsable {
            findings: report.findings.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::three_bus_model;

    fn model_ref_fixture() -> GridModelRef {
        GridModelRef {
            model_id: "feeder-a".to_string(),
            version: "1".to_string(),
            content_hash: String::new(),
        }
    }

    fn coverage() -> ModelCoverage {
        ModelCoverage {
            covered_feeder_ids: vec!["feeder-a".to_string()],
            covered_element_ids: vec!["bus_1".to_string()],
            coverage_statement: "covers feeder-a (three-bus radial fixture)".to_string(),
        }
    }

    #[test]
    fn lifecycle_transitions_follow_the_recorded_table() {
        assert!(ModelLifecycleState::Draft.can_transition_to(ModelLifecycleState::Active));
        assert!(ModelLifecycleState::Draft.can_transition_to(ModelLifecycleState::Retired));
        assert!(ModelLifecycleState::Active.can_transition_to(ModelLifecycleState::Retired));
        // No re-activation, no transitions out of Retired, no self-loops.
        assert!(!ModelLifecycleState::Active.can_transition_to(ModelLifecycleState::Draft));
        assert!(!ModelLifecycleState::Active.can_transition_to(ModelLifecycleState::Active));
        assert!(!ModelLifecycleState::Retired.can_transition_to(ModelLifecycleState::Active));
        assert!(!ModelLifecycleState::Retired.can_transition_to(ModelLifecycleState::Draft));
        assert!(!ModelLifecycleState::Retired.can_transition_to(ModelLifecycleState::Retired));
    }

    #[test]
    fn register_assigns_the_hash_and_refuses_caller_supplied_hashes() {
        let mut model = three_bus_model(model_ref_fixture());
        model.model_ref.content_hash = "caller-claims-this".to_string();
        let versioned =
            VersionedModel::register(model, ModelLifecycleState::Draft, coverage(), 1_000)
                .expect("registers");
        assert!(versioned.model_ref().content_hash.starts_with("fnv1a64:"));
        assert_ne!(versioned.model_ref().content_hash, "caller-claims-this");
        assert!(versioned.verify_content_hash());
    }

    #[test]
    fn content_hash_discriminates_content_changes() {
        let a = three_bus_model(model_ref_fixture());
        let mut b = three_bus_model(model_ref_fixture());
        b.branches[0].r_ohm += 0.01;
        assert_ne!(
            model_content_hash(&a),
            model_content_hash(&b),
            "a parameter change must change the hash"
        );
    }

    #[test]
    fn verify_content_hash_catches_a_silently_edited_model() {
        let model = three_bus_model(model_ref_fixture());
        let mut versioned =
            VersionedModel::register(model, ModelLifecycleState::Active, coverage(), 1_000)
                .expect("registers");
        assert!(versioned.verify_content_hash());
        // The silent edit (any in-place mutation of the artefact).
        versioned.model.branches[0].rated_current_a = 999.0;
        assert!(!versioned.verify_content_hash());
    }
}
