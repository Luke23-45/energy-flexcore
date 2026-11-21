//! Neryva FlexCore — use cases and orchestration, ports only (P1.08).
//!
//! The application layer of the hexagonal architecture (report §6.1–§6.3,
//! §8.1): the Stage 1 registry use cases the API layer composes and the
//! exit-gate workflow (P1.26) exercises. Every dependency is a port trait
//! (flexcore-ports) or a frozen vocabulary type (flexcore-domain,
//! flexcore-contracts, flexcore-events); nothing here touches a database,
//! a broker, a transport, or a serialization library — the adapters own all
//! of that (P1.13, P1.15, P1.19).
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`error`] | P1.08 step 5 | The error taxonomy: domain/validation, authorization, and infrastructure errors as distinct types; infrastructure failures carry an opaque code + message only |
//! | [`payload_encoding`] | P1.09 schema | The `EventPayloadEncoder` seam: the wire rendering of an event envelope is injected, because this crate imports no serialization library |
//! | [`register_tenant`] | report §10.1 row 1 | `RegisterTenant` — register a tenant (duplicate rejection on the identity-provider reference) |
//! | [`create_portfolio`] | report §10.1 row 2 | `CreatePortfolio` — create a portfolio under the caller's tenant |
//! | [`create_site`] | report §10.1 row 3 | `CreateSite` — create a site under a portfolio of the caller's tenant |
//! | [`register_asset`] | report §10.1 row 4; §15.3 `AssetRegistered` | `RegisterAsset` — register an asset under a site of the caller's tenant, emitting the frozen `AssetRegistered` event |
//! | [`update_asset_metadata`] | P2.03; §15.3 `AssetCapabilityUpdated` | `UpdateAssetMetadata` — update an asset's vendor/model, rated power/energy, and capability-profile reference (reference only in Phase 2), emitting the frozen `AssetCapabilityUpdated` event |
//! | [`deactivate_asset`] | P2.03; §15.3 `AssetAvailabilityChanged` | `DeactivateAsset` — deactivate an asset with a mandatory persisted reason (terminal lifecycle transition, P2.02), emitting the frozen `AssetAvailabilityChanged` event; the direct `AuditSink` append is a recorded gap (see the module header) |
//! | [`query_registry`] | P2.03 | `QueryRegistry` — tenant-scoped registry list/get reads, thin wrappers over the P1.07 repository ports; reads emit no events |
//! | `event_assembly` (private) | report §6.3 step 1; §15.5 | The shared outbox-write assembly: one transactional write per state change, pairing the repository receipt with the event's key, subject, and payload |
//! | [`telemetry_dedup`] | P2.09; §6.3, §6.4 | Duplicate and out-of-order handling: the record identity and per-record idempotency key (D-P3), the exact-vs-conflicting duplicate rule, the pure current-state (latest-`event_time`, never-regress) rule, the inbox-check consumer-side idempotency, and the batch deduplicator the P2.10 ingestion pipeline drives over the `TelemetryIngestStore`/`IngestionInbox` ports |
//! | [`availability`] | P2.16 step 3; §15.3 `AssetAvailabilityChanged` | The asset availability rule (DECIDED ONCE, pure): availability derived from the latest `availability` record's quality + boolean value plus the source connectivity signal; STALE/MISSING leave the previous availability unchanged, BAD is never trusted; the `AssetAvailabilityChanged` event fires exactly on a state change |
//! | [`forecast_fixture`] | P3.31; §11.6, §12.1, §15.3 `ForecastPublished` | The synthetic forecast fixture (P3.31): the labeled schema (values only accessible through the labeled accessor), the deterministic splitmix64 generator over the P2.13 exit-gate scenario's horizon/cadence/seed, the P3.26 PV-ramp error variant with a stated magnitude, the envelope input validation that rejects unlabelled/unversioned forecasts with named errors, and the pure `ForecastPublished` outbox assembly (payload gap recorded, see the module header) |
//! | [`flexibility_envelope`] | P3.12; §11.6, §24 step 3, §15.3 `FlexibilityEnvelopeUpdated` | The portfolio flexibility envelope as a CONSTRAINED aggregation (never a nameplate sum): the frozen P3.02 unknown-narrows rule reused per asset, per-interval forecast caps through the labeled accessor only, the conservative horizon-start energy cap for BESS, the shared site PCC coupling that binds the aggregate, the recorded correlation treatment (declared groups with a configured ASSUMPTION-labelled derate; no correlation coefficient invented), the explicit "confidence not supported" descriptor, and the pure `FlexibilityEnvelopeUpdated` outbox assembly (payload gap recorded, see the module header) |
//! | [`envelope_staleness`] | P3.13; §11.6, §10.4, §15.3 | The envelope staleness discipline: the pure event-driven invalidation over the frozen trigger payloads (`TelemetryQualityChanged`, `AssetAvailabilityChanged`, `AssetCapabilityUpdated`, `ForecastPublished`), horizon rollover, the configured maximum age bound (recorded default), the debounce-aware recomputation advice that can never weaken the staleness guarantee (see the module header), the schedule-envelope reference type, and the stale-never-used / superseded-version gates (enforcement wiring lands P3.22/P3.24) |
//!
//! # Recorded decisions (crate README has the full versions)
//!
//! - **Use-case shape**: one use-case struct per entity
//!   (`RegisterTenant`, `CreatePortfolio`, `CreateSite`, `RegisterAsset`),
//!   each taking the repository ports it needs plus the injected clock,
//!   ID generator, outbox, and payload encoder; composed by the API layer.
//!   The alternative — one orchestrated mega use case — was rejected: it
//!   would couple four registries into one struct and duplicate the
//!   per-entity duplicate/parent rules.
//! - **Transactional outbox**: every use case writes its state change and
//!   its domain event as **one** outbox write through the `EventOutbox`
//!   port, pairing the `StateChangeReceipt` the repository write returned
//!   with the event's idempotency key, destination subject, and serialized
//!   payload. The all-or-nothing execution inside one native transaction is
//!   the composition root's unit-of-work boundary plus the adapter's
//!   atomicity obligation (P1.13); the ports fakes evidence the ordering in
//!   tests.
//! - **Events**: `RegisterAsset` emits the frozen `AssetRegistered`
//!   (§15.3). The frozen vocabulary defines **no** creation events for
//!   tenant, portfolio, and site registration and none may be invented
//!   (P0.14), so those use cases emit `OperatorActionRecorded` (the
//!   §15.3 audit-area event: an operator action was recorded) — a recorded
//!   assumption for review.
//! - **Tenant context**: the tenant-scoped use cases take the domain
//!   `TenantContext` and never read a tenant identifier from a command
//!   field (P0.16 rule 5). `RegisterTenant` takes no context: the tenant
//!   registry root is not tenant-owned data (recorded ports decision).
//! - **Claims boundary gap (recorded)**: no code outside flexcore-domain
//!   can construct `AuthenticatedClaims` (P1.06 deliberate boundary), so no
//!   `TenantContext` can be produced anywhere yet. The `TenantContextSource`
//!   port (flexcore-ports) is the documented integration point for the API
//!   middleware; see the crate README for the full gap statement and what
//!   it blocks in tests.
//!
//! # Scope and control status
//!
//! Registry use cases for the design baseline only. The entire 0.x line is
//! simulation-only; nothing here authorises field control or device
//! writes. FlexCore is a supervisory orchestration layer and performs no
//! primary frequency response, synthetic inertia, relay protection,
//! inverter inner-loop control, or sub-cycle control.

pub mod availability;
pub mod create_portfolio;
pub mod create_site;
pub mod deactivate_asset;
pub mod error;
pub mod forecast_fixture;
pub mod payload_encoding;
pub mod query_registry;
pub mod register_asset;
pub mod register_tenant;
pub mod telemetry_dedup;
pub mod update_asset_metadata;

// P3.14/P3.15 (scheduling objective) and P3.16 (PCC tracking) — appended
// additively per the phase subtask file-ownership agreement; existing
// module declarations above are untouched.
pub mod pcc_tracking;
pub mod scheduling_objective;

// P3.12 (portfolio flexibility envelope) and P3.13 (envelope staleness and
// recomputation triggers) — appended additively per the same agreement.
pub mod envelope_staleness;
pub mod flexibility_envelope;

// P3.21 — optimization result contract: record assembly over the §12.6
// types (reused from flexcore-optimization-api — see the manifest note),
// input-snapshot linking with grid-model traceability (P2.02 Site
// reference), the pure `ScheduleProposed` outbox assembly, and the
// persistence-before-use proof (§16.1).
pub mod optimization_result_record;

// P3.23 — schedule comparison workflow: the shared-input comparability gate
// (cross-input comparisons REJECTED with the named IncomparableInputs error,
// D-P3.23-2), the per-candidate §19.4 scheduling metrics, the first Tier 1
// vs Tier 2 comparison case over the P3.18 cross-check record, and the
// recorded P3.28 REST-wiring note. Pure application functions + types; the
// persistence contract is flexcore-storage::schedule_store's
// ScheduleComparisonStore.
pub mod schedule_comparison;

// P3.24 — rolling-horizon re-scheduling: the recorded trigger set (REQUIRED
// per revision), the realised-state carry-forward, the frozen-past /
// re-optimize-future split, the schedule projection with the P3.11
// opt-out discipline, and the per-revision `ScheduleProposed` assembly over
// the P3.21 machinery. Pure application functions; no solver call, no I/O.
pub mod rolling_horizon;

// P4.11/P4.12/P4.13 — the command gates: creation + validation (P4.11),
// feasibility (P4.12), and authorization (P4.13) orchestration over the
// P4.10 lifecycle engine, with the collaborators injected as port traits
// (the seam pattern; see the module header). Appended additively per the
// phase subtask file-ownership agreement.
pub mod command_gates;

// P4.14 — the §14.3 command record schema: the flat field-level record,
// the creation/issue completeness validators, the §14.3 final-paragraph
// rejection rules, the per-asset acknowledgement types, and the full
// journal replay shared by P4.15/P4.16. Appended additively per the same
// agreement.
pub mod command_record;

// P4.15 — expiry (the recorded validity-window sweep), cancellation, and
// supersede over the P4.10 engine + the P4.14 replay, with the frozen
// DispatchExpired/DispatchCancelled/DispatchSuperseded events via the
// transactional outbox. Appended additively per the same agreement.
pub mod command_expiry;

// P4.16 — the simulation-only dispatch adapter: the DispatchAdapter
// surface is the `SimulatorDispatchAdapter` behind the
// `SimulatorDispatchTransport` port declared in this module (the crate
// cannot name flexcore-simulator); issue/acknowledgement/tracking over
// the P4.10–P4.15 machinery, with the hard simulator-only target guard.
// Appended additively per the same agreement.
pub mod dispatch_simulator;

// P4.17/P4.18 — asset-specific fallback policies (§14.4) with the bounded
// hold, escalation, and recovery rule: the §14.4 variant set with NO
// universal "set power to zero" fallback, the per-asset/site registry with
// the explicit require-operator-intervention default entry, FALLBACK entry
// from FAILED/EXPIRED only where the policy requires it (through the frozen
// §14.1 edges, never bypassing the P4.10 domain engine), the bounded hold
// that expires and escalates per policy (an audit record, NOT a command
// transition — FALLBACK is terminal in the frozen table), and the recovery
// rule: a fresh command through the FULL gate chain, no FALLBACK → ISSUED
// shortcut. Appended additively per the phase subtask file-ownership
// agreement.
pub mod fallback_policy;

// P4.19 — dispatch timing honesty (§14.5): the supervisory-setpoint-delivery
// boundary constants (no hard-real-time, frequency, inertia, relay, inner-
// loop, or sub-cycle claims anywhere), the measured acknowledgement-latency /
// tracking-deviation sample taken from the P4.10 transition journal on the
// §24 step 8 simulation path, and the measured-distribution report that
// always carries its conditions and embeds no guarantee and no target.
// Appended additively per the same agreement.
pub mod timing_honesty;

// P4.37 — the constrain/return consumption contract (§12.5 step 8): the
// scheduling-side mapping of the feasibility gate's frozen outcomes (the
// D-P4.07-1 routing rule consumed) — CONSTRAIN attaches the violated
// constraint + the constraining proposal as problem inputs of the
// constrained candidate re-entering the gate; RETURN re-enqueues the
// candidate into rolling-horizon re-optimization with EVERY violated
// constraint as a named input; REJECTED/HELD/INCONCLUSIVE/WITHIN-LIMITS
// route per the frozen rule with no scheduling action; an unconsumable
// constraint REJECTS with the recorded reason (D-P4.37-2). The outcome
// codes and the constraint-name grammar are MIRRORED from
// flexcore-grid-api (not an allowed dependency edge; the composition root
// converts), pinned by tests. Appended additively per the phase subtask
// file-ownership agreement.
pub mod feasibility_return;

// P4.31 — shadow evaluation (§21.4): the comparison method for dispatch
// proposals against the simulator's BASELINE (no-command) behaviour
// WITHOUT issuing anything — the read-only `ShadowBaselineSource` port
// declared here (the composition root implements it over the Phase 2
// simulator crate; the crate cannot name flexcore-simulator), the
// fail-closed per-instant classification (missing/stale/future-stamped/
// quantity-mismatched baselines are never read as agreement), the
// conservative overall verdict, and the report type that CARRIES its
// limits statement: shadow agreement is not a safety claim and shadow
// results are never evidence for field dispatch. Appended additively per
// the phase subtask file-ownership agreement.
pub mod shadow_evaluation;

// P4.28 — dispatch security controls (§17.3–§17.4): the payload-bound
// simulation signing/verification layer over the P4.16 keyring pattern
// (fnv1a64 integrity references, non-cryptographic, dev-labelled), the
// per-asset/channel sequence guard (replayed vs out-of-order refusals),
// the verifying-transport decorator (verify + sequence-admit before the
// inner transport; refusals are application-layer NACKs), and the
// cross-tenant denial audit record + SecurityAuditSink seam (the P4.23
// recorded open item; the port is declared here — the crate cannot depend
// on flexcore-audit). Appended additively per the phase subtask
// file-ownership agreement.
pub mod dispatch_security;

// P15.01 — the forecasting architecture record: the forecasts-as-inputs
// boundary (ADR-010 extension as an architecture record), the forecast
// artifact types (load/PV/price, per site/portfolio) and horizons
// (DA/intra-day), the producer permission gate
// (`telemetry.read`/`forecast.publish` only — no dispatch permission),
// and the provenance + signing mirror of the P8.32 model-artifact signing
// layer (typed; fail-closed default verifier). Appended additively per
// the phase subtask file-ownership agreement.
pub mod forecast_architecture;

// P15.02 — the load forecasting service design: persistence,
// seasonal-naive, and degree-day regression baselines as deterministic,
// registry-frozen references (the P8.06 freeze discipline applied to
// forecasting); EXPLICIT no-skill claims until an evaluation run (P15.05);
// the P3.10 SYNTHETIC_BASELINE_DISCLAIMER superseded only by evaluated
// baselines, never silently. Appended additively per the same agreement.
pub mod forecast_baselines;

// P15.03 — the PV generation forecasting design: weather-input
// requirements against the vendor-neutral weather port
// (flexcore-ports::weather_data), UNTRAINED model declarations, and the
// P15-E-1 build-vs-buy binding (vendor naming is a RECOMMENDATION label,
// never a register source; no vendor integration without the
// E-decision). Appended additively per the same agreement.
pub mod forecast_pv;

// P15.05 — the forecast evaluation framework (the X-8 extension): the
// metric vocabulary (MAE, RMSE, skill score vs persistence), the frozen
// backtesting protocol, pre-registered thresholds, and the STRUCTURAL
// emptiness of every result cell (the run-gate type has no public
// constructor — a filled record is unconstructable in-program). Appended
// additively per the same agreement.
pub mod forecast_evaluation;

// P15.06 — the model registry and forecast-artifact versioning:
// append-only + digest-chained, model→artifact lineage validated at
// registration, replay determinism (BTreeMap, injected timestamps),
// UNTRAINED as a first-class registered state, and
// registry-change-is-a-new-version. Appended additively per the same
// agreement.
pub mod model_registry;

// P15.12 — fleet health scoring: deviation-from-normal models per asset
// class, alarm normalization into prioritized issues, and availability
// KPIs independent of O&M (the Nispera pattern [S-53] as a DESIGN
// reference). Scores are RECORDS feeding operator views — never dispatch
// inputs (ADR-010 extension). Appended additively per the phase subtask
// file-ownership agreement (wave 2, Agent A).
pub mod fleet_health;

// P15.13 — telemetry-drift detection: reporting-rate expectations and
// value plausibility bounds per asset class (twin-lite — explicitly NOT
// grid state estimation [S-168]); every input is a NormalizedTelemetry
// record from the EXISTING Phase-2 TelemetrySource port — no parallel
// ingestion path; drift verdicts never rewrite quality states.
// Appended additively per the same agreement.
pub mod telemetry_drift;

// P15.14 — per-asset calibrated model records: declared thermal and
// efficiency model RECORDS with provenance (never fitted claims); the
// Phase 2 simulator profile's recorded absence ("no temperature,
// degradation, or state-of-health model") superseded AS RECORDED
// PARAMETERS — the profile file itself is untouched. Appended additively
// per the same agreement.
pub mod calibrated_model_records;

// P15.16 — forecast-to-schedule integration: envelopes and
// rolling-horizon scheduling consume real forecasts with the P3.13
// staleness discipline (composed, never restated), falling back LOUDLY
// to the P15.02 persistence baseline on staleness; the P3.31 fixture
// stays the labeled synthetic fallback source. Appended additively per
// the same agreement.
pub mod forecast_integration;

mod event_assembly;

pub use availability::{
    availability, availability_changed, AvailabilityAssessment, AvailabilitySignalInput,
    AvailabilityState,
};
pub use create_portfolio::{CreatePortfolio, CreatePortfolioCommand};
pub use create_site::{CreateSite, CreateSiteCommand};
pub use deactivate_asset::{DeactivateAsset, DeactivateAssetCommand};
pub use error::ApplicationError;
pub use forecast_fixture::{
    forecast_publication, generate_central_fixture, generate_error_pv_ramp_fixture,
    validate_forecast_input, FixtureGrid, FixtureMeta, FixtureMethod, ForecastErrorVariant,
    ForecastFixture, ForecastFixtureError, ForecastPublication, ForecastPublicationInput,
    ForecastQuantity, ForecastVersion, LabeledForecastSeries, LabeledSeriesValues, Provenance,
    UnvalidatedErrorVariant, UnvalidatedForecastInput, UnvalidatedSeries, CENTRAL_VARIANT,
    ERROR_PV_RAMP_VARIANT, FIXTURE_ID, PROVENANCE_SYNTHETIC,
};
pub use payload_encoding::EventPayloadEncoder;
pub use query_registry::QueryRegistry;
pub use register_asset::{RegisterAsset, RegisterAssetCommand};
pub use register_tenant::{RegisterTenant, RegisterTenantCommand};
pub use telemetry_dedup::{
    CurrentStateTransition, DedupError, DedupReport, DuplicateClassification, RecordDisposition,
    RecordIdentity, TelemetryDeduplicator, current_state_transition, delivery_key,
    duplicate_classification,
};
pub use update_asset_metadata::{UpdateAssetMetadata, UpdateAssetMetadataCommand};

// P3.12/P3.13 — appended additively per the phase subtask file-ownership
// agreement.
pub use envelope_staleness::{
    assert_usable_for_new_schedule, check_schedule_envelope_reference, evaluate_envelope_trigger,
    evaluate_max_age, DEBOUNCE_GUARANTEE_STATEMENT, DEFAULT_MAX_AGE_MILLIS,
    EnvelopeFreshness, EnvelopeReferenceError, EnvelopeTriggerEvent, RecomputationAdvice,
    ScheduleEnvelopeReference, StalenessConfig, StalenessReason, StalenessVerdict,
    STALE_ENVELOPE_POLICY,
};
pub use flexibility_envelope::{
    calculate_portfolio_envelope, envelope_updated_publications, AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED,
    CORRELATION_TREATMENT_STATEMENT, ENVELOPE_PAYLOAD_GAP_NOTE, ENVELOPE_TOLERANCE,
    AssetAvailabilityInput, AssetContribution, AssetEnvelopeInput, CorrelationTreatmentRecord,
    EnvelopeComputationInput, EnvelopeError, EnvelopeInputSnapshot, EnvelopeInterval,
    EnvelopeIntervalStatus, EnvelopeMetadata, EnvelopePublication, EnvelopePublicationInput,
    EnvelopeUnknownDimension, EnvelopeUnknownKind, EnvelopeVersion, InfeasibilityCause,
    LossAdjustment, PortfolioFlexibilityEnvelope, ProvenancedScalar, SharedExposureGroup,
    SiteCouplingInput, EnergyCouplingInput,
};

// P3.21 — appended additively per the phase subtask file-ownership
// agreement.
pub use optimization_result_record::{
    assemble_schedule_result_record, schedule_proposed_idempotency_key,
    schedule_proposed_publication, CANDIDATE_STATUS_STATEMENT, GRID_MODEL_PROVENANCE_STATEMENT,
    PERSIST_BEFORE_USE_STATEMENT, SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE, CapabilityVersionRef,
    ForecastSnapshotRef, GridModelTraceability, InputSnapshotRecord, PersistedResultRecord,
    ScheduleProposedPublication, ScheduleResultAssemblyInput, ScheduleResultError,
    ScheduleResultRecord,
};

// P3.23/P3.24 — appended additively per the phase subtask file-ownership
// agreement.
pub use rolling_horizon::{
    plan_rolling_horizon_roll, project_rolling_schedule,
    rolling_horizon_schedule_proposed_publication, AssetDropout, AssetSetpoint,
    CarriedAssetState, IntervalPlan, PriorRevision, RealisedAssetValue, RealisedInterval,
    RealisedState, RevisionIdentity, RollPlan, RollingHorizonError, RollingHorizonPublicationInput,
    RollingScheduleProjection, RollRequest, RollTrigger, CARRY_FORWARD_STATEMENT,
    FROZEN_INTERVAL_STATEMENT, ROLL_TRIGGER_STATEMENT, SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE,
    SCHEDULE_ROLL_TRIGGER_STRINGS,
};
pub use schedule_comparison::{
    compare_candidate_schedules, incomparability_label, tier1_tier2_comparison_case,
    CandidateComparisonEntry, CandidateModelingInputs, ComparisonError,
    ScheduleComparisonRecord, SharedComparisonInputs, Tier1Tier2ComparisonCaseInput,
    COMPARISON_API_WIRING_NOTE, COMPARISON_CANDIDATE_STATEMENT, COMPARISON_METRICS_STATEMENT,
    CROSS_INPUT_POLICY,
};

// P4.11/P4.12/P4.13 — appended additively per the same agreement.
pub use command_gates::{
    parse_feasibility_evidence_hash, pending_authorization_reference,
    feasibility_evidence_ref, AppliedSeparationOfDuties, ApprovalScope, AssetCapabilitySource,
    AssetGateStatus, AuthorizationDecision, AuthorizationDecisionRecord, AuthorizationGateResult,
    CommandAuthzAction, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGateError, CommandGatePolicy,
    CommandGateStore, CommandGateStoreError, CommandGateView, CommandGates,
    CreationCheck, DeclaredQuantities, EnvelopeGateFreshness, FeasibilityEvidence,
    FeasibilityGateResult, FeasibilityGateVerdict, GateResult, GateSeamError,
    ModelVersionGateStatus, ModelVersionSource, ProposedCommand, ProposedCommandRecord,
    ProposeCommandInput, ScheduleEnvelopeSource, ScheduleRevisionUsability,
    SeparationOfDuties, COMMAND_ENTITY, COMMAND_TRANSITION_ENTITY,
    FEASIBILITY_EVIDENCE_REF_PREFIX,
};

// P4.14 — appended additively per the same agreement.
pub use command_record::{
    duplicate_conflict_rejection, expiry_milestone_for, missing_evidence_rejection,
    out_of_sequence_rejection, outside_capability_rejection, ramp_outside_capability_rejection,
    replay_full_lifecycle, stale_rejection, AssetAcknowledgement, AssetAcknowledgementState,
    AssetCapabilityWindow, AuthorizationRecordFields, CommandRejection, CommandRejectionRule,
    CommandRecordError, CommandRecordFields, DuplicateVerdict, FleetAcknowledgement,
    FleetAcknowledgementState, RecordValidationPhase, PENDING_AUTHORIZATION_PREFIX,
    S143_FIELD_GROUPS, SUPERSEDED_EVIDENCE_PREFIX,
};

// P4.15 — appended additively per the same agreement.
pub use command_expiry::{
    sweep_validity_windows, CancellationOutcome, CommandExpiryError, CommandExpiryService,
    EventScope, ExpiryCandidate, ExpiryDecision, ExpiryOutcome, SupersedeOutcome,
    EXPIRY_EVENT_PAYLOAD_GAP_NOTE, EXPIRY_SWEEP_DECISION,
};

// P4.16 — appended additively per the same agreement.
pub use dispatch_simulator::{
    check_issue_refusals, evaluate_tracking, AdapterTargetCandidate, DispatchAdapterError,
    IssueOutcome, IssueRefusal, NonSimulatorTargetKind, SimulatedAcceptance, SimulatedObservation,
    SimulatorDispatchAdapter, SimulatorDispatchEnvelope, SimulatorDispatchTransport,
    SimulatorTargetValue, SimulatorTransportError, SimulationKeyring, SimulationSigningKey,
    TrackingAcceptanceCriterion, TrackingOutcome, TrackingTerminal, TrackingVerdict,
    TrackedQuantity, MAX_OBSERVATIONS_PER_POLL, SIMULATION_ONLY_STATEMENT,
    SIMULATOR_TRANSPORT_IDENTITY_PREFIX,
};

// P4.17/P4.18 — appended additively per the same agreement.
pub use fallback_policy::{
    BoundedHoldInterval, EscalationTarget, FallbackAction, FallbackActivationInput,
    FallbackActivationRecord, FallbackAuditError, FallbackAuditSink, FallbackDecision,
    FallbackEngine, FallbackEscalationRecord, FallbackPolicy, FallbackPolicyEntry,
    FallbackPolicyError, FallbackPolicyRegistry, FallbackScope, FallbackTrigger,
    HoldExpiryEvaluation, HoldValidityConditions, SimulatedOperatorReview,
    BOUNDED_HOLD_STATEMENT, DEFAULT_ENTRY_STATEMENT, DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE,
    ESCALATION_IS_NOT_A_TRANSITION_NOTE, FALLBACK_ACTOR_IDENTITY, FALLBACK_AUDIT_ENTITY,
    FALLBACK_ESCALATION_ENTITY, FALLBACK_POLICY_REF_PREFIX, NO_UNIVERSAL_FALLBACK_STATEMENT,
    RECOVERY_RULE_STATEMENT, SIMULATED_OPERATOR_REVIEW_LABEL, VALIDITY_CONDITIONS_RULE_STATEMENT,
};

// P4.19 — appended additively per the same agreement.
pub use timing_honesty::{
    measure_from_journal, DispatchTimingSample, HistogramBin, TimingConditions,
    TimingDistribution, TimingHonestyError, TimingQuantiles, TimingRecorder,
    MEASUREMENT_CONDITIONS_RULE, NO_LATENCY_GUARANTEE_STATEMENT, NOT_PROMISED_FUNCTIONS,
    SUPERVISORY_ONLY_STATEMENT, TIMING_BOUNDARY_STATEMENT,
};

// P4.37 — appended additively per the same agreement.
pub use feasibility_return::{
    parse_constraint_name, plan_consumption, validate_returned, ConstrainEvidence,
    ConstraintAnnotation, ConstraintCategory, ConsumptionAction, FeasibilityOutcome,
    FeasibilityReturnError, NoConsumptionRoute, ReOptimizationInput, ReturnedCandidate,
    CONSUMPTION_CONTRACT_STATEMENT, CONSTRAINT_CATEGORY_CODES, FAILURE_MODE_STATEMENT,
    GATE_OUTCOME_CODES, MIRROR_STATEMENT, OPEN_ITEM_STATEMENT,
};

// P4.28 — appended additively per the same agreement.
pub use dispatch_security::{
    channel_key, record_tenant_denial, sign_dispatch_payload, verify_dispatch_payload,
    DispatchEnvelopeVerifier, DispatchPayloadBinding, DispatchSecurityError,
    PayloadBoundVerifier, SecurityAuditError, SecurityAuditSink, SequenceGuard, SequenceRefusal,
    TenantDenialAuditRecord, TenantDenialKind, VerifyingDispatchTransport,
    DEFAULT_DISPATCH_CHANNEL, DISPATCH_SECURITY_STATEMENT, PAYLOAD_SIGNATURE_PREFIX,
    SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT, SIMULATION_DIGEST_ALG,
};

// P4.31 — appended additively per the same agreement.
pub use shadow_evaluation::{
    evaluate_shadow_comparison, NotComparableReason, ShadowBaselineSample, ShadowBaselineSource,
    ShadowComparisonConfig, ShadowComparisonEntry, ShadowComparisonInput, ShadowComparisonOutcome,
    ShadowEvaluationError, ShadowOverallVerdict, ShadowReport, NOTHING_ISSUED_STATEMENT,
    SHADOW_EVIDENCE_STATUS, SHADOW_LIMITS_STATEMENT, SHADOW_METHOD_STATEMENT,
};

// P15.01–P15.06 — the forecasting core (wave 1, Agent A), appended
// additively per the phase subtask file-ownership agreement.
pub use forecast_architecture::{
    validate_forecast_producer_permissions, verify_forecast_artifact, ForecastArtifactKind,
    ForecastArtifactProvenance, ForecastDigestVerdict,
    ForecastHorizon, ForecastPermissionError, ForecastProvenanceError, ForecastProvenanceVerdict,
    ForecastScope, NoForecastVerifier, UnsafeTestOnlyForecastDigest,
    FORECAST_DIGEST_ALGORITHMS, FORECAST_DIGEST_ALGORITHM_SHA256, FORECAST_PRODUCER_PERMISSIONS,
    FORECAST_PROVENANCE_SCHEMA_ID, FORECAST_REFUSAL_ALGORITHM_MISMATCH,
    FORECAST_REFUSAL_DIGEST_MISMATCH, FORECAST_REFUSAL_REF_MISMATCH,
    FORECAST_REFUSAL_UNSIGNED, FORECAST_REFUSAL_VERIFIER_REFUSED, FORECASTING_ARCHITECTURE_STATEMENT,
    KERNEL_PERMISSION_ALIGNMENT_SEAM, NEVER_GRANTED_PERMISSIONS_MIRROR,
};
pub use forecast_baselines::{
    degree_day_predict, fit_degree_day_regression, persistence_forecast, seasonal_naive_forecast,
    forecast_baseline_registry, DegreeDayFit, DegreeDaySample, ForecastBaselineError,
    ForecastBaselineKind, ForecastBaselineRecord, ForecastFreezeError, FrozenForecastBaseline,
    BASELINE_SUPERSESSION_RULE, FORECAST_BASELINE_REGISTRY_VERSION, NO_SKILL_STATEMENT,
    P3_10_SYNTHETIC_BASELINE_DISCLAIMER_MIRROR,
};
pub use forecast_evaluation::{
    mean_absolute_error, pointwise_errors, root_mean_squared_error, skill_score_vs_persistence,
    BacktestWindow, BacktestWindowError, EvaluationRunGate, EvaluationRunRecord, ForecastMetric,
    ForecastMetricError, RegisteredThreshold, ThresholdDirection, ThresholdError,
    EVALUATION_STATEMENT, FORECAST_EVAL_SET_ID, FORECAST_EVAL_SET_VERSION,
};
pub use forecast_pv::{
    pv_model_design_registry, PvDeclarationError, PvForecastModelDeclaration, PvModelDesignTrainingState,
    PvScopeClass, PvWeatherInput, NO_VENDOR_FIELD_STATEMENT, P15_E_1_STATUS,
    PV_FORECAST_DESIGN_STATEMENT,
};
pub use model_registry::{
    registry_content_digest, ModelRegistry, RegistryEntry, RegistryEntryKind, RegistryError,
    TrainingState, EMPTY_CHAIN_HEAD_DIGEST, MODEL_REGISTRY_STATEMENT, MODEL_REGISTRY_VERSION,
};

// P15.12–P15.14, P15.16 — the wave-2 Agent A application modules
// (health/records/integration), appended additively per the phase
// subtask file-ownership agreement.
pub use fleet_health::{
    availability_kpi, base_priority, evaluate_deviation, normalize_alarms,
    AvailabilityKpiRecord, DeviationObservation, FleetHealthError, HealthDeviationRecord,
    IssuePriority, NormalExpectation, PrioritizedIssue,
    AVAILABILITY_KPI_INDEPENDENCE_STATEMENT, HEALTH_SCORE_IS_A_RECORD_STATEMENT,
    NISPERA_PATTERN_STATEMENT, NORMAL_REFERENCE_IS_DECLARED_STATEMENT,
};
pub use telemetry_drift::{
    evaluate_plausibility, evaluate_plausibility_run, evaluate_reporting_rate,
    DriftError, PlausibilityBounds, PlausibilityViolation, RateDriftKind,
    RateDriftVerdict, ReportingRateDriftRecord, ReportingRateExpectation,
    DRIFT_RECORD_IS_NOT_A_COMMAND_STATEMENT, NO_PARALLEL_INGESTION_STATEMENT,
    QUALITY_NON_INTERFERENCE_STATEMENT, TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT,
};
pub use calibrated_model_records::{
    DeclaredConstantEfficiency, DeclaredEfficiencyCurve, DeclaredParameter,
    DeclaredThermalModel, EfficiencyBreakpoint, ModelRecordError,
    CALIBRATED_MODEL_RECORD_SCHEMA_VERSION, DECLARED_NOT_FITTED_STATEMENT,
    MODEL_RECORD_IS_NOT_A_COMMAND_STATEMENT, SIMULATOR_PROFILE_SUPERSESSION_STATEMENT,
};
pub use forecast_integration::{
    roll_trigger_for_fallback, select_forecast_source, fixture_fallback_reference,
    FallbackReason, ForecastSourceConfig, ForecastSourceDecision, PublishedForecastRef,
    DEFAULT_FORECAST_MAX_AGE_MILLIS, FALLBACK_IS_NOT_A_COMMAND_STATEMENT,
    FORECAST_TO_SCHEDULE_STATEMENT, STALENESS_REUSE_STATEMENT,
};

// P16.07 — price-signal ingestion RECORD layer (Agent B, wave 1): the
// DA/RT market price records over the P15.04 seam (consumed, never
// redefined), the two-copies discipline (analytics f64 vs
// settlement-adjacent decimal — never merged), the ISO
// price-publication SEAM (declared, unwired, [S-94] class), and the
// exact fixed-decimal money type every settlement-adjacent record
// shares (no float money — plan §4). Prices are inputs, never commands
// (ADR-010). Appended additively per the phase subtask file-ownership
// agreement.
pub mod fixed_decimal;
pub mod market_price_records;

// P16.08 — telemetry export pipeline DESIGN record (Agent B, wave 1):
// per-market export postures (ERCOT 2-second ICCP [S-100][S-101], NYISO
// 6-second SCADA + SOC [S-105][S-106], PJM ≥10 MW posture [S-90]),
// declared cadence budgets, the net-consumer normalization and
// capacity-weighted ramp-rate aggregation as DECLARED formulas, and the
// unwired export sink. No pipeline transmits; certification is a
// human-gated lab process. Appended additively per the same agreement.
pub mod telemetry_export_design;

// P16.09 — settlement ingestion (Agent B, wave 1): jurisdiction
// statement model types and parser SPECIFICATIONS with synthetic-fixture
// grammars (PJM Manual 29 CSV/XML + MSRS/BLI [S-93]; CAISO 5-minute
// SQMD [S-97]; EBGL Art. 52–55 15-minute ISP [S-107][S-108][S-109]) and
// the statement→expected reconciliation engine with the dispute queue.
// No real statement is parsed — none exists here. Appended additively
// per the same agreement.
pub mod settlement_models;
pub mod settlement_reconciliation;

// P16.10 — settlement-quality meter data (Agent B, wave 1): ANSI
// C12.19/C12.20 class-0.2/0.5 expectations as DECLARED meter records
// [S-31][S-33], the meter-device-resource mapping, and the dual-stream
// rule — settlement-interval meter data and seconds-level telemetry are
// distinct types with no conversion path [S-59][S-104]. Appended
// additively per the same agreement.
pub mod settlement_meter_data;

// P16.11 — performance evidence capture (Agent B, wave 1): the evidence
// RECORD types (signal-following, response timestamps, mileage as
// published, SOC traces, precision-score inputs with the UNVERIFIED
// RegUp/RegDn date marked [S-94][S-95]; German FCR 30 s / aFRR 5 min
// evidence classes [S-119]). Evidence only — no score is claimed.
// Appended additively per the same agreement.
pub mod performance_evidence;

// P16.12 — dispute workflow (Agent B, wave 1): line-item challenges
// against the P16.09 reconciliation queue, chain-linked evidence-bundle
// assembly (every entry carries a P11 chain-digest reference; the chain
// is consumed, never redefined), and resolution records. No dispute
// exists. Appended additively per the same agreement.
pub mod dispute_workflow;

// P16.01–P16.06 — the wave-1 Agent A market modules (the market-records
// store lives in flexcore-storage; the domain model in flexcore-domain;
// the three modules below are the application-layer engines). Appended
// additively per the phase-16 wave-1 file-ownership agreement.
// P16.04 — the program-configurable baseline engine (CBL 10-in-10 /
// 5-in-10 / day-matching with day-type + weather filters and the capped
// symmetric SDA, CAISO ≤20% [S-30]): the method is the PROGRAM'S declared
// configuration (LR-073) — an unsupported method is a RECORDED GAP ROW,
// never a silent approximation; determinism via the injected Clock and
// BTreeMap ordering.
pub mod baseline_engine;
// P16.05 — M&V event reports: event performance against the program's
// baseline, the same-day feedback pattern [S-43], evidence assembly from
// interval data with the §10.4 quality states; every number is a
// settlement-adjacent financial record (the P11 chain link is REQUIRED).
pub mod mv_event_report;
// P16.06 — the bid composition service: asset envelopes → aggregation-
// level bids per market structure (PJM single aggregation node [S-90];
// CAISO PDR two-ID load-shift [S-96]; ISO-NE DAM Asset tab [S-103]).
// The envelope remains the physical truth layer; operator-approved mode
// + an explicit per-market grant are REQUIRED before any bid is
// composed; no bid is submitted — none can be.
pub mod bid_composition;

// P16.13–P16.18 — the wave-2 Agent A market modules (billing, payment
// lifecycle, program events, capacity records, UK/EU LFM, jurisdiction
// register). Appended additively per the phase-16 wave-2 file-ownership
// agreement (records 41–47 under the phase records directory).
// P16.13 — revenue-share billing basics: contract-configurable splits
// (declared decimal-text shares summing to EXACTLY 100%), exact
// pooled-revenue disaggregation per asset (checked integer mantissa
// arithmetic — no division, no rounding, no float money), owner-
// statement DRAFT/RECONCILED records (no ISSUED/SENT/DELIVERED state
// exists), invoicing RECORDS (data only), and reconciliation back to
// the ISO/DSO statements with the P16.09 queue reason codes. NOT a
// retail CIS (plan §4; ADR-011).
pub mod revenue_share_billing;
// P16.14 — payment lifecycle transparency: the closed FORECASTED →
// EARNED → PAID vocabulary per asset/program (the Voltus pattern
// [S-47]), the frozen two-edge transition table, and every transition
// chain-linked (the P11 audit-chain LINK + the previous-transition
// identity; one currency per chain).
pub mod payment_lifecycle;
// P16.15 — program event management: the OpenADR event ↔ command-
// lifecycle linkage (external events become PROPOSED commands per
// asset, NEVER issued commands — the P14.05 mapping consumed by
// citation; the entry state is a constant, not a field), fail-closed
// opt-in/out handling (no decision = no candidate; the latest opt-out
// wins), and the closed two-state M&V report-delivery records
// [S-116][S-117].
pub mod program_event_management;
// P16.16 — capacity-market participation records, RECORD-ONLY depth:
// qualification-evidence checklists for PJM RPM / ISO-NE FCA19 /
// ISO-NE DECR, every date a SourcedDate (the re-verify-at-use rule is
// structurally mandatory), and the LR-075 PJM date conflict resolved
// BY RECORD (both sourced dates carried, neither selected). No
// qualification is claimed — no QUALIFIED state exists anywhere.
pub mod capacity_market_records;
// P16.17 — UK/EU LFM integration design records: the Piclo
// registration/tender/utilization/settlement flow DESIGNS (DECLARED
// status only — no stage executes), availability- and utilization-
// payment RECORD types (exact fixed-decimal money, currency as data;
// no payment exists), and P483 dual-participation stacking records
// naming the P16.02 stacking rule (the refusal lives in the
// enrollment store, consumed by citation — R-P16-7).
pub mod uk_lfm_records;
// P16.18 — the market jurisdiction register (types): per-tenant
// market enablement (NONE enabled by default; ENABLED_BY_D1 is
// constructable ONLY with the human owner's decision reference and
// the P11 audit-chain link), the closed enablement/posture
// vocabularies, and the sourced-timeline rows. The checked-in data
// lives in jurisdiction_register_data.
pub mod jurisdiction_register;
// P16.18 — the jurisdiction-register DATA file: ten checked-in
// jurisdiction rows (CAISO, ISO-NE, NYISO, PJM, MISO, SPP, ERCOT
// ADER, GB LFM, EU EBGL, FERC 2222 calendar) with sourced dates
// verified as of the research cut-off, the NYISO/PJM conflicts
// carried with notes on both sides, and ZERO enablement rows —
// no market is enabled by default.
pub mod jurisdiction_register_data;

// P17.06 (application assembly) — vehicle-proposal ingestion: the mapping
// of the adapter-relayed OCPP proposal messages (NotifyEVChargingNeeds /
// NotifyEVChargingSchedule / NotifyChargingLimit / ClearedChargingLimit —
// spellings UNVERIFIED [S-124][S-125]) onto the domain constraint records.
// Proposals are inputs and records, NEVER auto-commands (the
// ADR-010-EXT-01 pattern; the lifecycle entry state is structurally
// PROPOSED). The OCPP CSMS adapter is Agent A's P17.01
// `crates/adapters/protocol-ocpp/` inbound path — consumed BY CITATION
// (decision record docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 21_p17_06_vehicle_proposal_ingestion.md).
pub mod ev_proposal_ingest;
// P17.07 (application mapping) — MeterValues ingestion: the per-quantity
// reporting-cadence configuration, the MeterValuesSign hooks (the
// Eichrecht-grade path; block letters UNVERIFIED against the OCA text;
// the seam records OPAQUE signed values and NEVER verifies a signature or
// claims compliance [S-135][S-136]), the adapter-boundary quality mapping
// onto the FROZEN six-state vocabulary (undeclared codes are named
// errors; hold-or-lower; the non-silencing rule intact), and the
// measurand→quantity mapping. The worker wiring feeds the EXISTING P2.07
// pipeline (apps/worker/src/ev_ingest.rs) — no parallel ingestion path
// (decision record docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 22_p17_07_meter_values_ingestion.md).
pub mod ev_meter_values_ingest;
// P18.06 (phase 18, wave 1, Agent A) — the device-fleet STAGED ROLLOUT
// ORCHESTRATOR (control plane): cohorts (canary first, declared order,
// no skipping), pre-registered abort thresholds on cohort failure rates
// (X-8 structural: a plan without registered parameters is refused at
// plan time; the 5–10% canary band is a DEFAULT RECOMMENDATION, not a
// sourced figure), per-device attestation before authorization, and
// fleet-state visualization data. A NEW, separately-argued mechanism —
// the kernel plugin-rollout policy (P7.38) stays cohort/percentage-free
// BY CONSTRUCTION and byte-identical (scope-distinction record
// docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
// 06_p18_06_rollout_orchestrator_scope_distinction.md). Cohort
// membership and attestation verdicts are CONSUMED by citation from
// Agent B's P18.12 fleet registry and P18.13 attestation policy through
// injected ports (the dependency rule forbids core→composition edges).
// No rollout has ever run; no cohort exists; no device was authorized.
pub mod rollout_orchestrator;

// FIM-F14 (wave 5, Agent B) — dynamic operating envelopes from live DSO
// feeds: the DOE envelope feed SOURCE seam (FIM-F14.01 — the P15.03
// WeatherDataProvider port pattern mirrored: sync trait, opaque adapter
// errors, deterministic in-memory configured source, named
// unwired/feed-absent default that fails closed to the static limit) and
// the CONSTRAINT WIRING (FIM-F14.02/03/04 — the in-force envelope's
// ImportExportLimits override the StaticConnectionLimit as optimizer
// CONSTRAINT DATA on the declared connection-limit seat, the lapse
// reversion is telemetered with the FROZEN six-state telemetry quality
// vocabulary (STALE/MISSING; the 26-event vocabulary is untouched and no
// event is invented), and breach/curtailment records carry the envelope
// version in force). The domain vocabulary is flexcore-domain::
// doe_envelope (P14.15), CONSUMED, never restated; canonical MW pass
// through unchanged (P > 0 = export, no conversion). ADR-010 posture:
// the envelope is constraint data, never a command — a curtailment
// decision is a command-lifecycle outcome. WRITTEN-UNRUN; live DSO feed
// interop UNPROVEN — wiring a real feed is a HUMAN LAB GATE. Appended
// additively per the phase subtask file-ownership agreement.
pub mod doe_constraint_source;
pub mod doe_constraint_wiring;

// FIM-F14 — appended re-exports (same agreement).
pub use doe_constraint_source::{
    DoeEnvelopeSource, DoeSourceError, InMemoryDoeEnvelopeSource, UnwiredDoeEnvelopeSource,
    DOE_CONSTRAINT_SOURCE_STATEMENT, DOE_FEED_LAB_GATE_STATEMENT, DOE_FEED_SEAM_NOT_WIRED,
    DOE_FEED_SOURCE_IN_MEMORY,
};
pub use doe_constraint_wiring::{
    apply_connection_point_bounds, check_dispatch_against_constraint,
    curtail_proposed_to_bounds, doe_constraint_assumption_text,
    resolve_connection_point_constraint, resolve_connection_point_constraints,
    ConnectionBoundsSeat, CurtailmentOutcome, DispatchEnvelopeVerdict, DoeReversionObservation,
    DoeWiringError, EnvelopeConstraintState, PointPreviousEnvelope, ResolvedPointConstraint,
    StaticGovernanceReason, DOE_ADR010_STATEMENT, DOE_CONSTRAINT_WIRING_STATEMENT,
    DOE_ENVELOPE_VERSION_PREFIX, DOE_FAIL_CLOSED_STATEMENT, DOE_REVERSION_TELEMETRY_STATEMENT,
    DOE_UNIT_PASS_THROUGH_STATEMENT, FIM_F14_HONESTY_STATEMENT,
};
