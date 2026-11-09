// P1.06 — DispatchCommand construction validation (report §14.3; P0.10 §1).
//
// Implements the §14.3 command invariants as **construction validation**: a
// `DispatchCommand` cannot be constructed unless every required field group
// is present and valid. Construction failure is a `Result`, never a panic.
//
// What this type is and is not (deliberate scope boundary):
//
// - It is a **domain type**: the value shape of a dispatch command with its
//   §14.3 invariants, consumed by the command lifecycle state machine
//   (`command_state.rs`, P0.11) and later by the application layer.
// - It is **not a working dispatch path**. There is no issue/acknowledge/
//   track execution logic here — the frozen lifecycle (§14.1–§14.2) is a
//   state machine over states, and this module only validates the command
//   record that would enter it at PROPOSED. Issue, acknowledge, and tracking
//   are application-layer concerns with their own gates and adapters; the
//   entire 0.x line is simulation-only and nothing here authorises field
//   control or device writes.
//
// Two §14.3 invariants are **documented as caller-side duties** because a
// single constructor cannot observe them:
//
// - "unique command ID": the constructor rejects the nil UUID, but global
//   uniqueness (including the §14.3 duplicate-with-conflicting-payload
//   rejection) requires the command registry/persistence layer and is
//   enforced there.
// - "monotonic sequence/version per asset or control channel": the
//   constructor requires a strictly positive sequence, but ordering across
//   successive commands (out-of-sequence rejection) requires the per-asset /
//   per-channel registry and is enforced there.
//
// Field groups (one §14.3 bullet per group; tests exist per group):
//
// | §14.3 bullet | Representation |
// |---|---|
// | unique command ID | `CommandId` (non-nil; uniqueness is caller-side) |
// | tenant/portfolio/site/asset IDs | `CommandScope`, typed IDs, non-nil |
// | source subsystem + plugin/model version | `SourceOrigin` |
// | source schedule ID + revision | `ScheduleReference` (revision ≥ 1) |
// | reason/action + actor/service identity | `CommandIntent` |
// | target P/Q or supported control parameter | `ControlTarget` (contracts unit types) |
// | units and sign convention | typed units carry the unit; `DeclaredSignConventions` carries the sign (P0.07: P > 0 = export frozen; Q convention declared, never assumed) |
// | valid-from / valid-until | `flexcore_contracts::ValidityWindow` (validated type) |
// | duration and ramp | `ExecutionRequirements` (non-zero duration; finite positive ramp rates) |
// | monotonic sequence/version | `sequence: u64` ≥ 1 (ordering caller-side) |
// | idempotency key | non-empty `String` |
// | grid model + telemetry snapshot refs | `EvidenceReferences` |
// | validation evidence ref | `EvidenceReferences::validation_evidence` |
// | authorization decision ref | `AuthorizationReference` (scope, limits, expiry) |
// | signature or authenticated transport identity | `CommandAuthenticity` |
// | supersedes / superseded-by | `Option<Supersession>` |
//
// Scope and control status: vocabulary and invariants for the design
// baseline only. FlexCore is a supervisory orchestration layer and performs
// no primary frequency response, synthetic inertia, relay protection,
// inverter inner-loop control, or sub-cycle control (report §14.5).

use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::ValidityWindow;
use flexcore_contracts::units::{
    ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};

use crate::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};

/// Errors returned by [`DispatchCommand::try_new`], one variant per §14.3
/// field-group rule (plus `EmptyField`/`NilIdentifier` for the plain
/// string/identifier fields of a group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchCommandError {
    /// A required identifier was the nil UUID.
    NilIdentifier {
        /// Which identifier was nil.
        field: &'static str,
    },
    /// A required descriptive/reference field was empty or all whitespace.
    EmptyField {
        /// Which field was empty.
        field: &'static str,
    },
    /// The source schedule revision was zero; revisions start at 1.
    InvalidScheduleRevision,
    /// The command sequence was zero; sequences start at 1 (monotonicity
    /// across commands is a caller-side registry duty — see module header).
    InvalidSequence,
    /// The required execution duration was zero.
    InvalidDuration,
    /// A ramp rate was non-finite, zero, or negative.
    InvalidRampRate,
    /// A reactive-power target value was supplied without its declared sign
    /// convention (P0.07: never a bare Q).
    MissingReactiveConvention,
    /// A reactive sign convention was declared without a reactive target
    /// value.
    ReactiveConventionWithoutValue,
    /// The supported-control-parameter target had an empty name.
    InvalidTargetParameter,
    /// The supersedes/superseded-by group was present but named neither
    /// reference.
    InvalidSupersession,
}

impl std::fmt::Display for DispatchCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchCommandError::NilIdentifier { field } => {
                write!(f, "identifier `{field}` must not be the nil UUID")
            }
            DispatchCommandError::EmptyField { field } => {
                write!(f, "field `{field}` must not be empty")
            }
            DispatchCommandError::InvalidScheduleRevision => {
                write!(f, "schedule revision must be at least 1")
            }
            DispatchCommandError::InvalidSequence => {
                write!(f, "command sequence must be at least 1")
            }
            DispatchCommandError::InvalidDuration => {
                write!(f, "execution duration must be greater than zero")
            }
            DispatchCommandError::InvalidRampRate => {
                write!(f, "ramp rates must be finite and strictly positive")
            }
            DispatchCommandError::MissingReactiveConvention => write!(
                f,
                "a reactive-power target requires its declared sign convention (P0.07)"
            ),
            DispatchCommandError::ReactiveConventionWithoutValue => {
                write!(f, "a reactive sign convention was declared without a reactive target")
            }
            DispatchCommandError::InvalidTargetParameter => {
                write!(f, "supported control parameter name must not be empty")
            }
            DispatchCommandError::InvalidSupersession => {
                write!(f, "supersession must name at least one command reference")
            }
        }
    }
}

impl std::error::Error for DispatchCommandError {}

fn require_non_nil(uuid: FlexUuid, field: &'static str) -> Result<(), DispatchCommandError> {
    if uuid == FlexUuid::NIL {
        Err(DispatchCommandError::NilIdentifier { field })
    } else {
        Ok(())
    }
}

fn require_non_empty(value: &str, field: &'static str) -> Result<(), DispatchCommandError> {
    if value.trim().is_empty() {
        Err(DispatchCommandError::EmptyField { field })
    } else {
        Ok(())
    }
}

// --- Field groups ------------------------------------------------------------

/// The §14.3 tenant/portfolio/site/asset identifier group. All four are
/// required and must be non-nil; the scoping chain itself (which site belongs
/// to which portfolio under which tenant) is owned by the entities module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandScope {
    /// Owning tenant (P0.16 rule 1: tenant scoping on every command).
    pub tenant_id: TenantId,
    /// Owning portfolio.
    pub portfolio_id: PortfolioId,
    /// Target site.
    pub site_id: SiteId,
    /// Target asset.
    pub asset_id: AssetId,
}

/// The §14.3 source group: source subsystem and plugin/model version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceOrigin {
    /// The subsystem that produced the command (optimizer, policy, adapter,
    /// operator console, simulator — §14.2 PROPOSED row).
    pub subsystem: String,
    /// The plugin or model version of that source.
    pub component_version: String,
}

/// The §14.3 source-schedule group: schedule ID and revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleReference {
    /// The schedule the command realises.
    pub schedule_id: ScheduleId,
    /// The schedule revision (≥ 1).
    pub revision: u64,
}

/// The §14.3 intent group: reason/action and actor or service identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandIntent {
    /// Why the command was proposed.
    pub reason: String,
    /// What the command does.
    pub action: String,
    /// The human or service identity on whose behalf the command exists.
    pub actor_or_service: String,
}

/// The active-power sign convention (P0.07, frozen): **P > 0 means
/// export/injection into the grid** at the point of connection. This is the
/// only convention, so the enum has exactly one variant — it exists so the
/// convention is *recorded explicitly* on every command (§14.3) rather than
/// assumed silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivePowerSignConvention {
    /// `P > 0` = export at the point of connection (frozen P0.07 rule).
    ExportPositive,
}

/// The §14.3 "units and sign convention" group for a power-setpoint target.
///
/// Units are carried **structurally** by the contracts unit types
/// (`ActivePowerMW`, `ReactivePowerMVAr`) — a command can never carry a bare
/// `f64`. The active-power sign convention is the frozen `ExportPositive`
/// rule and is recorded explicitly; the reactive convention is **declared**
/// per grid model and device profile (P0.07) and must be present exactly
/// when a reactive target value is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredSignConventions {
    /// The recorded active-power sign convention (always the frozen
    /// export-positive rule).
    pub active: ActivePowerSignConvention,
    /// The declared reactive sign convention, required exactly when a
    /// reactive target value is present.
    pub reactive: Option<ReactiveSignConvention>,
}

/// A supported control parameter value (§14.3 "target P/Q or supported
/// control parameter"). Every variant is a contracts unit type — never a
/// bare `f64` — so the canonical unit travels with the value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlParameterValue {
    /// An active-power parameter in MW (frozen `P > 0 = export` sign rule).
    ActivePower(ActivePowerMW),
    /// A reactive-power parameter in MVAr, with its declared sign
    /// convention (P0.07).
    ReactivePower(ReactivePowerMVAr, ReactiveSignConvention),
    /// A SOC parameter as a fraction in [0,1] (P0.07).
    SocFraction(StateOfCharge),
}

/// The §14.3 target group: either a P/Q power setpoint or a supported
/// control parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlTarget {
    /// A power setpoint: active power in MW (required), optionally reactive
    /// power in MVAr with its declared sign convention.
    PowerSetpoint {
        /// Target active power (sign per the frozen export-positive rule).
        p: ActivePowerMW,
        /// Target reactive power, if the command controls Q.
        q: Option<ReactivePowerMVAr>,
        /// The recorded units/sign conventions for this target.
        conventions: DeclaredSignConventions,
    },
    /// A supported control parameter (asset-capability-specific; P0.10 §5
    /// vocabulary, Phase 3 implementation).
    SupportedParameter {
        /// The parameter name (non-empty).
        name: String,
        /// The typed parameter value (never a bare `f64`).
        value: ControlParameterValue,
    },
}

impl ControlTarget {
    /// Validates the cross-field rules of the target group.
    pub fn validate(&self) -> Result<(), DispatchCommandError> {
        match self {
            ControlTarget::PowerSetpoint { q, conventions, .. } => {
                if q.is_some() && conventions.reactive.is_none() {
                    return Err(DispatchCommandError::MissingReactiveConvention);
                }
                if q.is_none() && conventions.reactive.is_some() {
                    return Err(DispatchCommandError::ReactiveConventionWithoutValue);
                }
                Ok(())
            }
            ControlTarget::SupportedParameter { name, .. } => {
                require_non_empty(name, "parameter_name")?;
                Ok(())
            }
        }
    }
}

/// The §14.3 duration-and-ramp group: the required execution duration and
/// the required ramp limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampLimits {
    /// Maximum upward ramp in MW per second (canonical MW with a derived
    /// per-second rate; strictly positive and finite).
    pub max_up_mw_per_s: f64,
    /// Maximum downward ramp in MW per second (strictly positive and
    /// finite; the sign of the power change itself is expressed by the
    /// target).
    pub max_down_mw_per_s: f64,
}

/// The §14.3 duration and ramp requirements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionRequirements {
    /// How long the command remains in effect once issued (non-zero).
    pub duration: Duration,
    /// The ramp requirements (required by §14.3).
    pub ramp: RampLimits,
}

/// The §14.3 evidence group: grid model reference, telemetry snapshot
/// reference, and validation evidence reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReferences {
    /// The grid model the command was checked against.
    pub grid_model: String,
    /// The telemetry snapshot the command was based on.
    pub telemetry_snapshot: String,
    /// The validation evidence record (validation gate, §14.1).
    pub validation_evidence: String,
}

/// The §14.3 authorization decision reference: decision record, approved
/// scope, approved limits, and expiry (authorization gate, §14.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationReference {
    /// A reference to the authorization decision record.
    pub decision_reference: String,
    /// The approved scope.
    pub approved_scope: String,
    /// The approved limits.
    pub approved_limits: String,
    /// When the authorization expires (validated UTC timestamp type).
    pub expiry: flexcore_contracts::time_model::ValidUntil,
}

/// The §14.3 signature-or-transport group: how the command's origin and
/// integrity are attested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAuthenticity {
    /// A signature over the command (reference/identifier, not the raw
    /// cryptographic material — key handling is an application-layer and
    /// adapter concern).
    Signature(String),
    /// An authenticated transport identity (the attested identity of the
    /// transport channel the command arrived over).
    AuthenticatedTransport(String),
}

/// The §14.3 supersedes/superseded-by group, present when a command replaces
/// another. At least one direction must be named. The lifecycle effect of a
/// replacement (SUPERSEDED, frozen §14.1) is the state machine's concern,
/// not the constructor's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supersession {
    /// The command this command replaces.
    pub supersedes: Option<CommandId>,
    /// The command that replaces this command.
    pub superseded_by: Option<CommandId>,
}

// --- The command aggregate ---------------------------------------------------

/// **DispatchCommand** (P0.10 §1 row 10): the §14.3-validated value of a
/// dispatch command. Construction only via [`DispatchCommand::try_new`];
/// there is no `Default` and no public mutable field.
///
/// This is a **domain type, not a working dispatch path** — see the module
/// header.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchCommand {
    command_id: CommandId,
    scope: CommandScope,
    source: SourceOrigin,
    schedule: ScheduleReference,
    intent: CommandIntent,
    target: ControlTarget,
    validity: ValidityWindow,
    execution: ExecutionRequirements,
    sequence: u64,
    idempotency_key: String,
    evidence: EvidenceReferences,
    authorization: AuthorizationReference,
    authenticity: CommandAuthenticity,
    supersedes: Option<Supersession>,
}

impl DispatchCommand {
    /// Validates every §14.3 field group and constructs the command.
    ///
    /// Validation order follows the §14.3 bullet order, so for a command
    /// with several invalid groups the first (in §14.3 order) is reported.
    /// See the module header for the two invariants that are caller-side
    /// duties (command-ID uniqueness, per-asset/channel sequence
    /// monotonicity) and for the "domain type, not a dispatch path" scope.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        command_id: CommandId,
        scope: CommandScope,
        source: SourceOrigin,
        schedule: ScheduleReference,
        intent: CommandIntent,
        target: ControlTarget,
        validity: ValidityWindow,
        execution: ExecutionRequirements,
        sequence: u64,
        idempotency_key: String,
        evidence: EvidenceReferences,
        authorization: AuthorizationReference,
        authenticity: CommandAuthenticity,
        supersedes: Option<Supersession>,
    ) -> Result<Self, DispatchCommandError> {
        // 1. Unique command ID (uniqueness itself is caller-side).
        require_non_nil(command_id.uuid(), "command_id")?;
        // 2. Tenant, portfolio, site and asset IDs.
        require_non_nil(scope.tenant_id.uuid(), "tenant_id")?;
        require_non_nil(scope.portfolio_id.uuid(), "portfolio_id")?;
        require_non_nil(scope.site_id.uuid(), "site_id")?;
        require_non_nil(scope.asset_id.uuid(), "asset_id")?;
        // 3. Source subsystem and plugin/model version.
        require_non_empty(&source.subsystem, "source_subsystem")?;
        require_non_empty(&source.component_version, "source_component_version")?;
        // 4. Source schedule ID and schedule revision.
        require_non_nil(schedule.schedule_id.uuid(), "schedule_id")?;
        if schedule.revision == 0 {
            return Err(DispatchCommandError::InvalidScheduleRevision);
        }
        // 5. Reason/action and actor or service identity.
        require_non_empty(&intent.reason, "reason")?;
        require_non_empty(&intent.action, "action")?;
        require_non_empty(&intent.actor_or_service, "actor_or_service")?;
        // 6 + 7. Target with units and sign convention.
        target.validate()?;
        // 8. Valid-from/valid-until window: `validity` is a contracts
        //    `ValidityWindow`, a validated type (`valid_from ≤ valid_until`,
        //    UTC) — presence is structural and the rule cannot be violated.
        // 9. Duration and ramp.
        if execution.duration.is_zero() {
            return Err(DispatchCommandError::InvalidDuration);
        }
        let ramp = &execution.ramp;
        if !ramp.max_up_mw_per_s.is_finite()
            || !ramp.max_down_mw_per_s.is_finite()
            || ramp.max_up_mw_per_s <= 0.0
            || ramp.max_down_mw_per_s <= 0.0
        {
            return Err(DispatchCommandError::InvalidRampRate);
        }
        // 10. Monotonic sequence/version (ordering itself is caller-side).
        if sequence == 0 {
            return Err(DispatchCommandError::InvalidSequence);
        }
        // 11. Idempotency key.
        require_non_empty(&idempotency_key, "idempotency_key")?;
        // 12 + 13. Grid model, telemetry snapshot, validation evidence.
        require_non_empty(&evidence.grid_model, "grid_model")?;
        require_non_empty(&evidence.telemetry_snapshot, "telemetry_snapshot")?;
        require_non_empty(&evidence.validation_evidence, "validation_evidence")?;
        // 14. Authorization decision reference (scope, limits, expiry).
        require_non_empty(&authorization.decision_reference, "decision_reference")?;
        require_non_empty(&authorization.approved_scope, "approved_scope")?;
        require_non_empty(&authorization.approved_limits, "approved_limits")?;
        // 15. Signature or authenticated transport identity.
        match &authenticity {
            CommandAuthenticity::Signature(s) => {
                require_non_empty(s, "signature")?;
            }
            CommandAuthenticity::AuthenticatedTransport(s) => {
                require_non_empty(s, "transport_identity")?;
            }
        }
        // 16. Supersedes/superseded-by, when a replacement is declared.
        if let Some(s) = &supersedes {
            if s.supersedes.is_none() && s.superseded_by.is_none() {
                return Err(DispatchCommandError::InvalidSupersession);
            }
            if let Some(id) = &s.supersedes {
                require_non_nil(id.uuid(), "supersedes_command_id")?;
            }
            if let Some(id) = &s.superseded_by {
                require_non_nil(id.uuid(), "superseded_by_command_id")?;
            }
        }

        Ok(Self {
            command_id,
            scope,
            source,
            schedule,
            intent,
            target,
            validity,
            execution,
            sequence,
            idempotency_key,
            evidence,
            authorization,
            authenticity,
            supersedes,
        })
    }

    /// The command identifier.
    pub fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// The tenant/portfolio/site/asset scope.
    pub fn scope(&self) -> &CommandScope {
        &self.scope
    }

    /// The source subsystem and version.
    pub fn source(&self) -> &SourceOrigin {
        &self.source
    }

    /// The source schedule reference and revision.
    pub fn schedule(&self) -> &ScheduleReference {
        &self.schedule
    }

    /// The reason/action/actor intent.
    pub fn intent(&self) -> &CommandIntent {
        &self.intent
    }

    /// The control target.
    pub fn target(&self) -> &ControlTarget {
        &self.target
    }

    /// The valid-from/valid-until window.
    pub fn validity(&self) -> &ValidityWindow {
        &self.validity
    }

    /// The duration and ramp requirements.
    pub fn execution(&self) -> &ExecutionRequirements {
        &self.execution
    }

    /// The per-asset/channel sequence number (≥ 1).
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// The idempotency key.
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// The evidence references.
    pub fn evidence(&self) -> &EvidenceReferences {
        &self.evidence
    }

    /// The authorization decision reference.
    pub fn authorization(&self) -> &AuthorizationReference {
        &self.authorization
    }

    /// The authenticity attestation.
    pub fn authenticity(&self) -> &CommandAuthenticity {
        &self.authenticity
    }

    /// The supersedes/superseded-by group, when this command replaces (or is
    /// replaced by) another.
    pub fn supersedes(&self) -> Option<&Supersession> {
        self.supersedes.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_rng::SplitMix64;
    use flexcore_contracts::time_model::{ValidFrom, ValidUntil};

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
    const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9c";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9d";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9e";
    const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9f";
    const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7aa0";
    const UUID_OTHER: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7aa1";

    fn uuid(text: &str) -> FlexUuid {
        FlexUuid::parse(text).unwrap()
    }

    fn scope() -> CommandScope {
        CommandScope {
            tenant_id: TenantId::new(uuid(UUID_TENANT)),
            portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
            site_id: SiteId::new(uuid(UUID_SITE)),
            asset_id: AssetId::new(uuid(UUID_ASSET)),
        }
    }

    fn source() -> SourceOrigin {
        SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        }
    }

    fn schedule() -> ScheduleReference {
        ScheduleReference {
            schedule_id: ScheduleId::new(uuid(UUID_SCHEDULE)),
            revision: 3,
        }
    }

    fn intent() -> CommandIntent {
        CommandIntent {
            reason: "peak-shaving window 18:00-19:00".to_string(),
            action: "set-bess-power".to_string(),
            actor_or_service: "service:scheduler".to_string(),
        }
    }

    fn target() -> ControlTarget {
        ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(1.5).unwrap(),
            q: Some(ReactivePowerMVAr::new(-0.2).unwrap()),
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: Some(ReactiveSignConvention::InjectionPositive),
            },
        }
    }

    fn validity() -> ValidityWindow {
        ValidityWindow::new(
            ValidFrom::from_unix_millis(10_000).unwrap(),
            ValidUntil::from_unix_millis(70_000).unwrap(),
        )
        .unwrap()
    }

    fn execution() -> ExecutionRequirements {
        ExecutionRequirements {
            duration: Duration::from_secs(600),
            ramp: RampLimits {
                max_up_mw_per_s: 0.5,
                max_down_mw_per_s: 0.5,
            },
        }
    }

    fn evidence() -> EvidenceReferences {
        EvidenceReferences {
            grid_model: "grid-model:rev-42".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "evidence:validation-77".to_string(),
        }
    }

    fn authorization() -> AuthorizationReference {
        AuthorizationReference {
            decision_reference: "authz:decision-9".to_string(),
            approved_scope: "scope:bess-1-power".to_string(),
            approved_limits: "limits:±2.0 MW, Q ±0.5 MVAr".to_string(),
            expiry: ValidUntil::from_unix_millis(100_000).unwrap(),
        }
    }

    fn authenticity() -> CommandAuthenticity {
        CommandAuthenticity::Signature("sig:abc123".to_string())
    }

    fn supersedes() -> Option<Supersession> {
        Some(Supersession {
            supersedes: Some(CommandId::new(uuid(UUID_OTHER))),
            superseded_by: None,
        })
    }

    /// A fully valid argument set for [`DispatchCommand::try_new`].
    fn valid_args()
    -> (
        CommandId,
        CommandScope,
        SourceOrigin,
        ScheduleReference,
        CommandIntent,
        ControlTarget,
        ValidityWindow,
        ExecutionRequirements,
        u64,
        String,
        EvidenceReferences,
        AuthorizationReference,
        CommandAuthenticity,
        Option<Supersession>,
    ) {
        (
            CommandId::new(uuid(UUID_COMMAND)),
            scope(),
            source(),
            schedule(),
            intent(),
            target(),
            validity(),
            execution(),
            17,
            "idem:op-42".to_string(),
            evidence(),
            authorization(),
            authenticity(),
            supersedes(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        command_id: CommandId,
        scope: CommandScope,
        source: SourceOrigin,
        schedule: ScheduleReference,
        intent: CommandIntent,
        target: ControlTarget,
        validity: ValidityWindow,
        execution: ExecutionRequirements,
        sequence: u64,
        idempotency_key: String,
        evidence: EvidenceReferences,
        authorization: AuthorizationReference,
        authenticity: CommandAuthenticity,
        supersedes: Option<Supersession>,
    ) -> Result<DispatchCommand, DispatchCommandError> {
        DispatchCommand::try_new(
            command_id,
            scope,
            source,
            schedule,
            intent,
            target,
            validity,
            execution,
            sequence,
            idempotency_key,
            evidence,
            authorization,
            authenticity,
            supersedes,
        )
    }

    #[test]
    fn a_fully_valid_command_constructs() {
        let cmd = build(valid_args()).unwrap();
        assert_eq!(cmd.command_id().uuid(), uuid(UUID_COMMAND));
        assert_eq!(cmd.scope().tenant_id.uuid(), uuid(UUID_TENANT));
        assert_eq!(cmd.scope().asset_id.uuid(), uuid(UUID_ASSET));
        assert_eq!(cmd.schedule().revision, 3);
        assert_eq!(cmd.sequence(), 17);
        assert_eq!(cmd.idempotency_key(), "idem:op-42");
        assert_eq!(cmd.supersedes().unwrap().supersedes.unwrap().uuid(), uuid(UUID_OTHER));
        // The frozen active-power sign convention is recorded, not assumed.
        match cmd.target() {
            ControlTarget::PowerSetpoint { conventions, .. } => {
                assert_eq!(conventions.active, ActivePowerSignConvention::ExportPositive);
            }
            other => panic!("unexpected target: {other:?}"),
        }
    }

    #[test]
    fn a_valid_command_without_supersession_constructs() {
        // §14.3 requires the supersedes/superseded-by reference only "when a
        // command replaces another".
        let (a, b, c, d, e, f, g, h, i, j, k, l, m, _) = valid_args();
        let cmd = build(a, b, c, d, e, f, g, h, i, j, k, l, m, None).unwrap();
        assert!(cmd.supersedes().is_none());
    }

    #[test]
    fn a_supported_parameter_target_constructs() {
        // §14.3: "target P/Q **or supported control parameter**".
        let (a, b, c, d, _, _, g, h, i, j, k, l, m, n) = valid_args();
        let param = ControlTarget::SupportedParameter {
            name: "soc-target".to_string(),
            value: ControlParameterValue::SocFraction(StateOfCharge::new(0.8).unwrap()),
        };
        let cmd = build(a, b, c, d, intent(), param, g, h, i, j, k, l, m, n).unwrap();
        match cmd.target() {
            ControlTarget::SupportedParameter { name, .. } => {
                assert_eq!(name, "soc-target");
            }
            other => panic!("unexpected target: {other:?}"),
        }
    }

    // --- Per-field-group rejection tests (one per §14.3 bullet) -------------

    #[test]
    fn group_1_command_id_must_not_be_nil() {
        let (_, b, c, d, e, f, g, h, i, j, k, l, m, n) = valid_args();
        let err = build(
            CommandId::new(FlexUuid::NIL),
            b, c, d, e, f, g, h, i, j, k, l, m, n,
        )
        .unwrap_err();
        assert_eq!(err, DispatchCommandError::NilIdentifier { field: "command_id" });
    }

    #[test]
    fn group_2_every_scope_id_is_required_and_non_nil() {
        for (label, corrupt) in [
            ("tenant_id", 0usize),
            ("portfolio_id", 1),
            ("site_id", 2),
            ("asset_id", 3),
        ] {
            let (a, mut b, c, d, e, f, g, h, i, j, k, l, m, n) = valid_args();
            match corrupt {
                0 => b.tenant_id = TenantId::new(FlexUuid::NIL),
                1 => b.portfolio_id = PortfolioId::new(FlexUuid::NIL),
                2 => b.site_id = SiteId::new(FlexUuid::NIL),
                _ => b.asset_id = AssetId::new(FlexUuid::NIL),
            }
            let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
            assert_eq!(err, DispatchCommandError::NilIdentifier { field: label });
        }
    }

    #[test]
    fn group_3_source_subsystem_and_version_are_required() {
        let (a, b, mut c, d, e, f, g, h, i, j, k, l, m, n) = valid_args();
        c.subsystem = "  ".to_string();
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::EmptyField { field: "source_subsystem" });

        let (a, b, mut c, d, e, f, g, h, i, j, k, l, m, n) = valid_args();
        c.component_version = "".to_string();
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(
            err,
            DispatchCommandError::EmptyField { field: "source_component_version" }
        );
    }

    #[test]
    fn group_4_schedule_id_required_and_revision_at_least_one() {
        let (a, b, c, mut d, e, f, g, h, i, j, k, l, m, n) = valid_args();
        d.schedule_id = ScheduleId::new(FlexUuid::NIL);
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::NilIdentifier { field: "schedule_id" });

        let (a, b, c, mut d, e, f, g, h, i, j, k, l, m, n) = valid_args();
        d.revision = 0;
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::InvalidScheduleRevision);
    }

    #[test]
    fn group_5_reason_action_and_actor_are_required() {
        for field in ["reason", "action", "actor_or_service"] {
            let (a, b, c, d, mut e, f, g, h, i, j, k, l, m, n) = valid_args();
            match field {
                "reason" => e.reason = "".to_string(),
                "action" => e.action = "  ".to_string(),
                _ => e.actor_or_service = "".to_string(),
            }
            let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
            assert_eq!(err, DispatchCommandError::EmptyField { field });
        }
    }

    #[test]
    fn group_6_reactive_target_requires_its_declared_convention() {
        // Q without the declared convention (P0.07: never a bare Q).
        let (a, b, c, d, e, mut f, g, h, i, j, k, l, m, n) = valid_args();
        if let ControlTarget::PowerSetpoint { conventions, .. } = &mut f {
            conventions.reactive = None;
        }
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::MissingReactiveConvention);

        // Convention without a Q value.
        let (a, b, c, d, e, mut f, g, h, i, j, k, l, m, n) = valid_args();
        if let ControlTarget::PowerSetpoint { q, .. } = &mut f {
            *q = None;
        }
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::ReactiveConventionWithoutValue);

        // Empty parameter name on the supported-parameter branch.
        let (a, b, c, d, e, _, g, h, i, j, k, l, m, n) = valid_args();
        let param = ControlTarget::SupportedParameter {
            name: "".to_string(),
            value: ControlParameterValue::ActivePower(ActivePowerMW::new(1.0).unwrap()),
        };
        let err = build(a, b, c, d, e, param, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::InvalidTargetParameter);
    }

    #[test]
    fn group_7_units_and_sign_convention_are_structural_not_bare_f64() {
        // The "units and sign convention" invariant is enforced by the type
        // system: targets carry `ActivePowerMW` / `ReactivePowerMVAr` / a
        // declared `ReactiveSignConvention` — there is no bare-`f64`
        // constructor at all, so a unit-less target cannot even be
        // expressed. The recorded convention is asserted on a valid command
        // in `a_fully_valid_command_constructs`; the paired
        // value/convention rules are tested in
        // `group_6_reactive_target_requires_its_declared_convention`.
    }

    #[test]
    fn group_8_validity_window_is_a_validated_contracts_type() {
        // The window is a `flexcore_contracts::ValidityWindow`: `valid_from ≤
        // valid_until` and UTC are enforced by that type at its own
        // construction, so an inverted window cannot reach this constructor.
        // The parameter is mandatory (no Option), so absence is a compile
        // error. This test pins that a valid window is carried through.
        let (a, b, c, d, e, f, _, h, i, j, k, l, m, n) = valid_args();
        let cmd = build(a, b, c, d, e, f, validity(), h, i, j, k, l, m, n).unwrap();
        assert_eq!(cmd.validity().valid_from.unix_millis(), 10_000);
        assert_eq!(cmd.validity().valid_until.unix_millis(), 70_000);
    }

    #[test]
    fn group_9_duration_and_ramp_are_required_and_validated() {
        let (a, b, c, d, e, f, g, mut h, i, j, k, l, m, n) = valid_args();
        h.duration = Duration::ZERO;
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::InvalidDuration);

        for (up, down) in [(0.0, 1.0), (1.0, 0.0), (-1.0, 1.0), (f64::NAN, 1.0), (1.0, f64::INFINITY)]
        {
            let (a, b, c, d, e, f, g, mut h, i, j, k, l, m, n) = valid_args();
            h.ramp = RampLimits {
                max_up_mw_per_s: up,
                max_down_mw_per_s: down,
            };
            let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
            assert_eq!(err, DispatchCommandError::InvalidRampRate, "up={up} down={down}");
        }
    }

    #[test]
    fn group_10_sequence_must_be_positive() {
        let (a, b, c, d, e, f, g, h, _, j, k, l, m, n) = valid_args();
        let err = build(a, b, c, d, e, f, g, h, 0, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::InvalidSequence);
    }

    #[test]
    fn group_11_idempotency_key_is_required() {
        let (a, b, c, d, e, f, g, h, i, mut j, k, l, m, n) = valid_args();
        j = "  ".to_string();
        let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
        assert_eq!(err, DispatchCommandError::EmptyField { field: "idempotency_key" });
    }

    #[test]
    fn group_12_13_evidence_references_are_required() {
        for field in ["grid_model", "telemetry_snapshot", "validation_evidence"] {
            let (a, b, c, d, e, f, g, h, i, j, mut k, l, m, n) = valid_args();
            match field {
                "grid_model" => k.grid_model = "".to_string(),
                "telemetry_snapshot" => k.telemetry_snapshot = " ".to_string(),
                _ => k.validation_evidence = "".to_string(),
            }
            let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
            assert_eq!(err, DispatchCommandError::EmptyField { field });
        }
    }

    #[test]
    fn group_14_authorization_reference_fields_are_required() {
        for field in ["decision_reference", "approved_scope", "approved_limits"] {
            let (a, b, c, d, e, f, g, h, i, j, k, mut l, m, n) = valid_args();
            match field {
                "decision_reference" => l.decision_reference = "".to_string(),
                "approved_scope" => l.approved_scope = " ".to_string(),
                _ => l.approved_limits = "".to_string(),
            }
            let err = build(a, b, c, d, e, f, g, h, i, j, k, l, m, n).unwrap_err();
            assert_eq!(err, DispatchCommandError::EmptyField { field });
        }
    }

    #[test]
    fn group_15_signature_or_transport_identity_is_required() {
        let (a, b, c, d, e, f, g, h, i, j, k, l, _, n) = valid_args();
        let err = build(
            a, b, c, d, e, f, g, h, i, j, k, l,
            CommandAuthenticity::Signature("".to_string()),
            n,
        )
        .unwrap_err();
        assert_eq!(err, DispatchCommandError::EmptyField { field: "signature" });

        let (a, b, c, d, e, f, g, h, i, j, k, l, _, n) = valid_args();
        let err = build(
            a, b, c, d, e, f, g, h, i, j, k, l,
            CommandAuthenticity::AuthenticatedTransport("  ".to_string()),
            n,
        )
        .unwrap_err();
        assert_eq!(err, DispatchCommandError::EmptyField { field: "transport_identity" });
    }

    #[test]
    fn group_16_supersession_must_name_at_least_one_reference() {
        let (a, b, c, d, e, f, g, h, i, j, k, l, m, _) = valid_args();
        let err = build(
            a, b, c, d, e, f, g, h, i, j, k, l, m,
            Some(Supersession { supersedes: None, superseded_by: None }),
        )
        .unwrap_err();
        assert_eq!(err, DispatchCommandError::InvalidSupersession);

        let (a, b, c, d, e, f, g, h, i, j, k, l, m, _) = valid_args();
        let err = build(
            a, b, c, d, e, f, g, h, i, j, k, l, m,
            Some(Supersession {
                supersedes: Some(CommandId::new(FlexUuid::NIL)),
                superseded_by: None,
            }),
        )
        .unwrap_err();
        assert_eq!(
            err,
            DispatchCommandError::NilIdentifier { field: "supersedes_command_id" }
        );
    }

    // --- Property test (deterministic splitmix64 generator) -----------------

    /// One corruption per §14.3 field group plus `None` (valid). The property
    /// asserts `try_new` accepts **exactly** the valid combinations: a
    /// corrupted group yields its specific error, an uncorrupted set yields
    /// `Ok`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Corruption {
        None,
        NilCommandId,
        NilScopeTenant,
        EmptySourceSubsystem,
        ZeroScheduleRevision,
        EmptyReason,
        QWithoutConvention,
        ConventionWithoutQ,
        EmptyParameterName,
        ZeroDuration,
        NonFiniteRamp,
        ZeroSequence,
        EmptyIdempotencyKey,
        EmptyGridModel,
        EmptyDecisionReference,
        EmptySignature,
        EmptySupersession,
        NilSupersedesId,
    }

    const CORRUPTIONS: [Corruption; 18] = [
        Corruption::None,
        Corruption::NilCommandId,
        Corruption::NilScopeTenant,
        Corruption::EmptySourceSubsystem,
        Corruption::ZeroScheduleRevision,
        Corruption::EmptyReason,
        Corruption::QWithoutConvention,
        Corruption::ConventionWithoutQ,
        Corruption::EmptyParameterName,
        Corruption::ZeroDuration,
        Corruption::NonFiniteRamp,
        Corruption::ZeroSequence,
        Corruption::EmptyIdempotencyKey,
        Corruption::EmptyGridModel,
        Corruption::EmptyDecisionReference,
        Corruption::EmptySignature,
        Corruption::EmptySupersession,
        Corruption::NilSupersedesId,
    ];

    fn expected_error(c: Corruption) -> Option<DispatchCommandError> {
        Some(match c {
            Corruption::None => return None,
            Corruption::NilCommandId => DispatchCommandError::NilIdentifier { field: "command_id" },
            Corruption::NilScopeTenant => {
                DispatchCommandError::NilIdentifier { field: "tenant_id" }
            }
            Corruption::EmptySourceSubsystem => {
                DispatchCommandError::EmptyField { field: "source_subsystem" }
            }
            Corruption::ZeroScheduleRevision => DispatchCommandError::InvalidScheduleRevision,
            Corruption::EmptyReason => DispatchCommandError::EmptyField { field: "reason" },
            Corruption::QWithoutConvention => DispatchCommandError::MissingReactiveConvention,
            Corruption::ConventionWithoutQ => DispatchCommandError::ReactiveConventionWithoutValue,
            Corruption::EmptyParameterName => DispatchCommandError::InvalidTargetParameter,
            Corruption::ZeroDuration => DispatchCommandError::InvalidDuration,
            Corruption::NonFiniteRamp => DispatchCommandError::InvalidRampRate,
            Corruption::ZeroSequence => DispatchCommandError::InvalidSequence,
            Corruption::EmptyIdempotencyKey => {
                DispatchCommandError::EmptyField { field: "idempotency_key" }
            }
            Corruption::EmptyGridModel => DispatchCommandError::EmptyField { field: "grid_model" },
            Corruption::EmptyDecisionReference => {
                DispatchCommandError::EmptyField { field: "decision_reference" }
            }
            Corruption::EmptySignature => DispatchCommandError::EmptyField { field: "signature" },
            Corruption::EmptySupersession => DispatchCommandError::InvalidSupersession,
            Corruption::NilSupersedesId => {
                DispatchCommandError::NilIdentifier { field: "supersedes_command_id" }
            }
        })
    }

    fn build_corrupted(c: Corruption) -> Result<DispatchCommand, DispatchCommandError> {
        let (
            mut a,
            mut b,
            mut src,
            mut sch,
            mut int,
            mut tgt,
            val,
            mut exe,
            mut seq,
            mut idem,
            mut ev,
            mut auth,
            mut atm,
            mut sup,
        ) = valid_args();
        match c {
            Corruption::None => {}
            Corruption::NilCommandId => a = CommandId::new(FlexUuid::NIL),
            Corruption::NilScopeTenant => b.tenant_id = TenantId::new(FlexUuid::NIL),
            Corruption::EmptySourceSubsystem => src.subsystem = "".to_string(),
            Corruption::ZeroScheduleRevision => sch.revision = 0,
            Corruption::EmptyReason => int.reason = "".to_string(),
            Corruption::QWithoutConvention => {
                if let ControlTarget::PowerSetpoint { conventions, .. } = &mut tgt {
                    conventions.reactive = None;
                }
            }
            Corruption::ConventionWithoutQ => {
                if let ControlTarget::PowerSetpoint { q, .. } = &mut tgt {
                    *q = None;
                }
            }
            Corruption::EmptyParameterName => {
                tgt = ControlTarget::SupportedParameter {
                    name: "".to_string(),
                    value: ControlParameterValue::ActivePower(ActivePowerMW::new(1.0).unwrap()),
                };
            }
            Corruption::ZeroDuration => exe.duration = Duration::ZERO,
            Corruption::NonFiniteRamp => {
                exe.ramp = RampLimits {
                    max_up_mw_per_s: f64::NAN,
                    max_down_mw_per_s: 1.0,
                };
            }
            Corruption::ZeroSequence => seq = 0,
            Corruption::EmptyIdempotencyKey => idem = "".to_string(),
            Corruption::EmptyGridModel => ev.grid_model = "".to_string(),
            Corruption::EmptyDecisionReference => auth.decision_reference = "".to_string(),
            Corruption::EmptySignature => atm = CommandAuthenticity::Signature("".to_string()),
            Corruption::EmptySupersession => {
                sup = Some(Supersession { supersedes: None, superseded_by: None })
            }
            Corruption::NilSupersedesId => {
                sup = Some(Supersession {
                    supersedes: Some(CommandId::new(FlexUuid::NIL)),
                    superseded_by: None,
                })
            }
        }
        build(a, b, src, sch, int, tgt, val, exe, seq, idem, ev, auth, atm, sup)
    }

    #[test]
    fn property_try_new_rejects_exactly_the_invalid_combinations() {
        // Deterministic: fixed seeds, fixed iteration count. Three seeds to
        // cover different corruption orderings; every draw applies exactly
        // one corruption (or none) and the expected outcome is computed from
        // that corruption alone.
        for seed in [0x5EED_0001u64, 0xC0FF_EE02, 0x1234_5678] {
            let mut rng = SplitMix64::new(seed);
            for _ in 0..512 {
                let c = *rng.pick(&CORRUPTIONS);
                let result = build_corrupted(c);
                match expected_error(c) {
                    None => assert!(
                        result.is_ok(),
                        "seed {seed:#x}: valid combination rejected: {:?}",
                        result.err()
                    ),
                    Some(expected) => assert_eq!(
                        result.err(),
                        Some(expected),
                        "seed {seed:#x}: corruption {c:?} must yield its specific error"
                    ),
                }
            }
        }
    }

    #[test]
    fn property_generated_power_setpoints_always_carry_their_convention() {
        // Over generated (p, q, convention) combinations: whenever a
        // reactive value is present the declared convention must be present
        // and vice versa — the constructor accepts only the paired forms.
        let mut rng = SplitMix64::new(0xAB_CD_1234);
        for _ in 0..256 {
            let has_q = rng.below(2) == 0;
            let p = ActivePowerMW::new(rng.f64_in(-2.0, 2.0)).unwrap();
            let q = if has_q {
                Some(ReactivePowerMVAr::new(rng.f64_in(-0.5, 0.5)).unwrap())
            } else {
                None
            };
            let conventions = DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: if has_q && rng.below(2) == 0 {
                    Some(ReactiveSignConvention::InjectionPositive)
                } else if has_q {
                    Some(ReactiveSignConvention::ConsumptionPositive)
                } else {
                    None
                },
            };
            // When q is present, the constructed target always carries a
            // convention (the code above pairs them); when q is absent, no
            // convention. Verify by validation:
            let t = ControlTarget::PowerSetpoint { p, q, conventions };
            let paired = q.is_some() == conventions.reactive.is_some();
            assert_eq!(t.validate().is_ok(), paired);
        }
    }
}
