// P4.17 / P4.18 — Asset-specific fallback policies; bounded hold, escalation,
//         and recovery (report §14.4, §14.1–§14.2, §17.5; ledger P4.17–P4.18;
//         decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/17_19_fallback_timing.md`).
//
// THE GOVERNING RULE (§14.4, quoted verbatim in
// [`NO_UNIVERSAL_FALLBACK_STATEMENT`]): **there is no universal "set power to
// zero" fallback.** Fallback policy is configured PER ASSET/SITE and must be
// reviewed with the equipment/site operator — in this simulation-only 0.x
// line, that review is recorded as a `SIMULATED-OPERATOR` review by the
// power-systems engineer ([`SimulatedOperatorReview`]), explicitly labelled
// as such and NEVER a real operator sign-off.
//
// What this module implements:
//
// - **P4.17 — policy configuration.** The exact §14.4 variant set
//   ([`FallbackPolicyVariant`]): hold the last accepted setpoint for a
//   BOUNDED interval where approved; return to local EMS autonomous control;
//   preserve reserve/SOC constraints; stop accepting new external commands;
//   request a safe local operating mode; require operator intervention. A
//   policy is a NAMED set of these actions for an asset or a site
//   ([`FallbackPolicy`]); every asset/site resolves to either a named policy
//   or the EXPLICIT default entry "no fallback configured — require operator
//   intervention" ([`DEFAULT_ENTRY_STATEMENT`], [`FallbackPolicyRegistry::
//   entry_for`]) — the safe default is intervention, never silence.
// - **The validity-conditions rule (§14.4).** A last-known setpoint is NOT
//   automatically safe when topology, asset state, or network constraints
//   have changed. Every hold policy MUST state its validity conditions
//   ([`HoldValidityConditions`]); a hold policy with empty validity
//   conditions is a CONSTRUCTION ERROR
//   ([`FallbackPolicyError::HoldPolicyRequiresValidityConditions`]) — the
//   validity conditions are part of the policy record (the recorded risk
//   mitigation of P4.17).
// - **FALLBACK entry from FAILED and EXPIRED where the policy requires it**
//   (§14.1 frozen edges 26–27, guarded by the P4.10 domain engine — the
//   frozen-edge oracle; this module never bypasses `try_transition`): the
//   activation record carries WHICH policy fired, its parameters, and the
//   bounded interval where applicable ([`FallbackActivationRecord`]).
// - **P4.18 — the bounded hold.** A hold expires after its configured
//   interval and ESCALATES per policy ([`EscalationTarget`]: to
//   require-operator-intervention or to return-to-local-EMS-autonomous
//   control) — it NEVER holds indefinitely. The bound is enforced in code,
//   not by operator memory: the interval is a finite millisecond count
//   ([`BoundedHoldInterval`]) and every evaluation at or after the expiry
//   instant escalates ([`FallbackEngine::evaluate_hold_expiration`]).
//   **Escalation is NOT a command transition:** FALLBACK is terminal in the
//   frozen §14.1 table (the freeze draws no outgoing edge from it,
//   D-P4.10-6), so an escalation is an operational/audit record on the
//   activation ([`FallbackEscalationRecord`]) — never a state-machine edge.
// - **Recovery (P4.18 step 2).** After FALLBACK, recovery for the asset
//   requires a NEW command through the FULL PROPOSED → VALIDATED → FEASIBLE
//   → APPROVED path ([`RECOVERY_RULE_STATEMENT`]). No shortcut from FALLBACK
//   back to ISSUED exists — the frozen table gives FALLBACK no outgoing
//   edge, so a direct FALLBACK → ISSUED attempt is rejected by the domain
//   engine (`NotPermitted`); this module deliberately provides NO recovery
//   shortcut function.
// - **Audit trail (P4.18 step 4).** Every activation (and every escalation)
//   is recorded with policy, parameters, and outcome through the
//   [`FallbackAuditSink`] seam — the records the P4.30 audit wiring consumes.
//
// ## The seam statement (why the traits are injected)
//
// flexcore-application's dependency set (tools/check_dependency_rule.py)
// admits flexcore-contracts, flexcore-domain, flexcore-ports, and
// flexcore-events only. The collaborators therefore arrive as injected
// seams, implemented by the composition root:
//
// - [`FallbackAuditSink`] — over the Phase 4 command/audit persistence
//   (activation and escalation rows; the composition root implements it
//   over flexcore-storage; the P4.30 audit wiring consumes the same rows);
// - the command store is the P4.13 [`crate::command_gates::
//   CommandGateStore`] seam, REUSED (never restated) — the composition-root
//   implementation already delegates to the P4.10
//   `CommandRecordStore`; the journal replay is the P4.14
//   `replay_full_lifecycle` generalization, REUSED (never restated) — this
//   module never re-encodes a frozen edge;
// - the outbox, encoder, identifier generator, and controlled clock are the
//   same injected ports the P4.11–P4.13 gates use.
//
// Every seam error is fail-closed: an audit or store failure is an error,
// never a silently skipped record.
//
// ## Event vocabulary (recorded payload-fit gap)
//
// `DispatchFallbackActivated` (§15.3) is emitted through the transactional
// outbox on every FALLBACK entry. The frozen payload (flexcore-events) is a
// `Copy` struct carrying identity + time ONLY — it has no field for the
// policy name, parameters, or bounded interval. Those travel in (a) the
// transition journal's evidence reference (`fallback-policy:<name>`) and
// (b) the audit activation record. The gap is recorded in
// [`DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE`]; the frozen payload shape is NOT
// changed (P0.14).
//
// ## Control status
//
// The entire 0.x line is simulation-only (§21.4): a fallback activation
// here is a supervisory lifecycle fact about a SIMULATED command — it is
// not a device write, a protection action, or a safety function. Site-local
// safety and protection remain independent of FlexCore (§17.5 invariant
// 10). FlexCore performs no primary frequency response, synthetic inertia,
// relay protection, inverter inner-loop control, or sub-cycle control
// (§14.5).

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::command_lifecycle::{
    Actor, CommandLifecycle, CommandTransition, EvidenceReference, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{DispatchFallbackActivatedPayload, DomainEvent};
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::state_change::StateChangeReceipt;

use crate::command_gates::{CommandGateStore, CommandGateStoreError};
use crate::command_record::{replay_full_lifecycle, CommandRecordError};
use crate::error::ApplicationError;
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;

// --- Recorded constants ---------------------------------------------------------

/// The §14.4 governing rule, quoted with its source. A structural test
/// (tests/fallback_policy.rs) scans this crate's source for a universal
/// default and asserts this statement exists.
pub const NO_UNIVERSAL_FALLBACK_STATEMENT: &str = "There is no universal \"set power to zero\" \
fallback (report §14.4). Fallback policy is configured per asset/site and must be reviewed with \
the equipment/site operator (in simulation, recorded as a SIMULATED-OPERATOR review). The safe \
default for an asset/site with no named policy is the explicit require-operator-intervention \
entry — never silence, never an automatic setpoint change.";

/// The explicit default entry text (P4.17 acceptance criterion: every
/// asset/site has a named policy OR this explicit entry; the default is
/// intervention, never silence).
pub const DEFAULT_ENTRY_STATEMENT: &str =
    "no fallback configured — require operator intervention";

/// The §14.4 validity-conditions rule, quoted with its source. Enforced as a
/// construction error on every hold policy.
pub const VALIDITY_CONDITIONS_RULE_STATEMENT: &str = "A last-known setpoint is not automatically \
safe when topology, asset state, or network constraints have changed (report §14.4). Every \
hold-last-accepted-setpoint policy MUST state its validity conditions; a hold policy with empty \
validity conditions is a construction error, and the conditions are part of the policy record.";

/// The P4.18 recovery rule. There is no shortcut from FALLBACK to ISSUED:
/// the frozen §14.1 table gives FALLBACK no outgoing edge (D-P4.10-6), and
/// this module deliberately provides no recovery shortcut function.
pub const RECOVERY_RULE_STATEMENT: &str = "After FALLBACK, recovery for the asset requires a NEW \
command that passes the FULL PROPOSED → VALIDATED → FEASIBLE → APPROVED gate chain (report \
§14.4, §14.1). No shortcut from FALLBACK back to ISSUED exists; a direct FALLBACK → ISSUED \
attempt is rejected by the frozen-edge oracle (NotPermitted).";

/// The bounded-hold discipline (P4.18 step 1). Enforced in code, not by
/// operator memory (the recorded P4.18 risk mitigation).
pub const BOUNDED_HOLD_STATEMENT: &str = "A hold-last-accepted-setpoint fallback expires after \
its configured interval and escalates per policy (to require-operator-intervention or to \
return-to-local-EMS-autonomous-control). It NEVER holds indefinitely: the interval is a finite \
millisecond count enforced by type, and every evaluation at or after the expiry instant \
escalates. Indefinite holds that mask a failed asset are a recorded P4.18 failure mode.";

/// The escalation-is-not-a-transition note. FALLBACK is terminal in the
/// frozen table; escalation changes the OPERATIONAL posture, recorded in the
/// audit trail — never the command state.
pub const ESCALATION_IS_NOT_A_TRANSITION_NOTE: &str = "An escalation after hold expiry is an \
operational/audit record on the fallback activation, NOT a command transition: FALLBACK is \
terminal in the frozen §14.1 table (no outgoing edge, D-P4.10-6), so no state-machine edge is \
taken or invented.";

/// The simulated-operator review label. In this simulation-only line the
/// §14.4 operator review of a policy is recorded by the power-systems
/// engineer as a SIMULATED review — labelled as such, never a real operator
/// sign-off.
pub const SIMULATED_OPERATOR_REVIEW_LABEL: &str = "SIMULATED-OPERATOR (recorded by the \
power-systems engineer in the simulation-only 0.x line; not a real operator sign-off)";

/// The audit entity name of a fallback activation row (the composition
/// root's storage mapping; the P4.30 audit wiring consumes these rows).
pub const FALLBACK_AUDIT_ENTITY: &str = "dispatch_fallback_activations";

/// The audit entity name of a hold-expiry escalation row.
pub const FALLBACK_ESCALATION_ENTITY: &str = "dispatch_fallback_escalations";

/// The evidence-reference format recorded on the FAILED/EXPIRED → FALLBACK
/// journal entry: `fallback-policy:<name>` (the P4.10 `record_fallback`
/// contract: the evidence identifies the fallback policy that fired).
pub const FALLBACK_POLICY_REF_PREFIX: &str = "fallback-policy:";

/// The actor identity recorded on the FALLBACK transition (the platform
/// service performing the policy activation; §14.3 actor-or-service rule).
pub const FALLBACK_ACTOR_IDENTITY: &str = "service:fallback-orchestrator";

/// The recorded payload-fit gap for `DispatchFallbackActivated` (see the
/// module header's event-vocabulary section).
pub const DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE: &str = "The frozen §15.3 \
DispatchFallbackActivated payload (flexcore-events) carries identity and time only — it has no \
field for the policy name, parameters, or bounded interval (P0.14: the frozen shape is not \
changed). Those facts travel in (a) the transition journal's evidence reference \
(fallback-policy:<name>) and (b) the FallbackActivationRecord audit row. The composition root \
may add a separate, non-frozen detail event later; the frozen event itself is not extended.";

// --- Policy configuration (P4.17 step 1) -----------------------------------------

/// The exact §14.4 fallback-policy variant set. No other variant exists and
/// none may be invented: in particular there is NO "set power to zero"
/// variant (§14.4; [`NO_UNIVERSAL_FALLBACK_STATEMENT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FallbackPolicyVariant {
    /// Hold the last accepted setpoint for a bounded interval, WHERE THIS IS
    /// APPROVED. Requires validity conditions and a bounded interval
    /// (see [`FallbackAction::Hold`]).
    HoldLastAcceptedSetpoint,
    /// Return to local EMS autonomous control.
    ReturnToLocalEmsAutonomousControl,
    /// Preserve reserve/SOC constraints.
    PreserveReserveSocConstraints,
    /// Stop accepting new external commands.
    StopAcceptingNewExternalCommands,
    /// Request a safe local operating mode.
    RequestSafeLocalOperatingMode,
    /// Require operator intervention.
    RequireOperatorIntervention,
}

impl FallbackPolicyVariant {
    /// The recorded §14.4 text (the policy-record parameter form).
    pub fn as_str(self) -> &'static str {
        match self {
            FallbackPolicyVariant::HoldLastAcceptedSetpoint => {
                "hold the last accepted setpoint for a bounded interval where this is approved"
            }
            FallbackPolicyVariant::ReturnToLocalEmsAutonomousControl => {
                "return to local EMS autonomous control"
            }
            FallbackPolicyVariant::PreserveReserveSocConstraints => {
                "preserve reserve/SOC constraints"
            }
            FallbackPolicyVariant::StopAcceptingNewExternalCommands => {
                "stop accepting new external commands"
            }
            FallbackPolicyVariant::RequestSafeLocalOperatingMode => {
                "request a safe local operating mode"
            }
            FallbackPolicyVariant::RequireOperatorIntervention => "require operator intervention",
        }
    }
}

/// The escalation target of an expired bounded hold (P4.18 step 1: escalate
/// "to require-operator-intervention or return-to-local, per policy"). These
/// two are the ONLY escalation targets; anything else would be an invented
/// policy variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscalationTarget {
    /// The hold escalates to require-operator-intervention.
    RequireOperatorIntervention,
    /// The hold escalates to return-to-local-EMS-autonomous-control.
    ReturnToLocalEms,
}

impl EscalationTarget {
    /// The recorded text of the escalated posture.
    pub fn as_str(self) -> &'static str {
        match self {
            EscalationTarget::RequireOperatorIntervention => {
                FallbackPolicyVariant::RequireOperatorIntervention.as_str()
            }
            EscalationTarget::ReturnToLocalEms => {
                FallbackPolicyVariant::ReturnToLocalEmsAutonomousControl.as_str()
            }
        }
    }
}

/// The bounded hold interval of a hold policy (P4.18 step 1). A STRICTLY
/// POSITIVE, finite millisecond count — an indefinite hold is impossible by
/// construction ([`BOUNDED_HOLD_STATEMENT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedHoldInterval {
    millis: i64,
}

impl BoundedHoldInterval {
    /// Builds the interval; zero or negative is rejected (a zero interval
    /// would make the hold meaningless; a negative one is nonsense).
    pub fn from_millis(millis: i64) -> Result<Self, FallbackPolicyError> {
        if millis <= 0 {
            return Err(FallbackPolicyError::InvalidHoldInterval { millis });
        }
        Ok(Self { millis })
    }

    /// The interval in milliseconds.
    pub fn millis(self) -> i64 {
        self.millis
    }
}

/// The §14.4 validity conditions of a hold policy: the statements of WHEN
/// the held last-accepted setpoint is still valid (topology, asset state,
/// network constraints). EMPTY IS A CONSTRUCTION ERROR — a hold without
/// stated validity conditions cannot exist ([`VALIDITY_CONDITIONS_RULE_
/// STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldValidityConditions {
    conditions: Vec<String>,
}

impl HoldValidityConditions {
    /// Builds the conditions from at least one non-empty statement.
    pub fn new(conditions: Vec<String>) -> Result<Self, FallbackPolicyError> {
        if conditions.is_empty() {
            return Err(FallbackPolicyError::HoldPolicyRequiresValidityConditions);
        }
        if conditions.iter().any(|c| c.trim().is_empty()) {
            return Err(FallbackPolicyError::EmptyField {
                field: "validity_condition",
            });
        }
        Ok(Self { conditions })
    }

    /// The stated conditions (never empty — enforced at construction).
    pub fn conditions(&self) -> &[String] {
        &self.conditions
    }

    /// The recorded parameter text: the conditions joined as one string.
    pub fn as_recorded_text(&self) -> String {
        self.conditions.join("; ")
    }
}

/// One action of a named fallback policy. The hold action is the only one
/// with parameters (the bounded interval, the validity conditions, and the
/// escalation target of P4.18); the other five §14.4 variants are declarative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackAction {
    /// Hold the last accepted setpoint: the bounded interval, the REQUIRED
    /// validity conditions (§14.4), and the escalation target taken when the
    /// interval expires (P4.18).
    Hold {
        interval: BoundedHoldInterval,
        validity: HoldValidityConditions,
        escalation: EscalationTarget,
    },
    /// A declarative §14.4 variant.
    Declare(FallbackPolicyVariant),
}

impl FallbackAction {
    /// The recorded parameter text of the action.
    pub fn as_recorded_text(&self) -> String {
        match self {
            FallbackAction::Hold {
                interval,
                validity,
                escalation,
            } => format!(
                "hold last accepted setpoint for {} ms (validity conditions: {}; on expiry: {})",
                interval.millis(),
                validity.as_recorded_text(),
                escalation.as_str()
            ),
            FallbackAction::Declare(variant) => variant.as_str().to_string(),
        }
    }
}

/// The lifecycle trigger a named policy activates FALLBACK on (§14.1 frozen
/// edges 26–27: FAILED → FALLBACK "asset/site policy activated"; EXPIRED →
/// FALLBACK "policy requires").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FallbackTrigger {
    /// The command reached FAILED; the policy activates FALLBACK.
    OnFailure,
    /// The command reached EXPIRED; the policy requires FALLBACK.
    OnExpiry,
}

impl FallbackTrigger {
    /// The recorded text (audit parameter).
    pub fn as_str(self) -> &'static str {
        match self {
            FallbackTrigger::OnFailure => "on FAILED (asset/site policy activated)",
            FallbackTrigger::OnExpiry => "on EXPIRED (fallback policy requires)",
        }
    }
}

/// The scope a policy or entry is configured for (per asset/site, §14.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackScope {
    /// The policy applies to one asset at one site.
    Asset { site: SiteId, asset: AssetId },
    /// The policy applies site-wide (asset-level entries override it).
    Site { site: SiteId },
}

impl FallbackScope {
    /// Whether this scope covers the given site/asset pair. An asset scope
    /// covers only its own asset; a site scope covers every asset at the
    /// site.
    pub fn covers(&self, site: SiteId, asset: AssetId) -> bool {
        match self {
            FallbackScope::Asset {
                site: s,
                asset: a,
            } => *s == site && *a == asset,
            FallbackScope::Site { site: s } => *s == site,
        }
    }

    /// The recorded scope text (audit parameter).
    pub fn as_recorded_text(&self) -> String {
        match self {
            FallbackScope::Asset { site, asset } => {
                format!("asset:{asset} (site {site})")
            }
            FallbackScope::Site { site } => format!("site:{site}"),
        }
    }
}

/// A NAMED per-asset/site fallback policy (§14.4): a non-empty name, a
/// scope, at least one §14.4 action, and the triggers it activates FALLBACK
/// on. A hold action carries its REQUIRED validity conditions; two hold
/// actions in one policy are meaningless and rejected.
///
/// The §14.4 operator review is recorded separately on each activation
/// ([`SimulatedOperatorReview`]), so the policy record itself stays a pure
/// configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackPolicy {
    name: String,
    scope: FallbackScope,
    actions: Vec<FallbackAction>,
    activation_triggers: Vec<FallbackTrigger>,
}

impl FallbackPolicy {
    /// Builds a named policy. Rules (all enforced, all fail-closed):
    ///
    /// 1. the name is non-empty;
    /// 2. the scope is one of the two per-asset/site forms;
    /// 3. at least one §14.4 action is declared;
    /// 4. at most ONE hold action is declared (two bounded holds for one
    ///    scope are a configuration contradiction);
    /// 5. a hold action REQUIRES non-empty validity conditions (§14.4 —
    ///    [`HoldValidityConditions::new`] enforces this at its own
    ///    construction; the policy re-asserts it here so a hand-assembled
    ///    action cannot bypass the rule);
    /// 6. at least one activation trigger is declared (a policy that
    ///    activates on nothing is not a fallback policy).
    pub fn new(
        name: impl Into<String>,
        scope: FallbackScope,
        actions: Vec<FallbackAction>,
        activation_triggers: Vec<FallbackTrigger>,
    ) -> Result<Self, FallbackPolicyError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(FallbackPolicyError::EmptyField { field: "policy_name" });
        }
        if actions.is_empty() {
            return Err(FallbackPolicyError::NoActions {
                policy_name: name.clone(),
            });
        }
        let mut holds = 0usize;
        for action in &actions {
            if let FallbackAction::Hold { validity, .. } = action {
                holds += 1;
                // Rule 5: re-assert the validity-conditions rule at the
                // policy boundary (the type already enforces it; a direct
                // struct move cannot bypass `HoldValidityConditions::new`,
                // but the check documents the invariant here).
                if validity.conditions().is_empty() {
                    return Err(FallbackPolicyError::HoldPolicyRequiresValidityConditions);
                }
            }
        }
        if holds > 1 {
            return Err(FallbackPolicyError::MultipleHoldActions {
                policy_name: name.clone(),
            });
        }
        if activation_triggers.is_empty() {
            return Err(FallbackPolicyError::NoActivationTriggers {
                policy_name: name.clone(),
            });
        }
        Ok(Self {
            name,
            scope,
            actions,
            activation_triggers,
        })
    }

    /// The policy name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The configured scope.
    pub fn scope(&self) -> &FallbackScope {
        &self.scope
    }

    /// The declared §14.4 actions.
    pub fn actions(&self) -> &[FallbackAction] {
        &self.actions
    }

    /// The triggers the policy activates FALLBACK on.
    pub fn activation_triggers(&self) -> &[FallbackTrigger] {
        &self.activation_triggers
    }

    /// The bounded hold of this policy, when it declares one (at most one —
    /// enforced at construction).
    pub fn bounded_hold(&self) -> Option<(&BoundedHoldInterval, &HoldValidityConditions, EscalationTarget)> {
        self.actions.iter().find_map(|action| match action {
            FallbackAction::Hold {
                interval,
                validity,
                escalation,
            } => Some((interval, validity, *escalation)),
            FallbackAction::Declare(_) => None,
        })
    }

    /// Whether the policy declares "stop accepting new external commands"
    /// (§14.4). The command-creation wiring (P4.11 composition) consumes
    /// this — recorded integration point, D-P4.17-6.
    pub fn accepts_external_commands(&self) -> bool {
        !self.actions.iter().any(|action| {
            matches!(
                action,
                FallbackAction::Declare(FallbackPolicyVariant::StopAcceptingNewExternalCommands)
            )
        })
    }

    /// The full recorded parameter text (the audit "parameters" field).
    pub fn parameters_recorded_text(&self) -> String {
        self.actions
            .iter()
            .map(FallbackAction::as_recorded_text)
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// The evidence reference the FALLBACK journal entry carries
    /// (`fallback-policy:<name>`; the P4.10 `record_fallback` contract).
    pub fn policy_reference(&self) -> String {
        format!("{FALLBACK_POLICY_REF_PREFIX}{}", self.name)
    }
}

/// The per-asset/site fallback entry (P4.17 acceptance criterion): either a
/// NAMED policy or the EXPLICIT "no fallback configured — require operator
/// intervention" entry. The default is intervention, never silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPolicyEntry {
    /// A named, configured policy.
    Named(FallbackPolicy),
    /// The explicit default entry — recorded, never silent. `recorded_reason`
    /// states WHY no policy exists (e.g. "no policy registered for this
    /// scope" or a deployment's explicit choice).
    RequireIntervention {
        scope: FallbackScope,
        recorded_reason: String,
    },
}

impl FallbackPolicyEntry {
    /// The explicit default entry for a scope with the standard recorded
    /// reason ([`DEFAULT_ENTRY_STATEMENT`]).
    pub fn default_entry(scope: FallbackScope) -> Self {
        FallbackPolicyEntry::RequireIntervention {
            scope,
            recorded_reason: DEFAULT_ENTRY_STATEMENT.to_string(),
        }
    }

    /// The scope the entry is configured for.
    pub fn scope(&self) -> &FallbackScope {
        match self {
            FallbackPolicyEntry::Named(policy) => policy.scope(),
            FallbackPolicyEntry::RequireIntervention { scope, .. } => scope,
        }
    }

    /// The named policy, when this is one.
    pub fn named_policy(&self) -> Option<&FallbackPolicy> {
        match self {
            FallbackPolicyEntry::Named(policy) => Some(policy),
            FallbackPolicyEntry::RequireIntervention { .. } => None,
        }
    }
}

/// The per-asset/site fallback-policy registry. Asset-level entries override
/// site-level entries; an asset/site with NO matching entry resolves to the
/// EXPLICIT default require-intervention entry (never silence, never an
/// automatic setpoint change — §14.4).
#[derive(Debug, Clone, Default)]
pub struct FallbackPolicyRegistry {
    entries: Vec<FallbackPolicyEntry>,
}

impl FallbackPolicyRegistry {
    /// An empty registry: every asset/site then resolves to the explicit
    /// default entry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one entry; a duplicate scope is a configuration error
    /// (two policies for one scope are a contradiction, not an override).
    pub fn register(&mut self, entry: FallbackPolicyEntry) -> Result<(), FallbackPolicyError> {
        let scope = entry.scope();
        if self
            .entries
            .iter()
            .any(|existing| existing.scope() == scope)
        {
            return Err(FallbackPolicyError::DuplicatePolicyScope {
                scope: scope.as_recorded_text(),
            });
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Registers a named policy (convenience over [`Self::register`]).
    pub fn register_policy(&mut self, policy: FallbackPolicy) -> Result<(), FallbackPolicyError> {
        self.register(FallbackPolicyEntry::Named(policy))
    }

    /// Resolves the entry for a site/asset pair, in override order:
    /// asset-level named entry → site-level named entry → the EXPLICIT
    /// default require-intervention entry (constructed on the fly with the
    /// recorded standard reason — the default is intervention, recorded,
    /// never silence).
    pub fn entry_for(&self, site: SiteId, asset: AssetId) -> FallbackPolicyEntry {
        // Asset-level first (exact asset scope).
        if let Some(entry) = self.entries.iter().find(|entry| {
            matches!(entry.scope(), FallbackScope::Asset { .. })
                && entry.scope().covers(site, asset)
        }) {
            return entry.clone();
        }
        // Then site-level.
        if let Some(entry) = self.entries.iter().find(|entry| {
            matches!(entry.scope(), FallbackScope::Site { .. }) && entry.scope().covers(site, asset)
        }) {
            return entry.clone();
        }
        FallbackPolicyEntry::default_entry(FallbackScope::Asset { site, asset })
    }
}

// --- The simulated-operator review (P4.17 step 1, simulation form) ---------------

/// The recorded §14.4 policy review, in its simulation-only form: the
/// power-systems engineer records the review that the report assigns to the
/// equipment/site operator, labelled `SIMULATED-OPERATOR` — explicitly NOT a
/// real operator sign-off (recorded decision D-P4.17-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedOperatorReview {
    reviewer: String,
    note: String,
    reviewed_at_unix_millis: i64,
}

impl SimulatedOperatorReview {
    /// Builds the review; the reviewer and note are required non-empty.
    pub fn new(
        reviewer: impl Into<String>,
        note: impl Into<String>,
        reviewed_at_unix_millis: i64,
    ) -> Result<Self, FallbackPolicyError> {
        let reviewer = reviewer.into();
        let note = note.into();
        if reviewer.trim().is_empty() {
            return Err(FallbackPolicyError::EmptyField { field: "reviewer" });
        }
        if note.trim().is_empty() {
            return Err(FallbackPolicyError::EmptyField { field: "review_note" });
        }
        Ok(Self {
            reviewer,
            note,
            reviewed_at_unix_millis,
        })
    }

    /// The reviewer identity (e.g. `engineer:power-systems`).
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    /// The review note.
    pub fn note(&self) -> &str {
        &self.note
    }

    /// The review timestamp (controlled-clock millis).
    pub fn reviewed_at_unix_millis(&self) -> i64 {
        self.reviewed_at_unix_millis
    }

    /// The full recorded text: the SIMULATED-OPERATOR label, the reviewer,
    /// and the note — never renderable as a real operator sign-off.
    pub fn as_recorded_text(&self) -> String {
        format!(
            "{SIMULATED_OPERATOR_REVIEW_LABEL}: reviewer={}; note={}",
            self.reviewer, self.note
        )
    }
}

// --- The audit sink seam (P4.18 step 4) --------------------------------------------

/// An opaque audit-backend failure (code + human-safe message, the same
/// discipline as the application error taxonomy's infrastructure class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackAuditError {
    /// The opaque, stable failure code.
    pub code: String,
    /// The human-safe failure summary.
    pub message: String,
}

impl std::fmt::Display for FallbackAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fallback audit backend failure ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for FallbackAuditError {}

/// The audit-trail seam for fallback activations and escalations (P4.18
/// step 4: activations recorded with policy, parameters, and outcome). The
/// composition root implements it over flexcore-storage; the P4.30 audit
/// wiring consumes the same rows (recorded integration point, D-P4.18-4).
/// Fail-closed: an audit failure is an error, never a skipped record.
pub trait FallbackAuditSink {
    /// Records one fallback activation (or non-activation outcome — every
    /// decision is audit-visible).
    fn record_activation(
        &self,
        record: &FallbackActivationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError>;

    /// Records one bounded-hold expiry escalation.
    fn record_escalation(
        &self,
        record: &FallbackEscalationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError>;
}

// --- Activation records -------------------------------------------------------------

/// The audit-trail record of ONE fallback decision for one command (P4.17
/// step 2 + P4.18 step 4): which policy fired (or why none did), its
/// parameters, the bounded interval where applicable, the simulated operator
/// review, and the outcome. The P4.30 audit wiring consumes these records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackActivationRecord {
    /// The command the decision is about.
    pub command_id: CommandId,
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The owning portfolio (the frozen event payload needs it; audit copy).
    pub portfolio_id: PortfolioId,
    /// The target site.
    pub site_id: SiteId,
    /// The target asset.
    pub asset_id: AssetId,
    /// The lifecycle trigger the decision was evaluated for.
    pub trigger: FallbackTrigger,
    /// The policy name, or [`DEFAULT_ENTRY_STATEMENT`] for the default
    /// entry.
    pub policy_name: String,
    /// The policy evidence reference (`fallback-policy:<name>`), or the
    /// default-entry marker for the default entry.
    pub policy_reference: String,
    /// Whether this decision resolved to the explicit default entry.
    pub is_default_entry: bool,
    /// The recorded parameter text of the policy's actions (empty for the
    /// default entry).
    pub parameters: String,
    /// The bounded hold interval in millis, when the policy declares one.
    pub bounded_hold_millis: Option<i64>,
    /// The escalation target taken when the bounded hold expires.
    pub hold_escalation: Option<EscalationTarget>,
    /// The hold validity conditions (§14.4), when the policy declares a
    /// hold.
    pub validity_conditions: Option<Vec<String>>,
    /// The recorded SIMULATED-OPERATOR review, when a named policy fired.
    pub simulated_operator_review: Option<SimulatedOperatorReview>,
    /// The decision timestamp (controlled-clock millis).
    pub decided_at_unix_millis: i64,
    /// The outcome text (audit "outcome" field; P4.30 consumes it).
    pub outcome: String,
}

/// The audit-trail record of ONE bounded-hold expiry escalation (P4.18
/// step 1). This is an operational/audit record — NOT a command transition
/// ([`ESCALATION_IS_NOT_A_TRANSITION_NOTE`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackEscalationRecord {
    /// The command whose fallback activation escalated.
    pub command_id: CommandId,
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The target site.
    pub site_id: SiteId,
    /// The target asset.
    pub asset_id: AssetId,
    /// The policy that fired originally.
    pub policy_name: String,
    /// The escalation target taken (per policy).
    pub escalation_target: EscalationTarget,
    /// The configured bounded interval in millis (expired).
    pub bounded_hold_millis: i64,
    /// The activation instant (controlled-clock millis).
    pub activated_at_unix_millis: i64,
    /// The escalation instant (controlled-clock millis; ≥ activation +
    /// interval).
    pub escalated_at_unix_millis: i64,
    /// The outcome text (audit "outcome" field).
    pub outcome: String,
}

// --- Errors ---------------------------------------------------------------------------

/// The named errors of the fallback-policy module. Fail-closed: a
/// configuration or seam failure is an error, never silently degraded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPolicyError {
    /// A required field (policy name, validity condition, review field) was
    /// empty.
    EmptyField {
        /// Which field was empty.
        field: &'static str,
    },
    /// A bounded-hold interval was zero or negative (an indefinite hold is
    /// impossible by construction; a zero/negative one is invalid).
    InvalidHoldInterval {
        /// The rejected value (millis).
        millis: i64,
    },
    /// A policy declared no §14.4 action.
    NoActions {
        /// The rejected policy's name.
        policy_name: String,
    },
    /// A policy declared more than one hold action.
    MultipleHoldActions {
        /// The rejected policy's name.
        policy_name: String,
    },
    /// A policy declared no activation trigger.
    NoActivationTriggers {
        /// The rejected policy's name.
        policy_name: String,
    },
    /// A hold policy was constructed with empty validity conditions — the
    /// §14.4 validity-conditions rule is a CONSTRUCTION error, not a runtime
    /// check (the recorded P4.17 risk mitigation).
    HoldPolicyRequiresValidityConditions,
    /// Two entries were registered for the same scope.
    DuplicatePolicyScope {
        /// The colliding scope (recorded text).
        scope: String,
    },
    /// The command does not exist in the caller's tenant scope.
    UnknownCommand {
        /// The command ID (recorded text).
        command_id: String,
    },
    /// The command store seam failed.
    Store(CommandGateStoreError),
    /// The audit sink failed (fail-closed — a record is never skipped).
    Audit(FallbackAuditError),
    /// The outbox write failed after the state change was recorded (the
    /// composition root's unit of work resolves the pair).
    Event {
        /// The recorded detail.
        detail: String,
    },
    /// The persisted journal could not be replayed, or the domain engine
    /// rejected a transition while replaying/activating — fail-closed, never
    /// guessed around.
    InconsistentJournal {
        /// The command ID.
        command_id: String,
        /// The recorded detail.
        detail: String,
    },
    /// A timestamp could not be represented (controlled-clock discipline).
    InvalidTimestamp {
        /// The recorded detail.
        detail: String,
    },
}

impl std::fmt::Display for FallbackPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackPolicyError::EmptyField { field } => {
                write!(f, "field `{field}` must not be empty")
            }
            FallbackPolicyError::InvalidHoldInterval { millis } => {
                write!(f, "the bounded hold interval must be positive; got {millis} ms")
            }
            FallbackPolicyError::NoActions { policy_name } => {
                write!(f, "policy '{policy_name}' declares no fallback action")
            }
            FallbackPolicyError::MultipleHoldActions { policy_name } => write!(
                f,
                "policy '{policy_name}' declares more than one hold action; two bounded holds \
                 for one scope are a configuration contradiction"
            ),
            FallbackPolicyError::NoActivationTriggers { policy_name } => {
                write!(f, "policy '{policy_name}' declares no FALLBACK activation trigger")
            }
            FallbackPolicyError::HoldPolicyRequiresValidityConditions => write!(
                f,
                "a hold-last-accepted-setpoint policy must state its validity conditions \
                 (§14.4: a last-known setpoint is not automatically safe when topology, asset \
                 state, or network constraints have changed); empty validity conditions are a \
                 construction error"
            ),
            FallbackPolicyError::DuplicatePolicyScope { scope } => {
                write!(f, "a fallback entry is already registered for scope {scope}")
            }
            FallbackPolicyError::UnknownCommand { command_id } => {
                write!(f, "command {command_id} does not exist in this tenant scope")
            }
            FallbackPolicyError::Store(error) => write!(f, "command store: {error}"),
            FallbackPolicyError::Audit(error) => write!(f, "{error}"),
            FallbackPolicyError::Event { detail } => write!(f, "outbox write failed: {detail}"),
            FallbackPolicyError::InconsistentJournal { command_id, detail } => {
                write!(f, "command {command_id}: inconsistent journal: {detail}")
            }
            FallbackPolicyError::InvalidTimestamp { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for FallbackPolicyError {}

impl From<CommandGateStoreError> for FallbackPolicyError {
    fn from(error: CommandGateStoreError) -> Self {
        match error {
            CommandGateStoreError::NotFound { command_id } => {
                FallbackPolicyError::UnknownCommand { command_id }
            }
            other => FallbackPolicyError::Store(other),
        }
    }
}

impl From<LifecycleError> for FallbackPolicyError {
    fn from(error: LifecycleError) -> Self {
        FallbackPolicyError::InconsistentJournal {
            command_id: String::new(),
            detail: error.to_string(),
        }
    }
}

// --- The fallback engine -------------------------------------------------------------

/// The activation input: the command identity, its §14.3 scope (the
/// composition root derives site/asset/portfolio from the P4.14 command
/// record row — recorded integration point, D-P4.17-5), the trigger being
/// evaluated, and the recorded SIMULATED-OPERATOR review for the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackActivationInput {
    /// The command that reached FAILED or EXPIRED.
    pub command_id: CommandId,
    /// The command's portfolio (§14.3 scope).
    pub portfolio_id: PortfolioId,
    /// The command's site.
    pub site_id: SiteId,
    /// The command's asset.
    pub asset_id: AssetId,
    /// The trigger being evaluated (the state the command is actually in is
    /// verified against the store journal — the input cannot spoof it).
    pub trigger: FallbackTrigger,
    /// The recorded SIMULATED-OPERATOR review of the policy (P4.17 step 1).
    pub simulated_operator_review: SimulatedOperatorReview,
}

/// The outcome of one fallback decision. EVERY variant is audit-visible (the
/// engine writes an activation record in all three cases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackDecision {
    /// The named policy fired: the command moved to FALLBACK through the
    /// frozen edge, the audit record was written, and
    /// `DispatchFallbackActivated` was emitted.
    Activated {
        /// The audit activation record (policy, parameters, bounded
        /// interval, simulated review, outcome).
        record: FallbackActivationRecord,
        /// The journalled FAILED/EXPIRED → FALLBACK transition.
        transition: CommandTransition,
    },
    /// No named policy exists for the scope: the EXPLICIT default entry
    /// applies — operator intervention required, NO automatic fallback (no
    /// state transition, no event; the audit record records the decision).
    InterventionRequired {
        /// The audit activation record (`is_default_entry == true`).
        record: FallbackActivationRecord,
    },
    /// A named policy exists but does NOT require FALLBACK on this trigger
    /// (e.g. a policy that activates on FAILED only, evaluated for EXPIRED):
    /// no state transition, no event; the audit record records the decision.
    TriggerNotConfigured {
        /// The audit activation record.
        record: FallbackActivationRecord,
    },
}

/// The outcome of a bounded-hold expiry evaluation (P4.18 step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldExpiryEvaluation {
    /// The policy declares no bounded hold (nothing to expire; the other
    /// declared actions stand as recorded on the activation).
    NoBoundedHold,
    /// The hold is still inside its configured interval.
    HoldActive {
        /// The expiry instant (activation instant + interval, controlled-
        /// clock millis). At or after this instant the hold escalates.
        expires_at_unix_millis: i64,
    },
    /// The hold expired and ESCALATED per policy. The escalation record was
    /// written through the audit sink (fail-closed). This is NOT a command
    /// transition ([`ESCALATION_IS_NOT_A_TRANSITION_NOTE`]).
    Escalated {
        /// The escalation target (per policy).
        target: EscalationTarget,
        /// The audit escalation record.
        record: FallbackEscalationRecord,
    },
}

impl From<CommandRecordError> for FallbackPolicyError {
    fn from(error: CommandRecordError) -> Self {
        match error {
            CommandRecordError::InconsistentJournal { command_id, detail } => {
                FallbackPolicyError::InconsistentJournal { command_id, detail }
            }
            CommandRecordError::InvalidTimestamp { detail } => {
                FallbackPolicyError::InvalidTimestamp { detail }
            }
            other => FallbackPolicyError::InconsistentJournal {
                command_id: String::new(),
                detail: other.to_string(),
            },
        }
    }
}

/// The fallback engine (P4.17 activation + P4.18 bounded hold). One struct
/// per composition; all collaborators are injected (see the seam statement).
/// Timestamps come EXCLUSIVELY from the supplied controlled clock
/// ([`LifecycleClock`]) — never a wall clock.
pub struct FallbackEngine<'a> {
    /// The per-asset/site policy registry.
    pub policies: &'a FallbackPolicyRegistry,
    /// The command-record store seam (REUSED from the P4.11–P4.13 gates —
    /// the same trait instance the composition root implements over the
    /// P4.10 `CommandRecordStore`).
    pub store: &'a dyn CommandGateStore,
    /// The fallback audit-trail seam (activations + escalations).
    pub audit: &'a dyn FallbackAuditSink,
    /// The transactional outbox (events).
    pub outbox: &'a dyn EventOutbox,
    /// The event-payload encoder seam.
    pub encoder: &'a dyn EventPayloadEncoder,
    /// The identifier generator (event correlation ids).
    pub ids: &'a dyn IdGenerator,
    /// The controlled clock (every activation/escalation timestamp).
    pub clock: &'a dyn LifecycleClock,
}

impl<'a> FallbackEngine<'a> {
    /// Evaluates a FAILED/EXPIRED trigger and, WHERE THE POLICY REQUIRES IT,
    /// enters FALLBACK through the frozen edge (§14.1 edges 26–27, guarded
    /// by the P4.10 domain engine — this module never bypasses
    /// `try_transition`). Steps, in order:
    ///
    /// 1. load the command's journal from the store seam (tenant-scoped; a
    ///    missing command is `UnknownCommand`);
    /// 2. REPLAY the journal through the P4.10 domain engine (the P4.14
    ///    `replay_full_lifecycle` generalization, REUSED — pinned clocks,
    ///    exact persisted timestamps) so the transition applies from the
    ///    TRUE current state — a journal whose last state is not FAILED or
    ///    EXPIRED is an `InconsistentJournal` fail-closed error;
    /// 3. verify the requested trigger matches the actual state (FAILED vs
    ///    EXPIRED — the input cannot spoof it);
    /// 4. resolve the policy entry for the scope: no named policy →
    ///    [`FallbackDecision::InterventionRequired`] (the explicit default —
    ///    no transition, no event);
    /// 5. the policy does not list this trigger →
    ///    [`FallbackDecision::TriggerNotConfigured`] (no transition, no
    ///    event);
    /// 6. otherwise: `record_fallback` with the policy evidence reference,
    ///    persist through the store CAS, write the audit activation record,
    ///    and emit `DispatchFallbackActivated` through the transactional
    ///    outbox (the frozen payload carries identity + time only — the
    ///    recorded payload-fit gap, [`DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE`]).
    pub fn activate_fallback(
        &self,
        ctx: &TenantContext,
        input: FallbackActivationInput,
    ) -> Result<FallbackDecision, FallbackPolicyError> {
        // 1. Load the persisted journal (tenant-scoped).
        let view = self
            .store
            .load_command(ctx, input.command_id)
            .map_err(FallbackPolicyError::from)?
            .ok_or(FallbackPolicyError::UnknownCommand {
                command_id: input.command_id.to_string(),
            })?;

        // 2. Replay to the true current state (frozen-edge conformance: the
        //    domain engine is the only guard of record; the replay is the
        //    P4.14 generalization, REUSED — never restated here).
        let mut lifecycle = replay_full_lifecycle(input.command_id, &view)?;

        // 3. The trigger must match the actual state (FAILED vs EXPIRED).
        let expected_state = match input.trigger {
            FallbackTrigger::OnFailure => CommandState::Failed,
            FallbackTrigger::OnExpiry => CommandState::Expired,
        };
        if lifecycle.state() != expected_state {
            return Err(FallbackPolicyError::InconsistentJournal {
                command_id: input.command_id.to_string(),
                detail: format!(
                    "trigger {} does not match the command's actual state {}",
                    input.trigger.as_str(),
                    lifecycle.state().name()
                ),
            });
        }

        // 4. Resolve the policy entry for the scope.
        let entry = self.policies.entry_for(input.site_id, input.asset_id);
        let Some(policy) = entry.named_policy() else {
            // The explicit default entry: operator intervention required,
            // recorded — never silence, never an automatic setpoint change.
            let record = FallbackActivationRecord {
                command_id: input.command_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id: input.portfolio_id,
                site_id: input.site_id,
                asset_id: input.asset_id,
                trigger: input.trigger,
                policy_name: DEFAULT_ENTRY_STATEMENT.to_string(),
                policy_reference: format!("{FALLBACK_POLICY_REF_PREFIX}default"),
                is_default_entry: true,
                parameters: String::new(),
                bounded_hold_millis: None,
                hold_escalation: None,
                validity_conditions: None,
                simulated_operator_review: Some(input.simulated_operator_review),
                decided_at_unix_millis: self.clock.now_unix_millis(),
                outcome: "no named fallback policy: operator intervention required; no \
                          automatic fallback (the explicit default entry)"
                    .to_string(),
            };
            self.audit.record_activation(&record).map_err(FallbackPolicyError::Audit)?;
            return Ok(FallbackDecision::InterventionRequired { record });
        };

        // 5. The policy must require FALLBACK on this trigger.
        if !policy
            .activation_triggers()
            .iter()
            .any(|&trigger| trigger == input.trigger)
        {
            let record = FallbackActivationRecord {
                command_id: input.command_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id: input.portfolio_id,
                site_id: input.site_id,
                asset_id: input.asset_id,
                trigger: input.trigger,
                policy_name: policy.name().to_string(),
                policy_reference: policy.policy_reference(),
                is_default_entry: false,
                parameters: policy.parameters_recorded_text(),
                bounded_hold_millis: policy.bounded_hold().map(|(interval, _, _)| interval.millis()),
                hold_escalation: policy.bounded_hold().map(|(_, _, escalation)| escalation),
                validity_conditions: policy
                    .bounded_hold()
                    .map(|(_, validity, _)| validity.conditions().to_vec()),
                simulated_operator_review: Some(input.simulated_operator_review),
                decided_at_unix_millis: self.clock.now_unix_millis(),
                outcome: format!(
                    "policy does not require FALLBACK on this trigger; no state transition \
                     (policy triggers: {})",
                    policy
                        .activation_triggers()
                        .iter()
                        .map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            };
            self.audit.record_activation(&record).map_err(FallbackPolicyError::Audit)?;
            return Ok(FallbackDecision::TriggerNotConfigured { record });
        }

        // 6. Activate: the frozen edge, the audit record, the event.
        let policy_evidence =
            EvidenceReference::new(policy.policy_reference()).map_err(|_| {
                FallbackPolicyError::InconsistentJournal {
                    command_id: input.command_id.to_string(),
                    detail: "the assembled fallback policy reference is empty".to_string(),
                }
            })?;
        let actor = Actor::new(FALLBACK_ACTOR_IDENTITY).map_err(|_| {
            FallbackPolicyError::InconsistentJournal {
                command_id: input.command_id.to_string(),
                detail: "the fallback actor identity is empty".to_string(),
            }
        })?;
        let transition = lifecycle
            .record_fallback(policy_evidence, actor, self.clock)
            .map_err(FallbackPolicyError::from)?;
        let receipt = self
            .store
            .journal_transition(ctx, &transition)
            .map_err(FallbackPolicyError::from)?;

        let (bounded_hold_millis, hold_escalation, validity_conditions) =
            match policy.bounded_hold() {
                Some((interval, validity, escalation)) => (
                    Some(interval.millis()),
                    Some(escalation),
                    Some(validity.conditions().to_vec()),
                ),
                None => (None, None, None),
            };
        let record = FallbackActivationRecord {
            command_id: input.command_id,
            tenant_id: ctx.tenant_id(),
            portfolio_id: input.portfolio_id,
            site_id: input.site_id,
            asset_id: input.asset_id,
            trigger: input.trigger,
            policy_name: policy.name().to_string(),
            policy_reference: policy.policy_reference(),
            is_default_entry: false,
            parameters: policy.parameters_recorded_text(),
            bounded_hold_millis,
            hold_escalation,
            validity_conditions,
            simulated_operator_review: Some(input.simulated_operator_review),
            decided_at_unix_millis: self.clock.now_unix_millis(),
            outcome: format!(
                "FALLBACK entered from {} (frozen edge 26/27); policy fired with its recorded \
                 parameters",
                input.trigger.as_str()
            ),
        };
        self.audit.record_activation(&record).map_err(FallbackPolicyError::Audit)?;

        // `DispatchFallbackActivated` (§15.3) through the transactional
        // outbox. The frozen payload carries identity + time only; the
        // policy facts live in the journal evidence and the audit record
        // (the recorded payload-fit gap).
        let correlation = self.ids.new_uuid();
        let event_time = EventTime::from_unix_millis(self.clock.now_unix_millis()).map_err(
            |error| FallbackPolicyError::InvalidTimestamp {
                detail: format!("the controlled clock instant is not representable: {error}"),
            },
        )?;
        emit(
            receipt,
            DomainEvent::DispatchFallbackActivated(DispatchFallbackActivatedPayload {
                correlation_id: correlation,
                tenant_id: ctx.tenant_id(),
                portfolio_id: input.portfolio_id,
                site_id: input.site_id,
                asset_id: input.asset_id,
                command_id: input.command_id.uuid(),
                event_time,
            }),
            correlation,
            event_time,
            self.encoder,
            self.outbox,
        )
        .map_err(|error: ApplicationError| FallbackPolicyError::Event {
            detail: error.to_string(),
        })?;

        Ok(FallbackDecision::Activated { record, transition })
    }

    /// Evaluates the bounded hold of a recorded activation (P4.18 step 1):
    /// before the expiry instant the hold is ACTIVE; at or after it the hold
    /// ESCALATES per the policy's recorded escalation target and the
    /// escalation is written through the audit sink (fail-closed). The hold
    /// NEVER holds indefinitely — evaluation at or after the expiry instant
    /// always escalates, and the interval itself is finite by construction.
    ///
    /// The escalation is an audit record, NOT a command transition
    /// ([`ESCALATION_IS_NOT_A_TRANSITION_NOTE`]): FALLBACK is terminal in
    /// the frozen table.
    pub fn evaluate_hold_expiration(
        &self,
        ctx: &TenantContext,
        activation: &FallbackActivationRecord,
    ) -> Result<HoldExpiryEvaluation, FallbackPolicyError> {
        let Some(interval_millis) = activation.bounded_hold_millis else {
            return Ok(HoldExpiryEvaluation::NoBoundedHold);
        };
        let Some(escalation_target) = activation.hold_escalation else {
            return Err(FallbackPolicyError::InconsistentJournal {
                command_id: activation.command_id.to_string(),
                detail: "an activation record carries a bounded hold without an escalation \
                         target; the hold could never resolve"
                    .to_string(),
            });
        };
        let expires_at = activation
            .decided_at_unix_millis
            .checked_add(interval_millis)
            .ok_or(FallbackPolicyError::InvalidTimestamp {
                detail: "the hold expiry instant overflows the millis representation".to_string(),
            })?;
        let now = self.clock.now_unix_millis();
        if now < expires_at {
            return Ok(HoldExpiryEvaluation::HoldActive {
                expires_at_unix_millis: expires_at,
            });
        }
        // The hold expired: escalate (audit record, never a transition).
        let record = FallbackEscalationRecord {
            command_id: activation.command_id,
            tenant_id: ctx.tenant_id(),
            site_id: activation.site_id,
            asset_id: activation.asset_id,
            policy_name: activation.policy_name.clone(),
            escalation_target,
            bounded_hold_millis: interval_millis,
            activated_at_unix_millis: activation.decided_at_unix_millis,
            escalated_at_unix_millis: now,
            outcome: format!(
                "bounded hold expired after {interval_millis} ms; escalated to {} \
                 (recorded escalation, not a command transition)",
                escalation_target.as_str()
            ),
        };
        self.audit.record_escalation(&record).map_err(FallbackPolicyError::Audit)?;
        Ok(HoldExpiryEvaluation::Escalated { target: escalation_target, record })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_policy_without_validity_conditions_cannot_be_constructed() {
        // The §14.4 validity-conditions rule at the type boundary.
        assert_eq!(
            HoldValidityConditions::new(Vec::new()),
            Err(FallbackPolicyError::HoldPolicyRequiresValidityConditions)
        );
    }

    #[test]
    fn the_policy_reference_format_is_recorded() {
        let policy = FallbackPolicy::new(
            "hold-and-return",
            FallbackScope::Site {
                site: SiteId::new(FlexUuid::from_bytes([1u8; 16])),
            },
            vec![FallbackAction::Declare(FallbackPolicyVariant::ReturnToLocalEmsAutonomousControl)],
            vec![FallbackTrigger::OnFailure],
        )
        .expect("valid policy");
        assert_eq!(
            policy.policy_reference(),
            format!("{FALLBACK_POLICY_REF_PREFIX}hold-and-return")
        );
    }

    #[test]
    fn stop_accepting_external_commands_is_reported_by_the_policy() {
        let policy = FallbackPolicy::new(
            "isolate",
            FallbackScope::Site {
                site: SiteId::new(FlexUuid::from_bytes([2u8; 16])),
            },
            vec![FallbackAction::Declare(FallbackPolicyVariant::StopAcceptingNewExternalCommands)],
            vec![FallbackTrigger::OnExpiry],
        )
        .expect("valid policy");
        assert!(!policy.accepts_external_commands());
    }
}
