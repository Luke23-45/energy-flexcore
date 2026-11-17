// P14.14 — Inverter settings-management workflow (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 62_p14_14_settings_management_workflow.md`).
//
// Sources: [S-20] IEEE 1547-2018 (standards.ieee.org/standard/
// 1547-2018.html — the Clause 10 utility-rights statement is an UNVERIFIED
// transcription from secondary material; the standard text was NOT
// consulted), [S-156] SunSpec IEEE 2030.5 in production (the CSIP primacy
// practice). The implementation-time human gate verifies the Clause 10
// detail and the primacy convention against the primary sources; a
// confirmed mismatch is a recorded edit, never a silent reinterpretation
// (plan §7 note 6).
//
// THE LOAD-BEARING RULES:
//
// 1. **A settings change is a lifecycle command, never a direct adapter
//    write [S-20][S-156].** Every settings change — certified-baseline
//    update, aggregator schedule, utility override — enters the frozen
//    14-state command lifecycle at PROPOSED (the wave-1 PROPOSED-entry
//    pattern, records 02 §6 / 21 §3, reused verbatim) and passes the
//    validation, feasibility, and authorization gates like any other
//    candidate. There is no settings-specific write path that bypasses
//    the lifecycle; an adapter that applies a settings change to a device
//    outside an ISSUED lifecycle command is a defect, not a shortcut
//    (plan §7 note 4).
//
// 2. **The frozen 14-state / 27-edge set stays byte-identical.** This
//    module adds no edge: it consumes
//    `flexcore_domain::command_state::try_transition` and the frozen
//    table is the enforcement point. The only states a settings-workflow
//    CLIENT (an adapter, the repository) may construct are the PROPOSED
//    entry and the PROPOSED-gate REJECTED — exactly the wave-1
//    `ADAPTER_CONSTRUCTIBLE_STATES` pattern.
//
// 3. **Primacy/precedence: the utility wins [S-20][S-156].** The
//    precedence order over competing settings candidates is:
//    (a) a utility authority outranks an aggregator authority at ALL
//        times (the 1547-2018 Clause 10 override right — UNVERIFIED
//        transcription; the declaration, not the clause number, is the
//        load-bearing data);
//    (b) among equal authorities, the DECLARED primacy value decides
//        (lower wins — the CSIP production convention [S-156], matching
//        the wave-1 `resolve_primacy` rules);
//    (c) equal declared primacy: the earlier-declared candidate wins,
//        then the deterministic command-id order.
//    A candidate without a declared primacy is never defaulted into the
//    competition (declared-never-inferred — the L4 rule of record 04).
//
// 4. **The utility-override hook changes precedence, never the gates.**
//    A utility override is a lifecycle command like any other: it enters
//    at PROPOSED and passes all three gates. Clause 10 gives the utility
//    precedence among settings candidates; it does not give anyone
//    authorization to skip the platform's own approval gate. A test pins
//    this.
//
// 5. **The certified baseline is utility-owned.** Only a candidate whose
//    authority is [`SettingsAuthority::Utility`] can produce a
//    [`CertifiedBaseline`] record — the constructor refuses any other
//    authority by name. An aggregator cannot rewrite the baseline; its
//    schedules are separate, lower-precedence records.
//
// Control status: pure domain workflow vocabulary for the simulation-only
// 0.x line. No device write path exists here (the "write" happens, in
// simulation and later, only as a relayed ISSUED lifecycle command); the
// workflow module constructs proposals and records, never commands an
// adapter. FlexCore is supervisory; no inverter inner-loop control.

use flexcore_contracts::capability::Provenance;

use crate::command_state::{self, CommandState};
use crate::ids::AssetId;
use crate::ieee1547_settings::{FunctionSetting, SettingSource};

// --- Authorities ---------------------------------------------------------------

/// The authority behind a settings candidate (P14.14 §3 precedence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettingsAuthority {
    /// The utility (the distribution utility / jurisdiction authority).
    /// The owner of the certified baseline; holder of the override right
    /// (1547-2018 Clause 10 — UNVERIFIED transcription).
    Utility,
    /// The aggregator (FlexCore acting as the DER aggregator).
    Aggregator,
}

impl SettingsAuthority {
    /// The precedence rank: LOWER wins. Utility = 0 outranks Aggregator =
    /// 1 at ALL times (precedence rule (a) — the Clause 10 override right
    /// as DECLARED data; UNVERIFIED transcription).
    pub const UTILITY_RANK: u8 = 0;
    /// The aggregator's rank.
    pub const AGGREGATOR_RANK: u8 = 1;

    /// The precedence rank of this authority.
    pub const fn rank(self) -> u8 {
        match self {
            SettingsAuthority::Utility => SettingsAuthority::UTILITY_RANK,
            SettingsAuthority::Aggregator => SettingsAuthority::AGGREGATOR_RANK,
        }
    }

    /// The corresponding settings-record source (the P14.13 vocabulary).
    pub const fn setting_source(self) -> SettingSource {
        match self {
            SettingsAuthority::Utility => SettingSource::UtilityOverride,
            SettingsAuthority::Aggregator => SettingSource::AggregatorSchedule,
        }
    }

    /// Canonical name (records and error messages).
    pub const fn name(self) -> &'static str {
        match self {
            SettingsAuthority::Utility => "utility",
            SettingsAuthority::Aggregator => "aggregator",
        }
    }
}

// --- The settings-change candidate ----------------------------------------------

/// One proposed settings change, before the lifecycle: the authority behind
/// it, the target device, the function, the new setting, and the DECLARED
/// primacy (never inferred). Constructed only through
/// [`SettingsChangeCandidate::new`], which validates every field.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsChangeCandidate {
    /// The authority behind the change.
    pub authority: SettingsAuthority,
    /// The target device.
    pub asset: AssetId,
    /// The command id this change will carry through the lifecycle (the
    /// audit anchor; non-empty; deterministic tie-break key for rule (c)).
    pub command_id: String,
    /// The new setting for the function.
    pub setting: FunctionSetting,
    /// The DECLARED primacy (lower wins among equal authorities). `None` is
    /// legal but the candidate is then never defaulted into competition.
    pub declared_primacy: Option<u8>,
    /// The candidate's declared creation instant (Unix milliseconds, UTC
    /// — the contracts time-model rule). Declared platform data for the
    /// precedence tie-break; the workflow never reads a clock itself
    /// (the injected-Clock rule belongs to the lifecycle engine).
    pub created_unix_ms: i64,
    /// The human-safe provenance note (never raw wire text; the P1.08 rule).
    pub provenance_note: String,
}

impl SettingsChangeCandidate {
    /// Constructs and validates a candidate. The citation inside the
    /// setting and the provenance note must be non-empty.
    pub fn new(
        authority: SettingsAuthority,
        asset: AssetId,
        command_id: String,
        setting: FunctionSetting,
        declared_primacy: Option<u8>,
        created_unix_ms: i64,
        provenance_note: String,
    ) -> Result<Self, SettingsWorkflowError> {
        if command_id.trim().is_empty() {
            return Err(SettingsWorkflowError::MissingIdentifier("command_id"));
        }
        if provenance_note.trim().is_empty() {
            return Err(SettingsWorkflowError::MissingIdentifier("provenance_note"));
        }
        Ok(Self {
            authority,
            asset,
            command_id,
            setting,
            declared_primacy,
            created_unix_ms,
            provenance_note,
        })
    }
}

/// The lifecycle-shaped proposal a settings-workflow client constructs from
/// a candidate. The entry state is CONST [`CommandState::Proposed`] — the
/// wave-1 pattern (records 02 §6 / 21 §3), byte-for-byte the same rule:
/// no constructor names any other entry state, and the only other state
/// this module's callers may construct is the PROPOSED-gate rejection.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsChangeProposal {
    /// The validated candidate.
    pub candidate: SettingsChangeCandidate,
    /// The lifecycle entry state. ALWAYS `PROPOSED`.
    pub lifecycle_entry_state: CommandState,
}

impl SettingsChangeProposal {
    /// The lifecycle entry state of every settings-change proposal.
    pub const ENTRY_STATE: CommandState = CommandState::Proposed;

    /// The lifecycle states a settings-workflow CLIENT may construct, and
    /// nothing else: the PROPOSED entry and the PROPOSED-gate rejection
    /// (frozen transition 2). `ISSUED`, `APPROVED`, and every other forward
    /// state are produced by the protected gates — a client constructing
    /// one is a defect (plan §7 note 4).
    pub const CLIENT_CONSTRUCTIBLE_STATES: [CommandState; 2] =
        [CommandState::Proposed, CommandState::Rejected];

    /// Wraps a candidate into a proposal at PROPOSED. The device-declared
    /// function support gate runs FIRST: a change to a function the device
    /// has not declared support for is refused before the lifecycle is
    /// ever opened (the P14.13 "never a claim that any device accepts any
    /// function" rule, [S-19][S-20], enforced at the workflow).
    pub fn from_candidate(
        candidate: SettingsChangeCandidate,
        declared_function_support: &[crate::ieee1547_settings::SettingFunctionId],
    ) -> Result<Self, SettingsWorkflowError> {
        if !declared_function_support.contains(&candidate.setting.function) {
            return Err(SettingsWorkflowError::FunctionNotDeclared {
                function: candidate.setting.function.name(),
            });
        }
        Ok(Self { candidate, lifecycle_entry_state: Self::ENTRY_STATE })
    }

    /// Records a rejection AT THE PROPOSED GATE: the frozen table's
    /// PROPOSED → REJECTED edge (transition 2) is the only legal path. The
    /// frozen machine, not this module, is the authority.
    pub fn reject_at_gate(&self) -> Result<CommandState, SettingsWorkflowError> {
        command_state::try_transition(self.lifecycle_entry_state, CommandState::Rejected)
            .map_err(SettingsWorkflowError::FrozenTransitionRefused)
    }

    /// Whether the frozen table permits a transition from the entry state
    /// to `target` — the same helper the wave-1 mapping modules expose.
    /// Only the frozen edges ever return `Ok`.
    pub fn client_may_transition_to(
        &self,
        target: CommandState,
    ) -> Result<CommandState, SettingsWorkflowError> {
        command_state::try_transition(self.lifecycle_entry_state, target)
            .map_err(SettingsWorkflowError::FrozenTransitionRefused)
    }
}

// --- Precedence resolution -------------------------------------------------------

/// The outcome of resolving competing settings candidates.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSettings<'a> {
    /// The winning candidate (the one submitted to the lifecycle first).
    pub winner: &'a SettingsChangeCandidate,
    /// The losing candidates, in the deterministic tie-break order (they
    /// remain lifecycle candidates — losing a precedence comparison is not
    /// a rejection; SUPERSEDED invalidation is the lifecycle's own edge,
    /// decided by the platform, never by this module).
    pub losers: Vec<&'a SettingsChangeCandidate>,
}

/// Resolves competing settings candidates by the P14.14 precedence rules:
/// (a) authority rank FIRST (utility outranks aggregator always — the
///     Clause 10 override right as declared data);
/// (b) among equal authorities, declared primacy decides (lower wins);
///     a candidate with no declared primacy never beats one that declares;
/// (c) equal authority rank and equal primacy state: the earlier
///     `created_unix_ms` wins, then the deterministic command-id order.
///
/// Losing a precedence comparison is not a rejection: every candidate
/// remains a lifecycle candidate, and SUPERSEDED invalidation is the
/// lifecycle's own edge, decided by the platform — never by this module.
/// An empty input returns `None`.
pub fn resolve_competing_settings<'a>(
    candidates: &[&'a SettingsChangeCandidate],
) -> Option<ResolvedSettings<'a>> {
    if candidates.is_empty() {
        return None;
    }
    let mut ordered: Vec<&'a SettingsChangeCandidate> = candidates.to_vec();
    ordered.sort_by(|a, b| {
        a.authority
            .rank()
            .cmp(&b.authority.rank())
            .then_with(|| {
                // Declared primacy beats undeclared within the same
                // authority rank; `None` sorts last (never defaulted in
                // front of a declared value). Across different authority
                // ranks the rank comparison already decided.
                match (a.declared_primacy, b.declared_primacy) {
                    (Some(pa), Some(pb)) => pa.cmp(&pb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            })
            .then_with(|| a.created_unix_ms.cmp(&b.created_unix_ms))
            .then_with(|| a.command_id.cmp(&b.command_id))
    });
    let winner = ordered[0];
    let losers = ordered[1..].to_vec();
    Some(ResolvedSettings { winner, losers })
}

// --- The certified baseline (utility-owned) --------------------------------------

/// The certified baseline record produced by a lifecycle-approved utility
/// settings change (the P14.13 `CertifiedBaselineRecord` is the state-side
/// document; this constructor is the workflow-side ownership gate).
///
/// **Only a `SettingsAuthority::Utility` candidate can produce one** —
/// the constructor refuses any other authority by name. The record still
/// travels through the lifecycle: ownership decides WHO may propose a
/// baseline change, not whether the change passes the three gates.
#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedBaseline {
    /// The device the baseline belongs to.
    pub asset: AssetId,
    /// The baseline function settings (at least one).
    pub functions: Vec<FunctionSetting>,
    /// The certification/interconnection citation (non-empty).
    pub citation: String,
}

impl CertifiedBaseline {
    /// Records a certified baseline FROM a utility candidate. An aggregator
    /// candidate is a named refusal — the baseline is utility-owned.
    pub fn from_candidate(
        candidate: &SettingsChangeCandidate,
        functions: Vec<FunctionSetting>,
    ) -> Result<Self, SettingsWorkflowError> {
        if candidate.authority != SettingsAuthority::Utility {
            return Err(SettingsWorkflowError::BaselineNotUtilityOwned {
                authority: candidate.authority.name(),
            });
        }
        if functions.is_empty() {
            return Err(SettingsWorkflowError::MissingIdentifier("functions"));
        }
        if candidate.setting.source_citation.trim().is_empty() {
            return Err(SettingsWorkflowError::MissingIdentifier("source_citation"));
        }
        Ok(Self {
            asset: candidate.asset,
            functions,
            citation: candidate.setting.source_citation.clone(),
        })
    }
}

// --- Errors ----------------------------------------------------------------------

/// Errors the settings-management workflow reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWorkflowError {
    /// A required identifier or text field was missing/empty.
    MissingIdentifier(&'static str),
    /// The target device has not declared support for the function — the
    /// repository refuses the change (the P14.13 declared-support rule).
    FunctionNotDeclared {
        /// The unsupported function name.
        function: &'static str,
    },
    /// The frozen command state machine refused a transition (the only
    /// lifecycle authority in the platform).
    FrozenTransitionRefused(command_state::TransitionError),
    /// A non-utility authority attempted to produce a certified baseline.
    BaselineNotUtilityOwned {
        /// The offending authority name.
        authority: &'static str,
    },
}

impl std::fmt::Display for SettingsWorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsWorkflowError::MissingIdentifier(field) => {
                write!(f, "settings change is missing '{field}'")
            }
            SettingsWorkflowError::FunctionNotDeclared { function } => {
                write!(
                    f,
                    "device has not declared support for function '{function}'; \
                     the repository refuses the change (support is declared, never assumed)"
                )
            }
            SettingsWorkflowError::FrozenTransitionRefused(error) => {
                write!(
                    f,
                    "the frozen command state machine refused the transition: {error}"
                )
            }
            SettingsWorkflowError::BaselineNotUtilityOwned { authority } => {
                write!(
                    f,
                    "authority '{authority}' cannot produce a certified baseline; \
                     the baseline is utility-owned"
                )
            }
        }
    }
}

impl std::error::Error for SettingsWorkflowError {}

/// Convenience: the provenance a workflow-recorded settings record carries
/// (the §11.1 vocabulary; an aggregator schedule entered through the
/// lifecycle is `Configured` — the workflow applied a declared value).
pub const fn workflow_provenance(authority: SettingsAuthority) -> Provenance {
    let _ = authority;
    Provenance::Configured
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ieee1547_settings::{
        CurvePoint, CurveQuantity, SettingFunctionId, SettingValue, SettingValueEntry,
    };

    const ASSET: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";
    const ASSET_OTHER: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8c";

    fn asset(sample: &str) -> AssetId {
        AssetId::new(flexcore_contracts::ids::FlexUuid::parse(sample).expect("valid uuid"))
    }

    fn curve_points() -> Vec<CurvePoint> {
        vec![
            CurvePoint {
                x_quantity: CurveQuantity::PerUnitVoltage,
                x: 0.9,
                y_quantity: CurveQuantity::ReactivePowerPercent,
                y: 44.0,
            },
            CurvePoint {
                x_quantity: CurveQuantity::PerUnitVoltage,
                x: 1.0,
                y_quantity: CurveQuantity::ReactivePowerPercent,
                y: 0.0,
            },
            CurvePoint {
                x_quantity: CurveQuantity::PerUnitVoltage,
                x: 1.1,
                y_quantity: CurveQuantity::ReactivePowerPercent,
                y: -44.0,
            },
        ]
    }

    fn volt_var_setting(source: SettingSource) -> FunctionSetting {
        FunctionSetting::new(
            SettingFunctionId::VoltVar,
            vec![SettingValueEntry {
                quantity_note: "per_unit_voltage_to_reactive_power_percent".to_string(),
                value: SettingValue::Curve(curve_points()),
            }],
            source,
            Provenance::Configured,
            "declared citation (UNVERIFIED detail pending the standard-text gate)"
                .to_string(),
        )
        .expect("valid setting")
    }

    fn candidate(
        authority: SettingsAuthority,
        command_id: &str,
        primacy: Option<u8>,
        created_ms: i64,
    ) -> SettingsChangeCandidate {
        SettingsChangeCandidate::new(
            authority,
            asset(ASSET),
            command_id.to_string(),
            volt_var_setting(authority.setting_source()),
            primacy,
            created_ms,
            format!("{authority:?} schedule received (declared data)"),
        )
        .expect("valid candidate")
    }

    // -- primacy tests ----------------------------------------------------

    #[test]
    fn utility_outranks_aggregator_regardless_of_declared_primacy() {
        // Precedence rule (a): the utility candidate wins even against a
        // strictly lower declared primacy number on the aggregator side —
        // the Clause 10 override right as DECLARED authority rank.
        let aggregator =
            candidate(SettingsAuthority::Aggregator, "agg-1", Some(1), 100);
        let utility = candidate(SettingsAuthority::Utility, "util-1", Some(9), 900);
        let resolved =
            resolve_competing_settings(&[&aggregator, &utility]).expect("winner");
        assert_eq!(resolved.winner.command_id, "util-1");
        assert_eq!(
            resolved
                .losers
                .iter()
                .map(|c| c.command_id.as_str())
                .collect::<Vec<_>>(),
            vec!["agg-1"]
        );
    }

    #[test]
    fn equal_authorities_resolve_by_declared_primacy_then_creation_then_command_id() {
        // Rule (b): among aggregator candidates, lower declared primacy wins.
        let a = candidate(SettingsAuthority::Aggregator, "agg-a", Some(5), 300);
        let b = candidate(SettingsAuthority::Aggregator, "agg-b", Some(2), 100);
        let resolved = resolve_competing_settings(&[&a, &b]).expect("winner");
        assert_eq!(resolved.winner.command_id, "agg-b");
        // Rule (c): equal declared primacy — earlier creation wins.
        let c = candidate(SettingsAuthority::Aggregator, "agg-c", Some(2), 50);
        let d = candidate(SettingsAuthority::Aggregator, "agg-d", Some(2), 80);
        let resolved = resolve_competing_settings(&[&d, &c]).expect("winner");
        assert_eq!(resolved.winner.command_id, "agg-c");
        // Rule (c) final tie-break: equal primacy and creation — the
        // deterministic command-id order (order-independence pinned).
        let e = candidate(SettingsAuthority::Aggregator, "agg-e", Some(2), 80);
        let f = candidate(SettingsAuthority::Aggregator, "agg-f", Some(2), 80);
        let forward = resolve_competing_settings(&[&e, &f]).expect("winner");
        let reverse = resolve_competing_settings(&[&f, &e]).expect("winner");
        assert_eq!(forward.winner.command_id, reverse.winner.command_id);
        assert_eq!(forward.winner.command_id, "agg-e");
    }

    #[test]
    fn undeclared_primacy_is_never_defaulted_into_competition() {
        // The declared-never-inferred rule: an undeclared candidate never
        // wins while a same-rank declared candidate competes, and stays
        // recorded among the losers.
        let declared = candidate(SettingsAuthority::Aggregator, "agg-dec", Some(9), 900);
        let undeclared = candidate(SettingsAuthority::Aggregator, "agg-undec", None, 10);
        let resolved =
            resolve_competing_settings(&[&undeclared, &declared]).expect("winner");
        assert_eq!(resolved.winner.command_id, "agg-dec");
        assert!(resolved.losers.iter().any(|c| c.command_id == "agg-undec"));
        // With no declared primacy anywhere, earliest creation wins.
        let resolved =
            resolve_competing_settings(&[&undeclared]).expect("winner");
        assert_eq!(resolved.winner.command_id, "agg-undec");
        // Empty competition: no winner.
        assert!(resolve_competing_settings(&[]).is_none());
    }

    // -- override tests ----------------------------------------------------

    #[test]
    fn utility_override_enters_at_proposed_and_never_skips_the_gates() {
        // The override hook changes PRECEDENCE, never the lifecycle: a
        // utility override candidate enters at PROPOSED like any other
        // change, and every gate-skipping target is refused by the frozen
        // table itself.
        let override_candidate =
            candidate(SettingsAuthority::Utility, "util-override", Some(0), 0);
        let proposal = SettingsChangeProposal::from_candidate(
            override_candidate,
            &[SettingFunctionId::VoltVar],
        )
        .expect("valid proposal");
        assert_eq!(proposal.lifecycle_entry_state, CommandState::Proposed);
        assert_eq!(SettingsChangeProposal::ENTRY_STATE, CommandState::Proposed);
        let names: Vec<&str> = SettingsChangeProposal::CLIENT_CONSTRUCTIBLE_STATES
            .iter()
            .map(|s| s.name())
            .collect();
        assert_eq!(names, vec!["PROPOSED", "REJECTED"]);
        for target in [
            CommandState::Issued,
            CommandState::Approved,
            CommandState::Feasible,
            CommandState::Acknowledged,
            CommandState::Completed,
        ] {
            assert!(
                proposal.client_may_transition_to(target).is_err(),
                "override PROPOSED -> {} must be refused by the frozen table",
                target.name()
            );
        }
        assert_eq!(proposal.reject_at_gate(), Ok(CommandState::Rejected));
    }

    #[test]
    fn override_wins_precedence_without_touching_the_lifecycle() {
        // Same lifecycle for both candidates; only the precedence differs.
        let aggregator = candidate(SettingsAuthority::Aggregator, "agg-x", Some(0), 0);
        let utility = candidate(SettingsAuthority::Utility, "util-x", Some(255), 10_000);
        let resolved = resolve_competing_settings(&[&aggregator, &utility]).expect("winner");
        assert_eq!(resolved.winner.authority, SettingsAuthority::Utility);
        // The winning proposal is still at PROPOSED — override never
        // promotes lifecycle state.
        let proposal = SettingsChangeProposal::from_candidate(
            resolved.winner.clone(),
            &[SettingFunctionId::VoltVar],
        )
        .expect("valid proposal");
        assert_eq!(proposal.lifecycle_entry_state, CommandState::Proposed);
    }

    // -- lifecycle + workflow-gate tests -------------------------------------

    #[test]
    fn settings_changes_enter_at_proposed_and_the_frozen_table_refuses_shortcuts() {
        let candidate_value = candidate(SettingsAuthority::Aggregator, "agg-y", Some(3), 0);
        let proposal = SettingsChangeProposal::from_candidate(
            candidate_value,
            &[SettingFunctionId::VoltVar],
        )
        .expect("valid proposal");
        // The frozen edges from PROPOSED are exactly VALIDATED and
        // REJECTED.
        assert_eq!(
            proposal.client_may_transition_to(CommandState::Validated),
            Ok(CommandState::Validated)
        );
        assert_eq!(
            proposal.client_may_transition_to(CommandState::Rejected),
            Ok(CommandState::Rejected)
        );
        for target in CommandState::ALL {
            if matches!(target, CommandState::Validated | CommandState::Rejected) {
                continue;
            }
            assert!(
                proposal.client_may_transition_to(target).is_err(),
                "PROPOSED -> {} is not a frozen edge",
                target.name()
            );
        }
    }

    #[test]
    fn undeclared_device_function_support_refuses_the_change() {
        let candidate_value = candidate(SettingsAuthority::Aggregator, "agg-z", Some(3), 0);
        // The device declares support only for freq_watt — a volt_var
        // change is refused BEFORE the lifecycle opens (the P14.13 rule).
        assert_eq!(
            SettingsChangeProposal::from_candidate(
                candidate_value,
                &[SettingFunctionId::FreqWatt],
            ),
            Err(SettingsWorkflowError::FunctionNotDeclared { function: "volt_var" })
        );
    }

    #[test]
    fn certified_baseline_is_utility_owned() {
        let utility_candidate =
            candidate(SettingsAuthority::Utility, "util-baseline", None, 0);
        let aggregator_candidate =
            candidate(SettingsAuthority::Aggregator, "agg-baseline", None, 0);
        // The utility may produce the baseline record.
        CertifiedBaseline::from_candidate(
            &utility_candidate,
            vec![volt_var_setting(SettingSource::CertifiedBaseline)],
        )
        .expect("utility baseline");
        // The aggregator may NOT — a named refusal, regardless of the
        // candidate's lifecycle state.
        assert_eq!(
            CertifiedBaseline::from_candidate(
                &aggregator_candidate,
                vec![volt_var_setting(SettingSource::CertifiedBaseline)],
            ),
            Err(SettingsWorkflowError::BaselineNotUtilityOwned {
                authority: "aggregator",
            })
        );
        // An empty function set is a named refusal.
        assert_eq!(
            CertifiedBaseline::from_candidate(&utility_candidate, Vec::new()),
            Err(SettingsWorkflowError::MissingIdentifier("functions"))
        );
    }

    #[test]
    fn candidates_require_identifiers_and_notes() {
        let setting = volt_var_setting(SettingSource::AggregatorSchedule);
        assert_eq!(
            SettingsChangeCandidate::new(
                SettingsAuthority::Aggregator,
                asset(ASSET_OTHER),
                String::new(),
                setting.clone(),
                None,
                "note".to_string(),
            ),
            Err(SettingsWorkflowError::MissingIdentifier("command_id"))
        );
        assert_eq!(
            SettingsChangeCandidate::new(
                SettingsAuthority::Aggregator,
                asset(ASSET_OTHER),
                "cmd".to_string(),
                setting,
                None,
                "   ".to_string(),
            ),
            Err(SettingsWorkflowError::MissingIdentifier("provenance_note"))
        );
    }
}
