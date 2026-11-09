// P4.14 — Command invariants implementation (§14.3; ledger P4.14; report
//         §14.3, §10.2, §17.5).
//
// The §14.3 field-level command-record schema, the creation/issue
// completeness validators, the §14.3 final-paragraph rejection rules, and
// the per-asset (never portfolio-inferred) acknowledgement type.
//
// ## What is reused vs added (ledger P4.14 step 1)
//
// The §14.3 record VALUE is the P1.06 `DispatchCommand`
// (`flexcore_domain::dispatch_command`) — its constructor already validates
// every §14.3 field group and its module header maps each §14.3 bullet to
// its representation. It is REUSED, never restated. This module ADDS the
// pieces the P4.11–P4.13 gates explicitly deferred to P4.14 (decision
// record `11_13_command_gates.md`, "the full §14.3 field-level record
// schema is P4.14's"):
//
// - [`CommandRecordFields`] — the flat §14.3 field-level record: one named
//   field per §14.3 sub-item, built FROM a validated `DispatchCommand`
//   ([`CommandRecordFields::from_command`]) plus the authorization
//   decision fields (the placeholder at creation, the real decision at
//   issue — D-P4.11-3). This is the shape the composition root renders into
//   the persisted `command_payload_json` (flexcore-application has no
//   serializer; the rendering stays an injected seam).
// - [`CommandRecordFields::validate_creation`] /
//   [`CommandRecordFields::validate_issue`] — the two completeness
//   validators (ledger step 2): ANY missing field is a NAMED
//   [`CommandRecordError::MissingField`]/[`CommandRecordError::InvalidField`]
//   that blocks the transition. The recorded failure mode ("a field treated
//   as optional 'for now' becomes a permanent gap") is mitigated
//   structurally: every field of the table
//   [`S143_FIELD_GROUPS`] has its own named error and its own test, so
//   dropping a field from the struct breaks the completeness tests loudly.
// - [`CommandRejectionRule`] + one pure decision function per rule (ledger
//   step 3): stale, duplicated-with-conflicting-payload, out-of-sequence,
//   outside-capability, missing-required-evidence → REJECTED (§14.3 final
//   paragraph). The rules are VERDICTS — the gates and the dispatch adapter
//   (P4.16) route them through the lifecycle's three rejection points; this
//   module decides, it does not transition.
// - [`FleetAcknowledgement`] — per-asset acknowledgement states (ledger
//   step 4): a portfolio-level acknowledgement is NEVER inferred from the
//   first responding device (§14.3). Acknowledgement is application-layer
//   ACCEPTANCE, never physical delivery (§14.1) — the type and its
//   vocabulary say "acceptance", not "delivery".
//
// ## The recorded completeness table
//
// [`S143_FIELD_GROUPS`] is the normative mapping of the 17 §14.3 bullets to
// the named fields of [`CommandRecordFields`], in §14.3 order. The
// supersedes/superseded-by group is required ONLY "when a command replaces
// another" (§14.3 field 17) — every other field is required on EVERY
// command, at creation AND at issue. The completeness tests in
// `tests/command_record.rs` corrupt every field of this table and assert
// the named error; a field removed from the struct fails the build there.
//
// ## Control status
//
// The command lifecycle is a protected platform invariant (§0.2, §5.7);
// plugins may propose commands but can never authorize, issue, or select
// the control-authority mode. The entire 0.x line is simulation-only
// (§21.4): nothing here writes to a device, and an acknowledgement recorded
// anywhere in this crate is acceptance, never physical delivery. FlexCore
// performs no primary frequency response, synthetic inertia, relay
// protection, inverter inner-loop control, or sub-cycle control (§14.5).

use flexcore_contracts::time_model::ValidUntil;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_domain::command_lifecycle::{
    CommandLifecycle, CommandTransition, EvidenceReference, ExpiryMilestone, FailureReason,
    LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    CommandAuthenticity, ControlTarget, ControlParameterValue, DispatchCommand,
};
use flexcore_domain::ids::{AssetId, CommandId};
use flexcore_contracts::ids::FlexUuid;

use crate::command_gates::{pending_authorization_reference, CommandGateView};

// --- The recorded §14.3 completeness table -------------------------------------

/// The prefix of the authorization-reference placeholder recorded at
/// creation (D-P4.11-3: at PROPOSED no authorization decision exists). The
/// issue validator refuses a record still carrying this prefix — the
/// placeholder is never mistaken for a decision.
pub const PENDING_AUTHORIZATION_PREFIX: &str = "authz:pending:";

/// The recorded §14.3 completeness table: the 17 §14.3 bullets (ledger
/// P4.14 step 1, in ledger order) mapped to the named fields of
/// [`CommandRecordFields`]. This is the normative list the completeness
/// tests drive; a field removed from the struct (or from this table)
/// breaks those tests — the recorded mitigation for the "optional for now"
/// failure mode.
pub const S143_FIELD_GROUPS: [(&str, &[&str]); 17] = [
    ("unique_command_id", &["command_id"]),
    ("scope_ids", &["tenant_id", "portfolio_id", "site_id", "asset_id"]),
    (
        "source_subsystem_and_version",
        &["source_subsystem", "source_component_version"],
    ),
    ("schedule_reference", &["schedule_id", "schedule_revision"]),
    ("intent", &["reason", "action", "actor_or_service"]),
    ("target", &["target"]),
    ("units_and_sign_convention", &["units", "sign_convention"]),
    ("validity_window", &["valid_from_millis", "valid_until_millis"]),
    (
        "duration_and_ramp",
        &["duration_millis", "ramp_up_mw_per_s", "ramp_down_mw_per_s"],
    ),
    ("sequence", &["sequence"]),
    ("idempotency_key", &["idempotency_key"]),
    ("grid_model_ref", &["grid_model_ref"]),
    ("telemetry_snapshot_ref", &["telemetry_snapshot_ref"]),
    ("validation_evidence_ref", &["validation_evidence_ref"]),
    (
        "authorization_reference",
        &[
            "authorization_decision_ref",
            "approved_scope",
            "approved_limits",
            "authorization_expiry_millis",
        ],
    ),
    ("authenticity", &["authenticity"]),
    // §14.3: "supersedes/superseded-by reference WHEN a command replaces
    // another" — the only conditionally-required group.
    ("supersedes_reference", &["supersedes_command_id", "superseded_by_command_id"]),
];

// --- Errors ---------------------------------------------------------------------

/// The phase a completeness validation ran in (§14.3: the record is
/// validated at creation AND re-validated at issue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordValidationPhase {
    /// The creation completeness validation (before the record may enter
    /// the lifecycle at PROPOSED).
    Creation,
    /// The issue re-validation (before APPROVED → ISSUED; P4.16's adapter
    /// runs it).
    Issue,
}

impl RecordValidationPhase {
    /// The stable text form used in error details.
    pub fn as_str(self) -> &'static str {
        match self {
            RecordValidationPhase::Creation => "creation",
            RecordValidationPhase::Issue => "issue",
        }
    }
}

/// The named errors of the §14.3 record validators and the journal replay.
/// A missing or invalid field is NEVER silent and NEVER defaulted: it
/// blocks the transition with its own named error (ledger P4.14 step 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRecordError {
    /// A required §14.3 field was absent/empty at the given phase.
    MissingField {
        /// Which phase blocked the transition.
        phase: RecordValidationPhase,
        /// The field name (a name from [`S143_FIELD_GROUPS`]).
        field: &'static str,
    },
    /// A present §14.3 field carried an invalid value at the given phase.
    InvalidField {
        /// Which phase blocked the transition.
        phase: RecordValidationPhase,
        /// The field name.
        field: &'static str,
        /// The recorded detail.
        detail: String,
    },
    /// The authorization reference is still the creation-time placeholder
    /// (`authz:pending:…`) at ISSUE — a pending authorization is never a
    /// decision (D-P4.11-3; the P4.13 decision record is the real
    /// reference).
    AuthorizationPending {
        /// The command whose authorization is still pending.
        command_id: String,
    },
    /// A timestamp/bound was not representable (controlled-clock
    /// discipline).
    InvalidTimestamp {
        /// The recorded detail.
        detail: String,
    },
    /// A domain lifecycle guard rejected a replayed transition (the
    /// frozen-edge oracle or one of its reason/evidence rules).
    Lifecycle(LifecycleError),
    /// The persisted journal could not be replayed (an unmappable,
    /// incomplete, or mismatched entry) — fail-closed; never guessed
    /// around.
    InconsistentJournal {
        /// The command whose journal is inconsistent.
        command_id: String,
        /// The recorded detail.
        detail: String,
    },
}

impl std::fmt::Display for CommandRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandRecordError::MissingField { phase, field } => write!(
                f,
                "§14.3 {phase} validation: required field `{field}` is missing — the transition \
                 is blocked"
            ),
            CommandRecordError::InvalidField { phase, field, detail } => write!(
                f,
                "§14.3 {phase} validation: field `{field}` is invalid: {detail}"
            ),
            CommandRecordError::AuthorizationPending { command_id } => write!(
                f,
                "command {command_id} still carries the pending authorization placeholder at \
                 issue; a real authorization decision (P4.13) is required before ISSUED"
            ),
            CommandRecordError::InvalidTimestamp { detail } => write!(f, "{detail}"),
            CommandRecordError::Lifecycle(error) => write!(f, "lifecycle guard: {error}"),
            CommandRecordError::InconsistentJournal { command_id, detail } => {
                write!(f, "command {command_id}: inconsistent journal: {detail}")
            }
        }
    }
}

impl std::error::Error for CommandRecordError {}

impl From<LifecycleError> for CommandRecordError {
    fn from(error: LifecycleError) -> Self {
        CommandRecordError::Lifecycle(error)
    }
}

// --- The authorization decision fields --------------------------------------------

/// The §14.3 field-15 authorization-decision fields as the record schema
/// carries them: decision reference, approved scope, approved limits, and
/// expiry. At creation these hold the recorded placeholder
/// ([`AuthorizationRecordFields::pending_placeholder`]); at issue they hold
/// the real P4.13 [`AuthorizationDecisionRecord`](crate::command_gates::
/// AuthorizationDecisionRecord) values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRecordFields {
    /// The decision reference (`authz-decision:command/<id>@<t>` on a real
    /// decision; `authz:pending:command/<id>` at creation).
    pub decision_reference: String,
    /// The approved scope (recorded text).
    pub approved_scope: String,
    /// The approved limits (recorded text).
    pub approved_limits: String,
    /// When the authorization expires (UTC unix millis).
    pub expiry_unix_millis: i64,
}

impl AuthorizationRecordFields {
    /// The creation-time placeholder fields (D-P4.11-3): the pending
    /// reference, and scope/limits "pending" with the command's own
    /// valid-until as the bound. The placeholder is NEVER a decision — the
    /// issue validator refuses it.
    pub fn pending_placeholder(command_id: CommandId, valid_until_millis: i64) -> Self {
        Self {
            decision_reference: pending_authorization_reference(command_id),
            approved_scope: "pending".to_string(),
            approved_limits: "pending".to_string(),
            expiry_unix_millis: valid_until_millis,
        }
    }
}

// --- The flat §14.3 record schema --------------------------------------------------

/// The flat §14.3 field-level command record (P4.14 step 1): one named
/// field per §14.3 sub-item, per [`S143_FIELD_GROUPS`]. Built FROM a
/// validated `DispatchCommand` — it carries no validity of its own; the
/// validators below are the enforcement surface.
///
/// Units and sign conventions are CARRIED as explicit named fields
/// (`units`, `sign_convention`) rendered from the typed target — the
/// contracts unit types remain the structural guarantee (a command can
/// never carry a bare `f64`); the flat fields exist so the persisted
/// record shows the §14.3 declarations verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRecordFields {
    // Group 1 — unique command ID.
    pub command_id: String,
    // Group 2 — tenant, portfolio, site and asset IDs.
    pub tenant_id: String,
    pub portfolio_id: String,
    pub site_id: String,
    pub asset_id: String,
    // Group 3 — source subsystem and plugin/model version.
    pub source_subsystem: String,
    pub source_component_version: String,
    // Group 4 — source schedule ID and revision.
    pub schedule_id: String,
    pub schedule_revision: u64,
    // Group 5 — reason/action and actor or service identity.
    pub reason: String,
    pub action: String,
    pub actor_or_service: String,
    // Group 6 — target P/Q or supported control parameter (rendered
    // descriptor; the typed value lives on the `DispatchCommand`).
    pub target: String,
    // Group 7 — units and sign convention.
    pub units: String,
    pub sign_convention: String,
    // Group 8 — valid-from / valid-until (UTC unix millis).
    pub valid_from_millis: i64,
    pub valid_until_millis: i64,
    // Group 9 — duration and ramp requirements.
    pub duration_millis: i64,
    pub ramp_up_mw_per_s: f64,
    pub ramp_down_mw_per_s: f64,
    // Group 10 — monotonic sequence/version per asset or control channel.
    pub sequence: u64,
    // Group 11 — idempotency key.
    pub idempotency_key: String,
    // Group 12 — grid model reference.
    pub grid_model_ref: String,
    // Group 13 — telemetry snapshot reference.
    pub telemetry_snapshot_ref: String,
    // Group 14 — validation evidence reference.
    pub validation_evidence_ref: String,
    // Group 15 — authorization decision reference (scope, limits, expiry).
    pub authorization_decision_ref: String,
    pub approved_scope: String,
    pub approved_limits: String,
    pub authorization_expiry_millis: i64,
    // Group 16 — signature or authenticated transport identity.
    pub authenticity: String,
    // Group 17 — supersedes/superseded-by (only when a command replaces
    // another).
    pub supersedes_command_id: Option<String>,
    pub superseded_by_command_id: Option<String>,
}

impl CommandRecordFields {
    /// Builds the flat record from a §14.3-validated command plus the
    /// authorization fields (the placeholder at creation, the real P4.13
    /// decision at issue). Fails only on unrepresentable bounds.
    pub fn from_command(
        command: &DispatchCommand,
        authorization: &AuthorizationRecordFields,
    ) -> Result<Self, CommandRecordError> {
        let valid_from_millis = i64::try_from(command.validity().valid_from.unix_millis())
            .map_err(|_| CommandRecordError::InvalidTimestamp {
                detail: "the valid-from bound is out of range for unix millis".to_string(),
            })?;
        let valid_until_millis = i64::try_from(command.validity().valid_until.unix_millis())
            .map_err(|_| CommandRecordError::InvalidTimestamp {
                detail: "the valid-until bound is out of range for unix millis".to_string(),
            })?;
        // A duration longer than i64 millis cannot occur for a real
        // command; saturate like the gates do (the creation validator
        // compares it against the window span, so saturation cannot hide a
        // policy violation).
        let duration_millis =
            i64::try_from(command.execution().duration.as_millis()).unwrap_or(i64::MAX);

        let (target, units, sign_convention) = render_target(command.target());

        let authenticity = match command.authenticity() {
            CommandAuthenticity::Signature(s) => format!("signature:{s}"),
            CommandAuthenticity::AuthenticatedTransport(s) => {
                format!("authenticated_transport:{s}")
            }
        };

        let supersedes = command.supersedes();
        Ok(Self {
            command_id: command.command_id().to_string(),
            tenant_id: command.scope().tenant_id.to_string(),
            portfolio_id: command.scope().portfolio_id.to_string(),
            site_id: command.scope().site_id.to_string(),
            asset_id: command.scope().asset_id.to_string(),
            source_subsystem: command.source().subsystem.clone(),
            source_component_version: command.source().component_version.clone(),
            schedule_id: command.schedule().schedule_id.to_string(),
            schedule_revision: command.schedule().revision,
            reason: command.intent().reason.clone(),
            action: command.intent().action.clone(),
            actor_or_service: command.intent().actor_or_service.clone(),
            target,
            units,
            sign_convention,
            valid_from_millis,
            valid_until_millis,
            duration_millis,
            ramp_up_mw_per_s: command.execution().ramp.max_up_mw_per_s,
            ramp_down_mw_per_s: command.execution().ramp.max_down_mw_per_s,
            sequence: command.sequence(),
            idempotency_key: command.idempotency_key().to_string(),
            grid_model_ref: command.evidence().grid_model.clone(),
            telemetry_snapshot_ref: command.evidence().telemetry_snapshot.clone(),
            validation_evidence_ref: command.evidence().validation_evidence.clone(),
            authorization_decision_ref: authorization.decision_reference.clone(),
            approved_scope: authorization.approved_scope.clone(),
            approved_limits: authorization.approved_limits.clone(),
            authorization_expiry_millis: authorization.expiry_unix_millis,
            authenticity,
            supersedes_command_id: supersedes.and_then(|s| s.supersedes).map(|id| id.to_string()),
            superseded_by_command_id: supersedes
                .and_then(|s| s.superseded_by)
                .map(|id| id.to_string()),
        })
    }

    /// The CREATION completeness validation (§14.3; ledger P4.14 step 2):
    /// every required field present and valid, in [`S143_FIELD_GROUPS`]
    /// order (the first missing/invalid group in §14.3 order is reported —
    /// deterministic). The authorization group may carry the recorded
    /// pending placeholder at creation (D-P4.11-3) but must not be empty.
    pub fn validate_creation(&self) -> Result<(), CommandRecordError> {
        self.validate_common(RecordValidationPhase::Creation)?;
        Ok(())
    }

    /// The ISSUE re-validation (§14.3; ledger P4.14 step 2): every
    /// required field present and valid, AND the authorization group must
    /// carry a REAL decision (the pending placeholder blocks the
    /// transition — a command is never issued on a pending authorization),
    /// AND the authorization must not have expired at `now_unix_millis`.
    pub fn validate_issue(&self, now_unix_millis: i64) -> Result<(), CommandRecordError> {
        self.validate_common(RecordValidationPhase::Issue)?;
        if self
            .authorization_decision_ref
            .starts_with(PENDING_AUTHORIZATION_PREFIX)
        {
            return Err(CommandRecordError::AuthorizationPending {
                command_id: self.command_id.clone(),
            });
        }
        if self.authorization_expiry_millis <= now_unix_millis {
            return Err(CommandRecordError::InvalidField {
                phase: RecordValidationPhase::Issue,
                field: "authorization_expiry_millis",
                detail: format!(
                    "the authorization expired at {} (now {now_unix_millis}); an expired \
                     authorization cannot back an issue",
                    self.authorization_expiry_millis
                ),
            });
        }
        Ok(())
    }

    /// The shared per-field checks, in §14.3 order. Every check produces
    /// its named error; nothing is defaulted.
    fn validate_common(&self, phase: RecordValidationPhase) -> Result<(), CommandRecordError> {
        let miss = |field: &'static str| CommandRecordError::MissingField { phase, field };
        let invalid = |field: &'static str, detail: String| CommandRecordError::InvalidField {
            phase,
            field,
            detail,
        };
        // Group 1 — unique command ID (uniqueness itself is the store's
        // insert conflict, caller-side by the P1.06 recorded boundary).
        require_non_empty(&self.command_id, miss("command_id"))?;
        // Group 2 — scope IDs.
        require_non_empty(&self.tenant_id, miss("tenant_id"))?;
        require_non_empty(&self.portfolio_id, miss("portfolio_id"))?;
        require_non_empty(&self.site_id, miss("site_id"))?;
        require_non_empty(&self.asset_id, miss("asset_id"))?;
        // Group 3 — source subsystem and plugin/model version.
        require_non_empty(&self.source_subsystem, miss("source_subsystem"))?;
        require_non_empty(&self.source_component_version, miss("source_component_version"))?;
        // Group 4 — schedule reference.
        require_non_empty(&self.schedule_id, miss("schedule_id"))?;
        if self.schedule_revision == 0 {
            return Err(invalid(
                "schedule_revision",
                "the schedule revision must be at least 1".to_string(),
            ));
        }
        // Group 5 — intent.
        require_non_empty(&self.reason, miss("reason"))?;
        require_non_empty(&self.action, miss("action"))?;
        require_non_empty(&self.actor_or_service, miss("actor_or_service"))?;
        // Group 6 — target.
        require_non_empty(&self.target, miss("target"))?;
        // Group 7 — units and sign convention.
        require_non_empty(&self.units, miss("units"))?;
        require_non_empty(&self.sign_convention, miss("sign_convention"))?;
        // Group 8 — validity window (inverted windows are invalid here even
        // though the contracts type makes them unrepresentable at
        // construction — defence in depth for the persisted record).
        if self.valid_from_millis > self.valid_until_millis {
            return Err(invalid(
                "valid_from_millis",
                format!(
                    "valid-from {} is after valid-until {} — inverted validity window",
                    self.valid_from_millis, self.valid_until_millis
                ),
            ));
        }
        // Group 9 — duration and ramp.
        if self.duration_millis <= 0 {
            return Err(invalid(
                "duration_millis",
                "the execution duration must be greater than zero".to_string(),
            ));
        }
        for (field, rate) in [
            ("ramp_up_mw_per_s", self.ramp_up_mw_per_s),
            ("ramp_down_mw_per_s", self.ramp_down_mw_per_s),
        ] {
            if !rate.is_finite() || rate <= 0.0 {
                return Err(invalid(
                    field,
                    "ramp rates must be finite and strictly positive".to_string(),
                ));
            }
        }
        // Group 10 — monotonic sequence (≥ 1; ordering is caller-side per
        // the P1.06 recorded boundary, checked by the P4.14
        // out-of-sequence rejection rule against the per-asset registry).
        if self.sequence == 0 {
            return Err(invalid(
                "sequence",
                "the command sequence must be at least 1".to_string(),
            ));
        }
        // Group 11 — idempotency key.
        require_non_empty(&self.idempotency_key, miss("idempotency_key"))?;
        // Groups 12–14 — evidence references.
        require_non_empty(&self.grid_model_ref, miss("grid_model_ref"))?;
        require_non_empty(&self.telemetry_snapshot_ref, miss("telemetry_snapshot_ref"))?;
        require_non_empty(&self.validation_evidence_ref, miss("validation_evidence_ref"))?;
        // Group 15 — authorization reference (placeholder allowed only at
        // creation; the phase-specific checks add the rest).
        require_non_empty(&self.authorization_decision_ref, miss("authorization_decision_ref"))?;
        require_non_empty(&self.approved_scope, miss("approved_scope"))?;
        require_non_empty(&self.approved_limits, miss("approved_limits"))?;
        // Group 16 — authenticity.
        require_non_empty(&self.authenticity, miss("authenticity"))?;
        // Group 17 — supersedes: only required when a replacement is
        // declared, and then at least one direction must be named.
        if self.supersedes_command_id.is_none() && self.superseded_by_command_id.is_none() {
            // Both absent: legal UNLESS the record is SUPPOSED to declare a
            // replacement — which the flat record cannot know; the
            // `DispatchCommand` group carries that intent. The declared-but-
            // empty case is unrepresentable (the fields are `Option` and
            // `from_command` maps them 1:1). Both `Some` is legal (a
            // mid-chain replacement).
        }
        Ok(())
    }
}

fn require_non_empty(
    value: &str,
    error: CommandRecordError,
) -> Result<(), CommandRecordError> {
    if value.trim().is_empty() {
        Err(error)
    } else {
        Ok(())
    }
}

/// Renders the target group into its (descriptor, units, sign convention)
/// triple. The descriptor is recorded text; the typed unit value travels on
/// the `DispatchCommand` — the rendering never re-parses numbers.
fn render_target(target: &ControlTarget) -> (String, String, String) {
    const ACTIVE_SIGN: &str = "P>0=export";
    match target {
        ControlTarget::PowerSetpoint { q, conventions, .. } => {
            let units = if q.is_some() {
                "MW + MVAr".to_string()
            } else {
                "MW".to_string()
            };
            let sign = match conventions.reactive {
                Some(ReactiveSignConvention::InjectionPositive) => {
                    format!("{ACTIVE_SIGN}; Q>0=injection (declared)")
                }
                Some(ReactiveSignConvention::ConsumptionPositive) => {
                    format!("{ACTIVE_SIGN}; Q>0=consumption (declared)")
                }
                None => ACTIVE_SIGN.to_string(),
            };
            (
                "power_setpoint".to_string(),
                units,
                sign,
            )
        }
        ControlTarget::SupportedParameter { name, value } => {
            let (units, sign) = match value {
                ControlParameterValue::ActivePower(_) => ("MW".to_string(), ACTIVE_SIGN.to_string()),
                ControlParameterValue::ReactivePower(_, convention) => (
                    "MVAr".to_string(),
                    match convention {
                        ReactiveSignConvention::InjectionPositive => {
                            "Q>0=injection (declared)".to_string()
                        }
                        ReactiveSignConvention::ConsumptionPositive => {
                            "Q>0=consumption (declared)".to_string()
                        }
                    },
                ),
                ControlParameterValue::SocFraction(_) => {
                    ("SOC fraction".to_string(), "n/a (fraction in [0,1])".to_string())
                }
            };
            (format!("parameter:{name}"), units, sign)
        }
    }
}

// --- §14.3 final-paragraph rejection rules (verdicts, not transitions) --------------

/// The five §14.3 final-paragraph rejection rules (ledger P4.14 step 3).
/// Each rule's stable code is the `failing_check` text the gates record on
/// the REJECTED journal row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandRejectionRule {
    /// Stale: the validity window ended before the command was processed
    /// (§10.3 end-exclusive: `now >= valid_until`).
    Stale,
    /// Duplicated with a conflicting payload: the same idempotency key
    /// carrying a DIFFERENT payload (an exact duplicate is an idempotent
    /// replay, never a rejection).
    DuplicatedConflictingPayload,
    /// Out of sequence: the per-asset/per-channel sequence is not strictly
    /// greater than the last recorded one.
    OutOfSequence,
    /// Outside capability: the target (or required ramp) does not fit the
    /// asset's declared capability window.
    OutsideCapability,
    /// Missing required evidence: the journal lacks the evidence reference
    /// the command's current state requires.
    MissingRequiredEvidence,
}

impl CommandRejectionRule {
    /// The stable check code recorded as the failing check.
    pub fn code(self) -> &'static str {
        match self {
            CommandRejectionRule::Stale => "stale",
            CommandRejectionRule::DuplicatedConflictingPayload => {
                "duplicated_conflicting_payload"
            }
            CommandRejectionRule::OutOfSequence => "out_of_sequence",
            CommandRejectionRule::OutsideCapability => "outside_capability",
            CommandRejectionRule::MissingRequiredEvidence => "missing_required_evidence",
        }
    }
}

/// One rejection verdict: which §14.3 rule fired and the recorded detail
/// (audit text). The gates/adapters route a verdict through the lifecycle's
/// matching rejection point; this module never transitions state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRejection {
    /// The §14.3 rule that fired.
    pub rule: CommandRejectionRule,
    /// The recorded detail.
    pub detail: String,
}

impl CommandRejection {
    fn new(rule: CommandRejectionRule, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
        }
    }

    /// The stable check code (the rule's code; convenience for the
    /// journal's failing-check field).
    pub fn code(&self) -> &'static str {
        self.rule.code()
    }
}

/// Rule STALE (§14.3): the validity window has ended at `now_unix_millis`
/// (§10.3 end-exclusive: `now >= valid_until`). `None` while the window is
/// still open.
pub fn stale_rejection(valid_until_millis: i64, now_unix_millis: i64) -> Option<CommandRejection> {
    if now_unix_millis >= valid_until_millis {
        Some(CommandRejection::new(
            CommandRejectionRule::Stale,
            format!(
                "valid-until {valid_until_millis} passed at {now_unix_millis} (end-exclusive); \
                 the command is stale"
            ),
        ))
    } else {
        None
    }
}

/// The duplicate classification (§14.3 "duplicated with conflicting
/// payloads"): the same idempotency key with an IDENTICAL payload is an
/// idempotent replay (no new record, no rejection); the same key with a
/// DIFFERENT payload is the conflicting duplicate that gets REJECTED;
/// a new key is a new command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateVerdict {
    /// A new command (no prior record under this idempotency key).
    New,
    /// The same idempotency key and the same payload — an idempotent
    /// replay; the caller returns the original record's outcome.
    IdempotentReplay,
    /// The same idempotency key with a different payload — REJECTED.
    ConflictingPayload,
}

/// Rule DUPLICATED-WITH-CONFLICTING-PAYLOAD (§14.3): `Some` exactly when
/// the idempotency key matches an existing record and the payload
/// fingerprint differs.
pub fn duplicate_conflict_rejection(
    same_idempotency_key: bool,
    same_payload: bool,
) -> Option<CommandRejection> {
    if same_idempotency_key && !same_payload {
        Some(CommandRejection::new(
            CommandRejectionRule::DuplicatedConflictingPayload,
            "the idempotency key matches an existing command but the payload differs; a \
             conflicting duplicate is rejected (an exact duplicate would be an idempotent replay)",
        ))
    } else {
        None
    }
}

/// Rule OUT-OF-SEQUENCE (§14.3): the incoming per-asset/per-channel
/// sequence must be STRICTLY greater than the last recorded one. The first
/// command of a channel (no recorded predecessor) is never out of sequence.
pub fn out_of_sequence_rejection(
    incoming_sequence: u64,
    last_recorded_sequence: Option<u64>,
) -> Option<CommandRejection> {
    match last_recorded_sequence {
        Some(last) if incoming_sequence <= last => Some(CommandRejection::new(
            CommandRejectionRule::OutOfSequence,
            format!(
                "incoming sequence {incoming_sequence} is not greater than the last recorded \
                 sequence {last} for this asset/control channel"
            ),
        )),
        _ => None,
    }
}

/// The asset's declared capability window (the outside-capability rule's
/// reference frame). The source is the asset's capability profile (the
/// P4.11 `AssetCapabilitySource` seam's declared quantities, plus the
/// magnitude bounds the profile declares); the window is CONFIGURATION,
/// never inferred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssetCapabilityWindow {
    /// The minimum active-power setpoint (MW; signed, frozen `P>0=export`).
    pub min_p_mw: f64,
    /// The maximum active-power setpoint (MW).
    pub max_p_mw: f64,
    /// The minimum reactive-power setpoint (MVAr), when the profile
    /// declares reactive capability. `None` = reactive targets are outside
    /// capability.
    pub min_q_mvar: Option<f64>,
    /// The maximum reactive-power setpoint (MVAr), when declared.
    pub max_q_mvar: Option<f64>,
    /// The declared maximum upward ramp (MW per second).
    pub max_up_ramp_mw_per_s: f64,
    /// The declared maximum downward ramp (MW per second).
    pub max_down_ramp_mw_per_s: f64,
}

/// Rule OUTSIDE-CAPABILITY (§14.3): the target value (or a required ramp
/// rate) does not fit the declared capability window. SOC fractions are
/// structurally bounded to [0,1] by the contracts type — a SOC target can
/// never be outside capability on magnitude.
pub fn outside_capability_rejection(
    target: &ControlTarget,
    window: &AssetCapabilityWindow,
) -> Option<CommandRejection> {
    match target {
        ControlTarget::PowerSetpoint { p, q, .. } => {
            let pv = p.value();
            if !pv.is_finite() || pv < window.min_p_mw || pv > window.max_p_mw {
                return Some(CommandRejection::new(
                    CommandRejectionRule::OutsideCapability,
                    format!(
                        "active-power setpoint {pv} MW is outside the declared window \
                         [{}, {}] MW",
                        window.min_p_mw, window.max_p_mw
                    ),
                ));
            }
            match (q, window.min_q_mvar, window.max_q_mvar) {
                (Some(q), Some(min_q), Some(max_q)) => {
                    let qv = q.value();
                    if !qv.is_finite() || qv < min_q || qv > max_q {
                        return Some(CommandRejection::new(
                            CommandRejectionRule::OutsideCapability,
                            format!(
                                "reactive-power setpoint {qv} MVAr is outside the declared \
                                 window [{min_q}, {max_q}] MVAr"
                            ),
                        ));
                    }
                    None
                }
                (Some(_), _, _) => Some(CommandRejection::new(
                    CommandRejectionRule::OutsideCapability,
                    "the asset's capability window declares no reactive-power range; a \
                     reactive target is outside capability",
                )),
                (None, _, _) => None,
            }
        }
        ControlTarget::SupportedParameter { name, value } => match value {
            ControlParameterValue::ActivePower(p) => {
                let pv = p.value();
                if !pv.is_finite() || pv < window.min_p_mw || pv > window.max_p_mw {
                    return Some(CommandRejection::new(
                        CommandRejectionRule::OutsideCapability,
                        format!(
                            "parameter '{name}' active-power value {pv} MW is outside the \
                             declared window [{}, {}] MW",
                            window.min_p_mw, window.max_p_mw
                        ),
                    ));
                }
                None
            }
            ControlParameterValue::ReactivePower(q, _) => {
                let qv = q.value();
                match (window.min_q_mvar, window.max_q_mvar) {
                    (Some(min_q), Some(max_q)) => {
                        if !qv.is_finite() || qv < min_q || qv > max_q {
                            return Some(CommandRejection::new(
                                CommandRejectionRule::OutsideCapability,
                                format!(
                                    "parameter '{name}' reactive-power value {qv} MVAr is \
                                     outside the declared window [{min_q}, {max_q}] MVAr"
                                ),
                            ));
                        }
                        None
                    }
                    _ => Some(CommandRejection::new(
                        CommandRejectionRule::OutsideCapability,
                        format!(
                            "parameter '{name}' carries a reactive-power value but the \
                             capability window declares no reactive range"
                        ),
                    )),
                }
            }
            // SOC is a fraction in [0,1] by construction (P0.07).
            ControlParameterValue::SocFraction(_) => None,
        },
    }
}

/// Rule OUTSIDE-CAPABILITY, ramp half (§14.3 "outside capability" covers
/// the ramp requirements too): either required ramp rate exceeding the
/// declared capability is outside capability.
pub fn ramp_outside_capability_rejection(
    required_up_mw_per_s: f64,
    required_down_mw_per_s: f64,
    window: &AssetCapabilityWindow,
) -> Option<CommandRejection> {
    if required_up_mw_per_s > window.max_up_ramp_mw_per_s {
        return Some(CommandRejection::new(
            CommandRejectionRule::OutsideCapability,
            format!(
                "required upward ramp {required_up_mw_per_s} MW/s exceeds the declared \
                 capability {} MW/s",
                window.max_up_ramp_mw_per_s
            ),
        ));
    }
    if required_down_mw_per_s > window.max_down_ramp_mw_per_s {
        return Some(CommandRejection::new(
            CommandRejectionRule::OutsideCapability,
            format!(
                "required downward ramp {required_down_mw_per_s} MW/s exceeds the declared \
                 capability {} MW/s",
                window.max_down_ramp_mw_per_s
            ),
        ));
    }
    None
}

/// The forward-path rank of a state for the missing-evidence rule. Only the
/// forward states have evidence requirements (the gate rejections and
/// failures are governed by their own rules and the P4.10 journal-reporting
/// table).
fn forward_rank(state: CommandState) -> Option<u8> {
    match state {
        CommandState::Proposed => Some(0),
        CommandState::Validated => Some(1),
        CommandState::Feasible => Some(2),
        CommandState::Approved => Some(3),
        CommandState::Issued => Some(4),
        CommandState::Acknowledged => Some(5),
        CommandState::Tracking => Some(6),
        _ => None,
    }
}

/// Rule MISSING-REQUIRED-EVIDENCE (§14.3): the journal lacks the evidence
/// reference the command's current state requires. A command at/after
/// VALIDATED must carry its validation evidence; at/after FEASIBLE its
/// (separate) feasibility evidence; at/after APPROVED its authorization
/// decision evidence. Evidence carried as an empty string counts as
/// missing. `None` for the non-forward states (rejections, failures,
/// expiry, cancellation, supersession, fallback) — their journal rules are
/// the P4.10 reporting table's, not this rule's.
pub fn missing_evidence_rejection(
    state: CommandState,
    journal: &[CommandTransition],
) -> Option<CommandRejection> {
    let rank = forward_rank(state)?;
    let required: [Option<CommandState>; 3] = [
        (rank >= 1).then_some(CommandState::Validated),
        (rank >= 2).then_some(CommandState::Feasible),
        (rank >= 3).then_some(CommandState::Approved),
    ];
    for required_state in required.into_iter().flatten() {
        let entry = journal.iter().find(|entry| entry.to == required_state);
        let missing = match entry {
            None => true,
            Some(entry) => match &entry.evidence {
                None => true,
                Some(evidence) => evidence.as_str().trim().is_empty(),
            },
        };
        if missing {
            return Some(CommandRejection::new(
                CommandRejectionRule::MissingRequiredEvidence,
                format!(
                    "the journal carries no {} evidence reference, which state {} requires",
                    required_state.name(),
                    state.name()
                ),
            ));
        }
    }
    None
}

// --- Per-asset acknowledgement (§14.3 partial fleet success) ------------------------

/// The acknowledgement state of ONE asset's command. "Acknowledgement" is
/// application-layer ACCEPTANCE (§14.1) — the vocabulary deliberately says
/// acceptance/refusal/timeout, never "delivered".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetAcknowledgementState {
    /// The command was issued and no acceptance has arrived yet.
    AwaitingAcceptance,
    /// The gateway/device ACCEPTED the command at the application layer —
    /// receipt/acceptance, NOT physical delivery.
    Accepted,
    /// The gateway/device refused the command (NACK).
    Refused,
    /// The acceptance wait timed out.
    TimedOut,
    /// The transport to the gateway/device failed.
    TransportFailed,
}

impl AssetAcknowledgementState {
    /// The stable recorded text.
    pub fn as_str(self) -> &'static str {
        match self {
            AssetAcknowledgementState::AwaitingAcceptance => "awaiting_acceptance",
            AssetAcknowledgementState::Accepted => "accepted",
            AssetAcknowledgementState::Refused => "refused",
            AssetAcknowledgementState::TimedOut => "timed_out",
            AssetAcknowledgementState::TransportFailed => "transport_failed",
        }
    }
}

/// The acknowledgement record of ONE asset's command (§14.3: "partial fleet
/// success is represented per asset"). This is the unit of fleet reporting:
/// a portfolio-level acknowledgement is NEVER inferred from the first
/// responding device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetAcknowledgement {
    /// The command this acknowledgement belongs to.
    pub command_id: CommandId,
    /// The asset the command targets.
    pub asset_id: AssetId,
    /// The per-asset acknowledgement state.
    pub state: AssetAcknowledgementState,
    /// The acceptance record reference (present exactly when accepted).
    pub acceptance_reference: Option<String>,
    /// The observation instant (controlled clock, UTC unix millis).
    pub at_unix_millis: Option<i64>,
}

impl AssetAcknowledgement {
    /// Builds one per-asset acknowledgement. The acceptance reference is
    /// required exactly for the `Accepted` state (an acceptance without its
    /// record is not recordable); the instant is required for every state
    /// except `AwaitingAcceptance`.
    pub fn new(
        command_id: CommandId,
        asset_id: AssetId,
        state: AssetAcknowledgementState,
        acceptance_reference: Option<String>,
        at_unix_millis: Option<i64>,
    ) -> Result<Self, CommandRecordError> {
        if state == AssetAcknowledgementState::Accepted {
            let reference = acceptance_reference.as_deref().unwrap_or("");
            if reference.trim().is_empty() {
                return Err(CommandRecordError::InvalidField {
                    phase: RecordValidationPhase::Creation,
                    field: "acceptance_reference",
                    detail: "an accepted acknowledgement requires its acceptance record \
                             reference"
                        .to_string(),
                });
            }
        }
        if state != AssetAcknowledgementState::AwaitingAcceptance && at_unix_millis.is_none() {
            return Err(CommandRecordError::MissingField {
                phase: RecordValidationPhase::Creation,
                field: "at_unix_millis",
            });
        }
        if state != AssetAcknowledgementState::Accepted && acceptance_reference.is_some() {
            return Err(CommandRecordError::InvalidField {
                phase: RecordValidationPhase::Creation,
                field: "acceptance_reference",
                detail: format!(
                    "an acknowledgement in state '{}' carries no acceptance reference",
                    state.as_str()
                ),
            });
        }
        Ok(Self {
            command_id,
            asset_id,
            state,
            acceptance_reference,
            at_unix_millis,
        })
    }
}

/// The fleet-level acknowledgement aggregation (§14.3 final paragraph).
/// The aggregate is COMPUTED from every per-asset state — never from the
/// first responding device: `PartiallyAccepted` is the answer as soon as
/// ANY asset has not accepted, no matter how many have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetAcknowledgement {
    per_asset: Vec<AssetAcknowledgement>,
}

impl FleetAcknowledgement {
    /// Aggregates the per-asset acknowledgements. The set must be non-empty
    /// and must not name the same asset twice (one acknowledgement state
    /// per asset per fleet operation).
    pub fn new(per_asset: Vec<AssetAcknowledgement>) -> Result<Self, CommandRecordError> {
        if per_asset.is_empty() {
            return Err(CommandRecordError::InvalidField {
                phase: RecordValidationPhase::Creation,
                field: "per_asset",
                detail: "a fleet acknowledgement must carry at least one per-asset entry"
                    .to_string(),
            });
        }
        let mut seen: Vec<String> = Vec::new();
        for ack in &per_asset {
            let key = ack.asset_id.to_string();
            if seen.contains(&key) {
                return Err(CommandRecordError::InvalidField {
                    phase: RecordValidationPhase::Creation,
                    field: "per_asset",
                    detail: format!("asset {key} appears twice in the fleet acknowledgement"),
                });
            }
            seen.push(key);
        }
        Ok(Self { per_asset })
    }

    /// The per-asset entries, in construction order.
    pub fn per_asset(&self) -> &[AssetAcknowledgement] {
        &self.per_asset
    }

    /// How many assets accepted (application-layer acceptance, not
    /// delivery).
    pub fn accepted_count(&self) -> usize {
        self.per_asset
            .iter()
            .filter(|ack| ack.state == AssetAcknowledgementState::Accepted)
            .count()
    }

    /// The total number of assets in the fleet operation.
    pub fn total(&self) -> usize {
        self.per_asset.len()
    }

    /// Whether EVERY asset accepted. This — not "the first device
    /// responded" — is the only path to a fleet-level `true`.
    pub fn fully_accepted(&self) -> bool {
        self.accepted_count() == self.per_asset.len()
    }

    /// The computed fleet-level state (§14.3: never inferred from the first
    /// responding device — `PartiallyAccepted` is the answer while ANY
    /// asset is outstanding).
    pub fn portfolio_state(&self) -> FleetAcknowledgementState {
        let accepted = self.accepted_count();
        let total = self.per_asset.len();
        if accepted == total {
            FleetAcknowledgementState::AllAccepted { assets: total }
        } else if accepted == 0 {
            FleetAcknowledgementState::NoneAccepted { assets: total }
        } else {
            FleetAcknowledgementState::PartiallyAccepted {
                accepted,
                assets: total,
            }
        }
    }
}

/// The computed fleet-level acknowledgement state (see
/// [`FleetAcknowledgement::portfolio_state`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetAcknowledgementState {
    /// Every asset accepted.
    AllAccepted {
        /// The number of assets.
        assets: usize,
    },
    /// Some but not all assets accepted — the ONLY honest answer for a
    /// partially-responding fleet.
    PartiallyAccepted {
        /// How many accepted.
        accepted: usize,
        /// How many assets in total.
        assets: usize,
    },
    /// No asset accepted.
    NoneAccepted {
        /// The number of assets.
        assets: usize,
    },
}

// --- The P4.15/P4.16 shared replay ---------------------------------------------------

/// A pinned controlled clock replaying ONE persisted journal timestamp
/// (same pattern as the command gates' replay: persisted timestamps are
/// reproduced, never invented).
struct PinnedClock {
    at_unix_millis: i64,
}

impl flexcore_domain::command_lifecycle::LifecycleClock for PinnedClock {
    fn now_unix_millis(&self) -> i64 {
        self.at_unix_millis
    }
}

/// The expiry milestone the frozen table annotates for `state` (the
/// application-side mirror of the domain's state→milestone mapping; the
/// conformance test in `tests/command_expiry.rs` drives the REAL domain
/// engine against this mirror for every state). `None` for states with no
/// EXPIRED edge (frozen edges 10, 15, 20, 24 exist only from APPROVED,
/// ISSUED, ACKNOWLEDGED, TRACKING).
pub fn expiry_milestone_for(state: CommandState) -> Option<ExpiryMilestone> {
    match state {
        CommandState::Approved => Some(ExpiryMilestone::BeforeIssue),
        CommandState::Issued => Some(ExpiryMilestone::BeforeAcknowledgement),
        CommandState::Acknowledged => Some(ExpiryMilestone::BeforeTrackingCompletion),
        CommandState::Tracking => Some(ExpiryMilestone::BeforeCompletion),
        _ => None,
    }
}

/// Maps a persisted FAILED journal reason back to the frozen-table trigger
/// (the reason texts are the domain engine's `FailureReason::reason_text`
/// values — the recorded round-trip contract). Unknown reasons fail closed.
fn failure_reason_from_text(text: &str) -> Option<FailureReason> {
    const ALL: [FailureReason; 6] = [
        FailureReason::Nack,
        FailureReason::TransportTimeout,
        FailureReason::TransportFailure,
        FailureReason::NoExpectedResponse,
        FailureReason::TrackingDeviation,
        FailureReason::TrackingTimeout,
    ];
    ALL.into_iter().find(|reason| reason.reason_text() == text)
}

/// The evidence reference format of a SUPERSEDED journal row (D-P4.10-3:
/// the backward link value is carried as `command:<uuid>`).
pub const SUPERSEDED_EVIDENCE_PREFIX: &str = "command:";

/// Replays a FULL persisted journal through the domain engine so the next
/// transition applies from the true current state. This is the P4.15/P4.16
/// generalization of the command gates' gate-only replay: every frozen
/// edge is mapped to its domain transition function, with the persisted
/// actor, reason, evidence, and timestamp carried through. Fail-closed: an
/// unmappable entry, a missing required reason/evidence, or an unparseable
/// supersede link is a named error — never guessed around.
///
/// Evidence is REQUIRED on the entries the P4.10 reporting table requires
/// it for (→ VALIDATED / FEASIBLE / APPROVED / ISSUED / ACKNOWLEDGED /
/// TRACKING / COMPLETED / SUPERSEDED / FALLBACK, and on gate rejections —
/// this crate's emitters always record it). FAILED / EXPIRED / CANCELLED
/// entries carry reason instead.
pub fn replay_full_lifecycle(
    command_id: CommandId,
    view: &CommandGateView,
) -> Result<CommandLifecycle, CommandRecordError> {
    use flexcore_domain::command_lifecycle::{AuthorizationOutcome, Gate, GateOutcome};

    let inconsistent = |detail: String| CommandRecordError::InconsistentJournal {
        command_id: command_id.to_string(),
        detail,
    };
    let valid_until = ValidUntil::from_unix_millis(view.valid_until_millis)
        .map_err(|error| CommandRecordError::InvalidTimestamp {
            detail: format!("the persisted validity bound is not representable: {error}"),
        })?;
    let mut lifecycle = CommandLifecycle::propose(command_id, valid_until)?;

    for entry in &view.journal {
        let clock = PinnedClock {
            at_unix_millis: entry.at_unix_millis,
        };
        let actor = entry.actor.clone();
        let evidence = entry.evidence.clone();
        let reason = entry.reason.clone();
        let require_evidence = || -> Result<EvidenceReference, CommandRecordError> {
            evidence.clone().ok_or_else(|| {
                inconsistent(format!(
                    "journal entry {} -> {} carries no evidence reference",
                    entry.from.name(),
                    entry.to.name()
                ))
            })
        };
        let require_reason = || -> Result<String, CommandRecordError> {
            reason.clone().filter(|r| !r.trim().is_empty()).ok_or_else(|| {
                inconsistent(format!(
                    "journal entry {} -> {} carries no reason",
                    entry.from.name(),
                    entry.to.name()
                ))
            })
        };
        match (entry.from, entry.to) {
            (CommandState::Proposed, CommandState::Validated) => {
                lifecycle.record_validation(
                    GateOutcome::pass(require_evidence()?),
                    actor,
                    &clock,
                )?;
            }
            (CommandState::Proposed, CommandState::Rejected) => {
                expect_gate(entry, Gate::Validation, &inconsistent)?;
                lifecycle.record_validation(
                    GateOutcome::fail(require_evidence()?, require_reason()?)?,
                    actor,
                    &clock,
                )?;
            }
            (CommandState::Validated, CommandState::Feasible) => {
                lifecycle.record_feasibility(
                    GateOutcome::pass(require_evidence()?),
                    actor,
                    &clock,
                )?;
            }
            (CommandState::Validated, CommandState::Rejected) => {
                expect_gate(entry, Gate::Feasibility, &inconsistent)?;
                lifecycle.record_feasibility(
                    GateOutcome::fail(require_evidence()?, require_reason()?)?,
                    actor,
                    &clock,
                )?;
            }
            (CommandState::Feasible, CommandState::Approved) => {
                lifecycle.record_authorization(
                    AuthorizationOutcome::approve(require_evidence()?, actor),
                    &clock,
                )?;
            }
            (CommandState::Feasible, CommandState::Rejected) => {
                expect_gate(entry, Gate::Authorization, &inconsistent)?;
                lifecycle.record_authorization(
                    AuthorizationOutcome::deny(require_evidence()?, actor, require_reason()?)?,
                    &clock,
                )?;
            }
            (CommandState::Approved, CommandState::Issued) => {
                lifecycle.record_issue(require_evidence()?, actor, &clock)?;
            }
            (CommandState::Issued, CommandState::Acknowledged) => {
                lifecycle.record_acknowledgement(require_evidence()?, actor, &clock)?;
            }
            (CommandState::Acknowledged, CommandState::Tracking) => {
                lifecycle.record_tracking_started(require_evidence()?, actor, &clock)?;
            }
            (CommandState::Tracking, CommandState::Completed) => {
                lifecycle.record_completion(require_evidence()?, actor, &clock)?;
            }
            (CommandState::Issued, CommandState::Failed)
            | (CommandState::Acknowledged, CommandState::Failed)
            | (CommandState::Tracking, CommandState::Failed) => {
                let reason = require_reason()?;
                let trigger = failure_reason_from_text(&reason).ok_or_else(|| {
                    inconsistent(format!(
                        "failure reason '{reason}' is not a frozen-table trigger"
                    ))
                })?;
                lifecycle.record_failure(trigger, actor, &clock)?;
            }
            (
                CommandState::Approved
                | CommandState::Issued
                | CommandState::Acknowledged
                | CommandState::Tracking,
                CommandState::Expired,
            ) => {
                let milestone = expiry_milestone_for(entry.from).ok_or_else(|| {
                    inconsistent(format!(
                        "state {} has no EXPIRED edge in the frozen table",
                        entry.from.name()
                    ))
                })?;
                lifecycle.record_expiry(milestone, actor, &clock)?;
            }
            (CommandState::Approved, CommandState::Cancelled)
            | (CommandState::Issued, CommandState::Cancelled) => {
                lifecycle.record_cancellation(require_reason()?, actor, &clock)?;
            }
            (_, CommandState::Superseded) => {
                let evidence = require_evidence()?;
                let replacement = evidence
                    .as_str()
                    .strip_prefix(SUPERSEDED_EVIDENCE_PREFIX)
                    .and_then(|uuid_text| FlexUuid::parse(uuid_text).ok())
                    .ok_or_else(|| {
                        inconsistent(format!(
                            "the SUPERSEDED evidence reference '{}' does not carry a \
                             `command:<uuid>` backward link",
                            evidence.as_str()
                        ))
                    })?;
                lifecycle.record_superseded_by(CommandId::new(replacement), actor, &clock)?;
            }
            (CommandState::Failed, CommandState::Fallback)
            | (CommandState::Expired, CommandState::Fallback) => {
                lifecycle.record_fallback(require_evidence()?, actor, &clock)?;
            }
            (from, to) => {
                return Err(inconsistent(format!(
                    "journal entry {} -> {} is not a frozen-table edge",
                    from.name(),
                    to.name()
                )));
            }
        }
    }
    Ok(lifecycle)
}

/// Asserts a REJECTED journal entry names the expected gate (the P4.10
/// reporting rule: the gate is present exactly on gate transitions and gate
/// rejections).
fn expect_gate(
    entry: &CommandTransition,
    expected: flexcore_domain::command_lifecycle::Gate,
    inconsistent: &dyn Fn(String) -> CommandRecordError,
) -> Result<(), CommandRecordError> {
    match entry.gate {
        Some(gate) if gate == expected => Ok(()),
        other => Err(inconsistent(format!(
            "journal entry {} -> {} names gate {:?} where {} was required",
            entry.from.name(),
            entry.to.name(),
            other,
            expected.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_s143_table_has_exactly_the_seventeen_bullets() {
        assert_eq!(S143_FIELD_GROUPS.len(), 17);
        assert_eq!(S143_FIELD_GROUPS[0].0, "unique_command_id");
        assert_eq!(S143_FIELD_GROUPS[16].0, "supersedes_reference");
    }

    #[test]
    fn the_pending_prefix_matches_the_recorded_placeholder_format() {
        let command_id = CommandId::new(FlexUuid::from_bytes([9u8; 16]));
        let placeholder = AuthorizationRecordFields::pending_placeholder(command_id, 1_000);
        assert!(placeholder
            .decision_reference
            .starts_with(PENDING_AUTHORIZATION_PREFIX));
    }

    #[test]
    fn stale_rejection_is_end_exclusive() {
        assert!(stale_rejection(1_000, 999).is_none());
        assert!(stale_rejection(1_000, 1_000).is_some());
        assert!(stale_rejection(1_000, 2_000).is_some());
    }

    #[test]
    fn duplicate_rules_classify_exactly_the_conflicting_case() {
        assert!(duplicate_conflict_rejection(false, false).is_none());
        assert!(duplicate_conflict_rejection(false, true).is_none());
        assert!(duplicate_conflict_rejection(true, true).is_none());
        let rejection = duplicate_conflict_rejection(true, false).unwrap();
        assert_eq!(rejection.rule, CommandRejectionRule::DuplicatedConflictingPayload);
    }

    #[test]
    fn sequence_rule_rejects_equal_and_lower() {
        assert!(out_of_sequence_rejection(1, None).is_none());
        assert!(out_of_sequence_rejection(5, Some(4)).is_none());
        assert!(out_of_sequence_rejection(4, Some(4)).is_some());
        assert!(out_of_sequence_rejection(3, Some(4)).is_some());
    }

    #[test]
    fn the_capability_ramp_half_fires_independently() {
        let window = AssetCapabilityWindow {
            min_p_mw: -2.0,
            max_p_mw: 2.0,
            min_q_mvar: Some(-0.5),
            max_q_mvar: Some(0.5),
            max_up_ramp_mw_per_s: 1.0,
            max_down_ramp_mw_per_s: 1.0,
        };
        assert!(ramp_outside_capability_rejection(0.5, 0.5, &window).is_none());
        assert!(ramp_outside_capability_rejection(2.0, 0.5, &window).is_some());
        assert!(ramp_outside_capability_rejection(0.5, 2.0, &window).is_some());
    }

    // Referenced so the error type's Display stays covered without a full
    // fixture here (the suites in tests/ drive the rest).
    #[test]
    fn error_display_names_the_phase_and_field() {
        let error = CommandRecordError::MissingField {
            phase: RecordValidationPhase::Issue,
            field: "idempotency_key",
        };
        let text = error.to_string();
        assert!(text.contains("issue"));
        assert!(text.contains("idempotency_key"));
        let lifecycle_error = CommandRecordError::Lifecycle(LifecycleError::EmptyField {
            field: "actor",
        });
        assert!(lifecycle_error.to_string().contains("lifecycle guard"));
    }
}
