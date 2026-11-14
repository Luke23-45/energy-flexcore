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
