// P4.11 / P4.12 / P4.13 — The command gates: creation + validation (P4.11),
//         feasibility (P4.12), authorization (P4.13) (report §14.1–§14.3,
//         §12.5, §17.3, §17.5; ledger P4.11–P4.13; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/11_13_command_gates.md`).
//
// The application-layer orchestration of the THREE DISTINCT §14.1 gates over
// the P4.10 lifecycle engine (`flexcore_domain::command_lifecycle` — the
// frozen-edge oracle), the §14.3 `DispatchCommand` validators, the P3.22
// schedule-revision discipline, and the P4.03 model-version health:
//
// - **P4.11 — creation + validation.** A command is created FROM AN APPROVED
//   SCHEDULE only: the creation-source gate refuses a schedule revision that
//   is SUPERSEDED or unknown (rejected AT CREATION — never merely at issue).
//   The validation gate (PROPOSED → VALIDATED | REJECTED) re-checks schema
//   through the existing `DispatchCommand` validators, then freshness (the
//   P3.13 envelope staleness gate; the referenced grid model ACTIVE per
//   P4.03), asset checks (exists in the caller's tenant, capability model
//   present, quantities declared), ramp against the declared capability,
//   and the command-policy checks. Every rejection names its failing check.
// - **P4.12 — feasibility.** VALIDATED → FEASIBLE | REJECTED through the
//   §12.5 network gate (steps 1–4 here; the limit verdict arrives via the
//   injected runner seam). Feasibility evidence is a SEPARATE reference
//   from validation evidence — distinct journal rows, a distinct field on
//   [`FeasibilityGateResult`], never merged (§14.1; the explicit
//   P4.11/P4.12 anti-conflation rule). A command whose source revision
//   turned SUPERSEDED is rejected here too (the P4.10
//   `ensure_schedule_revision_usable` pre-wiring and the grid gate's
//   revision check agree — the seam delegates to the former by contract).
//   Feasibility evidence references THE MODEL HASH; a model change
//   invalidates it — [`CommandGates::ensure_command_issueable`] is the
//   check P4.16's adapter calls before ISSUED (§17.5 invariant 3: no
//   command without required network feasibility evidence for the declared
//   operating profile reaches ISSUED).
// - **P4.13 — authorization.** FEASIBLE → APPROVED | REJECTED by a REQUIRED
//   human/policy authorization. There is NO auto-approve path: APPROVED is
//   produced by exactly one operation
//   ([`CommandGates::record_command_approval`]), which requires (a) an
//   allow decision from the injected authorization evaluator at the
//   recorded scope, (b) an approval scope that covers the command's asset,
//   (c) an approver distinct from the proposer where separation of duties
//   is configured (§17.4), and (d) an expiry in the future. The recorded
//   [`AuthorizationDecisionRecord`] carries authorizing actor, role, scope,
//   limits, expiry (§17.3), decision time, and the EXACT validation +
//   feasibility evidence IDs it covers.
//
// ## The seam statement (why the traits live here)
//
// flexcore-application's dependency set (tools/check_dependency_rule.py;
// the manifest is not this subtask's file) admits flexcore-contracts,
// flexcore-domain, flexcore-ports, flexcore-events, and
// flexcore-optimization-api ONLY. The gate's collaborators therefore arrive
// as the injected port traits declared below, implemented by the
// composition root:
//
// - [`CommandGateStore`] — over the P4.10
//   `flexcore_storage::command_store::CommandRecordStore` surface (insert,
//   journal, load, and `ensure_schedule_revision_usable` mapped to
//   [`ScheduleRevisionUsability`]);
// - [`ScheduleEnvelopeSource`] — over the P3.13 staleness gates;
// - [`ModelVersionSource`] — over the P4.03 model-version registry/health
//   (ONLY an Active model may back a feasibility check);
// - [`AssetCapabilitySource`] — over the Phase 2 asset registry + P3.01/04
//   capability data;
// - [`CommandFeasibilityRunner`] — over `flexcore_grid_api::feasibility::
//   run_feasibility_gate` plus the P4.06/P4.07 limit verdict (the
//   composition root converts the grid gate's evidence-backed outcome into
//   [`FeasibilityGateVerdict`]);
// - [`CommandAuthorizationEvaluator`] — over `flexcore_authz::policy::
//   PolicySet` with the recorded `flexcore_authz::phase4_policies` grants.
//
// This is the same injected-seam pattern the P4.04 gate used for its four
// sources (recorded there as the solution to the same dependency-direction
// constraint). Every seam ERROR is fail-closed: it rejects or errors, never
// degrades to a pass.
//
// ## Event vocabulary (recorded gap)
//
// The frozen §15.3 vocabulary has `DispatchProposed`, `DispatchRejected`,
// and `DispatchApproved` — emitted here through the transactional outbox
// (state change first, then the outbox write, paired through the store
// receipt; the composition root's unit of work makes the pair atomic).
// There is deliberately NO `DispatchValidated` or `DispatchFeasible` event
// in §15.3 and none may be invented (P0.14): the VALIDATED and FEASIBLE
// facts live in the transition journal, which is the evidence of record.
//
// P4.30: the §15.3 grid-validation event `FeasibilityCheckCompleted` is
// emitted here for EVERY check the §12.5 runner completes with a verdict
// (pass, limit violation, fail-closed — D-P4.30-1). The pre-verdict
// rejection path (the source-revision usability check) emits NO
// `FeasibilityCheckCompleted`: no check ran there — the fact of record is
// `DispatchRejected` alone. The event's `correlation_id` is the command id
// (the deterministic, replay-safe convention of the rejection events); the
// frozen payload carries no command/attempt field (recorded payload-fit
// gap), so the attempt identity travels in the journal's feasibility
// evidence reference and the persisted feasibility-check record (P4.30,
// migration 0017).
//
// ## Control status
//
// Validation, feasibility, and authorization are distinct gates (§14.1);
// network feasibility is not operator authorization; an acknowledgement is
// not physical delivery. The command lifecycle is a protected platform
// invariant: plugins may propose and request validation but can never
// authorize, issue, or select the control-authority mode (§5.7). The
// entire 0.x line is simulation-only: an APPROVED command authorizes a
// simulated issue (P4.16) at most — never a device write. FlexCore performs
// no primary frequency response, synthetic inertia, relay protection,
// inverter inner-loop control, or sub-cycle control (§14.5). ISSUED is NOT
// recorded here — the dispatch adapter is P4.16's.

use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, ValidUntil, ValidityWindow};
use flexcore_domain::command_lifecycle::{
    Actor, AuthorizationOutcome, CommandLifecycle, CommandTransition, EvidenceReference,
    GateOutcome, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    AuthorizationReference, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    ControlParameterValue, DispatchCommand, DispatchCommandError, EvidenceReferences,
    ExecutionRequirements, ScheduleReference, SourceOrigin, Supersession,
};
use flexcore_domain::ids::{AssetId, CommandId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{
    DispatchApprovedPayload, DispatchProposedPayload, DispatchRejectedPayload, DomainEvent,
    FeasibilityCheckCompletedPayload,
};
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::state_change::StateChangeReceipt;

use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;

// --- Recorded constants --------------------------------------------------------

/// The outbox/state-change entity name of the command record row (migration
/// 0016 table `dispatch_commands`).
pub const COMMAND_ENTITY: &str = "dispatch_commands";

/// The outbox/state-change entity name of the transition journal (migration
/// 0016 table `dispatch_command_transitions`).
pub const COMMAND_TRANSITION_ENTITY: &str = "dispatch_command_transitions";

/// The feasibility-evidence reference format (a recorded contract, parsed by
/// [`parse_feasibility_evidence_hash`]):
/// `feasibility:attempt/<attempt_id>:model/<model_id>:hash/<content_hash>`.
pub const FEASIBILITY_EVIDENCE_REF_PREFIX: &str = "feasibility:attempt/";

/// Builds the feasibility-evidence reference in the recorded format.
pub fn feasibility_evidence_ref(attempt_id: &str, model_id: &str, content_hash: &str) -> String {
    format!("{FEASIBILITY_EVIDENCE_REF_PREFIX}{attempt_id}:model/{model_id}:hash/{content_hash}")
}

/// Extracts the model content hash from a feasibility-evidence reference in
/// the recorded format. `None` for a reference the format does not describe
/// (callers fail closed on `None` — never "assume current").
pub fn parse_feasibility_evidence_hash(reference: &str) -> Option<&str> {
    let rest = reference.strip_prefix(FEASIBILITY_EVIDENCE_REF_PREFIX)?;
    let hash = rest.split(":hash/").nth(1)?;
    if hash.trim().is_empty() {
        None
    } else {
        Some(hash)
    }
}

/// The §14.3 authorization-reference placeholder recorded on a PROPOSED
/// command. The §14.3 record shape requires an authorization decision
/// reference, but at PROPOSED no authorization decision EXISTS (approval is
/// the third gate). The placeholder is never a decision: the real decision
/// is the P4.13 [`AuthorizationDecisionRecord`] plus the journal's APPROVED
/// entry — the evidence of record (content-immutability rule, D-P4.10-2).
pub fn pending_authorization_reference(command_id: CommandId) -> String {
    format!("authz:pending:command/{command_id}")
}

// --- Injected seams --------------------------------------------------------------

/// The usability of a schedule revision, mirroring the P3.22
/// `is_revision_superseded` surface AND the P4.10
/// `CommandRecordStore::ensure_schedule_revision_usable` semantics:
/// `Superseded` and `Unknown` are BOTH rejected — unknown is never "usable".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleRevisionUsability {
    /// The revision exists and is the active one.
    Active,
    /// The revision exists but a newer approved revision replaced it.
    Superseded,
    /// The revision does not exist in the caller's tenant scope.
    Unknown,
}

/// The store seam over the P4.10 command-record persistence surface. The
/// composition root implements it over
/// `flexcore_storage::command_store::CommandRecordStore` (the error
/// vocabulary maps 1:1; see [`CommandGateStoreError`]).
pub trait CommandGateStore {
    /// Inserts one NEW command record (always PROPOSED — the freeze's
    /// initial marker). A duplicate is [`CommandGateStoreError::Conflict`].
    fn insert_proposed(
        &self,
        ctx: &TenantContext,
        record: &ProposedCommandRecord,
    ) -> Result<StateChangeReceipt, CommandGateStoreError>;

    /// Journalls ONE transition (the compare-and-swap state change plus the
    /// immutable journal row, in the adapter's one transaction). A lost race
    /// is [`CommandGateStoreError::TransitionConflict`].
    fn journal_transition(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransition,
    ) -> Result<StateChangeReceipt, CommandGateStoreError>;

    /// Loads one command's validity bound and full transition journal in the
    /// caller's tenant scope; `None` when the command does not exist there
    /// (existence never leaks across a tenant boundary, P0.16 rule 5).
    fn load_command(
        &self,
        ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<Option<CommandGateView>, CommandGateStoreError>;

    /// The schedule-revision usability check — the composition-root
    /// implementation MUST delegate to the P4.10
    /// `CommandRecordStore::ensure_schedule_revision_usable` (which calls
    /// the P3.22 surface), so this seam and the persistence-layer gate
    /// check AGREE by construction.
    fn schedule_revision_state(
        &self,
        ctx: &TenantContext,
        schedule_id: &ScheduleId,
        revision: u64,
    ) -> Result<ScheduleRevisionUsability, CommandGateStoreError>;
}

/// One command record to insert (the write shape; mirrors the P4.10
/// `CommandRecordRow` with typed command/tenant identifiers). Identifier
/// fields are canonical hyphenated UUID text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedCommandRecord {
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The unique command ID.
    pub command_id: CommandId,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The target site (canonical UUID text).
    pub site_id: String,
    /// The target asset (canonical UUID text).
    pub asset_id: String,
    /// The source schedule chain (canonical UUID text; P3.22).
    pub schedule_id: String,
    /// The source schedule revision (≥ 1).
    pub schedule_revision: u64,
    /// The valid-until bound (UTC millis) the EXPIRED state support
    /// compares against.
    pub valid_until_millis: i64,
    /// The command THIS command replaces (§14.3 field 16, forward link).
    pub supersedes_command_id: Option<String>,
    /// The full §14.3 command record as JSON text (the composition root's
    /// rendering; opaque here — P4.14 owns the field-level schema).
    pub command_payload_json: String,
}

/// A loaded command: the validity bound plus the full transition journal
/// (the audit view; journal order is the persisted order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGateView {
    /// The valid-until bound (UTC millis).
    pub valid_until_millis: i64,
    /// The full journal, in order.
    pub journal: Vec<CommandTransition>,
}

impl CommandGateView {
    /// The current state derived from the journal (PROPOSED when empty).
    pub fn state(&self) -> CommandState {
        self.journal
            .last()
            .map(|entry| entry.to)
            .unwrap_or(CommandState::Proposed)
    }
}

/// Errors of the store seam — the 1:1 mapping of the P4.10
/// `CommandStoreError` vocabulary the composition root implements over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandGateStoreError {
    /// A command with this ID already exists (a re-proposal is a NEW
    /// command; command rows are immutable).
    Conflict { command_id: String },
    /// The command does not exist in the caller's tenant scope.
    NotFound { command_id: String },
    /// The referenced schedule revision is SUPERSEDED.
    RevisionSuperseded { schedule_id: String, revision: u64 },
    /// The referenced schedule revision does not exist in the caller's
    /// tenant scope (unknown is never usable).
    RevisionUnknown { schedule_id: String, revision: u64 },
    /// The compare-and-swap transition lost: the command's stored state is
    /// not the state the caller transitioned from.
    TransitionConflict { command_id: String, expected_from: String },
    /// Storage backend failure — opaque code + message only.
    Backend { code: String, message: String },
}

impl std::fmt::Display for CommandGateStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandGateStoreError::Conflict { command_id } => {
                write!(f, "command {command_id} already exists; a re-proposal is a new command")
            }
            CommandGateStoreError::NotFound { command_id } => {
                write!(f, "command {command_id} does not exist in this tenant scope")
            }
            CommandGateStoreError::RevisionSuperseded { schedule_id, revision } => write!(
                f,
                "schedule {schedule_id} revision {revision} is SUPERSEDED and may not back a command"
            ),
            CommandGateStoreError::RevisionUnknown { schedule_id, revision } => write!(
                f,
                "schedule {schedule_id} revision {revision} does not exist in this tenant scope"
            ),
            CommandGateStoreError::TransitionConflict {
                command_id,
                expected_from,
            } => write!(
                f,
                "command {command_id} is no longer in state {expected_from}; the transition \
                 applies only from the state the caller saw"
            ),
            CommandGateStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for CommandGateStoreError {}

/// Errors of the non-store seams. Fail-closed by convention: a seam error
/// never degrades to a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateSeamError {
    /// Which seam failed (`schedule_envelope`, `model_version`,
    /// `asset_capability`, `feasibility_runner`).
    pub seam: &'static str,
    /// The recorded failure detail (audit text).
    pub detail: String,
}

impl std::fmt::Display for GateSeamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gate seam '{}' failed: {}", self.seam, self.detail)
    }
}

impl std::error::Error for GateSeamError {}

/// The envelope-freshness verdict for the command's source schedule, the
/// P3.13 staleness gates as the validation gate consumes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeGateFreshness {
    /// The schedule's envelope is fresh.
    Fresh,
    /// The envelope is stale; the recorded P3.13 reason travels.
    Stale { reason: String },
}

/// The P3.13 envelope-freshness source, keyed by the command's schedule
/// reference (the envelope version belongs to the schedule, not to the
/// §14.3 command record).
pub trait ScheduleEnvelopeSource {
    fn freshness(
        &self,
        ctx: &TenantContext,
        schedule: &ScheduleReference,
    ) -> Result<EnvelopeGateFreshness, GateSeamError>;
}

/// The model-version status of the grid model a command references, per the
/// P4.03 lifecycle: ONLY an `Active` model may back a feasibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelVersionGateStatus {
    /// The model version is Active; its content hash travels (the P4.12
    /// evidence-binding hook compares against it).
    Active { content_hash: String },
    /// The model version exists but is not Active (`state` names the
    /// lifecycle state, e.g. `draft` / `retired`).
    NotActive { state: String },
    /// The model version is unknown in the caller's scope.
    Unknown,
}

/// The P4.03 model-version source.
pub trait ModelVersionSource {
    fn status(
        &self,
        ctx: &TenantContext,
        grid_model_ref: &str,
    ) -> Result<ModelVersionGateStatus, GateSeamError>;
}

/// The quantities the asset's capability model declares. A quantity the
/// command targets but the capability does not declare is a REJECTION —
/// undeclared is never unconstrained (the P3.02 rule carried forward).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredQuantities {
    /// Active-power (MW) setpoints are declared.
    pub active_power: bool,
    /// Reactive-power (MVAr) setpoints are declared.
    pub reactive_power: bool,
    /// SOC-target support is declared.
    pub soc: bool,
    /// The declared maximum upward ramp (MW per second).
    pub max_up_ramp_mw_per_s: f64,
    /// The declared maximum downward ramp (MW per second).
    pub max_down_ramp_mw_per_s: f64,
}

/// The asset-check outcome for the validation gate.
#[derive(Debug, Clone, PartialEq)]
pub enum AssetGateStatus {
    /// The asset exists in the caller's tenant, its capability model is
    /// present, and the declared quantities/ramps travel.
    Usable(DeclaredQuantities),
    /// The asset does not exist in the caller's tenant scope. A foreign
    /// asset and a missing asset are NEVER distinguished (P0.16 rule 5:
    /// existence never leaks across a tenant boundary).
    UnknownInTenantScope,
    /// The asset exists but carries no capability model.
    CapabilityModelMissing,
}

/// The Phase 2 asset-registry + capability source.
pub trait AssetCapabilitySource {
    fn check(
        &self,
        ctx: &TenantContext,
        asset: AssetId,
    ) -> Result<AssetGateStatus, GateSeamError>;
}

/// The candidate references the feasibility runner needs beyond the
/// schedule ID + revision (which travel on the command itself — the gate
/// passes them from the command, so the refs cannot diverge from the
/// command's own schedule binding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFeasibilityRefs {
    /// The §12.6 output candidate identifier.
    pub candidate_id: String,
    /// The envelope version the schedule was computed against (P3.12/13).
    pub envelope_version: String,
    /// The §12.6 result-record snapshot reference (P3.21).
    pub result_record_snapshot_ref: String,
}

/// The feasibility gate verdict, as the command gate consumes it. The
/// composition root converts `flexcore_grid_api::feasibility::
/// GatePipelineOutcome` plus the P4.06/P4.07 limit verdict into this type:
/// steps 1–4 evidence, the limit check, and the reject/constrain/return
/// routing stay in the grid crate; the command gate sees only the
/// evidence-backed verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeasibilityGateVerdict {
    /// The candidate passed the network checks within declared scope and
    /// tolerances (§14.2 FEASIBLE row).
    Passed {
        /// The attempt identifier (the `FeasibilityAttemptRecord` id).
        attempt_id: String,
        /// The model identifier the check evaluated against.
        model_id: String,
        /// THE MODEL HASH the evidence binds to (P4.12 risk note: a model
        /// change invalidates the evidence).
        model_content_hash: String,
        /// The margin/diagnostics summary (§12.5 step 7).
        margins: String,
        /// The coverage statement/limitations (§12.5 step 6).
        coverage: String,
    },
    /// A network limit is violated; `constraint` names it (§14.1 edge 4:
    /// capability / network failure — the ledger P4.12 test names the
    /// transformer constraint).
    LimitViolated {
        attempt_id: String,
        model_id: String,
        model_content_hash: String,
        constraint: String,
    },
    /// Validator/model/infrastructure failure — fail-closed (§12.5 closing
    /// rule; §17.5 invariant 9). The reject-vs-HOLD classification is
    /// P4.08's policy table; the command gate records REJECTED with the
    /// fail-closed reason (see the decision doc, D-P4.12-3).
    FailClosed {
        attempt_id: String,
        model_id: Option<String>,
        model_content_hash: Option<String>,
        reason: String,
    },
}

/// The §12.5 network feasibility runner seam (P4.12 step 1).
pub trait CommandFeasibilityRunner {
    fn run(
        &self,
        ctx: &TenantContext,
        schedule_id: &ScheduleId,
        revision: u64,
        refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError>;
}

/// The command action vocabulary of the authorization surface (the
/// application-side mirror of the `flexcore_authz::action::Action`
/// `command.*` variants; the composition root maps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandAuthzAction {
    /// `command.create` — the P4.11 creation gate.
    Create,
    /// `command.validate` — the P4.11 validation gate.
    Validate,
    /// `command.approve` — the P4.13 authorization gate.
    Approve,
    /// `command.read` — command records and journals.
    Read,
}

impl CommandAuthzAction {
    /// The canonical action id (matches the flexcore-authz surface).
    pub fn id(self) -> &'static str {
        match self {
            CommandAuthzAction::Create => "command.create",
            CommandAuthzAction::Validate => "command.validate",
            CommandAuthzAction::Approve => "command.approve",
            CommandAuthzAction::Read => "command.read",
        }
    }
}

/// The scope an approval is evaluated and recorded at (the application-side
/// mirror of the flexcore-authz `Scope` levels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalScope {
    /// Tenant-level scope (covers every asset of the tenant).
    Tenant,
    /// Site-level scope (covers the site's assets).
    Site(SiteId),
    /// Asset-level scope (covers exactly this asset).
    Asset(AssetId),
}

impl ApprovalScope {
    /// The recorded text form (the `approved_scope` field, §14.3 field 14).
    pub fn recorded_text(&self) -> String {
        match self {
            ApprovalScope::Tenant => "tenant".to_string(),
            ApprovalScope::Site(site) => format!("site:{site}"),
            ApprovalScope::Asset(asset) => format!("asset:{asset}"),
        }
    }

    /// Whether this scope covers the given site/asset (the P4.13 scope-
    /// coverage rule: Tenant ⊇ site ⊇ asset; a narrower scope covers only
    /// its own identity).
    pub fn covers(&self, site: SiteId, asset: AssetId) -> bool {
        match self {
            ApprovalScope::Tenant => true,
            ApprovalScope::Site(scope_site) => *scope_site == site,
            ApprovalScope::Asset(scope_asset) => *scope_asset == asset,
        }
    }
}

/// The authorization request the gate evaluates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandAuthorizationRequest {
    /// The acting principal (approver) identity.
    pub subject: String,
    /// The acting principal's role id on the authorization surface.
    pub role: String,
    /// The action requested.
    pub action: CommandAuthzAction,
    /// The scope the action is evaluated at.
    pub scope: ApprovalScope,
}

/// The authorization decision (never a bare bool — the reason and rule
/// reference travel for the audit record, §16.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// Allowed; the matched rule reference is the allow evidence.
    Allowed { rule_reference: String },
    /// Denied; the reason names the deny case (unknown action, no matching
    /// rule, scope denied, …).
    Denied { reason: String },
}

/// The authorization evaluator seam (over `flexcore_authz::policy::PolicySet`
/// with the recorded `phase4_policies` grants). Deny-by-default is the
/// evaluator's obligation: an absent grant is a denial.
pub trait CommandAuthorizationEvaluator {
    fn evaluate(
        &self,
        ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision;
}

// --- Gate policy configuration ---------------------------------------------------

/// The separation-of-duties setting (§17.4: "the approver must not be the
/// same identity that proposed the command where separation of duties is
/// configured"). The recorded default is ENFORCED; the applied policy is
/// recorded on every authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeparationOfDuties {
    /// The approver must differ from the proposing identity; a match
    /// rejects the approval.
    Enforced,
    /// Separation of duties is not configured for this deployment; the
    /// decision record says so explicitly (never silently absent).
    NotConfigured,
}

/// The separation-of-duties policy APPLIED to one decision (recorded, §17.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppliedSeparationOfDuties {
    /// Enforced: the approver was verified distinct from the proposer.
    Enforced,
    /// Not configured for this deployment (recorded explicitly).
    NotConfigured,
}

impl AppliedSeparationOfDuties {
    /// The recorded text form.
    pub fn recorded_text(self) -> &'static str {
        match self {
            AppliedSeparationOfDuties::Enforced => "enforced",
            AppliedSeparationOfDuties::NotConfigured => "not_configured",
        }
    }
}

/// The gate policy configuration (deployment settings; recorded defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandGatePolicy {
    /// The separation-of-duties setting for the authorization gate.
    pub separation_of_duties: SeparationOfDuties,
    /// An optional deployment cap on the execution duration; `None` means
    /// no additional cap beyond the validity-window consistency check.
    pub max_execution_duration: Option<Duration>,
}

impl Default for CommandGatePolicy {
    fn default() -> Self {
        // Recorded defaults: separation of duties ENFORCED (§17.4), no
        // additional duration cap beyond the window-consistency check.
        Self {
            separation_of_duties: SeparationOfDuties::Enforced,
            max_execution_duration: None,
        }
    }
}

// --- Gate errors ------------------------------------------------------------------

/// The named errors of the command gates. A gate REJECTION (a failing
/// check that moves the command to REJECTED) is NOT an error — it is a
/// verdict value; errors are everything that prevented a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandGateError {
    /// The §14.3 schema validation (the `DispatchCommand` validators)
    /// rejected the record; the inner error names the field group.
    Schema(DispatchCommandError),
    /// The operation does not apply to the command's current state (the
    /// application-layer form of the gate separation; the domain engine
    /// enforces the same rule structurally).
    WrongState {
        /// The state the command is in.
        current: CommandState,
        /// Which operation was attempted.
        operation: &'static str,
    },
    /// The CREATION-SOURCE gate rejected the command before any record
    /// existed (P4.11 step 1: a command is created FROM AN APPROVED
    /// SCHEDULE only). No command record and no lifecycle journal entry
    /// exist — the rejection is this named error (recorded decision
    /// D-P4.11-2).
    CreationRejected {
        /// Which creation check failed.
        check: CreationCheck,
        /// The recorded detail (audit text).
        detail: String,
    },
    /// The store seam failed.
    Store(CommandGateStoreError),
    /// A non-store seam failed (fail-closed).
    Seam(GateSeamError),
    /// The outbox write failed after the state change was recorded (the
    /// composition root's unit of work resolves the pair — recorded
    /// ordering convention, §6.3 step 1).
    Event { detail: String },
    /// The persisted journal could not be replayed (an unmappable or
    /// incomplete entry) — fail-closed; never guessed around. Also covers
    /// domain-engine rejections surfaced while replaying or transitioning.
    InconsistentJournal { command_id: String, detail: String },
    /// A timestamp could not be represented (controlled-clock discipline).
    InvalidTimestamp { detail: String },
    /// The feasibility evidence's model hash no longer matches the active
    /// model version (the P4.03 hook; §17.5 invariant 3). The command
    /// cannot be issued.
    ModelEvidenceStale {
        /// The hash the feasibility evidence binds to.
        evidence_hash: String,
        /// The active model version's hash.
        active_hash: String,
    },
    /// A FEASIBLE-or-later command carries no feasibility evidence in its
    /// journal — a persistence inconsistency, fail-closed.
    MissingFeasibilityEvidence { command_id: String },
    /// A domain lifecycle guard rejected the transition (the frozen-edge
    /// oracle or one of its reason/evidence rules).
    Lifecycle(LifecycleError),
}

/// The creation-source checks (P4.11 step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreationCheck {
    /// The referenced schedule revision is SUPERSEDED — rejected at
    /// creation, not merely at issue (ledger P4.11 step 4).
    SourceScheduleRevisionSuperseded,
    /// The referenced schedule revision does not exist in the caller's
    /// tenant scope (unknown is never usable).
    SourceScheduleRevisionUnknown,
}

impl CreationCheck {
    /// The stable check code (the `check` of the rejection).
    pub fn code(self) -> &'static str {
        match self {
            CreationCheck::SourceScheduleRevisionSuperseded => {
                "source_schedule_revision_superseded"
            }
            CreationCheck::SourceScheduleRevisionUnknown => "source_schedule_revision_unknown",
        }
    }
}

impl std::fmt::Display for CommandGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandGateError::Schema(error) => write!(f, "command schema rejected: {error}"),
            CommandGateError::WrongState { current, operation } => write!(
                f,
                "operation `{operation}` does not apply to a command in state {}",
                current.name()
            ),
            CommandGateError::CreationRejected { check, detail } => {
                write!(f, "command creation rejected at check '{}': {detail}", check.code())
            }
            CommandGateError::Store(error) => write!(f, "command store: {error}"),
            CommandGateError::Seam(error) => write!(f, "{error}"),
            CommandGateError::Event { detail } => write!(f, "outbox write failed: {detail}"),
            CommandGateError::InconsistentJournal { command_id, detail } => {
                write!(f, "command {command_id}: inconsistent journal: {detail}")
            }
            CommandGateError::InvalidTimestamp { detail } => write!(f, "{detail}"),
            CommandGateError::ModelEvidenceStale {
                evidence_hash,
                active_hash,
            } => write!(
                f,
                "the feasibility evidence binds to model hash '{evidence_hash}' but the active \
                 model version hashes to '{active_hash}'; the model changed and the evidence is \
                 invalidated — the command cannot be issued"
            ),
            CommandGateError::MissingFeasibilityEvidence { command_id } => {
                write!(f, "command {command_id} carries no feasibility evidence in its journal")
            }
            CommandGateError::Lifecycle(error) => write!(f, "lifecycle guard: {error}"),
        }
    }
}

impl std::error::Error for CommandGateError {}

impl From<LifecycleError> for CommandGateError {
    fn from(error: LifecycleError) -> Self {
        CommandGateError::Lifecycle(error)
    }
}

// --- Gate results ------------------------------------------------------------------

/// The outcome of one gate execution. A PASS and a REJECTION are both
/// verdicts (the transition happened, the journal row exists, the event was
/// emitted where the frozen vocabulary has one); only infrastructure
/// failures are `Err`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    /// Whether the gate passed.
    pub passed: bool,
    /// The named failing check (present exactly on a rejection).
    pub failing_check: Option<String>,
    /// The gate's own evidence reference (distinct per gate — the
    /// anti-conflation rule).
    pub evidence_reference: String,
    /// The journalled transition (from, to, actor, timestamp, gate, reason,
    /// evidence).
    pub transition: CommandTransition,
}

/// The feasibility evidence record (a SEPARATE evidence surface from the
/// validation evidence — §14.1; the P4.12 anti-conflation rule). The model
/// hash travels as a STRUCTURED field (not only by parsing the reference).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeasibilityEvidence {
    /// The evidence reference in the recorded format
    /// (`feasibility:attempt/<id>:model/<mid>:hash/<hash>`).
    pub reference: String,
    /// The attempt identifier.
    pub attempt_id: String,
    /// The model identifier the check evaluated against.
    pub model_id: String,
    /// THE MODEL HASH the evidence binds to.
    pub model_content_hash: String,
    /// The margin/diagnostics summary (§12.5 step 7).
    pub margins: String,
    /// The coverage statement/limitations (§12.5 step 6).
    pub coverage: String,
}

/// The result of the feasibility gate: the verdict plus the feasibility
/// evidence (present exactly on a pass).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeasibilityGateResult {
    /// The gate verdict (passed / rejected with the named constraint).
    pub result: GateResult,
    /// The feasibility evidence — present exactly when the gate passed.
    pub feasibility_evidence: Option<FeasibilityEvidence>,
}

/// The P4.13 authorization decision record (§14.3 field 14; §17.3): the
/// recorded facts of one REQUIRED human/policy authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDecisionRecord {
    /// The decision reference (the journal's evidence reference for the
    /// APPROVED entry, and the third distinct evidence surface — validation
    /// ≠ feasibility ≠ authorization).
    pub decision_reference: String,
    /// The authorizing actor identity.
    pub authorizing_actor: String,
    /// The authorizing role id.
    pub authorizing_role: String,
    /// The approved scope, in the recorded text form.
    pub approved_scope: String,
    /// The approved limits (recorded as supplied by the approver).
    pub approved_limits: String,
    /// The decision timestamp (controlled clock, UTC millis).
    pub decided_at_unix_millis: i64,
    /// When the authorization expires (UTC millis; recorded per §17.3).
    pub expiry_unix_millis: i64,
    /// The separation-of-duties policy applied to this decision.
    pub separation_of_duties: AppliedSeparationOfDuties,
    /// The EXACT validation evidence ID this decision covers.
    pub covered_validation_evidence: String,
    /// The EXACT feasibility evidence ID this decision covers.
    pub covered_feasibility_evidence: String,
}

/// The result of the authorization gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationGateResult {
    /// Whether the command was APPROVED.
    pub approved: bool,
    /// The denial reason (present exactly on a denial).
    pub denial_reason: Option<String>,
    /// The decision record (present exactly on an approval).
    pub decision_record: Option<AuthorizationDecisionRecord>,
    /// The journalled transition.
    pub transition: CommandTransition,
}

/// A command created by the P4.11 creation gate.
#[derive(Debug, Clone, PartialEq)]
pub struct ProposedCommand {
    /// The §14.3-validated command record.
    pub command: DispatchCommand,
}

// --- The creation input -------------------------------------------------------------

/// Everything [`CommandGates::create_proposed_command`] needs to build the
/// §14.3 record. The authorization decision reference is NOT an input: at
/// PROPOSED no authorization decision exists, so the gate records the
/// `authz:pending:` placeholder (see [`pending_authorization_reference`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ProposeCommandInput {
    /// The unique command ID (uniqueness is enforced by the store insert).
    pub command_id: CommandId,
    /// The tenant/portfolio/site/asset scope.
    pub scope: CommandScope,
    /// The source subsystem and plugin/model version (§14.3).
    pub source: SourceOrigin,
    /// The source schedule ID + revision (§14.3; the creation-source gate
    /// checks it).
    pub schedule: ScheduleReference,
    /// The reason/action/actor intent.
    pub intent: CommandIntent,
    /// The control target.
    pub target: ControlTarget,
    /// The validity window.
    pub validity: ValidityWindow,
    /// The duration and ramp requirements.
    pub execution: ExecutionRequirements,
    /// The monotonic per-asset/channel sequence (≥ 1).
    pub sequence: u64,
    /// The idempotency key.
    pub idempotency_key: String,
    /// The grid-model and telemetry-snapshot references (§14.3 fields 12–13).
    pub evidence: EvidenceReferences,
    /// The signature or authenticated transport identity (§14.3 field 15).
    pub authenticity: CommandAuthenticity,
    /// The supersedes reference, when this command replaces another.
    pub supersedes: Option<Supersession>,
    /// The full §14.3 command record rendered as JSON text by the caller's
    /// serialization seam (opaque here; P4.14 owns the field-level schema).
    /// Must be non-empty.
    pub command_payload_json: String,
}

// --- The gates ------------------------------------------------------------------------

/// A small controlled clock pinned to one instant — used for (a) REPLAYING
/// persisted journal entries through the domain engine (the replay must
/// reproduce the persisted timestamps exactly) and (b) stamping a gate
/// transition with the SAME instant the gate's evidence reference carries.
struct PinnedClock {
    at_unix_millis: i64,
}

impl LifecycleClock for PinnedClock {
    fn now_unix_millis(&self) -> i64 {
        self.at_unix_millis
    }
}

/// The command gates (P4.11 creation + validation, P4.12 feasibility,
/// P4.13 authorization). One struct per composition; all collaborators are
/// injected (see the seam statement in the module header). Timestamps come
/// EXCLUSIVELY from the supplied controlled clock ([`LifecycleClock`]) —
/// never a wall clock.
pub struct CommandGates<'a> {
    /// The command-record store seam.
    pub store: &'a dyn CommandGateStore,
    /// The P3.13 envelope-freshness seam.
    pub envelopes: &'a dyn ScheduleEnvelopeSource,
    /// The P4.03 model-version seam.
    pub models: &'a dyn ModelVersionSource,
    /// The asset-registry/capability seam.
    pub assets: &'a dyn AssetCapabilitySource,
    /// The §12.5 feasibility runner seam.
    pub feasibility: &'a dyn CommandFeasibilityRunner,
    /// The authorization evaluator seam.
    pub authz: &'a dyn CommandAuthorizationEvaluator,
    /// The transactional outbox (events).
    pub outbox: &'a dyn EventOutbox,
    /// The event-payload encoder seam.
    pub encoder: &'a dyn EventPayloadEncoder,
    /// The identifier generator (event correlation ids).
    pub ids: &'a dyn IdGenerator,
    /// The controlled clock (every gate timestamp).
    pub clock: &'a dyn LifecycleClock,
    /// The deployment gate policy (separation of duties, duration cap).
    pub policy: CommandGatePolicy,
}

impl<'a> CommandGates<'a> {
    // ------------------------------------------------------------------
    // P4.11 — creation
    // ------------------------------------------------------------------

    /// Creates a PROPOSED command FROM AN APPROVED SCHEDULE ONLY (P4.11
    /// step 1): the §14.3 record is validated through the existing
    /// `DispatchCommand` validators, the creation-source gate checks the
    /// schedule revision (SUPERSEDED and UNKNOWN are rejected AT CREATION),
    /// the record is inserted in PROPOSED, and `DispatchProposed` is
    /// emitted. The journal starts empty (the freeze's initial marker
    /// `[*] → PROPOSED` is not a transition).
    pub fn create_proposed_command(
        &self,
        ctx: &TenantContext,
        input: ProposeCommandInput,
    ) -> Result<ProposedCommand, CommandGateError> {
        let valid_until_millis = i64::try_from(input.validity.valid_until.unix_millis())
            .map_err(|_| CommandGateError::InvalidTimestamp {
                detail: "the validity bound is out of range for unix millis".to_string(),
            })?;
        // 1. Schema: the §14.3 field-group validators (P1.06), reused —
        //    never restated. The authorization reference is the pending
        //    placeholder (no authorization decision exists at PROPOSED).
        let pending_expiry =
            ValidUntil::from_unix_millis(valid_until_millis).map_err(|error| {
                CommandGateError::InvalidTimestamp {
                    detail: format!("the validity bound is not representable: {error}"),
                }
            })?;
        let authorization = AuthorizationReference {
            decision_reference: pending_authorization_reference(input.command_id),
            approved_scope: "pending".to_string(),
            approved_limits: "pending".to_string(),
            expiry: pending_expiry,
        };
        let command = DispatchCommand::try_new(
            input.command_id,
            input.scope.clone(),
            input.source.clone(),
            input.schedule.clone(),
            input.intent.clone(),
            input.target.clone(),
            input.validity.clone(),
            input.execution,
            input.sequence,
            input.idempotency_key.clone(),
            input.evidence.clone(),
            authorization,
            input.authenticity.clone(),
            input.supersedes.clone(),
        )
        .map_err(CommandGateError::Schema)?;

        // 2. The creation-source gate: the command references the schedule
        //    ID + revision; only an ACTIVE revision in the caller's tenant
        //    scope may back it (P3.22 chain + the APPROVED-carrying
        //    semantics the store seam delegates to).
        let usability = self
            .store
            .schedule_revision_state(
                ctx,
                &command.schedule().schedule_id,
                command.schedule().revision,
            )
            .map_err(CommandGateError::Store)?;
        match usability {
            ScheduleRevisionUsability::Active => {}
            ScheduleRevisionUsability::Superseded => {
                return Err(CommandGateError::CreationRejected {
                    check: CreationCheck::SourceScheduleRevisionSuperseded,
                    detail: format!(
                        "schedule {} revision {} is SUPERSEDED; a command may only be created \
                         from the active revision",
                        command.schedule().schedule_id,
                        command.schedule().revision
                    ),
                });
            }
            ScheduleRevisionUsability::Unknown => {
                return Err(CommandGateError::CreationRejected {
                    check: CreationCheck::SourceScheduleRevisionUnknown,
                    detail: format!(
                        "schedule {} revision {} does not exist in this tenant scope; unknown \
                         is never usable",
                        command.schedule().schedule_id,
                        command.schedule().revision
                    ),
                });
            }
        }

        // 3. Persist the PROPOSED record (content immutable from here).
        if input.command_payload_json.trim().is_empty() {
            return Err(CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "the command payload rendering is empty".to_string(),
            });
        }
        let record = ProposedCommandRecord {
            tenant_id: ctx.tenant_id(),
            command_id: command.command_id(),
            portfolio_id: command.scope().portfolio_id.to_string(),
            site_id: command.scope().site_id.to_string(),
            asset_id: command.scope().asset_id.to_string(),
            schedule_id: command.schedule().schedule_id.to_string(),
            schedule_revision: command.schedule().revision,
            valid_until_millis,
            supersedes_command_id: command
                .supersedes()
                .and_then(|s| s.supersedes)
                .map(|id| id.to_string()),
            command_payload_json: input.command_payload_json,
        };
        let receipt = self
            .store
            .insert_proposed(ctx, &record)
            .map_err(CommandGateError::Store)?;

        // 4. Emit `DispatchProposed` (the §15.3 event exists for PROPOSED).
        let correlation = self.ids.new_uuid();
        self.emit_dispatch_event(
            receipt,
            DomainEvent::DispatchProposed(DispatchProposedPayload {
                correlation_id: correlation,
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                command_id: command.command_id().uuid(),
                event_time: self.event_time()?,
            }),
            correlation,
        )?;

        Ok(ProposedCommand { command })
    }

    // ------------------------------------------------------------------
    // P4.11 — the validation gate
    // ------------------------------------------------------------------

    /// Runs the VALIDATION gate (P4.11 step 2; §14.2 VALIDATED): schema
    /// (re-checked through the `ControlTarget` validators), freshness (the
    /// P3.13 envelope gate; the referenced grid model ACTIVE per P4.03),
    /// asset checks (exists in tenant, capability model present, quantities
    /// declared), ramp, SOC-declaration, and the command-policy checks.
    ///
    /// Applies ONLY from PROPOSED. On pass: PROPOSED → VALIDATED with the
    /// validation evidence reference. On fail: PROPOSED → REJECTED with the
    /// failing check named (and `DispatchRejected` emitted). There is NO
    /// `DispatchValidated` event in the frozen §15.3 vocabulary — the
    /// VALIDATED fact lives in the journal (recorded gap, module header).
    pub fn run_validation_gate(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        actor: &Actor,
    ) -> Result<GateResult, CommandGateError> {
        let view = self.load(ctx, command.command_id())?;
        if view.state() != CommandState::Proposed {
            return Err(CommandGateError::WrongState {
                current: view.state(),
                operation: "run_validation_gate",
            });
        }
        let mut lifecycle = self.replay(command.command_id(), &view)?;

        // The checks, in the §14.2 VALIDATED order. The first failure wins
        // (one failing check per rejection, deterministically).
        let failing = self.validation_checks(ctx, command);

        // One instant for the evidence reference AND the transition stamp
        // (pinned controlled clock).
        let at = self.clock.now_unix_millis();
        let pinned = PinnedClock { at_unix_millis: at };
        let evidence_reference = match &failing {
            Some(check) => {
                format!("validation:command/{}@{at}:check/{check}", command.command_id())
            }
            None => format!("validation:command/{}@{at}", command.command_id()),
        };
        let evidence = EvidenceReference::new(evidence_reference.clone()).map_err(|_| {
            CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "the assembled validation evidence reference is empty".to_string(),
            }
        })?;
        let outcome = match &failing {
            Some(check) => GateOutcome::fail(evidence, check.clone())?,
            None => GateOutcome::pass(evidence),
        };
        let transition = lifecycle.record_validation(outcome, actor.clone(), &pinned)?;
        let receipt = self.persist(ctx, &transition)?;

        match failing {
            Some(check) => {
                self.emit_rejected(ctx, command, receipt, correlation_of(&transition))?;
                Ok(GateResult {
                    passed: false,
                    failing_check: Some(check),
                    evidence_reference,
                    transition,
                })
            }
            None => Ok(GateResult {
                passed: true,
                failing_check: None,
                evidence_reference,
                transition,
            }),
        }
    }

    /// The validation checks in §14.2 order; `Some(check)` is the first
    /// failing check's stable code. Every seam error maps to a fail-closed
    /// check code — never to a pass.
    fn validation_checks(&self, ctx: &TenantContext, command: &DispatchCommand) -> Option<String> {
        // 1. Schema — the construction already validated every §14.3 field
        //    group; the cross-field target rules are re-checked here so the
        //    gate's schema surface is explicit.
        if command.target().validate().is_err() {
            return Some("schema_target_invalid".to_string());
        }

        // 2. Freshness — the P3.13 envelope staleness gate on the source
        //    schedule's envelope.
        match self.envelopes.freshness(ctx, command.schedule()) {
            Ok(EnvelopeGateFreshness::Fresh) => {}
            Ok(EnvelopeGateFreshness::Stale { reason }) => {
                return Some(format!("envelope_stale:{reason}"));
            }
            Err(error) => return Some(format!("envelope_check_failed:{}", error.detail)),
        }

        // 3. Freshness — the referenced grid model must be ACTIVE (P4.03:
        //    only an Active model may back a feasibility check; an inactive
        //    model is never silently accepted).
        match self.models.status(ctx, &command.evidence().grid_model) {
            Ok(ModelVersionGateStatus::Active { .. }) => {}
            Ok(ModelVersionGateStatus::NotActive { state }) => {
                return Some(format!("model_version_not_active:{state}"));
            }
            Ok(ModelVersionGateStatus::Unknown) => return Some("model_version_unknown".to_string()),
            Err(error) => return Some(format!("model_check_failed:{}", error.detail)),
        }

        // 4. Asset checks — exists in the caller's tenant, capability model
        //    present, quantities declared (the target's quantities must all
        //    be declared; undeclared is never unconstrained).
        let declared = match self.assets.check(ctx, command.scope().asset_id) {
            Ok(AssetGateStatus::Usable(declared)) => declared,
            Ok(AssetGateStatus::UnknownInTenantScope) => {
                return Some("asset_unknown_in_tenant_scope".to_string());
            }
            Ok(AssetGateStatus::CapabilityModelMissing) => {
                return Some("asset_capability_model_missing".to_string());
            }
            Err(error) => return Some(format!("asset_check_failed:{}", error.detail)),
        };
        match command.target() {
            ControlTarget::PowerSetpoint { q, .. } => {
                if !declared.active_power {
                    return Some("quantity_not_declared:active_power".to_string());
                }
                if q.is_some() && !declared.reactive_power {
                    return Some("quantity_not_declared:reactive_power".to_string());
                }
            }
            ControlTarget::SupportedParameter { value, .. } => match value {
                ControlParameterValue::ActivePower(_) => {
                    if !declared.active_power {
                        return Some("quantity_not_declared:active_power".to_string());
                    }
                }
                ControlParameterValue::ReactivePower(_, _) => {
                    if !declared.reactive_power {
                        return Some("quantity_not_declared:reactive_power".to_string());
                    }
                }
                ControlParameterValue::SocFraction(_) => {
                    if !declared.soc {
                        return Some("quantity_not_declared:soc".to_string());
                    }
                }
            },
        }

        // 5. Ramp — the command's required ramp rates must both fit the
        //    declared capability.
        let ramp = &command.execution().ramp;
        if ramp.max_up_mw_per_s > declared.max_up_ramp_mw_per_s {
            return Some("ramp_up_exceeds_declared_capability".to_string());
        }
        if ramp.max_down_mw_per_s > declared.max_down_ramp_mw_per_s {
            return Some("ramp_down_exceeds_declared_capability".to_string());
        }

        // 6. SOC value: the fraction's [0,1] bound is structural
        //    (`StateOfCharge`); the declaration check above is the SOC
        //    surface of this gate (recorded: no SOC-range re-check exists
        //    because the type makes an out-of-range fraction unrepresentable).

        // 7. Command-policy checks: the execution duration must fit inside
        //    the validity window (a duration longer than the window can
        //    never complete before expiry) and inside the deployment cap,
        //    when configured.
        let window_span = command.validity().valid_until.unix_millis()
            - command.validity().valid_from.unix_millis();
        let duration_millis = i64::try_from(command.execution().duration.as_millis())
            .unwrap_or(i64::MAX);
        if i128::from(duration_millis) > window_span {
            return Some("duration_exceeds_validity_window".to_string());
        }
        if let Some(cap) = self.policy.max_execution_duration {
            let cap_millis = i64::try_from(cap.as_millis()).unwrap_or(i64::MAX);
            if duration_millis > cap_millis {
                return Some("command_policy_duration_exceeded".to_string());
            }
        }

        None
    }

    // ------------------------------------------------------------------
    // P4.12 — the feasibility gate
    // ------------------------------------------------------------------

    /// Runs the FEASIBILITY gate for one command (P4.12; §12.5, §14.1 edge
    /// 3/4). Applies ONLY from VALIDATED.
    ///
    /// 1. The schedule-revision check runs AGAIN here (the P4.10
    ///    `ensure_schedule_revision_usable` pre-wiring and the grid gate's
    ///    revision check agree): a revision SUPERSEDED since validation
    ///    rejects the command through THIS gate.
    /// 2. The §12.5 pipeline runs via the runner seam; the verdict moves
    ///    the command: pass → FEASIBLE, limit violated → REJECTED with the
    ///    constraint named, fail-closed → REJECTED with the reason.
    /// 3. The feasibility evidence is a SEPARATE reference from the
    ///    validation evidence (distinct journal rows; distinct field on
    ///    [`FeasibilityGateResult`]) and references THE MODEL HASH.
    pub fn run_feasibility_gate(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        refs: &CommandFeasibilityRefs,
        actor: &Actor,
    ) -> Result<FeasibilityGateResult, CommandGateError> {
        let view = self.load(ctx, command.command_id())?;
        if view.state() != CommandState::Validated {
            return Err(CommandGateError::WrongState {
                current: view.state(),
                operation: "run_feasibility_gate",
            });
        }
        let mut lifecycle = self.replay(command.command_id(), &view)?;

        // The schedule-revision gate check (both layers agree — the seam
        // delegates to the P4.10 `ensure_schedule_revision_usable`).
        let usability = self
            .store
            .schedule_revision_state(
                ctx,
                &command.schedule().schedule_id,
                command.schedule().revision,
            )
            .map_err(CommandGateError::Store)?;
        if usability != ScheduleRevisionUsability::Active {
            let check = match usability {
                ScheduleRevisionUsability::Superseded => "source_schedule_revision_superseded",
                _ => "source_schedule_revision_unknown",
            };
            let at = self.clock.now_unix_millis();
            let reference =
                format!("feasibility:command/{}@{at}:check/{check}", command.command_id());
            return self.reject_through_feasibility_gate(
                ctx,
                &mut lifecycle,
                command,
                actor,
                &check,
                reference,
                // The usability pre-check is NOT a completed §12.5 check:
                // no check ran, so no `FeasibilityCheckCompleted` is
                // emitted (D-P4.30-1) — `DispatchRejected` is the fact of
                // record.
                None,
            );
        }

        // The §12.5 network pipeline (steps 1–4 + the P4.06/P4.07 verdict,
        // via the seam).
        let verdict = self
            .feasibility
            .run(
                ctx,
                &command.schedule().schedule_id,
                command.schedule().revision,
                refs,
            )
            .map_err(CommandGateError::Seam)?;
        match verdict {
            FeasibilityGateVerdict::Passed {
                attempt_id,
                model_id,
                model_content_hash,
                margins,
                coverage,
            } => {
                let reference =
                    feasibility_evidence_ref(&attempt_id, &model_id, &model_content_hash);
                let evidence =
                    EvidenceReference::new(reference.clone()).map_err(|_| {
                        CommandGateError::InconsistentJournal {
                            command_id: command.command_id().to_string(),
                            detail: "the assembled feasibility evidence reference is empty"
                                .to_string(),
                        }
                    })?;
                let transition = lifecycle
                    .record_feasibility(GateOutcome::pass(evidence), actor.clone(), self.clock)
                    .map_err(CommandGateError::from)?;
                // The FEASIBLE fact has no §15.3 event (recorded gap), but
                // the COMPLETED CHECK does (P4.30, D-P4.30-1):
                // `FeasibilityCheckCompleted` is emitted through the
                // transactional outbox, paired with the same transition
                // receipt (the one-receipt-per-event reading recorded in
                // the P3.28 routes — one receipt descriptor may carry more
                // than one paired event of the SAME state change).
                let receipt = self.persist(ctx, &transition)?;
                self.emit_feasibility_check_completed(ctx, command, receipt, correlation_of(&transition))?;
                Ok(FeasibilityGateResult {
                    result: GateResult {
                        passed: true,
                        failing_check: None,
                        evidence_reference: reference.clone(),
                        transition,
                    },
                    feasibility_evidence: Some(FeasibilityEvidence {
                        reference,
                        attempt_id,
                        model_id,
                        model_content_hash,
                        margins,
                        coverage,
                    }),
                })
            }
            FeasibilityGateVerdict::LimitViolated {
                attempt_id,
                model_id,
                model_content_hash,
                constraint,
            } => {
                let reference =
                    feasibility_evidence_ref(&attempt_id, &model_id, &model_content_hash);
                let check = format!("network_limit:{constraint}");
                self.reject_through_feasibility_gate(
                    ctx,
                    &mut lifecycle,
                    command,
                    actor,
                    &check,
                    reference,
                    // The check RAN and completed with a verdict: the
                    // completion fact is emitted (D-P4.30-1).
                    Some(attempt_id),
                )
            }
            FeasibilityGateVerdict::FailClosed {
                attempt_id,
                model_id,
                model_content_hash,
                reason,
            } => {
                // Fail-closed: reject with the reason named (§12.5 closing
                // rule; §17.5 invariant 9). The reject-vs-HOLD split is
                // P4.08's policy table — recorded boundary (D-P4.12-3).
                let at = self.clock.now_unix_millis();
                let reference = match (&model_id, &model_content_hash) {
                    (Some(model_id), Some(hash)) => {
                        feasibility_evidence_ref(&attempt_id, model_id, hash)
                    }
                    _ => format!("feasibility:attempt/{attempt_id}:fail-closed@{at}"),
                };
                let check = format!("fail_closed:{reason}");
                self.reject_through_feasibility_gate(
                    ctx,
                    &mut lifecycle,
                    command,
                    actor,
                    &check,
                    reference,
                    // The check RAN and completed (fail-closed is a verdict,
                    // never a pass): the completion fact is emitted.
                    Some(attempt_id),
                )
            }
        }
    }

    /// The P4.16 issue-time hook (P4.12 step 3; §17.5 invariant 3): an
    /// APPROVED command whose feasibility evidence's model hash no longer
    /// matches the ACTIVE model version cannot be issued. The dispatch
    /// adapter (P4.16) calls this before ISSUED with the active model
    /// version's content hash.
    ///
    /// Fails closed: a non-APPROVED command, a journal without the
    /// feasibility evidence, or an unparseable evidence reference is an
    /// error — never "assume current".
    pub fn ensure_command_issueable(
        &self,
        ctx: &TenantContext,
        command_id: CommandId,
        active_model_content_hash: &str,
    ) -> Result<(), CommandGateError> {
        let view = self.load(ctx, command_id)?;
        if view.state() != CommandState::Approved {
            return Err(CommandGateError::WrongState {
                current: view.state(),
                operation: "issue",
            });
        }
        let feasibility_evidence = view
            .journal
            .iter()
            .find(|entry| entry.to == CommandState::Feasible)
            .and_then(|entry| entry.evidence.as_ref())
            .ok_or(CommandGateError::MissingFeasibilityEvidence {
                command_id: command_id.to_string(),
            })?;
        let evidence_hash = parse_feasibility_evidence_hash(feasibility_evidence.as_str())
            .ok_or_else(|| CommandGateError::InconsistentJournal {
                command_id: command_id.to_string(),
                detail: format!(
                    "the feasibility evidence reference '{}' does not carry a model hash",
                    feasibility_evidence.as_str()
                ),
            })?;
        if evidence_hash != active_model_content_hash {
            return Err(CommandGateError::ModelEvidenceStale {
                evidence_hash: evidence_hash.to_string(),
                active_hash: active_model_content_hash.to_string(),
            });
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // P4.13 — the authorization gate
    // ------------------------------------------------------------------

    /// Records the AUTHORIZATION gate (P4.13; §14.1 edge 6/7, §17.3).
    /// Applies ONLY from FEASIBLE — APPROVED without FEASIBLE is impossible
    /// here exactly as it is in the domain engine (the domain guard test
    /// lives in `flexcore-domain/tests/command_lifecycle.rs`
    /// `approved_without_feasible_evidence_is_impossible_by_construction`;
    /// the application guard is the state check plus the fact that THIS is
    /// the only operation in the crate that can produce APPROVED).
    ///
    /// The approval requires, in order — each failure is a DENIAL that
    /// moves FEASIBLE → REJECTED with the reason and the denying actor:
    ///
    /// 1. an ALLOW decision from the authorization evaluator for
    ///    `command.approve` at the recorded scope (deny-by-default);
    /// 2. an approval scope that COVERS the command's asset;
    /// 3. an approver distinct from the proposing identity, where
    ///    separation of duties is configured (§17.4);
    /// 4. an expiry in the future (an already-expired authorization cannot
    ///    approve; later APPROVED → EXPIRED handling is P4.15's).
    ///
    /// On approval the [`AuthorizationDecisionRecord`] carries authorizing
    /// actor, role, scope, limits, expiry, decision time, the applied
    /// separation-of-duties policy, and the EXACT validation + feasibility
    /// evidence IDs the decision covers. `DispatchApproved` is emitted.
    pub fn record_command_approval(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        request: &CommandAuthorizationRequest,
        approved_limits: &str,
        expiry: ValidUntil,
    ) -> Result<AuthorizationGateResult, CommandGateError> {
        let view = self.load(ctx, command.command_id())?;
        if view.state() != CommandState::Feasible {
            return Err(CommandGateError::WrongState {
                current: view.state(),
                operation: "record_command_approval",
            });
        }
        let mut lifecycle = self.replay(command.command_id(), &view)?;
        let approver = Actor::new(request.subject.clone()).map_err(|_| {
            CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "the approver identity is empty".to_string(),
            }
        })?;

        // The exact evidence IDs this decision covers (§17.5 invariant 3:
        // the authorization rests on BOTH prior gates' evidence; the IDs
        // are taken from the journal — the evidence of record — never from
        // caller input).
        let covered_validation = self
            .journal_evidence(&view, CommandState::Validated)
            .ok_or_else(|| CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "no validation evidence in the journal of a FEASIBLE command".to_string(),
            })?;
        let covered_feasibility = self
            .journal_evidence(&view, CommandState::Feasible)
            .ok_or_else(|| CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "no feasibility evidence in the journal of a FEASIBLE command".to_string(),
            })?;

        // 1. The policy decision at the recorded scope (deny-by-default).
        let policy_decision = self.authz.evaluate(
            ctx,
            &CommandAuthorizationRequest {
                subject: request.subject.clone(),
                role: request.role.clone(),
                action: CommandAuthzAction::Approve,
                scope: request.scope,
            },
        );
        let mut denial = match &policy_decision {
            AuthorizationDecision::Allowed { .. } => None,
            AuthorizationDecision::Denied { reason } => {
                Some(format!("authorization_denied:{reason}"))
            }
        };

        // 2. The approval scope must cover the command's asset.
        if denial.is_none()
            && !request
                .scope
                .covers(command.scope().site_id, command.scope().asset_id)
        {
            denial = Some(format!(
                "approval_scope_does_not_cover_command_asset:{}",
                request.scope.recorded_text()
            ));
        }

        // 3. Separation of duties (§17.4): the approver must not be the
        //    proposing identity where configured. The proposing identity is
        //    the command's own `actor_or_service` (§14.3 field 5).
        let applied_sod = match self.policy.separation_of_duties {
            SeparationOfDuties::Enforced => {
                if request.subject == command.intent().actor_or_service {
                    return self.reject_authorization(
                        ctx,
                        &mut lifecycle,
                        command,
                        &approver,
                        &covered_validation,
                        &covered_feasibility,
                        "self-approval prohibited (separation of duties, §17.4)",
                    );
                }
                AppliedSeparationOfDuties::Enforced
            }
            SeparationOfDuties::NotConfigured => AppliedSeparationOfDuties::NotConfigured,
        };

        // 4. The authorization must not already be expired (§17.3
        //    expirations; P4.15 owns the later APPROVED → EXPIRED sweep).
        if denial.is_none() && expiry.unix_millis() <= i128::from(self.clock.now_unix_millis()) {
            denial = Some("authorization_expired_before_decision".to_string());
        }

        if let Some(reason) = denial {
            return self.reject_authorization(
                ctx,
                &mut lifecycle,
                command,
                &approver,
                &covered_validation,
                &covered_feasibility,
                &reason,
            );
        }

        // Approve: build the decision record and transition.
        let at = self.clock.now_unix_millis();
        let expiry_millis =
            i64::try_from(expiry.unix_millis()).map_err(|_| CommandGateError::InvalidTimestamp {
                detail: "the authorization expiry is out of range for unix millis".to_string(),
            })?;
        let decision_reference = format!("authz-decision:command/{}@{at}", command.command_id());
        let record = AuthorizationDecisionRecord {
            decision_reference: decision_reference.clone(),
            authorizing_actor: request.subject.clone(),
            authorizing_role: request.role.clone(),
            approved_scope: request.scope.recorded_text(),
            approved_limits: approved_limits.to_string(),
            decided_at_unix_millis: at,
            expiry_unix_millis: expiry_millis,
            separation_of_duties: applied_sod,
            covered_validation_evidence: covered_validation,
            covered_feasibility_evidence: covered_feasibility,
        };
        let decision =
            EvidenceReference::new(decision_reference.clone()).map_err(|_| {
                CommandGateError::InconsistentJournal {
                    command_id: command.command_id().to_string(),
                    detail: "the assembled authorization decision reference is empty".to_string(),
                }
            })?;
        let transition = lifecycle
            .record_authorization(AuthorizationOutcome::approve(decision, approver), self.clock)
            .map_err(CommandGateError::from)?;
        let receipt = self.persist(ctx, &transition)?;
        let correlation = self.ids.new_uuid();
        self.emit_dispatch_event(
            receipt,
            DomainEvent::DispatchApproved(DispatchApprovedPayload {
                correlation_id: correlation,
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                command_id: command.command_id().uuid(),
                event_time: self.event_time()?,
            }),
            correlation,
        )?;
        Ok(AuthorizationGateResult {
            approved: true,
            denial_reason: None,
            decision_record: Some(record),
            transition,
        })
    }

    /// Records an authorization DENIAL: FEASIBLE → REJECTED with the denial
    /// reason and the denying actor (§14.1 edge 7; ledger P4.13 step 3).
    #[allow(clippy::too_many_arguments)]
    fn reject_authorization(
        &self,
        ctx: &TenantContext,
        lifecycle: &mut CommandLifecycle,
        command: &DispatchCommand,
        approver: &Actor,
        covered_validation: &str,
        covered_feasibility: &str,
        reason: &str,
    ) -> Result<AuthorizationGateResult, CommandGateError> {
        let at = self.clock.now_unix_millis();
        // The denial decision reference names the command and the instant;
        // the reason travels in the journal row. The denial record is the
        // third distinct evidence surface.
        let decision_reference =
            format!("authz-denial:command/{}@{at}", command.command_id());
        let decision =
            EvidenceReference::new(decision_reference.clone()).map_err(|_| {
                CommandGateError::InconsistentJournal {
                    command_id: command.command_id().to_string(),
                    detail: "the assembled denial reference is empty".to_string(),
                }
            })?;
        let outcome = AuthorizationOutcome::deny(
            decision,
            approver.clone(),
            format!(
                "{reason} [covers validation evidence {covered_validation}; feasibility \
                 evidence {covered_feasibility}]"
            ),
        )?;
        let transition = lifecycle
            .record_authorization(outcome, self.clock)
            .map_err(CommandGateError::from)?;
        let receipt = self.persist(ctx, &transition)?;
        self.emit_rejected(ctx, command, receipt, correlation_of(&transition))?;
        Ok(AuthorizationGateResult {
            approved: false,
            denial_reason: Some(reason.to_string()),
            decision_record: None,
            transition,
        })
    }

    // ------------------------------------------------------------------
    // Shared machinery
    // ------------------------------------------------------------------

    /// Loads a command; a missing command is a `NotFound` store error.
    fn load(
        &self,
        ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<CommandGateView, CommandGateError> {
        self.store
            .load_command(ctx, command_id)
            .map_err(CommandGateError::Store)?
            .ok_or(CommandGateError::Store(CommandGateStoreError::NotFound {
                command_id: command_id.to_string(),
            }))
    }

    /// The evidence reference of the journal entry that entered `state`.
    fn journal_evidence(&self, view: &CommandGateView, state: CommandState) -> Option<String> {
        view.journal
            .iter()
            .find(|entry| entry.to == state)
            .and_then(|entry| entry.evidence.as_ref())
            .map(|evidence| evidence.as_str().to_string())
    }

    /// Replays the persisted journal through the domain engine so the next
    /// gate transition applies from the true current state. The replay uses
    /// the PERSISTED timestamps (a pinned controlled clock per entry), so
    /// monotonicity is reproduced, never invented. Gates operate only from
    /// PROPOSED / VALIDATED / FEASIBLE; a journal containing any other
    /// entry is a persistence inconsistency, fail-closed.
    fn replay(
        &self,
        command_id: CommandId,
        view: &CommandGateView,
    ) -> Result<CommandLifecycle, CommandGateError> {
        let valid_until =
            ValidUntil::from_unix_millis(view.valid_until_millis).map_err(|error| {
                CommandGateError::InvalidTimestamp {
                    detail: format!("the persisted validity bound is not representable: {error}"),
                }
            })?;
        let mut lifecycle = CommandLifecycle::propose(command_id, valid_until)?;
        for entry in &view.journal {
            let clock = PinnedClock {
                at_unix_millis: entry.at_unix_millis,
            };
            let evidence = entry.evidence.clone().ok_or_else(|| {
                CommandGateError::InconsistentJournal {
                    command_id: command_id.to_string(),
                    detail: format!(
                        "journal entry to {} carries no evidence reference",
                        entry.to.name()
                    ),
                }
            })?;
            match (entry.from, entry.to) {
                (CommandState::Proposed, CommandState::Validated) => {
                    lifecycle
                        .record_validation(GateOutcome::pass(evidence), entry.actor.clone(), &clock)?;
                }
                (CommandState::Validated, CommandState::Feasible) => {
                    lifecycle
                        .record_feasibility(GateOutcome::pass(evidence), entry.actor.clone(), &clock)?;
                }
                _ => {
                    return Err(CommandGateError::InconsistentJournal {
                        command_id: command_id.to_string(),
                        detail: format!(
                            "journal entry {} -> {} is not replayable by the command gates",
                            entry.from.name(),
                            entry.to.name()
                        ),
                    });
                }
            }
        }
        Ok(lifecycle)
    }

    /// Records a feasibility-gate REJECTION, persists the transition, and
    /// emits `DispatchRejected` — plus `FeasibilityCheckCompleted` when a
    /// §12.5 check actually ran and completed with the verdict (`check_id`
    /// is `Some(attempt_id)`); the usability pre-check passes `None` and
    /// emits only `DispatchRejected` (D-P4.30-1). Both events pair with
    /// the SAME transition receipt (the recorded one-receipt-per-event
    /// reading of the P3.28 routes).
    #[allow(clippy::too_many_arguments)]
    fn reject_through_feasibility_gate(
        &self,
        ctx: &TenantContext,
        lifecycle: &mut CommandLifecycle,
        command: &DispatchCommand,
        actor: &Actor,
        check: &str,
        evidence_reference: String,
        check_id: Option<String>,
    ) -> Result<FeasibilityGateResult, CommandGateError> {
        let evidence = EvidenceReference::new(evidence_reference.clone()).map_err(|_| {
            CommandGateError::InconsistentJournal {
                command_id: command.command_id().to_string(),
                detail: "the assembled rejection evidence reference is empty".to_string(),
            }
        })?;
        let transition = lifecycle
            .record_feasibility(GateOutcome::fail(evidence, check.to_string())?, actor.clone(), self.clock)
            .map_err(CommandGateError::from)?;
        let receipt = self.persist(ctx, &transition)?;
        let correlation = correlation_of(&transition);
        // Both events of the SAME state change pair with the SAME receipt
        // descriptor (the recorded one-receipt-per-event reading of the
        // P3.28 routes; the receipt is Clone for exactly this pairing).
        self.emit_rejected(ctx, command, receipt.clone(), correlation)?;
        if check_id.is_some() {
            self.emit_feasibility_check_completed(ctx, command, receipt, correlation)?;
        }
        Ok(FeasibilityGateResult {
            result: GateResult {
                passed: false,
                failing_check: Some(check.to_string()),
                evidence_reference,
                transition,
            },
            feasibility_evidence: None,
        })
    }

    /// Persists one transition through the store seam, returning the receipt
    /// the event write pairs with.
    fn persist(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransition,
    ) -> Result<StateChangeReceipt, CommandGateError> {
        self.store
            .journal_transition(ctx, transition)
            .map_err(CommandGateError::Store)
    }

    /// Emits `DispatchRejected` (the §15.3 event exists for REJECTED),
    /// paired with the transition's store receipt.
    fn emit_rejected(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        receipt: StateChangeReceipt,
        correlation: FlexUuid,
    ) -> Result<(), CommandGateError> {
        self.emit_dispatch_event(
            receipt,
            DomainEvent::DispatchRejected(DispatchRejectedPayload {
                correlation_id: correlation,
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                command_id: command.command_id().uuid(),
                event_time: self.event_time()?,
            }),
            correlation,
        )
    }

    /// Emits one dispatch event through the transactional outbox (state
    /// change first, then the outbox write — the P1.08 ordering).
    fn emit_dispatch_event(
        &self,
        state_change: StateChangeReceipt,
        event: DomainEvent,
        correlation: FlexUuid,
    ) -> Result<(), CommandGateError> {
        emit(
            state_change,
            event,
            correlation,
            self.event_time()?,
            self.encoder,
            self.outbox,
        )
        .map_err(|error| CommandGateError::Event {
            detail: error.to_string(),
        })
    }

    /// Emits `FeasibilityCheckCompleted` (the §15.3 grid-validation event;
    /// P4.30 step 2), paired with the feasibility transition's store
    /// receipt. The correlation is the command id — the deterministic,
    /// replay-safe convention of this module's events (the outbox key is
    /// `feasibility_check_completed:<command uuid>`; the gate admits at
    /// most ONE runner-verdict check per command, so the key is unique per
    /// event instance). The frozen payload carries identity + time only —
    /// the verdict detail travels in the journal evidence and the
    /// persisted feasibility-check record (migration 0017).
    fn emit_feasibility_check_completed(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        state_change: StateChangeReceipt,
        correlation: FlexUuid,
    ) -> Result<(), CommandGateError> {
        self.emit_dispatch_event(
            state_change,
            DomainEvent::FeasibilityCheckCompleted(FeasibilityCheckCompletedPayload {
                correlation_id: correlation,
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                event_time: self.event_time()?,
            }),
            correlation,
        )
    }

    /// The controlled clock's current instant as an `EventTime`.
    fn event_time(&self) -> Result<EventTime, CommandGateError> {
        EventTime::from_unix_millis(self.clock.now_unix_millis()).map_err(|error| {
            CommandGateError::InvalidTimestamp {
                detail: format!("the controlled clock instant is not representable: {error}"),
            }
        })
    }
}

/// The event correlation id for a rejected transition: derived from the
/// command id (deterministic, replay-safe; the outbox store enforces key
/// uniqueness per event INSTANCE, and each rejection is one instance).
fn correlation_of(transition: &CommandTransition) -> FlexUuid {
    transition.command_id.uuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feasibility_evidence_reference_format_round_trips() {
        let reference = feasibility_evidence_ref("attempt-7", "feeder-a", "fnv1a64:deadbeef");
        assert_eq!(
            parse_feasibility_evidence_hash(&reference),
            Some("fnv1a64:deadbeef")
        );
        assert_eq!(parse_feasibility_evidence_hash("validation:command/x@1"), None);
        assert_eq!(
            parse_feasibility_evidence_hash("feasibility:attempt/a:model/m:hash/"),
            None
        );
    }

    #[test]
    fn the_pending_authorization_reference_names_the_command() {
        let command_id = CommandId::new(FlexUuid::from_bytes([7u8; 16]));
        let reference = pending_authorization_reference(command_id);
        assert!(reference.starts_with("authz:pending:command/"));
        assert!(reference.contains(&command_id.to_string()));
    }

    #[test]
    fn approval_scope_coverage_is_tenant_superset_site_superset_asset() {
        let site = SiteId::new(FlexUuid::from_bytes([1u8; 16]));
        let asset = AssetId::new(FlexUuid::from_bytes([2u8; 16]));
        let other_site = SiteId::new(FlexUuid::from_bytes([3u8; 16]));
        let other_asset = AssetId::new(FlexUuid::from_bytes([4u8; 16]));
        assert!(ApprovalScope::Tenant.covers(site, asset));
        assert!(ApprovalScope::Site(site).covers(site, asset));
        assert!(!ApprovalScope::Site(other_site).covers(site, asset));
        assert!(ApprovalScope::Asset(asset).covers(site, asset));
        assert!(!ApprovalScope::Asset(other_asset).covers(site, asset));
        assert_eq!(
            ApprovalScope::Asset(asset).recorded_text(),
            format!("asset:{asset}")
        );
    }

    #[test]
    fn the_command_authz_action_ids_match_the_authz_surface() {
        assert_eq!(CommandAuthzAction::Create.id(), "command.create");
        assert_eq!(CommandAuthzAction::Validate.id(), "command.validate");
        assert_eq!(CommandAuthzAction::Approve.id(), "command.approve");
        assert_eq!(CommandAuthzAction::Read.id(), "command.read");
    }
}
