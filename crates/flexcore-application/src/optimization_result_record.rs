// P3.21 — Optimization result contract: record assembly, input-snapshot
//         linking, `ScheduleProposed` emission, and the persistence-before-use
//         rule (report §12.6, §15.3, §16.1; ledger P3.21).
//
// The §12.6 contract TYPES live in `flexcore-optimization-api` (`result`,
// `config`, `determinism`, `infeasibility`) and are REUSED here verbatim —
// this module does not restate a single one of them. What P3.21 adds is the
// application-side WIRING the port contract leaves open:
//
// 1. **Record assembly** ([`assemble_schedule_result_record`]): a §12.6
//    result plus the input snapshot it was computed from plus the grid-model
//    traceability become one [`ScheduleResultRecord`]. The assembly runs the
//    port-level completeness gate (`flexcore_optimization_api::result`
//    `OptimizationResult::validate_complete`) FIRST — a result missing any
//    required field never becomes a schedule record — and then completes the
//    result's optional wiring fields (`input_snapshot_id`,
//    `grid_model_version`) from the snapshot and traceability, and merges
//    the recorded warnings (P3.15 [`ScalingWarning`]s; the P3.18
//    `SOCP_EXACTNESS_CAVEAT` on every Tier 2 result).
// 2. **Input snapshot** ([`InputSnapshotRecord`]): exactly the constituents
//    ledger P3.21 step 2 requires — capability versions (P3.01 schema
//    version per asset), envelope version (P3.12/P3.13 types), forecast
//    version (P3.31, through the labeled accessor so the provenance label
//    travels with it), objective configuration (P3.14, verbatim
//    `record_fields` — synthetic labels survive into the record), solver
//    configuration (P3.19 `SolverConfig` canonical JSON + digest), and the
//    telemetry snapshot reference.
// 3. **Grid-model traceability** ([`GridModelTraceability`]): the §12.6
//    "grid model version" comes from the Phase 2 Site grid-model reference
//    (P2.02) — the version of the network model the site references — and
//    the record states WHICH site's reference supplied it. There is no
//    Phase 3 or Phase 4 grid model; none may be invented.
// 4. **`ScheduleProposed` emission** ([`schedule_proposed_publication`]):
//    the pure outbox assembly over the FROZEN §15.3 payload. A
//    `ScheduleProposed` is a fact that a CANDIDATE schedule was proposed —
//    never that it is feasible, approved, or dispatchable (feasibility,
//    authorization, and dispatch are Phase 4 gates). The frozen payload
//    cannot carry the result-record reference; the gap is recorded in
//    [`SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE`] (the D-FX-9 / D-P3.12-7
//    precedent) and the reference travels in the deterministic idempotency
//    key and the persisted record.
// 5. **Persistence before use** ([`PERSIST_BEFORE_USE_STATEMENT`],
//    [`PersistedResultRecord`]): the record is persisted in PostgreSQL
//    before any downstream use (§16.1). The publication function accepts a
//    [`PersistedResultRecord`] proof and verifies it against the record, so
//    an event for an unpersisted record is unrepresentable in the assembled
//    output. The storage-side contract (the `ScheduleRecordStore` trait and
//    row types) lives in `flexcore-storage::schedule_store`; the PostgreSQL
//    implementation (migration 0013) lives in the postgres adapter, and the
//    insert runs inside the caller's unit of work together with the
//    `ScheduleProposed` outbox row (one native transaction — §15.5).
//
// Dependency-rule note (recorded): this module REUSES the §12.6 types from
// `flexcore-optimization-api`, so `flexcore-application` depends on
// `flexcore-optimization-api`. That edge is consistent with the hexagonal
// layering — `flexcore-optimization-api` is a port-contract crate in the
// spirit of `flexcore-ports` (no infrastructure, core-crate rules apply to
// it) — but `tools/check_dependency_rule.py` predates it and must have the
// edge added to `ALLOWED_INTERNAL_EDGES["flexcore-application"]` by the
// coordinator (the checker file was outside this subtask's ownership; see
// the decision record, D-P3.21-7).
//
// Control status: candidate-schedule bookkeeping for the simulation-only
// 0.x line. Nothing here validates against a network model, authorizes a
// command, or writes to a device; every reference to "the schedule" below
// means the CANDIDATE the solver returned.

use flexcore_contracts::capability::CAPABILITY_SCHEMA_VERSION;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{DomainEvent, ScheduleProposedPayload};
use flexcore_events::subject::{subject_for, Subject};
use flexcore_optimization_api::clarabel_provider::SOCP_EXACTNESS_CAVEAT;
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::result::{
    OptimizationResult, ResultCompletenessError, SolveStatus,
};

use crate::envelope_staleness::ScheduleEnvelopeReference;
use crate::flexibility_envelope::EnvelopeVersion;
use crate::forecast_fixture::{ForecastVersion, LabeledSeriesValues, Provenance};
use crate::scheduling_objective::{ObjectiveConfig, ScalingWarning};

// --- Recorded statements ------------------------------------------------------

/// The persistence-before-use rule, stated once (ledger P3.21 step 3; report
/// §16.1, §15.5).
pub const PERSIST_BEFORE_USE_STATEMENT: &str = "The optimization result record \
     is persisted in PostgreSQL BEFORE any downstream use (report §16.1): the \
     composition root opens one unit of work, inserts the schedule-result \
     record through the ScheduleRecordStore, appends the ScheduleProposed \
     outbox row with the SAME deterministic idempotency key, and commits — \
     one native transaction, outbox + state atomic (§15.5). The publisher \
     releases the event only after the commit, so no consumer can observe \
     the event before the record exists.";

/// The candidate-status statement carried in the record's documentation and
/// repeated wherever the record is described: a solved schedule is a
/// CANDIDATE — never "feasible" (Phase 4 owns the feasibility gate).
pub const CANDIDATE_STATUS_STATEMENT: &str = "A schedule that solves is a \
     CANDIDATE: §12.6 status is the SOLVER outcome, not a network-feasibility, \
     authorization, or dispatch verdict. Validation, feasibility, and \
     approval are distinct Phase 4 gates.";

/// The `ScheduleProposed` payload-fit gap, recorded honestly (the D-FX-9 /
/// D-P3.12-7 precedent): the frozen `ScheduleProposedPayload` (P0.14/P1.09)
/// carries correlation/tenant/portfolio/site/asset + event_time only — NO
/// result-record reference, NO candidate id, NO status. flexcore-events was
/// frozen for this subtask, so (a) the nil asset id denotes "the schedule
/// scope, not a single asset" (the envelope's recorded convention), and (b)
/// the result-record reference travels in the deterministic idempotency key
/// `schedule_proposed:<portfolio>:<site>:<candidate_id>` (republication of
/// one candidate's proposal dedupes durably) and in the persisted record
/// (`schedule_results.schedule_proposed_key`). **Needs extension:**
/// additive `serde(default)` payload fields (`candidate_id`,
/// `result_record_ref`) per the P2.16 precedent, for a future subtask that
/// owns flexcore-events. The full record is the persisted
/// [`ScheduleResultRecord`], retrievable by candidate id.
pub const SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE: &str = "The frozen \
     ScheduleProposedPayload carries correlation/tenant/portfolio/site/asset \
     + event_time only: no candidate id and no result-record reference. The \
     reference travels in the idempotency key \
     schedule_proposed:<portfolio>:<site>:<candidate_id> and in the \
     persisted record; the nil asset id denotes the schedule scope. Needs \
     extension: additive serde(default) payload fields per the P2.16 \
     precedent, for a future subtask that owns flexcore-events.";

/// Where the §12.6 "grid model version" comes from, stated once (ledger
/// P3.21 step 2): the Phase 2 Site grid-model reference (P2.02) — the
/// version of the network model the site references. No Phase 3 or Phase 4
/// grid model exists; none may be invented.
pub const GRID_MODEL_PROVENANCE_STATEMENT: &str = "The recorded \
     grid_model_version is the version of the network model referenced by \
     the Phase 2 Site (P2.02 grid_model_reference) named in the record's \
     traceability; it is NOT a Phase 3 or Phase 4 grid model (neither \
     exists). The traceability names the site whose reference supplied it.";

// --- Input snapshot -----------------------------------------------------------

/// One asset's capability-version entry (ledger P3.21 step 2: "capability
/// versions"). The schema version is the P3.01 frozen contract version the
/// asset's capability document validated against; the profile reference is
/// the P2.02 asset capability-profile reference string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityVersionRef {
    /// The asset key (registry ID string form, input order).
    pub asset_key: String,
    /// The asset's capability-profile reference (P2.02 reference string).
    pub capability_profile_reference: String,
    /// The capability-contract schema version the asset's document used
    /// (must equal [`CAPABILITY_SCHEMA_VERSION`]).
    pub capability_schema_version: u32,
}

/// The forecast reference as consumed through the labeled accessor
/// ([`LabeledSeriesValues`]): the version and the provenance label travel
/// together, so the record can never cite an unlabelled forecast (the
/// four-place labelling rule; the snapshot is the fourth place).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastSnapshotRef {
    /// The forecast version (P3.31).
    pub forecast_version: ForecastVersion,
    /// The provenance label (always `Synthetic` in v0.2.0; enforced at
    /// assembly — a future provenance variant must be a recorded decision
    /// before it can enter a schedule record).
    pub forecast_provenance: Provenance,
}

/// The §12.6 input snapshot of one schedule record (ledger P3.21 step 2):
/// capability versions, envelope version, forecast version, objective
/// configuration, solver configuration, and the telemetry snapshot
/// reference. Every field is required — there is no "unknown snapshot".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSnapshotRecord {
    /// The per-asset capability versions (P3.01 schema version + P2.02
    /// profile reference per asset, input order).
    pub capability_versions: Vec<CapabilityVersionRef>,
    /// The envelope version the schedule was computed against (P3.12/P3.13).
    pub envelope: ScheduleEnvelopeReference,
    /// The forecast reference with its provenance label (P3.31).
    pub forecast: ForecastSnapshotRef,
    /// The objective configuration as ordered record fields — verbatim
    /// [`ObjectiveConfig::record_fields`], so the SYNTHETIC provenance
    /// label, every term's weight/unit/sign, and the §7.5 exclusion
    /// statement survive into the persisted record.
    pub objective_config_record: Vec<(String, String)>,
    /// The explicit objective name (P3.14: no universal/unnamed objective).
    pub objective_name: String,
    /// The solver configuration as its verifiable canonical JSON
    /// ([`SolverConfig::canonical_json`] — every configured value readable,
    /// not a blob).
    pub solver_config_json: String,
    /// The solver-configuration digest ([`SolverConfig::config_digest`]) —
    /// cross-checks the canonical JSON.
    pub solver_config_digest: String,
    /// The telemetry snapshot reference the solve consumed (caller-supplied
    /// pointer into the Phase 2 telemetry store).
    pub telemetry_snapshot_reference: String,
}

impl InputSnapshotRecord {
    /// Assembles and validates the input snapshot from its constituents.
    /// The forecast is taken as [`LabeledSeriesValues`] — the labeled
    /// accessor output — so an unlabelled forecast cannot be cited.
    pub fn assemble(
        capability_versions: &[CapabilityVersionRef],
        envelope_version: &EnvelopeVersion,
        forecast: LabeledSeriesValues<'_>,
        objective_config: &ObjectiveConfig,
        solver_config: &SolverConfig,
        telemetry_snapshot_reference: &str,
    ) -> Result<Self, ScheduleResultError> {
        if capability_versions.is_empty() {
            return Err(ScheduleResultError::EmptySnapshotField {
                field: "capability_versions",
            });
        }
        for capability in capability_versions {
            if capability.asset_key.trim().is_empty() {
                return Err(ScheduleResultError::EmptySnapshotField {
                    field: "capability_versions[].asset_key",
                });
            }
            if capability.capability_profile_reference.trim().is_empty() {
                return Err(ScheduleResultError::EmptySnapshotField {
                    field: "capability_versions[].capability_profile_reference",
                });
            }
            if capability.capability_schema_version != CAPABILITY_SCHEMA_VERSION {
                return Err(ScheduleResultError::CapabilitySchemaVersionMismatch {
                    asset_key: capability.asset_key.clone(),
                    found: capability.capability_schema_version,
                    supported: CAPABILITY_SCHEMA_VERSION,
                });
            }
        }
        if solver_config.validate().is_err() {
            return Err(ScheduleResultError::InvalidSolverConfig);
        }
        if telemetry_snapshot_reference.trim().is_empty() {
            return Err(ScheduleResultError::EmptySnapshotField {
                field: "telemetry_snapshot_reference",
            });
        }
        // The provenance gate: v0.2.0 has exactly one forecast source and it
        // is labelled synthetic. The match is exhaustive — a future
        // provenance variant cannot silently enter a schedule record.
        let provenance = match forecast.provenance {
            Provenance::Synthetic => Provenance::Synthetic,
        };
        Ok(Self {
            capability_versions: capability_versions.to_vec(),
            envelope: ScheduleEnvelopeReference::new(envelope_version.clone()),
            forecast: ForecastSnapshotRef {
                forecast_version: forecast.forecast_version.clone(),
                forecast_provenance: provenance,
            },
            objective_config_record: objective_config.record_fields(),
            objective_name: objective_config.name().to_string(),
            solver_config_json: solver_config.canonical_json(),
            solver_config_digest: solver_config.config_digest(),
            telemetry_snapshot_reference: telemetry_snapshot_reference.to_string(),
        })
    }

    /// The deterministic input-snapshot reference string: carried into the
    /// result's `input_snapshot_id` and into the persisted record, so a
    /// result references the EXACT snapshot used (ledger acceptance).
    pub fn reference(&self) -> String {
        format!(
            "schedule_input_snapshot:{}:{}:{}:{}:telemetry={}:assets={}",
            self.envelope.envelope_version,
            self.forecast.forecast_version,
            self.forecast.forecast_provenance.as_str(),
            self.solver_config_digest,
            self.telemetry_snapshot_reference,
            self.capability_versions.len(),
        )
    }
}

// --- Grid-model traceability --------------------------------------------------

/// The §12.6 grid-model traceability (ledger P3.21 step 2): the grid-model
/// version plus WHICH site's Phase 2 reference supplied it. Constructed
/// exclusively from a domain [`flexcore_domain::entities::Site`] — there is
/// no way to record a grid-model version detached from its site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridModelTraceability {
    site_id: SiteId,
    site_grid_model_reference: String,
    grid_model_version: String,
}

impl GridModelTraceability {
    /// Builds the traceability from a Phase 2 site and the version of the
    /// network model its reference identifies. The site's own
    /// `grid_model_reference` (P2.02, non-empty by the entity invariant)
    /// is recorded verbatim next to the version.
    pub fn from_site(
        site: &flexcore_domain::entities::Site,
        grid_model_version: &str,
    ) -> Result<Self, ScheduleResultError> {
        if grid_model_version.trim().is_empty() {
            return Err(ScheduleResultError::EmptyGridModelVersion);
        }
        Ok(Self {
            site_id: site.site_id(),
            site_grid_model_reference: site.grid_model_reference().to_string(),
            grid_model_version: grid_model_version.to_string(),
        })
    }

    /// The site whose grid-model reference supplied the version.
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// The verbatim P2.02 site grid-model reference.
    pub fn site_grid_model_reference(&self) -> &str {
        &self.site_grid_model_reference
    }

    /// The grid-model version recorded on the §12.6 result.
    pub fn grid_model_version(&self) -> &str {
        &self.grid_model_version
    }

    /// The one-line traceability statement for records and audit views.
    pub fn traceability_statement(&self) -> String {
        format!(
            "grid model version {} supplied by site {} (Phase 2 grid_model_reference: {})",
            self.grid_model_version, self.site_id, self.site_grid_model_reference
        )
    }
}

// --- The assembled schedule-result record -------------------------------------

/// The assembled P3.21 schedule-result record: the §12.6 result (completed
/// with its snapshot and grid-model wiring) plus the snapshot and the
/// grid-model traceability it cites. This is the object persisted by the
/// storage contract and referenced by `ScheduleProposed`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleResultRecord {
    tenant_id: TenantId,
    portfolio_id: PortfolioId,
    site_id: SiteId,
    result: OptimizationResult,
    input_snapshot: InputSnapshotRecord,
    grid_model: GridModelTraceability,
}

/// The assembly inputs: the §12.6 result from the `Optimizer` port, the
/// validated input snapshot, the grid-model traceability, and the objective
/// evaluation's scaling warnings (P3.15) — merged into the record's warning
/// list, never dropped.
pub struct ScheduleResultAssemblyInput {
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    pub site_id: SiteId,
    /// The §12.6 result as the provider returned it. It must already
    /// satisfy the port-level completeness gate; assembly consumes it and
    /// completes the wiring fields.
    pub result: OptimizationResult,
    pub grid_model: GridModelTraceability,
    pub input_snapshot: InputSnapshotRecord,
    /// The P3.15 scaling warnings from the objective evaluation. Rendered
    /// into the record's warnings (deterministically, in input order).
    pub objective_scaling_warnings: Vec<ScalingWarning>,
}

/// Assembles the schedule-result record (§12.6 completion; ledger P3.21
/// steps 1–2). Runs the port-level completeness gate first, then:
///
/// - completes `result.input_snapshot_id` with the snapshot's deterministic
///   reference (the result now references the exact snapshot used);
/// - completes `result.grid_model_version` from the traceability (and
///   verifies the traceability's site is the record's site);
/// - merges the P3.15 scaling warnings and — for every Tier 2 problem
///   class — the P3.18 `SOCP_EXACTNESS_CAVEAT` into `result.warnings`
///   (idempotently: an already-present statement is not duplicated).
pub fn assemble_schedule_result_record(
    input: ScheduleResultAssemblyInput,
) -> Result<ScheduleResultRecord, ScheduleResultError> {
    // Gate 1: the port-level §12.6 completeness. A result missing any
    // required field never becomes a schedule record.
    input.result.validate_complete()?;

    // Gate 2: the grid-model traceability must name THE record's site —
    // a traceability from a different site is a wiring error, not a note.
    if input.grid_model.site_id() != input.site_id {
        return Err(ScheduleResultError::GridModelSiteMismatch {
            record_site: input.site_id,
            traceability_site: input.grid_model.site_id(),
        });
    }

    let mut result = input.result;
    // Wire the snapshot and grid-model fields, refusing to overwrite a
    // provider-set value that disagrees (a silent overwrite could mask a
    // mis-wired solve; the ledger acceptance requires the result to
    // reference the EXACT snapshot used).
    let snapshot_reference = input.input_snapshot.reference();
    match result.input_snapshot_id.as_deref() {
        None => result.input_snapshot_id = Some(snapshot_reference),
        Some(existing) if existing == snapshot_reference => {}
        Some(existing) => {
            return Err(ScheduleResultError::SnapshotReferenceMismatch {
                recorded: existing.to_string(),
                expected: snapshot_reference,
            });
        }
    }
    match result.grid_model_version.as_deref() {
        None => {
            result.grid_model_version = Some(input.grid_model.grid_model_version().to_string())
        }
        Some(existing) if existing == input.grid_model.grid_model_version() => {}
        Some(existing) => {
            return Err(ScheduleResultError::GridModelVersionMismatch {
                recorded: existing.to_string(),
                expected: input.grid_model.grid_model_version().to_string(),
            });
        }
    }

    // Warnings: the P3.15 scaling warnings, then the SOCP caveat for every
    // Tier 2 result. Deterministic order: provider warnings, scaling
    // warnings, caveat. Nothing is deduplicated except the caveat itself
    // (a provider may legitimately repeat distinct warnings).
    for warning in &input.objective_scaling_warnings {
        result.warnings.push(render_scaling_warning(warning));
    }
    if result.problem_class.tier() == 2 && !result.warnings.iter().any(|w| w == SOCP_EXACTNESS_CAVEAT)
    {
        result.warnings.push(SOCP_EXACTNESS_CAVEAT.to_string());
    }

    Ok(ScheduleResultRecord {
        tenant_id: input.tenant_id,
        portfolio_id: input.portfolio_id,
        site_id: input.site_id,
        result,
        input_snapshot: input.input_snapshot,
        grid_model: input.grid_model,
    })
}

/// Renders one P3.15 [`ScalingWarning`] as a deterministic warning string
/// for the record's warning list.
fn render_scaling_warning(warning: &ScalingWarning) -> String {
    format!(
        "scaling_warning: term '{}' dominates '{}' (|{:.6}| / |{:.6}| = {:.3} >= threshold {:.3}); \
         scaling review required (P3.15 step 2)",
        warning.term.as_str(),
        warning.other_term.as_str(),
        warning.term_total,
        warning.other_term_total,
        warning.ratio,
        warning.threshold,
    )
}

impl ScheduleResultRecord {
    /// The owning tenant (P0.16 rule 1: `tenant_id` on every tenant-owned
    /// record).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The portfolio the schedule was computed for.
    pub fn portfolio_id(&self) -> PortfolioId {
        self.portfolio_id
    }

    /// The site the schedule was computed for (and whose Phase 2
    /// grid-model reference supplied the version).
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// The completed §12.6 result record.
    pub fn result(&self) -> &OptimizationResult {
        &self.result
    }

    /// The input snapshot the result references.
    pub fn input_snapshot(&self) -> &InputSnapshotRecord {
        &self.input_snapshot
    }

    /// The grid-model traceability.
    pub fn grid_model(&self) -> &GridModelTraceability {
        &self.grid_model
    }

    /// The candidate identity (§12.6 "output candidate ID"). Guaranteed
    /// present: the assembly's completeness gate rejects a result without
    /// one.
    pub fn candidate_id(&self) -> FlexUuid {
        self.result
            .candidate_id
            .expect("validated by the assembly's completeness gate")
    }

    /// The candidate identity in its canonical hyphenated text form — the
    /// record reference the storage layer, the idempotency key, and the
    /// event key all use.
    pub fn record_reference(&self) -> String {
        self.candidate_id().to_string()
    }

    /// The solver outcome status (the closed six-status vocabulary).
    pub fn status(&self) -> SolveStatus {
        self.result.status
    }

    /// The P3.21-level completeness check over the ASSEMBLED record: the
    /// port-level gate plus the wiring guarantees (snapshot reference set
    /// and equal to the snapshot's own reference; grid-model version set
    /// and equal to the traceability's version). Used by the completeness
    /// tests and available to the composition root before persistence.
    pub fn validate_record_complete(&self) -> Result<(), ScheduleResultError> {
        self.result
            .validate_complete()
            .map_err(ScheduleResultError::IncompleteResult)?;
        let snapshot_reference = self.input_snapshot.reference();
        if self.result.input_snapshot_id.as_deref() != Some(snapshot_reference.as_str()) {
            return Err(ScheduleResultError::SnapshotReferenceMismatch {
                recorded: self.result.input_snapshot_id.clone().unwrap_or_default(),
                expected: snapshot_reference,
            });
        }
        if self.result.grid_model_version.as_deref() != Some(self.grid_model.grid_model_version())
        {
            return Err(ScheduleResultError::GridModelVersionMismatch {
                recorded: self.result.grid_model_version.clone().unwrap_or_default(),
                expected: self.grid_model.grid_model_version().to_string(),
            });
        }
        // Residuals/violations: populated, or explicitly not available with
        // a non-empty reason (the port gate checks the reason; the presence
        // check for solving statuses is recorded policy, not an error —
        // per-solver diagnostics availability is the §12.6 contract).
        Ok(())
    }
}

// --- Persistence proof + ScheduleProposed emission -----------------------------

/// The composition-root proof that the record was persisted (§16.1). Built
/// from the storage receipt: the storage-side
/// `ScheduleRecordStore::insert_schedule_record` (flexcore-storage) returns
/// its own receipt; the composition root converts it into this proof, whose
/// `record_reference` and `idempotency_key` the publication function then
/// verifies against the record — a receipt for a DIFFERENT record or a
/// foreign event key cannot back a `ScheduleProposed`.
///
/// The binding is by composition-root convention (the same receipt-ordering
/// convention as the P1.08 `event_assembly` helper): the type removes the
/// "publish without a receipt" shape, and the equality checks remove the
/// "receipt for another record" shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedResultRecord {
    record_reference: String,
    idempotency_key: String,
}

impl PersistedResultRecord {
    /// Builds the proof from the storage receipt's values. Both fields must
    /// be non-empty.
    pub fn new(
        record_reference: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, ScheduleResultError> {
        let record_reference = record_reference.into();
        let idempotency_key = idempotency_key.into();
        if record_reference.trim().is_empty() {
            return Err(ScheduleResultError::EmptySnapshotField {
                field: "persisted.record_reference",
            });
        }
        if idempotency_key.trim().is_empty() {
            return Err(ScheduleResultError::EmptySnapshotField {
                field: "persisted.idempotency_key",
            });
        }
        Ok(Self {
            record_reference,
            idempotency_key,
        })
    }

    /// The persisted record's reference (candidate id text).
    pub fn record_reference(&self) -> &str {
        &self.record_reference
    }

    /// The idempotency key the record was persisted with.
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
}

/// The deterministic `ScheduleProposed` idempotency key for one candidate:
/// `schedule_proposed:<portfolio>:<site>:<candidate_id>` (see
/// [`SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE`] — the result-record reference
/// travels here until the payload is extended).
pub fn schedule_proposed_idempotency_key(
    portfolio_id: PortfolioId,
    site_id: SiteId,
    candidate_id: FlexUuid,
) -> String {
    format!("schedule_proposed:{portfolio_id}:{site_id}:{candidate_id}")
}

/// One assembled `ScheduleProposed` publication: the event envelope, its
/// destination subject (the frozen P1.14 scheme:
/// `flexcore.v1.scheduling.schedule_proposed`), and the idempotency key.
pub struct ScheduleProposedPublication {
    /// The envelope carrying the frozen `ScheduleProposed` event.
    pub envelope: EventEnvelope,
    /// The destination subject.
    pub destination_subject: Subject,
    /// The idempotency key — identical to the key the record was persisted
    /// with (verified at assembly).
    pub idempotency_key: String,
}

/// Assembles the `ScheduleProposed` publication for one persisted candidate
/// (§15.3; ledger P3.21 step 3). PURE assembly — no I/O; the wiring into
/// the transactional outbox lands with P3.28 through `event_assembly`.
///
/// Requires the persistence proof ([`PersistedResultRecord`]) and verifies
/// it: the proof's record reference must be this record's reference and its
/// idempotency key must equal this record's deterministic key. An event for
/// an unpersisted or mismatched record is unrepresentable.
pub fn schedule_proposed_publication(
    record: &ScheduleResultRecord,
    persisted: &PersistedResultRecord,
    correlation_id: FlexUuid,
    event_time: EventTime,
) -> Result<ScheduleProposedPublication, ScheduleResultError> {
    let expected_reference = record.record_reference();
    if persisted.record_reference() != expected_reference {
        return Err(ScheduleResultError::PersistenceReferenceMismatch {
            expected: expected_reference,
            found: persisted.record_reference().to_string(),
        });
    }
    let expected_key = schedule_proposed_idempotency_key(
        record.portfolio_id(),
        record.site_id(),
        record.candidate_id(),
    );
    if persisted.idempotency_key() != expected_key {
        return Err(ScheduleResultError::PersistenceKeyMismatch {
            expected: expected_key,
            found: persisted.idempotency_key().to_string(),
        });
    }
    // The frozen payload is asset-scoped; a schedule is portfolio/site-
    // scoped. The nil asset id denotes the schedule scope (recorded
    // convention; see SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE).
    let payload = ScheduleProposedPayload {
        correlation_id,
        tenant_id: record.tenant_id(),
        portfolio_id: record.portfolio_id(),
        site_id: record.site_id(),
        asset_id: AssetId::new(FlexUuid::nil()),
        event_time,
    };
    let event = DomainEvent::ScheduleProposed(payload);
    let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &expected_key, event)
        .map_err(|_| ScheduleResultError::EventEnvelopeRejected)?;
    let destination_subject = subject_for(&envelope.event);
    Ok(ScheduleProposedPublication {
        idempotency_key: envelope.idempotency_key.as_str().to_string(),
        destination_subject,
        envelope,
    })
}

// --- Errors --------------------------------------------------------------------

/// The P3.21 assembly/emission errors. The port-level completeness errors
/// are wrapped, never restated (single home: `flexcore-optimization-api`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleResultError {
    /// The §12.6 result failed the port-level completeness gate.
    IncompleteResult(ResultCompletenessError),
    /// A snapshot field that must be present is missing or empty (`field`
    /// names it).
    EmptySnapshotField {
        field: &'static str,
    },
    /// An asset's capability document carries a schema version other than
    /// the frozen P3.01 version.
    CapabilitySchemaVersionMismatch {
        asset_key: String,
        found: u32,
        supported: u32,
    },
    /// The solver configuration failed validation.
    InvalidSolverConfig,
    /// The grid-model version is missing or empty.
    EmptyGridModelVersion,
    /// The grid-model traceability names a site other than the record's
    /// site — the version would not be traceable to the record's own site.
    GridModelSiteMismatch {
        record_site: SiteId,
        traceability_site: SiteId,
    },
    /// The result already carried an `input_snapshot_id` that differs from
    /// the snapshot's own reference — refusing to overwrite it (the
    /// result must reference the EXACT snapshot used).
    SnapshotReferenceMismatch {
        recorded: String,
        expected: String,
    },
    /// The result already carried a `grid_model_version` that differs from
    /// the traceability.
    GridModelVersionMismatch {
        recorded: String,
        expected: String,
    },
    /// The persistence proof references a different record than the one
    /// being published.
    PersistenceReferenceMismatch {
        expected: String,
        found: String,
    },
    /// The persistence proof carries a different idempotency key than the
    /// record's deterministic key.
    PersistenceKeyMismatch {
        expected: String,
        found: String,
    },
    /// The events crate rejected the assembled envelope.
    EventEnvelopeRejected,
}

impl std::fmt::Display for ScheduleResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleResultError::IncompleteResult(err) => {
                write!(f, "the §12.6 result is incomplete: {err}")
            }
            ScheduleResultError::EmptySnapshotField { field } => {
                write!(f, "input snapshot field '{field}' is missing or empty")
            }
            ScheduleResultError::CapabilitySchemaVersionMismatch {
                asset_key,
                found,
                supported,
            } => write!(
                f,
                "asset '{asset_key}' carries capability schema version {found}; \
                 the frozen P3.01 contract version is {supported}"
            ),
            ScheduleResultError::InvalidSolverConfig => {
                write!(f, "the solver configuration failed validation (P3.19)")
            }
            ScheduleResultError::EmptyGridModelVersion => {
                write!(f, "the grid-model version is missing or empty")
            }
            ScheduleResultError::GridModelSiteMismatch {
                record_site,
                traceability_site,
            } => write!(
                f,
                "the grid-model traceability names site {traceability_site} but the \
                 record's site is {record_site}; the version would not be traceable \
                 to the record's own site reference"
            ),
            ScheduleResultError::SnapshotReferenceMismatch { recorded, expected } => write!(
                f,
                "the result's input_snapshot_id '{recorded}' does not match the input \
                 snapshot's deterministic reference '{expected}'"
            ),
            ScheduleResultError::GridModelVersionMismatch { recorded, expected } => write!(
                f,
                "the result's grid_model_version '{recorded}' does not match the recorded \
                 grid-model traceability '{expected}'"
            ),
            ScheduleResultError::PersistenceReferenceMismatch { expected, found } => write!(
                f,
                "the persistence proof references record '{found}' but the publication \
                 is for '{expected}'"
            ),
            ScheduleResultError::PersistenceKeyMismatch { expected, found } => write!(
                f,
                "the persistence proof carries idempotency key '{found}' but the record's \
                 deterministic key is '{expected}'"
            ),
            ScheduleResultError::EventEnvelopeRejected => {
                write!(f, "the events crate rejected the assembled ScheduleProposed envelope")
            }
        }
    }
}

impl std::error::Error for ScheduleResultError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScheduleResultError::IncompleteResult(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast_fixture::ForecastVersion;
    use flexcore_optimization_api::problem::ProblemClass;
    use std::time::Duration;

    fn candidate() -> FlexUuid {
        FlexUuid::parse("00000000-0000-0000-0000-00000000b21d").expect("parses")
    }

    fn sample_result(status: SolveStatus, class: ProblemClass) -> OptimizationResult {
        OptimizationResult {
            status,
            problem_class: class,
            solver_name: "test-solver".to_string(),
            solver_version: "1.0.0".to_string(),
            provider_id: "test-provider".to_string(),
            provider_version: "0.1.0".to_string(),
            objective_value: Some(12.5),
            objective_units: Some("currency-units".to_string()),
            primal: flexcore_optimization_api::result::Diagnostics::Available(
                flexcore_optimization_api::result::PrimalDiagnostics {
                    values: vec![flexcore_optimization_api::result::VariableValue {
                        name: "P_export_t0".to_string(),
                        unit: "MW".to_string(),
                        value: 1.5,
                    }],
                },
            ),
            dual: flexcore_optimization_api::result::Diagnostics::NotAvailable {
                reason: "the test solver does not expose duals".to_string(),
            },
            residuals: flexcore_optimization_api::result::Diagnostics::Available(
                flexcore_optimization_api::result::ResidualReport {
                    max_primal_violation: Some(0.0),
                    max_dual_violation: None,
                    iteration_count: Some(7),
                },
            ),
            infeasibility_evidence: flexcore_optimization_api::result::Diagnostics::NotAvailable {
                reason: "not an infeasible result".to_string(),
            },
            runtime: Duration::from_millis(12),
            resources: None,
            candidate_id: Some(candidate()),
            input_snapshot_id: None,
            data_timestamp: None,
            grid_model_version: None,
            uncertainty_and_assumptions: vec![
                "SYNTHETIC objective inputs (D1/D2 OPEN)".to_string()
            ],
            warnings: vec![],
            determinism: Some(flexcore_optimization_api::result::DeterminismRecord {
                solver_version: "1.0.0".to_string(),
                thread_count: 1,
                time_limit_ms: None,
                config_digest: "fnv1a64:0000000000000000".to_string(),
            }),
        }
    }

    fn snapshot() -> InputSnapshotRecord {
        let capability = CapabilityVersionRef {
            asset_key: "asset-1".to_string(),
            capability_profile_reference: "bess-profile-v1".to_string(),
            capability_schema_version: CAPABILITY_SCHEMA_VERSION,
        };
        let envelope_version = EnvelopeVersion::new("env-v1").expect("valid version");
        let forecast_version = ForecastVersion::new("v1").expect("valid version");
        let objective = ObjectiveConfig::default_synthetic();
        let solver_config = SolverConfig::default();
        let forecast_values = LabeledSeriesValues {
            provenance: Provenance::Synthetic,
            forecast_version: &forecast_version,
            quantity: crate::forecast_fixture::ForecastQuantity::PvOutputMw,
            values: &[1.0],
        };
        InputSnapshotRecord::assemble(
            &[capability],
            &envelope_version,
            forecast_values,
            &objective,
            &solver_config,
            "telemetry-snapshot-2026-11-07",
        )
        .expect("assembles")
    }

    #[test]
    fn snapshot_reference_is_deterministic_and_complete() {
        let a = snapshot().reference();
        let b = snapshot().reference();
        assert_eq!(a, b, "the reference is deterministic");
        for constituent in [
            "schedule_input_snapshot:",
            "env-v1",
            "v1",
            "synthetic",
            "telemetry=telemetry-snapshot-2026-11-07",
            "assets=1",
        ] {
            assert!(a.contains(constituent), "reference '{a}' lacks '{constituent}'");
        }
        // The solver config digest is part of the reference, so a different
        // configuration yields a different snapshot reference.
        let mut other_solver = SolverConfig::default();
        other_solver.time_limit = Some(Duration::from_secs(30));
        let forecast_version = ForecastVersion::new("v1").expect("valid version");
        let forecast_values = LabeledSeriesValues {
            provenance: Provenance::Synthetic,
            forecast_version: &forecast_version,
            quantity: crate::forecast_fixture::ForecastQuantity::PvOutputMw,
            values: &[1.0],
        };
        let other = InputSnapshotRecord::assemble(
            &[CapabilityVersionRef {
                asset_key: "asset-1".to_string(),
                capability_profile_reference: "bess-profile-v1".to_string(),
                capability_schema_version: CAPABILITY_SCHEMA_VERSION,
            }],
            &EnvelopeVersion::new("env-v1").expect("valid version"),
            forecast_values,
            &ObjectiveConfig::default_synthetic(),
            &other_solver,
            "telemetry-snapshot-2026-11-07",
        )
        .expect("assembles");
        assert_ne!(a, other.reference(), "the solver config discriminates");
    }

    #[test]
    fn assembly_completes_the_wiring_fields_and_merges_warnings() {
        let mut result = sample_result(SolveStatus::Optimal, ProblemClass::LinearProgram);
        result.warnings.push("provider: benign".to_string());
        let record = assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            grid_model: GridModelTraceability {
                site_id: SiteId::new(FlexUuid::nil()),
                site_grid_model_reference: "site-grid-ref-1".to_string(),
                grid_model_version: "grid-model-v1".to_string(),
            },
            input_snapshot: snapshot(),
            objective_scaling_warnings: vec![],
            result,
        })
        .expect("assembles");

        // The wiring fields are completed from the snapshot and traceability.
        assert_eq!(
            record.result().input_snapshot_id.as_deref(),
            Some(record.input_snapshot().reference().as_str())
        );
        assert_eq!(
            record.result().grid_model_version.as_deref(),
            Some("grid-model-v1")
        );
        record.validate_record_complete().expect("complete");
        // A Tier 1 result does NOT carry the SOCP caveat.
        assert!(!record
            .result()
            .warnings
            .iter()
            .any(|w| w == SOCP_EXACTNESS_CAVEAT));
        // The provider warning survived.
        assert!(record.result().warnings.iter().any(|w| w == "provider: benign"));
    }

    #[test]
    fn tier2_results_carry_the_socp_caveat_exactly_once() {
        let mut result =
            sample_result(SolveStatus::Optimal, ProblemClass::ContinuousSecondOrderConeProgram);
        // The provider already recorded the caveat (the P3.18 behaviour):
        // assembly must not duplicate it.
        result.warnings.push(SOCP_EXACTNESS_CAVEAT.to_string());
        let record = assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            grid_model: GridModelTraceability {
                site_id: SiteId::new(FlexUuid::nil()),
                site_grid_model_reference: "site-grid-ref-1".to_string(),
                grid_model_version: "grid-model-v1".to_string(),
            },
            input_snapshot: snapshot(),
            objective_scaling_warnings: vec![],
            result,
        })
        .expect("assembles");
        assert_eq!(
            record
                .result()
                .warnings
                .iter()
                .filter(|w| *w == SOCP_EXACTNESS_CAVEAT)
                .count(),
            1,
            "the SOCP caveat appears exactly once"
        );
    }

    #[test]
    fn a_result_missing_a_required_field_is_rejected() {
        // Missing candidate id (the §12.6 required field).
        let mut result = sample_result(SolveStatus::Optimal, ProblemClass::LinearProgram);
        result.candidate_id = None;
        let error = assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            grid_model: GridModelTraceability {
                site_id: SiteId::new(FlexUuid::nil()),
                site_grid_model_reference: "site-grid-ref-1".to_string(),
                grid_model_version: "grid-model-v1".to_string(),
            },
            input_snapshot: snapshot(),
            objective_scaling_warnings: vec![],
            result,
        })
        .expect_err("missing candidate id must fail");
        assert_eq!(
            error,
            ScheduleResultError::IncompleteResult(ResultCompletenessError::MissingCandidateId)
        );
    }

    #[test]
    fn the_traceability_must_name_the_records_own_site() {
        let other_site = SiteId::new(FlexUuid::parse("00000000-0000-0000-0000-0000000000a5").expect("parses"));
        let result = sample_result(SolveStatus::Optimal, ProblemClass::LinearProgram);
        let error = assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            grid_model: GridModelTraceability {
                site_id: other_site,
                site_grid_model_reference: "foreign-site-grid".to_string(),
                grid_model_version: "grid-model-v1".to_string(),
            },
            input_snapshot: snapshot(),
            objective_scaling_warnings: vec![],
            result,
        })
        .expect_err("a foreign site's traceability must fail");
        assert!(matches!(
            error,
            ScheduleResultError::GridModelSiteMismatch { .. }
        ));
    }

    #[test]
    fn the_publication_requires_the_matching_persistence_proof() {
        let record = assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            grid_model: GridModelTraceability {
                site_id: SiteId::new(FlexUuid::nil()),
                site_grid_model_reference: "site-grid-ref-1".to_string(),
                grid_model_version: "grid-model-v1".to_string(),
            },
            input_snapshot: snapshot(),
            objective_scaling_warnings: vec![],
            result: sample_result(SolveStatus::Optimal, ProblemClass::LinearProgram),
        })
        .expect("assembles");
        let key = schedule_proposed_idempotency_key(
            record.portfolio_id(),
            record.site_id(),
            record.candidate_id(),
        );
        let persisted =
            PersistedResultRecord::new(record.record_reference(), key.clone()).expect("proof");

        // The matching proof assembles the event.
        let publication = schedule_proposed_publication(
            &record,
            &persisted,
            FlexUuid::nil(),
            EventTime::from_unix_millis(1_700_000_000_000).expect("valid"),
        )
        .expect("publishes");
        assert_eq!(publication.idempotency_key, key);
        assert_eq!(
            publication.destination_subject.as_str(),
            "flexcore.v1.scheduling.schedule_proposed"
        );

        // A proof for another record is rejected.
        let foreign = PersistedResultRecord::new("other-record", key.clone()).expect("proof");
        assert!(matches!(
            schedule_proposed_publication(
                &record,
                &foreign,
                FlexUuid::nil(),
                EventTime::from_unix_millis(1_700_000_000_000).expect("valid"),
            ),
            Err(ScheduleResultError::PersistenceReferenceMismatch { .. })
        ));

        // A proof with a foreign key is rejected.
        let foreign_key =
            PersistedResultRecord::new(record.record_reference(), "other:key").expect("proof");
        assert!(matches!(
            schedule_proposed_publication(
                &record,
                &foreign_key,
                FlexUuid::nil(),
                EventTime::from_unix_millis(1_700_000_000_000).expect("valid"),
            ),
            Err(ScheduleResultError::PersistenceKeyMismatch { .. })
        ));
    }
}
