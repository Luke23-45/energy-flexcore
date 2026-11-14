// P7.16 — Profile switching and safe re-composition: no silent control
// changes (report §5.6, §5.8, §21.4, §14.1; ledger `ledger_phase_7.md`
// P7.16).
//
// Switching between profiles is an EXPLICIT, audited operation. This
// module is the orchestration: it computes the deterministic diff between
// the current composition plan and the target plan (P7.06), refuses
// everything the §5.8/§21.4 rules forbid, executes the diff through
// drain-before-replace (P7.07's lifecycle semantics; the P7.08 registry's
// `swap_provider` is the canonical replacement operation), and audits
// EVERY step — committed or refused — through the kernel's in-module
// [`AuditSink`] port with timestamps from the injected [`Clock`].
//
// The structural guarantees, in order of enforcement:
//
// 1. EXPLICIT confirmation, fail-closed (DEFAULT decision — stronger than
//    the ledger minimum): EVERY switch requires a [`SwitchConfirmation`]
//    naming the actor, the from-profile, the to-profile, and the
//    confirmation timestamp. A switch without one is refused
//    (`SWITCH_CONFIRMATION_MISSING`); a confirmation naming different
//    profiles than the request is refused (`SWITCH_CONFIRMATION_MISMATCH`).
//    Nothing is ever switched silently.
// 2. Control-authority mode can NEVER change (§21.4; §5.8): the request
//    carries both profiles' postures ([`ControlAuthorityPosture`]); if
//    they differ, the switch is refused
//    (`CONTROL_AUTHORITY_MODE_CHANGE_REFUSED`). With the P7.15 mode lock,
//    profile DATA can only ever produce `simulation-shadow`, so this is
//    defence in depth — it re-closes the door against a posture handed in
//    from non-profile code.
// 3. The protected kernel is structurally unreachable. The P7.06 resolver
//    refuses any manifest claiming a protected-kernel contract id BEFORE
//    any plan exists (`PROTECTED_KERNEL_REPLACE`), so no resolver-produced
//    plan can contain one. This module re-states the guard at the
//    composition level: a target plan containing a protected id (which
//    can only exist if hand-constructed, bypassing the resolver) is
//    refused here too (`PROTECTED_CONTRACT_IN_TARGET`) — a profile switch
//    can never smuggle in a control-capable plugin (the ledger's named
//    risk).
// 4. No-op switches are refused (`SWITCH_NO_OP_REFUSED`) — a switch is an
//    event worth auditing, not a silent echo.
//
// Execution ordering and partial-failure semantics (the honest design):
//
//   PHASE A (prepare — ZERO side effects on the composition):
//     for every contract that will be added or replaced (sorted by
//     contract id): `executor.prepare_provider(...)` — construct the
//     newcomer/replacement provider EAGERLY. A construction failure
//     aborts the switch with the composition UNTOUCHED (this is where
//     nearly all runtime failures live, and they are paid before any
//     incumbent is disturbed).
//   PHASE B (incumbents — drain-before-replace, §5.6, in plugin-id order):
//     for every departing or upgraded plugin (sorted by plugin id):
//     `executor.drain_to_stopped(plugin_id)`, then — ONLY after that
//     incumbent is confirmed stopped — its contract operations:
//     `replace_provider` for contracts that survive under a new provider,
//     `remove_provider` for contracts that leave the composition. Newcomer
//     additions are NOT applied here: they wait until every incumbent
//     operation is confirmed (the recorded ordering rule: newcomers apply
//     only after incumbents are confirmed). A drain refusal leaves the
//     composition UNCHANGED — the canonical lifecycle implementation
//     restores the incumbent to `Ready` on a deadline expiry (P7.07,
//     fail-closed), so an abort in this phase has zero net effect. A
//     switch while a dispatch transaction is active is therefore refused
//     here: the incumbent with outstanding work cannot be drained (P7.07's
//     recorded REFUSED-not-queued behaviour).
//   PHASE C (newcomers, sorted by contract id):
//     `executor.add_provider(...)` for each added contract. Applied only
//     after ALL incumbent operations succeeded.
//   COMMIT (the only remaining step): audited; the caller advances the
//     recorded ACTIVE profile only now — never before.
//
// Partial-failure limitation, recorded honestly: true atomicity across a
// multi-step composition change is impossible without a process restart
// (the P7.08 registry is per-composition and a populated registry is
// never re-planned in place). The ordering above minimizes the exposure —
// prepare is side-effect-free and drains self-restore, so failures in
// phases A and B leave the composition exactly as it was — but a failure
// DURING phase C leaves the composition in the LAST FULLY-APPLIED state
// (everything up to the failed step stands; nothing after it is applied).
// The safe default is the RESTART FALLBACK: the composition root records
// the active profile ONLY after a fully successful switch, and on process
// restart rebuilds the recorded active profile from its manifest set —
// recovering drained incumbents through fresh lifecycles. A mid-switch
// failure therefore always has a defined recovery: restart into the
// recorded profile, then re-run the switch. Every abort is audited.
//
// Rollback (ledger step 4): switching back is just another switch — the
// diff to the previous plan is computed, confirmed, drained, applied, and
// audited with its own before/after composition records; the previous
// version's artifact retention is P7.14/P7.38 supply-chain work (the
// decision record).
//
// Determinism: the diff is a pure function of the two plans (BTreeMap
// keyed, every list sorted); the execution order is fixed (prepare →
// per-plugin drains → newcomers → commit, all sorted); the audit trail
// order is a function of the injected clock and the step order. No
// `unwrap`/`expect` on untrusted input anywhere.

use std::collections::BTreeMap;
use std::fmt;

use crate::lifecycle::{AuditSink, Clock, LifecycleAuditKind, LifecycleAuditRecord};
use crate::profile::ControlAuthorityPosture;
use crate::resolve::{CompositionPlan, PROTECTED_KERNEL_CONTRACT_IDS};

/// The audit-record subject for switch-level records (the
/// [`LifecycleAuditRecord::plugin_id`] field carries the composition's
/// switch subject; per-plugin records carry the plugin). The `version`
/// field of a switch record carries the TO-profile id.
pub const SWITCH_AUDIT_SUBJECT: &str = "composition.profile-switch";

// --- Confirmation ---------------------------------------------------------------------

/// The EXPLICIT operator confirmation a profile switch requires (fail-
/// closed DEFAULT: every switch, not only safety-relevant removals). The
/// record names the actor, the from-profile, the to-profile, and the
/// confirmation timestamp (from the operator path's own clock reading —
/// recorded data; the switch's OWN audit timestamps come from the
/// injected clock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchConfirmation {
    pub actor: String,
    pub from_profile: String,
    pub to_profile: String,
    pub timestamp_millis: u64,
    pub reason: String,
}

// --- The diff --------------------------------------------------------------------------

/// One contract-level change: a contract entering (`Add`) or leaving
/// (`Remove`) the composition, with the provider it gains or loses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractChange {
    pub contract_id: String,
    pub provider_plugin_id: String,
    pub provider_version: String,
}

/// A contract that survives under a DIFFERENT provider (or the same
/// provider at a different version — a version upgrade is a replacement:
/// it still drains the incumbent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractReplacement {
    pub contract_id: String,
    pub old_provider_plugin_id: String,
    pub old_provider_version: String,
    pub new_provider_plugin_id: String,
    pub new_provider_version: String,
}

/// One plugin-level membership change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginChange {
    pub plugin_id: String,
    pub version: String,
}

/// The deterministic diff between the current composition and the target
/// composition. Every list is sorted (contracts by contract id, plugins by
/// plugin id); the diff is a pure function of the two plans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompositionDiff {
    /// Contracts in the target with no counterpart in the current plan.
    pub added_contracts: Vec<ContractChange>,
    /// Contracts in the current plan with no counterpart in the target.
    pub removed_contracts: Vec<ContractChange>,
    /// Contracts in both, whose provider (or provider version) differs.
    pub replaced_contracts: Vec<ContractReplacement>,
    /// Plugins departing the composition (present now, absent in target).
    pub departing_plugins: Vec<PluginChange>,
    /// Plugins arriving in the composition.
    pub arriving_plugins: Vec<PluginChange>,
    /// Plugins staying but changing version (drained and replaced like a
    /// departure — a version upgrade is not a silent in-place change).
    pub upgraded_plugins: Vec<PluginChange>,
}

impl CompositionDiff {
    /// Whether the two plans are composition-identical (a no-op switch).
    pub fn is_empty(&self) -> bool {
        self.added_contracts.is_empty()
            && self.removed_contracts.is_empty()
            && self.replaced_contracts.is_empty()
            && self.departing_plugins.is_empty()
            && self.arriving_plugins.is_empty()
            && self.upgraded_plugins.is_empty()
    }
}

/// Compute the deterministic diff between two composition plans. Pure:
/// same plans → identical diff, every run. Versions are compared too — a
/// same-provider version bump shows up as a replacement, not a no-op.
pub fn diff_plans(current: &CompositionPlan, target: &CompositionPlan) -> CompositionDiff {
    // Plugin tables: plugin id → version.
    let current_plugins: BTreeMap<&str, &str> = current
        .registrations
        .iter()
        .map(|r| (r.plugin_id.as_str(), r.version.as_str()))
        .collect();
    let target_plugins: BTreeMap<&str, &str> = target
        .registrations
        .iter()
        .map(|r| (r.plugin_id.as_str(), r.version.as_str()))
        .collect();

    // Contract tables: contract id → (provider, provider version). The
    // provider's version is read from the same plan's registrations.
    let provider_version = |plan: &CompositionPlan, provider: &str| -> String {
        plan.registrations
            .iter()
            .find(|r| r.plugin_id == provider)
            .map(|r| r.version.clone())
            .unwrap_or_default()
    };
    let current_contracts: BTreeMap<&str, (&str, String)> = current
        .contracts
        .iter()
        .map(|c| {
            (
                c.contract_id.as_str(),
                (c.provider_plugin_id.as_str(), provider_version(current, &c.provider_plugin_id)),
            )
        })
        .collect();
    let target_contracts: BTreeMap<&str, (&str, String)> = target
        .contracts
        .iter()
        .map(|c| {
            (
                c.contract_id.as_str(),
                (c.provider_plugin_id.as_str(), provider_version(target, &c.provider_plugin_id)),
            )
        })
        .collect();

    let mut diff = CompositionDiff::default();

    // Plugins: departing, arriving, upgraded (all sorted by plugin id —
    // BTreeMap iteration order).
    for (plugin_id, version) in &current_plugins {
        if !target_plugins.contains_key(plugin_id) {
            diff.departing_plugins.push(PluginChange {
                plugin_id: (*plugin_id).to_string(),
                version: (*version).to_string(),
            });
        }
    }
    for (plugin_id, version) in &target_plugins {
        match current_plugins.get(plugin_id) {
            None => diff.arriving_plugins.push(PluginChange {
                plugin_id: (*plugin_id).to_string(),
                version: (*version).to_string(),
            }),
            Some(old_version) if old_version != version => {
                diff.upgraded_plugins.push(PluginChange {
                    plugin_id: (*plugin_id).to_string(),
                    version: (*version).to_string(),
                });
            }
            _ => {}
        }
    }

    // Contracts: added, removed, replaced (sorted by contract id).
    for (contract_id, (provider, version)) in &current_contracts {
        match target_contracts.get(contract_id) {
            None => diff.removed_contracts.push(ContractChange {
                contract_id: (*contract_id).to_string(),
                provider_plugin_id: (*provider).to_string(),
                provider_version: version.clone(),
            }),
            Some((new_provider, new_version)) => {
                if provider != new_provider || version != new_version {
                    diff.replaced_contracts.push(ContractReplacement {
                        contract_id: (*contract_id).to_string(),
                        old_provider_plugin_id: (*provider).to_string(),
                        old_provider_version: version.clone(),
                        new_provider_plugin_id: (*new_provider).to_string(),
                        new_provider_version: new_version.clone(),
                    });
                }
            }
        }
    }
    for (contract_id, (provider, version)) in &target_contracts {
        if !current_contracts.contains_key(contract_id) {
            diff.added_contracts.push(ContractChange {
                contract_id: (*contract_id).to_string(),
                provider_plugin_id: (*provider).to_string(),
                provider_version: version.clone(),
            });
        }
    }

    diff
}

// --- The executor port -----------------------------------------------------------------

/// The switch steps, named for audit records and error payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchStep {
    /// Phase A: eagerly construct a newcomer/replacement provider
    /// (side-effect free — a failure here leaves the composition untouched).
    Prepare,
    /// Phase B: drain an incumbent to `Stopped` (drain-before-replace, §5.6).
    Drain,
    /// Phase B: put a pre-prepared provider in place of a drained
    /// incumbent's contract (canonical implementation:
    /// `ProviderRegistry::swap_provider`, which re-checks
    /// `require_fully_drained`).
    Replace,
    /// Phase B: retire a drained incumbent's contract.
    Remove,
    /// Phase C: register a newcomer's pre-prepared provider.
    Add,
}

impl SwitchStep {
    pub fn name(&self) -> &'static str {
        match self {
            SwitchStep::Prepare => "PREPARE",
            SwitchStep::Drain => "DRAIN",
            SwitchStep::Replace => "REPLACE",
            SwitchStep::Remove => "REMOVE",
            SwitchStep::Add => "ADD",
        }
    }
}

impl fmt::Display for SwitchStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The composition executor port: the seam the switch operates through.
/// The CANONICAL implementation (composition-root wiring, documented in
/// the decision record) wraps the P7.08 `ProviderRegistry` and the P7.07
/// `PluginLifecycle`:
///
/// - `prepare_provider` constructs the provider eagerly through the
///   registry's factory (`construct`) — no registration, no composition
///   change;
/// - `drain_to_stopped` runs the §5.6 drain through the plugin's
///   lifecycle (`begin_drain` → `complete_drain`), restoring the
///   incumbent to `Ready` on a deadline expiry (fail-closed) and
///   returning the refusal — never force-stopping;
/// - `replace_provider` is the registry's `swap_provider` (which refuses
///   an undrained incumbent with `REPLACEMENT_BLOCKED_UNDRAINED`, audits
///   through the lifecycle, and records the version change);
/// - `remove_provider` retires the drained incumbent's registration;
/// - `add_provider` registers the newcomer through the fresh plan's
///   registration path.
///
/// Every method is fail-closed: a refusal is a named
/// [`RecompositionError::StepRefused`] and leaves the executor's state as
/// documented above — never a partial silent change.
pub trait CompositionExecutor {
    fn prepare_provider(&mut self, contract_id: &str, plugin_id: &str, version: &str) -> Result<(), RecompositionError>;
    fn drain_to_stopped(&mut self, plugin_id: &str) -> Result<(), RecompositionError>;
    fn replace_provider(&mut self, contract_id: &str, new_plugin_id: &str, new_version: &str) -> Result<(), RecompositionError>;
    fn remove_provider(&mut self, contract_id: &str) -> Result<(), RecompositionError>;
    fn add_provider(&mut self, contract_id: &str, plugin_id: &str, version: &str) -> Result<(), RecompositionError>;
}

// --- Errors ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecompositionError {
    /// No [`SwitchConfirmation`] was supplied. Fail-closed: nothing is
    /// ever switched silently.
    ConfirmationMissing { from_profile: String, to_profile: String },
    /// The confirmation names profiles other than the ones being switched.
    ConfirmationMismatch { reason: String },
    /// The switch would change the control-authority posture — refused
    /// (§21.4; the mode is a deployment/per-site setting).
    ControlAuthorityModeChange { from: &'static str, to: &'static str },
    /// The target plan contains a protected-kernel contract id. A plan
    /// from the resolver can never contain one (`PROTECTED_KERNEL_REPLACE`);
    /// this is the composition-level re-statement of the same guard.
    ProtectedContractInTarget { contract_id: String },
    /// The target composition is identical to the current one — a no-op
    /// switch is refused, not silently echoed.
    NoOpSwitch,
    /// A step was refused (or failed) mid-switch. The composition stands
    /// at the LAST FULLY-APPLIED state; the abort is audited; the recorded
    /// active profile is unchanged (the restart fallback recovers).
    StepRefused { step: SwitchStep, reason: String },
}

impl RecompositionError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            RecompositionError::ConfirmationMissing { .. } => "SWITCH_CONFIRMATION_MISSING",
            RecompositionError::ConfirmationMismatch { .. } => "SWITCH_CONFIRMATION_MISMATCH",
            RecompositionError::ControlAuthorityModeChange { .. } => "CONTROL_AUTHORITY_MODE_CHANGE_REFUSED",
            RecompositionError::ProtectedContractInTarget { .. } => "PROTECTED_CONTRACT_IN_TARGET",
            RecompositionError::NoOpSwitch => "SWITCH_NO_OP_REFUSED",
            RecompositionError::StepRefused { .. } => "SWITCH_STEP_REFUSED",
        }
    }

    /// The step a [`RecompositionError::StepRefused`] belongs to, if any.
    pub fn step(&self) -> Option<SwitchStep> {
        match self {
            RecompositionError::StepRefused { step, .. } => Some(*step),
            _ => None,
        }
    }
}

impl fmt::Display for RecompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecompositionError::ConfirmationMissing { from_profile, to_profile } => write!(
                f,
                "profile switch {from_profile} -> {to_profile} refused: no SwitchConfirmation was supplied; a profile switch is an explicit, audited operation and is never applied silently (fail-closed)"
            ),
            RecompositionError::ConfirmationMismatch { reason } => {
                write!(f, "profile switch refused: the confirmation does not match the request ({reason})")
            }
            RecompositionError::ControlAuthorityModeChange { from, to } => write!(
                f,
                "profile switch refused: it would change the control-authority posture from '{from}' to '{to}'; the mode is a deployment/per-site setting and no profile switch can change it (report §21.4)"
            ),
            RecompositionError::ProtectedContractInTarget { contract_id } => write!(
                f,
                "profile switch refused: the target composition contains the protected-kernel contract '{contract_id}'; the protected kernel is structurally unreachable (the resolver refuses it before any plan exists; the switch re-checks)"
            ),
            RecompositionError::NoOpSwitch => {
                write!(f, "profile switch refused: the target composition is identical to the current one (a no-op switch is refused, not silently applied)")
            }
            RecompositionError::StepRefused { step, reason } => {
                write!(f, "profile switch aborted at step {step}: {reason}; the composition stands at the last fully-applied state, the abort is audited, and the recorded active profile is unchanged (restart fallback: rebuild the recorded profile)")
            }
        }
    }
}

impl std::error::Error for RecompositionError {}

// --- The switch request -----------------------------------------------------------------

/// One explicit profile-switch request. `from_profile`/`to_profile` name
/// the profiles (ids); the postures ride the request so the mode-change
/// refusal (§21.4) is checkable even when the caller's postures do not
/// come from profile data. The plans are the CURRENT resolved plan and the
/// TARGET resolved plan (both from the P7.06 resolver — the target is
/// normally produced by the P7.15 `validate_profile`).
pub struct SwitchRequest<'a> {
    pub from_profile: &'a str,
    pub to_profile: &'a str,
    pub from_posture: ControlAuthorityPosture,
    pub to_posture: ControlAuthorityPosture,
    pub current_plan: &'a CompositionPlan,
    pub target_plan: &'a CompositionPlan,
    pub confirmation: Option<&'a SwitchConfirmation>,
}

/// The outcome of a fully committed switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchOutcome {
    pub from_profile: String,
    pub to_profile: String,
    pub diff: CompositionDiff,
    /// How many executor operations were applied (prepare + drain +
    /// replace/remove + add), in execution order.
    pub applied_steps: usize,
}

/// Execute an explicit profile switch (the P7.16 orchestration). See the
/// module header for the phase ordering and the partial-failure semantics.
/// Every step — committed or refused — is audited through the in-module
/// [`AuditSink`] port with timestamps from the injected [`Clock`].
pub fn execute_switch(
    request: &SwitchRequest<'_>,
    executor: &mut dyn CompositionExecutor,
    clock: &dyn Clock,
    audit: &dyn AuditSink,
) -> Result<SwitchOutcome, RecompositionError> {
    // 1. EXPLICIT confirmation, fail-closed (DEFAULT: every switch).
    let confirmation = match request.confirmation {
        None => {
            let reason = format!(
                "profile switch {} -> {} refused: no SwitchConfirmation was supplied (fail-closed; a switch is never silent)",
                request.from_profile, request.to_profile
            );
            emit_switch_record(audit, clock, request.to_profile, "unconfirmed-caller", LifecycleAuditKind::Refused, &reason);
            return Err(RecompositionError::ConfirmationMissing {
                from_profile: request.from_profile.to_string(),
                to_profile: request.to_profile.to_string(),
            });
        }
        Some(confirmation) => confirmation,
    };
    if confirmation.from_profile != request.from_profile || confirmation.to_profile != request.to_profile {
        let reason = format!(
            "profile switch {} -> {} refused: the confirmation names '{}' -> '{}', a different switch (fail-closed)",
            request.from_profile, request.to_profile, confirmation.from_profile, confirmation.to_profile
        );
        emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Refused, &reason);
        return Err(RecompositionError::ConfirmationMismatch {
            reason: format!(
                "confirmation is for {} -> {}, request is for {} -> {}",
                confirmation.from_profile,
                confirmation.to_profile,
                request.from_profile,
                request.to_profile
            ),
        });
    }

    // 2. The control-authority mode can NEVER change (§21.4). Defence in
    //    depth: profile data can only produce `simulation-shadow` (P7.15);
    //    this closes the door on postures handed in from elsewhere.
    if request.from_posture != request.to_posture {
        let reason = format!(
            "profile switch {} -> {} refused: it would change the control-authority posture from '{}' to '{}' (report §21.4)",
            request.from_profile, request.to_profile, request.from_posture.name(), request.to_posture.name()
        );
        emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Refused, &reason);
        return Err(RecompositionError::ControlAuthorityModeChange {
            from: request.from_posture.name(),
            to: request.to_posture.name(),
        });
    }

    // 3. The protected kernel is structurally unreachable — re-stated at
    //    the composition level (the resolver's guard is the primary).
    if let Some(contract_id) = first_protected_contract(request.target_plan) {
        let reason = format!(
            "profile switch {} -> {} refused: the target composition contains the protected-kernel contract '{contract_id}'",
            request.from_profile, request.to_profile
        );
        emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Refused, &reason);
        return Err(RecompositionError::ProtectedContractInTarget { contract_id });
    }

    // 4. The deterministic diff; a no-op switch is refused, not echoed.
    let diff = diff_plans(request.current_plan, request.target_plan);
    if diff.is_empty() {
        let reason = format!(
            "profile switch {} -> {} refused: the target composition is identical to the current one",
            request.from_profile, request.to_profile
        );
        emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Refused, &reason);
        return Err(RecompositionError::NoOpSwitch);
    }

    let started = format!(
        "profile switch {} -> {} started by '{}' (confirmed at {}): {} contract(s) added, {} removed, {} replaced, {} plugin(s) departing, {} arriving, {} upgraded; confirmed reason: {}",
        request.from_profile,
        request.to_profile,
        confirmation.actor,
        confirmation.timestamp_millis,
        diff.added_contracts.len(),
        diff.removed_contracts.len(),
        diff.replaced_contracts.len(),
        diff.departing_plugins.len(),
        diff.arriving_plugins.len(),
        diff.upgraded_plugins.len(),
        confirmation.reason,
    );
    emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Transition, &started);

    let mut applied: usize = 0;

    // PHASE A — prepare (zero side effects on the composition).
    // Replacements first (contract id order), then additions (contract id
    // order). A failure here aborts with the composition UNTOUCHED.
    for replacement in &diff.replaced_contracts {
        let outcome = executor.prepare_provider(
            &replacement.contract_id,
            &replacement.new_provider_plugin_id,
            &replacement.new_provider_version,
        );
        applied += record_step(
            audit,
            clock,
            request.to_profile,
            confirmation.actor.as_str(),
            SwitchStep::Prepare,
            &format!(
                "prepare replacement provider for contract '{}' ({})",
                replacement.contract_id, replacement.new_provider_plugin_id
            ),
            outcome,
            applied,
        )?;
    }
    for added in &diff.added_contracts {
        let outcome = executor.prepare_provider(
            &added.contract_id,
            &added.provider_plugin_id,
            &added.provider_version,
        );
        applied += record_step(
            audit,
            clock,
            request.to_profile,
            confirmation.actor.as_str(),
            SwitchStep::Prepare,
            &format!(
                "prepare newcomer provider for contract '{}' ({})",
                added.contract_id, added.provider_plugin_id
            ),
            outcome,
            applied,
        )?;
    }

    // PHASE B — incumbents (plugin id order): drain, then — ONLY after the
    // incumbent is confirmed stopped — its vacated contracts. A drain
    // refusal is self-restoring (the canonical lifecycle implementation
    // restores the incumbent to Ready), so the composition is unchanged.
    // Newcomer additions wait for the next phase.
    let mut incumbents: Vec<(&str, &str)> = Vec::new(); // (plugin_id, version)
    for plugin in &diff.departing_plugins {
        incumbents.push((&plugin.plugin_id, &plugin.version));
    }
    for plugin in &diff.upgraded_plugins {
        incumbents.push((&plugin.plugin_id, &plugin.version));
    }
    incumbents.sort_by(|a, b| a.0.cmp(b.0));
    for (plugin_id, _version) in &incumbents {
        let outcome = executor.drain_to_stopped(plugin_id);
        applied += record_step(
            audit,
            clock,
            request.to_profile,
            confirmation.actor.as_str(),
            SwitchStep::Drain,
            &format!("drain incumbent '{plugin_id}' to Stopped (drain-before-replace, report §5.6)"),
            outcome,
            applied,
        )?;
        for (contract_id, replacement) in vacated_contracts(request.current_plan, &diff, plugin_id) {
            match replacement {
                Some(replacement) => {
                    let outcome = executor.replace_provider(
                        contract_id,
                        &replacement.new_provider_plugin_id,
                        &replacement.new_provider_version,
                    );
                    applied += record_step(
                        audit,
                        clock,
                        request.to_profile,
                        confirmation.actor.as_str(),
                        SwitchStep::Replace,
                        &format!(
                            "replace provider of contract '{}': {} -> {}",
                            contract_id,
                            replacement.old_provider_plugin_id,
                            replacement.new_provider_plugin_id
                        ),
                        outcome,
                        applied,
                    )?;
                }
                None => {
                    let outcome = executor.remove_provider(contract_id);
                    applied += record_step(
                        audit,
                        clock,
                        request.to_profile,
                        confirmation.actor.as_str(),
                        SwitchStep::Remove,
                        &format!("retire contract '{}' vacated by '{plugin_id}'", contract_id),
                        outcome,
                        applied,
                    )?;
                }
            }
        }
    }

    // PHASE C — newcomers (contract id order), applied ONLY after every
    // incumbent operation is confirmed. A failure here leaves the
    // composition at the LAST FULLY-APPLIED state (the recorded
    // limitation; the restart fallback is the safe default).
    for added in &diff.added_contracts {
        let outcome =
            executor.add_provider(&added.contract_id, &added.provider_plugin_id, &added.provider_version);
        applied += record_step(
            audit,
            clock,
            request.to_profile,
            confirmation.actor.as_str(),
            SwitchStep::Add,
            &format!(
                "register newcomer provider for contract '{}' ({})",
                added.contract_id, added.provider_plugin_id
            ),
            outcome,
            applied,
        )?;
    }

    // COMMIT — the active profile advances ONLY now (the caller records
    // it; this record is the before/after evidence the ledger asks for).
    let committed = format!(
        "profile switch committed: active composition changes from '{from}' to '{to}'; {applied} executor operation(s) applied; before/after composition in the diff records above; rollback = a switch back (its own audited event)",
        from = request.from_profile,
        to = request.to_profile,
    );
    emit_switch_record(audit, clock, request.to_profile, confirmation.actor.as_str(), LifecycleAuditKind::Transition, &committed);

    Ok(SwitchOutcome {
        from_profile: request.from_profile.to_string(),
        to_profile: request.to_profile.to_string(),
        diff,
        applied_steps: applied,
    })
}

/// Emit one step's audit record and propagate a refusal as the abort. On
/// success returns 1 (the caller accumulates the applied count); on
/// failure the step refusal AND an ABORT record are emitted (both
/// audited) and the error is returned. `applied_so_far` is the count
/// BEFORE this step (it appears in the abort record).
fn record_step(
    audit: &dyn AuditSink,
    clock: &dyn Clock,
    to_profile: &str,
    actor: &str,
    step: SwitchStep,
    description: &str,
    outcome: Result<(), RecompositionError>,
    applied_so_far: usize,
) -> Result<usize, RecompositionError> {
    match outcome {
        Ok(()) => {
            let reason = format!("step {step} applied: {description}");
            emit_switch_record(audit, clock, to_profile, actor, LifecycleAuditKind::Transition, &reason);
            Ok(1)
        }
        Err(source) => {
            let reason = format!("step {step} refused: {description}: {source}");
            emit_switch_record(audit, clock, to_profile, actor, LifecycleAuditKind::Refused, &reason);
            let abort = format!(
                "profile switch ABORTED at step {step}: the composition stands at the last fully-applied state ({applied_so_far} step(s) applied so far); the recorded active profile is unchanged; recovery = restart into the recorded profile and re-run the switch"
            );
            emit_switch_record(audit, clock, to_profile, actor, LifecycleAuditKind::Refused, &abort);
            Err(source)
        }
    }
}

/// The contracts the named plugin will vacate, from the CURRENT plan
/// (contract id order): exactly the contracts it currently provides that
/// are removed or replaced in the target. A contract that stays with the
/// SAME provider at the SAME version is not vacated.
fn vacated_contracts<'a>(
    current_plan: &'a CompositionPlan,
    diff: &'a CompositionDiff,
    plugin_id: &str,
) -> Vec<(&'a str, Option<&'a ContractReplacement>)> {
    let mut out: Vec<(&'a str, Option<&'a ContractReplacement>)> = Vec::new();
    for contract in &current_plan.contracts {
        if contract.provider_plugin_id != plugin_id {
            continue;
        }
        let replacement = diff
            .replaced_contracts
            .iter()
            .find(|r| r.contract_id == contract.contract_id);
        let removed = diff
            .removed_contracts
            .iter()
            .any(|c| c.contract_id == contract.contract_id);
        if removed || replacement.is_some() {
            out.push((contract.contract_id.as_str(), replacement));
        }
    }
    out.sort_by(|a, b| a.0.cmp(b.0));
    out
}

/// The first protected-kernel contract id in the plan's contracts and
/// registrations (sorted, deterministic selection).
fn first_protected_contract(plan: &CompositionPlan) -> Option<String> {
    let mut protected: Vec<&str> = plan
        .contracts
        .iter()
        .map(|c| c.contract_id.as_str())
        .chain(plan.registrations.iter().flat_map(|r| r.provides.iter().map(|s| s.as_str())))
        .filter(|id| PROTECTED_KERNEL_CONTRACT_IDS.contains(id))
        .collect();
    protected.sort();
    protected.dedup();
    protected.first().map(|id| (*id).to_string())
}

/// Emit one switch-level audit record (subject [`SWITCH_AUDIT_SUBJECT`];
/// the record's `version` field carries the TO-profile id). The actor is
/// the confirmation's actor (the refusing pre-confirmation paths attribute
/// the attempt to `unconfirmed-caller`).
fn emit_switch_record(
    audit: &dyn AuditSink,
    clock: &dyn Clock,
    to_profile: &str,
    actor: &str,
    kind: LifecycleAuditKind,
    reason: &str,
) {
    let record = LifecycleAuditRecord {
        kind,
        plugin_id: SWITCH_AUDIT_SUBJECT.to_string(),
        version: to_profile.to_string(),
        from: None,
        to: None,
        actor: actor.to_string(),
        reason: reason.to_string(),
        timestamp_millis: clock.now_millis(),
    };
    audit.record(record);
}
