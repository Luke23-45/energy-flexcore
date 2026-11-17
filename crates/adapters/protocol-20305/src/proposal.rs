// P14.02 §5 — The DERControl → command-lifecycle mapping (decision record
// 02_p14_02_aggregator_server.md §5; phase-14 context §3 constraint 1;
// ADR-010 extended).
//
// The rule, verbatim from the phase constraints: **external control objects
// (2030.5 DERControl, OpenADR events) enter as PROPOSED lifecycle commands,
// never as issued setpoints.** An adapter that constructs an APPROVED or
// ISSUED state from external input is a defect, not a shortcut (plan §7.4).
//
// This module enforces the rule with the frozen state machine itself
// (`flexcore-domain::command_state`): a DERControl is mapped into an
// [`ExternalControlProposal`] whose lifecycle entry state is CONSTructed
// [`CommandState::Proposed`] — there is no constructor that can name any
// other entry state. The only other lifecycle state this adapter may
// record is [`CommandState::Rejected`] (the PROPOSED → REJECTED edge,
// frozen transition 2: malformed/unsupported) — and only through
// [`ExternalControlProposal::reject_at_gate`], which runs the frozen
// `try_transition` so the rejection is legal BY THE TABLE, not by
// convention.
//
// What this module deliberately does NOT do: it does not submit the
// proposal anywhere (the ingest port is a later seam, P14.17/P14.05), does
// not run the validation/feasibility/authorization gates (the platform
// owns them), and never names ISSUED, ACKNOWLEDGED, or any forward state —
// those are produced by the protected gates, never by an adapter.

use serde::{Deserialize, Serialize};

use flexcore_domain::command_state::{self, CommandState};
use flexcore_protocol_ports::scope::ProtocolScopeDeclaration;

use crate::addressing::resolve_primacy;
use crate::model::{Sfdi, TimeType};

/// Errors the external-event mapping reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalError {
    /// The source control lacks a required identifier.
    MissingIdentifier(&'static str),
    /// The adapter's scope gate refused the mapping (e.g. the manifest
    /// declares MonitorOnly and cannot ingest external control objects).
    ScopeRefused(flexcore_protocol_ports::scope::ProtocolScopeError),
    /// The requested lifecycle transition is not in the frozen table.
    FrozenTransitionRefused(command_state::TransitionError),
}

impl std::fmt::Display for ProposalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalError::MissingIdentifier(field) => {
                write!(f, "source control is missing '{field}'")
            }
            ProposalError::ScopeRefused(error) => {
                write!(f, "the adapter's declared scope refused the mapping: {error}")
            }
            ProposalError::FrozenTransitionRefused(error) => {
                write!(f, "the frozen command state machine refused the transition: {error}")
            }
        }
    }
}

impl std::error::Error for ProposalError {}

/// The source of an external control object (P14.02 §5). One variant —
/// the IEEE 2030.5 DERControl. Other protocols (OpenADR, 1547 settings
/// changes) add their own variants in THEIR crates with the same entry
/// rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalEventSource {
    /// An IEEE 2030.5 DERControl received from a utility/DR program
    /// (delivered over the AggregatorServer's resource tree [S-1][S-3]).
    DerControl {
        /// The DERProgram mRID the control belongs to.
        program_mrid: String,
        /// The DERControl mRID.
        control_mrid: String,
        /// The enrolled device SFDI the control targets.
        sfdi: Sfdi,
    },
}

/// The proposal an adapter produces from an external control object
/// (P14.02 §5). The lifecycle entry state is CONST [`CommandState::Proposed`];
/// no constructor can name another entry state.
///
/// Deliberately NOT serde-deriving: the frozen domain `CommandState` is a
/// protected platform vocabulary that carries no serialization surface, and
/// a proposal is an in-memory hand-off to the lifecycle, never a wire or
/// storage record. If a persistence surface is ever needed, it maps THROUGH
/// a dedicated record type — it does not add serde to the protected
/// vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalControlProposal {
    /// Where the control object came from.
    pub source: ExternalEventSource,
    /// The lifecycle entry state. ALWAYS `PROPOSED` — enforced by
    /// construction (`CommandState::Proposed` is a const, and no field
    /// write path exists outside this module).
    pub lifecycle_entry_state: CommandState,
    /// The window start of the source control, when declared (platform
    /// data for the validation gate; the adapter runs no gate itself).
    pub window_start: Option<TimeType>,
    /// The recorded primacy of the issuing authority, when declared
    /// (platform data for the primacy resolution; see `addressing`).
    pub declared_primacy: Option<u8>,
    /// The human-safe provenance note (never raw wire text; the P1.08
    /// rule).
    pub provenance_note: String,
}

impl ExternalControlProposal {
    /// The lifecycle entry state of every adapter-produced proposal.
    pub const ENTRY_STATE: CommandState = CommandState::Proposed;

    /// The lifecycle states an adapter may record, and nothing else
    /// (P14.02 §5): the PROPOSED entry, and the PROPOSED-gate rejection
    /// (frozen transition 2). `ISSUED`, `APPROVED`, and every other
    /// forward state are produced by the protected gates — an adapter
    /// constructing one is a defect.
    pub const ADAPTER_CONSTRUCTIBLE_STATES: [CommandState; 2] =
        [CommandState::Proposed, CommandState::Rejected];

    /// Maps an external DERControl into a proposal at PROPOSED. The scope
    /// gate runs first: an adapter whose declared scope does not cover
    /// [`flexcore_protocol_ports::scope::ResourceClass::Control`] and the
    /// external-event capability cannot ingest control objects at all.
    pub fn from_der_control(
        scope: &ProtocolScopeDeclaration,
        source: ExternalEventSource,
        window_start: Option<TimeType>,
        declared_primacy: Option<u8>,
        provenance_note: String,
    ) -> Result<Self, ProposalError> {
        // Fail-closed scope gate BEFORE any mapping.
        scope
            .assert_supports(
                flexcore_protocol_ports::scope::ProtocolRole::Server,
                false,
                &[flexcore_protocol_ports::scope::ResourceClass::Control],
            )
            .map_err(ProposalError::ScopeRefused)?;
        // Identifiers must be present (named failure, never an empty
        // mRID slipping through).
        match &source {
            ExternalEventSource::DerControl {
                program_mrid,
                control_mrid,
                ..
            } => {
                if program_mrid.trim().is_empty() {
                    return Err(ProposalError::MissingIdentifier("program_mrid"));
                }
                if control_mrid.trim().is_empty() {
                    return Err(ProposalError::MissingIdentifier("control_mrid"));
                }
            }
        }
        if provenance_note.trim().is_empty() {
            return Err(ProposalError::MissingIdentifier("provenance_note"));
        }
        Ok(Self {
            source,
            lifecycle_entry_state: Self::ENTRY_STATE,
            window_start,
            declared_primacy,
            provenance_note,
        })
    }

    /// Records a rejection AT THE PROPOSED GATE: the proposal is
    /// malformed or unsupported, and the frozen table's PROPOSED →
    /// REJECTED edge (transition 2) is the only legal path. Any other
    /// target state is refused by `try_transition` — the frozen machine,
    /// not this module, is the authority.
    pub fn reject_at_gate(&self) -> Result<CommandState, ProposalError> {
        command_state::try_transition(self.lifecycle_entry_state, CommandState::Rejected)
            .map_err(ProposalError::FrozenTransitionRefused)
    }

    /// The gate-skipping check the conformance tests exercise (P14.04):
    /// whether a target state would be refused for an adapter-constructed
    /// transition FROM the proposal's entry state. Returns `Ok(target)`
    /// only for the two adapter-constructible states.
    pub fn adapter_may_transition_to(
        &self,
        target: CommandState,
    ) -> Result<CommandState, ProposalError> {
        command_state::try_transition(self.lifecycle_entry_state, target)
            .map_err(ProposalError::FrozenTransitionRefused)
    }
}

/// Convenience: builds [`crate::addressing::PrioritizedControl`] candidates
/// from proposals that carry a declared primacy, so the primacy resolution
/// rules can be applied to external controls without duplicating the shape.
/// Proposals without a declared primacy are skipped (an undeclared primacy
/// is never defaulted — the declared-never-inferred rule).
pub fn prioritized_candidates(
    proposals: &[ExternalControlProposal],
    utility_authority: bool,
) -> Vec<crate::addressing::PrioritizedControl> {
    let mut candidates = Vec::new();
    for proposal in proposals {
        let (Some(primacy), Some(created)) = (proposal.declared_primacy, proposal.window_start)
        else {
            continue;
        };
        let mrid = match &proposal.source {
            ExternalEventSource::DerControl { control_mrid, .. } => control_mrid.clone(),
        };
        candidates.push(crate::addressing::PrioritizedControl {
            mrid,
            primacy,
            created,
            utility_authority,
        });
    }
    candidates
}

/// Resolves which of several competing external controls should become the
/// PROPOSED candidate first, using the declared primacy rules. Returns the
/// winning proposal by mRID, or `None` when no candidate carries both a
/// declared primacy and a window start.
pub fn resolve_competing_controls(
    proposals: &[ExternalControlProposal],
    utility_authority: bool,
) -> Option<String> {
    resolve_primacy(&prioritized_candidates(proposals, utility_authority))
        .map(|winner| winner.mrid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_protocol_ports::scope::{
        ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
    };

    fn scope_with_control() -> ProtocolScopeDeclaration {
        // Delegates to the reviewed spelling in crate::scope (FRV1-R6-004,
        // final review R6) — one reviewed constructor, no drift.
        crate::scope::aggregator_scope(crate::ADAPTER_ID).expect("valid scope")
    }

    fn source() -> ExternalEventSource {
        ExternalEventSource::DerControl {
            program_mrid: "prog-1".to_string(),
            control_mrid: "ctrl-1".to_string(),
            sfdi: Sfdi::new(123_456_789).expect("valid sfdi"),
        }
    }

    #[test]
    fn proposals_enter_at_proposed_and_nothing_else() {
        let proposal = ExternalControlProposal::from_der_control(
            &scope_with_control(),
            source(),
            Some(TimeType(100)),
            Some(3),
            "DERControl received via the aggregator's resource tree".to_string(),
        )
        .expect("valid proposal");
        assert_eq!(proposal.lifecycle_entry_state, CommandState::Proposed);
        assert_eq!(ExternalControlProposal::ENTRY_STATE, CommandState::Proposed);
        // The constructible set is exactly {PROPOSED, REJECTED}.
        let names: Vec<&str> = ExternalControlProposal::ADAPTER_CONSTRUCTIBLE_STATES
            .iter()
            .map(|s| s.name())
            .collect();
        assert_eq!(names, vec!["PROPOSED", "REJECTED"]);
    }

    #[test]
    fn gate_skipping_transitions_are_refused_by_the_frozen_table() {
        let proposal = ExternalControlProposal::from_der_control(
            &scope_with_control(),
            source(),
            None,
            None,
            "note".to_string(),
        )
        .expect("valid proposal");
        // No adapter may walk PROPOSED → ISSUED (no shortcut edge exists).
        assert!(matches!(
            proposal.adapter_may_transition_to(CommandState::Issued),
            Err(ProposalError::FrozenTransitionRefused(_))
        ));
        // Nor PROPOSED → APPROVED, PROPOSED → FEASIBLE, PROPOSED → COMPLETED.
        for target in [
            CommandState::Approved,
            CommandState::Feasible,
            CommandState::Completed,
            CommandState::Acknowledged,
        ] {
            assert!(
                proposal.adapter_may_transition_to(target).is_err(),
                "PROPOSED -> {} must be refused by the frozen table",
                target.name()
            );
        }
        // The only legal adapter-recorded transitions:
        assert_eq!(
            proposal.adapter_may_transition_to(CommandState::Rejected),
            Ok(CommandState::Rejected)
        );
        assert_eq!(
            proposal.adapter_may_transition_to(CommandState::Validated),
            Ok(CommandState::Validated)
        );
        // The rejection helper runs the same frozen edge.
        assert_eq!(
            proposal.reject_at_gate(),
            Ok(CommandState::Rejected)
        );
    }

    #[test]
    fn mapping_is_refused_outside_scope_or_with_missing_identifiers() {
        // A scope without the Control resource class cannot ingest.
        let no_control_scope = ProtocolScopeDeclaration::new(
            crate::ADAPTER_ID.to_string(),
            crate::PROTOCOL_ID.to_string(),
            vec![ProtocolRole::Server],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Discovery],
            vec![NotModelled::new(
                "no_control_mapping".to_string(),
                "this scope maps no control resources".to_string(),
            )
            .expect("valid")],
            "external control objects enter the command lifecycle at PROPOSED; this \
             adapter is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope");
        assert!(matches!(
            ExternalControlProposal::from_der_control(
                &no_control_scope,
                source(),
                None,
                None,
                "note".to_string(),
            ),
            Err(ProposalError::ScopeRefused(_))
        ));
        // An empty control mRID is a named refusal.
        let empty_mrid = ExternalEventSource::DerControl {
            program_mrid: "prog-1".to_string(),
            control_mrid: "  ".to_string(),
            sfdi: Sfdi::new(1).expect("valid sfdi"),
        };
        assert_eq!(
            ExternalControlProposal::from_der_control(
                &scope_with_control(),
                empty_mrid,
                None,
                None,
                "note".to_string(),
            ),
            Err(ProposalError::MissingIdentifier("control_mrid"))
        );
        // An empty provenance note is a named refusal (silence is never
        // provenance).
        assert_eq!(
            ExternalControlProposal::from_der_control(
                &scope_with_control(),
                source(),
                None,
                None,
                "  ".to_string(),
            ),
            Err(ProposalError::MissingIdentifier("provenance_note"))
        );
    }

    #[test]
    fn competing_controls_resolve_by_declared_primacy() {
        // Utility candidate (primacy 1) beats aggregator candidate
        // (primacy 5): the 1547-2018 Clause 10 override path is the
        // DECLARED primacy enrollment, applied by the shared rules.
        let utility = ExternalControlProposal::from_der_control(
            &scope_with_control(),
            ExternalEventSource::DerControl {
                program_mrid: "prog-util".to_string(),
                control_mrid: "util-ctrl".to_string(),
                sfdi: Sfdi::new(1).expect("valid sfdi"),
            },
            Some(TimeType(200)),
            Some(1),
            "utility program".to_string(),
        )
        .expect("valid proposal");
        let aggregator = ExternalControlProposal::from_der_control(
            &scope_with_control(),
            source(),
            Some(TimeType(100)),
            Some(5),
            "aggregator program".to_string(),
        )
        .expect("valid proposal");
        let winner = resolve_competing_controls(&[aggregator.clone(), utility.clone()], false)
            .expect("a winner resolves");
        assert_eq!(winner, "util-ctrl");
        // A proposal without a declared primacy is never defaulted into
        // the competition.
        let undeclared = ExternalControlProposal::from_der_control(
            &scope_with_control(),
            source(),
            Some(TimeType(50)),
            None,
            "no primacy declared".to_string(),
        )
        .expect("valid proposal");
        assert!(resolve_competing_controls(&[undeclared], false).is_none());
    }
}
