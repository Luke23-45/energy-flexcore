// P16.12 — Dispute workflow: line-item challenge tracking, chain-linked
//         evidence-bundle assembly, and resolution records over the
//         P16.09 reconciliation queue.
//
// Phase 16 plan §2 row P16.12; blocking-order row §3 ("P16.09
// reconciliation queue → P16.12 dispute queue"). Sources: the P16.09
// record ([S-93][S-97][S-107] statement classes) and the P11 audit-chain
// treatment (context constraint 6: every financial record inherits the
// chain; LR-203 production-crypto requirement inherited).
//
// WHAT THIS IS (decided once):
//
// - **Disputes reference the reconciliation queue — they are never
//   self-invented.** A challenge is raised AGAINST a P16.09
//   [`DisputeQueueEntry`]; the queue entry reference is mandatory. A
//   dispute with no reconciliation-queue linkage is unconstructable.
// - **Every evidence bundle inherits the P11 chain treatment by
//   citation** (context constraint 6): each bundle entry carries an
//   audit-chain digest REFERENCE (64 hex characters — the P11 chain's
//   32-byte digest rendering). An entry without a valid chain-digest
//   reference is refused — a bundle is chain-linked or it does not
//   exist. The chain ITSELF is consumed, never redefined: this module
//   stores references; the append/verify mechanics stay in
//   flexcore-audit (P11) at the composition root (the application crate
//   cannot depend on it — the dependency rule).
// - **Bundles are immutable**: assembled once, never edited. New
//   evidence is a NEW bundle version referencing the dispute.
// - **The state machine is a frozen transition table** (the P0.11
//   pattern): exactly the recorded transitions, no shortcuts. A
//   resolution REQUIRES a bundle reference — no dispute is resolved
//   without its chain-linked evidence.
// - **No dispute exists, no resolution is claimed** (honesty rules): no
//   statement has ever been received, so no dispute has ever been
//   opened. Every fixture in the tests is synthetic.
//
// Control status: pure workflow records for the simulation-only 0.x
// line. No dispute exists; no settlement adjustment is ever applied by
// this module (the reconciliation engine's own rule holds: its output
// is evidence, and resolution is a recorded human process).

use std::collections::BTreeMap;

/// The dispute-workflow schema version (a change is a NEW version).
pub const DISPUTE_WORKFLOW_SCHEMA_VERSION: u32 = 1;

/// The reconciliation-linkage statement (pinned by tests).
pub const DISPUTES_REFERENCE_THE_QUEUE_STATEMENT: &str = "A dispute is raised AGAINST a \
     P16.09 reconciliation-queue entry; the queue entry reference is mandatory and the \
     linkage is exact. A dispute with no reconciliation-queue linkage is \
     unconstructable — disputes are never self-invented.";

/// The chain-inheritance statement (pinned by tests).
pub const BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT: &str = "Every evidence-bundle entry \
     carries an audit-chain digest reference (the P11 chain treatment; the LR-203 \
     production-crypto requirement inherited). An entry without a valid 64-hex \
     chain-digest reference is refused — a bundle is chain-linked or it does not exist. \
     The chain mechanics stay in flexcore-audit (P11) at the composition root; this \
     module stores references and never redefines the chain.";

/// The no-dispute-exists statement (pinned by tests).
pub const NO_DISPUTE_EXISTS_STATEMENT: &str = "No dispute exists in this program: no \
     settlement statement has ever been received (none exists here), so no challenge, \
     bundle, or resolution is real. Every fixture value in the tests is synthetic; no \
     settlement adjustment is ever applied by this module — resolution is a recorded \
     human process.";

/// The closed dispute-state vocabulary (the frozen transition table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisputeState {
    /// The dispute was opened against a queue entry.
    Opened,
    /// The chain-linked evidence bundle was assembled.
    EvidenceAssembled,
    /// The dispute was submitted to the counterparty process.
    Submitted,
    /// The dispute reached a recorded resolution (terminal).
    Resolved,
    /// The dispute was withdrawn before resolution (terminal).
    Withdrawn,
}

impl DisputeState {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            DisputeState::Opened => "opened",
            DisputeState::EvidenceAssembled => "evidence-assembled",
            DisputeState::Submitted => "submitted",
            DisputeState::Resolved => "resolved",
            DisputeState::Withdrawn => "withdrawn",
        }
    }
}

/// The frozen transition table: exactly these edges, no more, no fewer
/// (the P0.11 pattern). Resolved and Withdrawn are terminal.
const TRANSITIONS: &[(DisputeState, DisputeState)] = &[
    (DisputeState::Opened, DisputeState::EvidenceAssembled),
    (DisputeState::EvidenceAssembled, DisputeState::Submitted),
    (DisputeState::Opened, DisputeState::Withdrawn),
    (DisputeState::EvidenceAssembled, DisputeState::Withdrawn),
    (DisputeState::Submitted, DisputeState::Withdrawn),
    (DisputeState::Submitted, DisputeState::Resolved),
];

/// Why a dispute workflow operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisputeError {
    /// The transition is not in the frozen table (no shortcut edges).
    InvalidTransition { from: DisputeState, to: DisputeState },
    /// A reference field was empty.
    EmptyReference { field: &'static str },
    /// The P16.09 reason code was unrecognized (a challenge's kind is
    /// derived from the queue entry's reason — an unknown code is a
    /// refusal, never a generic "other").
    UnrecognizedReasonCode { code: String },
    /// A chain-digest reference was not 64 hex characters (the P11
    /// digest rendering) — the entry is refused, never accepted loose.
    InvalidChainDigestRef { reason: String },
    /// A resolution without an evidence-bundle reference (no dispute is
    /// resolved without its chain-linked evidence).
    ResolutionWithoutBundle,
}

impl DisputeError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            DisputeError::InvalidTransition { .. } => "DISPUTE_INVALID_TRANSITION",
            DisputeError::EmptyReference { .. } => "DISPUTE_EMPTY_REFERENCE",
            DisputeError::UnrecognizedReasonCode { .. } => "DISPUTE_UNRECOGNIZED_REASON_CODE",
            DisputeError::InvalidChainDigestRef { .. } => "DISPUTE_INVALID_CHAIN_DIGEST_REF",
            DisputeError::ResolutionWithoutBundle => "DISPUTE_RESOLUTION_WITHOUT_BUNDLE",
        }
    }
}

impl std::fmt::Display for DisputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisputeError::InvalidTransition { from, to } => write!(
                f,
                "the transition {} → {} is not in the frozen table; there are no \
                 shortcut edges (the P0.11 pattern)",
                from.as_str(),
                to.as_str()
            ),
            DisputeError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            DisputeError::UnrecognizedReasonCode { code } => write!(
                f,
                "the reconciliation reason code '{code}' is not one of the P16.09 \
                 queue codes; the challenge kind is refused, never generalized"
            ),
            DisputeError::InvalidChainDigestRef { reason } => write!(
                f,
                "the chain-digest reference is invalid: {reason} — the bundle entry \
                 is refused, never accepted loose (the P11 chain treatment)"
            ),
            DisputeError::ResolutionWithoutBundle => write!(
                f,
                "a resolution without an evidence-bundle reference is refused: no \
                 dispute is resolved without its chain-linked evidence"
            ),
        }
    }
}

impl std::error::Error for DisputeError {}

/// The frozen-transition check: exactly the recorded edges, terminal
/// states stay terminal.
pub fn transition_dispute(
    from: DisputeState,
    to: DisputeState,
) -> Result<DisputeState, DisputeError> {
    if TRANSITIONS
        .iter()
        .any(|(a, b)| *a == from && *b == to)
    {
        Ok(to)
    } else {
        Err(DisputeError::InvalidTransition { from, to })
    }
}

/// The closed challenge-kind vocabulary, derived from the P16.09 queue
/// reason codes (never invented, never generalized).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChallengeKind {
    /// The stated amount differs from the expected amount
    /// (`LINE_AMOUNT_MISMATCH`).
    AmountMismatch,
    /// The statement omitted a line the expected set carried
    /// (`LINE_MISSING_IN_STATEMENT`).
    LineMissing,
    /// The statement carried a line the expected set did not
    /// (`LINE_UNEXPECTED_IN_STATEMENT`).
    LineUnexpected,
}

impl ChallengeKind {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ChallengeKind::AmountMismatch => "amount-mismatch",
            ChallengeKind::LineMissing => "line-missing",
            ChallengeKind::LineUnexpected => "line-unexpected",
        }
    }
}

/// Derives the challenge kind from the P16.09 queue entry's reason code.
/// An unrecognized code is a named refusal — never a generic "other"
/// bucket ([`DisputeError::UnrecognizedReasonCode`]).
pub fn challenge_kind_from_reason_code(code: &str) -> Result<ChallengeKind, DisputeError> {
    match code {
        "LINE_AMOUNT_MISMATCH" => Ok(ChallengeKind::AmountMismatch),
        "LINE_MISSING_IN_STATEMENT" => Ok(ChallengeKind::LineMissing),
        "LINE_UNEXPECTED_IN_STATEMENT" => Ok(ChallengeKind::LineUnexpected),
        _ => Err(DisputeError::UnrecognizedReasonCode {
            code: code.to_string(),
        }),
    }
}

/// One line-item challenge: a dispute opened against one P16.09 queue
/// entry. The queue linkage is mandatory ([`DISPUTES_REFERENCE_THE_QUEUE_STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineItemChallenge {
    /// The challenge reference (caller-assigned; non-empty).
    pub challenge_ref: String,
    /// The P16.09 reconciliation-queue entry the challenge is raised
    /// against (exact reference).
    pub queue_entry_ref: String,
    /// The settlement line reference under dispute (verbatim from the
    /// queue entry).
    pub line_ref: String,
    /// The statement reference the line belongs to (verbatim).
    pub statement_ref: String,
    /// The challenge kind (derived from the queue entry's reason code —
    /// see [`challenge_kind_from_reason_code`]).
    pub kind: ChallengeKind,
    /// The recorded opener (an operator/policy identity; never a plugin
    /// — the control-authority rule holds for disputes too).
    pub raised_by: String,
}

impl LineItemChallenge {
    /// Validates and constructs one challenge (fail-closed).
    pub fn new(
        challenge_ref: String,
        queue_entry_ref: String,
        line_ref: String,
        statement_ref: String,
        kind: ChallengeKind,
        raised_by: String,
    ) -> Result<Self, DisputeError> {
        if challenge_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "challenge_ref",
            });
        }
        if queue_entry_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "queue_entry_ref",
            });
        }
        if line_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "line_ref" });
        }
        if statement_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "statement_ref",
            });
        }
        if raised_by.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "raised_by" });
        }
        Ok(Self {
            challenge_ref,
            queue_entry_ref,
            line_ref,
            statement_ref,
            kind,
            raised_by,
        })
    }
}

/// One evidence-bundle entry: a reference to one evidence record plus
/// its P11 audit-chain linkage (the chain digest reference — 64 hex
/// characters, the P11 chain's 32-byte digest rendering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundleEntry {
    /// The entry reference (non-empty).
    pub entry_ref: String,
    /// The evidence record reference (a P16.11 evidence record, a
    /// command record, or a market publication reference — non-empty).
    pub evidence_ref: String,
    /// The P11 chain namespace the entry is chained under (non-empty).
    pub chain_namespace: String,
    /// The chain-digest reference: exactly 64 hex characters.
    pub chain_digest_hex: String,
}

impl EvidenceBundleEntry {
    /// Validates and constructs one entry; the digest reference is
    /// validated as exactly 64 hex characters ([`BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT`]).
    pub fn new(
        entry_ref: String,
        evidence_ref: String,
        chain_namespace: String,
        chain_digest_hex: String,
    ) -> Result<Self, DisputeError> {
        if entry_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "entry_ref" });
        }
        if evidence_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "evidence_ref",
            });
        }
        if chain_namespace.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "chain_namespace",
            });
        }
        validate_chain_digest_ref(&chain_digest_hex)?;
        Ok(Self {
            entry_ref,
            evidence_ref,
            chain_namespace,
            chain_digest_hex,
        })
    }
}

/// Validates a chain-digest reference: exactly 64 ASCII hex characters
/// (the P11 chain's 32-byte digest rendering).
fn validate_chain_digest_ref(hex: &str) -> Result<(), DisputeError> {
    if hex.len() != 64 {
        return Err(DisputeError::InvalidChainDigestRef {
            reason: format!(
                "the reference is {} characters; the P11 digest rendering is exactly 64",
                hex.len()
            ),
        });
    }
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DisputeError::InvalidChainDigestRef {
            reason: "the reference carries a non-hex character".to_string(),
        });
    }
    Ok(())
}

/// One evidence bundle: the immutable, chain-linked evidence set for one
/// dispute. Assembled once ([`NO_DISPUTE_EXISTS_STATEMENT`] and the
/// immutability rule); a new-evidence bundle is a NEW bundle referencing
/// the same dispute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    /// The bundle reference (non-empty).
    pub bundle_ref: String,
    /// The dispute (challenge reference) the bundle backs.
    pub dispute_ref: String,
    /// The chain-linked entries, in insertion order (the assembly order
    /// is part of the record).
    pub entries: Vec<EvidenceBundleEntry>,
}

impl EvidenceBundle {
    /// Assembles one bundle (fail-closed): a non-empty entry set, every
    /// entry chain-linked. An EMPTY bundle is refused — a bundle with no
    /// evidence would be an argument without its chain, and the honesty
    /// rule forbids accepting it.
    pub fn assemble(
        bundle_ref: String,
        dispute_ref: String,
        entries: Vec<EvidenceBundleEntry>,
    ) -> Result<Self, DisputeError> {
        if bundle_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "bundle_ref" });
        }
        if dispute_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "dispute_ref" });
        }
        if entries.is_empty() {
            return Err(DisputeError::EmptyReference { field: "entries" });
        }
        Ok(Self {
            bundle_ref,
            dispute_ref,
            entries,
        })
    }

    /// The entries indexed by their chain namespace (BTreeMap —
    /// deterministic order, the kernel determinism rule).
    pub fn entries_by_namespace(&self) -> BTreeMap<&str, Vec<&EvidenceBundleEntry>> {
        let mut map: BTreeMap<&str, Vec<&EvidenceBundleEntry>> = BTreeMap::new();
        for entry in &self.entries {
            map.entry(entry.chain_namespace.as_str())
                .or_default()
                .push(entry);
        }
        map
    }
}

/// The closed resolution vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionKind {
    /// The challenged line was confirmed as filed (the dispute fails).
    Upheld,
    /// The counterparty accepted an adjustment (recorded as data — the
    /// adjustment itself is re-ingested through the P16.09 pipeline,
    /// never applied in place here).
    AdjustedByCounterparty,
    /// The dispute was rejected by the counterparty process.
    Rejected,
}

impl ResolutionKind {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ResolutionKind::Upheld => "upheld",
            ResolutionKind::AdjustedByCounterparty => "adjusted-by-counterparty",
            ResolutionKind::Rejected => "rejected",
        }
    }
}

/// One resolution record: the recorded outcome of a submitted dispute.
/// A resolution REQUIRES the bundle reference (no dispute is resolved
/// without its chain-linked evidence — [`DisputeError::ResolutionWithoutBundle`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeResolutionRecord {
    /// The resolution reference (non-empty).
    pub resolution_ref: String,
    /// The dispute (challenge reference) being resolved.
    pub dispute_ref: String,
    /// The evidence-bundle reference that backs the resolution
    /// (mandatory).
    pub bundle_ref: String,
    /// The resolution kind.
    pub kind: ResolutionKind,
    /// The recorded resolver (an operator/policy identity).
    pub resolved_by: String,
    /// A recorded note (may be empty — the kind and identity are the
    /// substance).
    pub note: String,
}

impl DisputeResolutionRecord {
    /// Validates and constructs one resolution (fail-closed: the bundle
    /// reference is mandatory; the resolver is mandatory).
    pub fn new(
        resolution_ref: String,
        dispute_ref: String,
        bundle_ref: String,
        kind: ResolutionKind,
        resolved_by: String,
        note: String,
    ) -> Result<Self, DisputeError> {
        if resolution_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference {
                field: "resolution_ref",
            });
        }
        if dispute_ref.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "dispute_ref" });
        }
        if bundle_ref.trim().is_empty() {
            return Err(DisputeError::ResolutionWithoutBundle);
        }
        if resolved_by.trim().is_empty() {
            return Err(DisputeError::EmptyReference { field: "resolved_by" });
        }
        Ok(Self {
            resolution_ref,
            dispute_ref,
            bundle_ref,
            kind,
            resolved_by,
            note,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture value below is
// SYNTHETIC (no dispute exists); every expected value is hand-derived
// from the frozen rules. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-hex fixture digest (the P11 digest rendering length).
    const FIXTURE_DIGEST: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(DISPUTES_REFERENCE_THE_QUEUE_STATEMENT.contains("P16.09"));
        assert!(DISPUTES_REFERENCE_THE_QUEUE_STATEMENT.contains("unconstructable"));
        assert!(BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT.contains("P11 chain treatment"));
        assert!(BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT.contains("64-hex"));
        assert!(NO_DISPUTE_EXISTS_STATEMENT.contains("No dispute exists"));
    }

    #[test]
    fn the_frozen_transition_table_has_no_shortcuts() {
        // The recorded edges.
        assert!(transition_dispute(DisputeState::Opened, DisputeState::EvidenceAssembled).is_ok());
        assert!(transition_dispute(DisputeState::EvidenceAssembled, DisputeState::Submitted).is_ok());
        assert!(transition_dispute(DisputeState::Submitted, DisputeState::Resolved).is_ok());
        assert!(transition_dispute(DisputeState::Submitted, DisputeState::Withdrawn).is_ok());
        assert!(transition_dispute(DisputeState::Opened, DisputeState::Withdrawn).is_ok());
        assert!(transition_dispute(DisputeState::EvidenceAssembled, DisputeState::Withdrawn).is_ok());
        // No shortcut edges: no Opened→Submitted, no Opened→Resolved, no
        // EvidenceAssembled→Resolved (a resolution requires submission).
        let err = transition_dispute(DisputeState::Opened, DisputeState::Submitted)
            .expect_err("shortcut refused");
        assert_eq!(err.name(), "DISPUTE_INVALID_TRANSITION");
        assert!(transition_dispute(DisputeState::Opened, DisputeState::Resolved).is_err());
        assert!(transition_dispute(DisputeState::EvidenceAssembled, DisputeState::Resolved).is_err());
        // Terminal states stay terminal.
        assert!(transition_dispute(DisputeState::Resolved, DisputeState::Opened).is_err());
        assert!(transition_dispute(DisputeState::Withdrawn, DisputeState::Submitted).is_err());
    }

    #[test]
    fn challenge_kinds_derive_only_from_the_p16_09_reason_codes() {
        assert_eq!(
            challenge_kind_from_reason_code("LINE_AMOUNT_MISMATCH").expect("mapped"),
            ChallengeKind::AmountMismatch
        );
        assert_eq!(
            challenge_kind_from_reason_code("LINE_MISSING_IN_STATEMENT").expect("mapped"),
            ChallengeKind::LineMissing
        );
        assert_eq!(
            challenge_kind_from_reason_code("LINE_UNEXPECTED_IN_STATEMENT").expect("mapped"),
            ChallengeKind::LineUnexpected
        );
        // An unknown code is refused, never generalized.
        let err = challenge_kind_from_reason_code("SOMETHING_ELSE")
            .expect_err("unknown code refused");
        assert_eq!(err.name(), "DISPUTE_UNRECOGNIZED_REASON_CODE");
    }

    #[test]
    fn challenges_require_the_queue_linkage() {
        let challenge = LineItemChallenge::new(
            "CHL-1".to_string(),
            "SYN-STMT#SYN-L1#mismatch".to_string(),
            "SYN-L1".to_string(),
            "SYN-STMT".to_string(),
            ChallengeKind::AmountMismatch,
            "operator-fixture".to_string(),
        )
        .expect("valid challenge");
        assert_eq!(challenge.kind.as_str(), "amount-mismatch");
        // A missing queue-entry reference is a named refusal.
        let err = LineItemChallenge::new(
            "CHL-2".to_string(),
            "  ".to_string(),
            "SYN-L1".to_string(),
            "SYN-STMT".to_string(),
            ChallengeKind::AmountMismatch,
            "operator-fixture".to_string(),
        )
        .expect_err("empty queue linkage refused");
        assert_eq!(err.name(), "DISPUTE_EMPTY_REFERENCE");
    }

    #[test]
    fn bundle_entries_are_chain_linked_or_refused() {
        let entry = EvidenceBundleEntry::new(
            "ENT-1".to_string(),
            "EVI-1".to_string(),
            "tenant-fixture".to_string(),
            FIXTURE_DIGEST.to_string(),
        )
        .expect("valid chain-linked entry");
        assert_eq!(entry.chain_digest_hex, FIXTURE_DIGEST);
        // Not 64 characters -> refused.
        let err = EvidenceBundleEntry::new(
            "ENT-2".to_string(),
            "EVI-1".to_string(),
            "tenant-fixture".to_string(),
            "0123456789abcdef".to_string(),
        )
        .expect_err("short digest refused");
        assert_eq!(err.name(), "DISPUTE_INVALID_CHAIN_DIGEST_REF");
        // Non-hex characters -> refused.
        let err = EvidenceBundleEntry::new(
            "ENT-3".to_string(),
            "EVI-1".to_string(),
            "tenant-fixture".to_string(),
            "zz".to_string(),
        )
        .expect_err("non-hex digest refused");
        assert_eq!(err.name(), "DISPUTE_INVALID_CHAIN_DIGEST_REF");
    }

    #[test]
    fn bundles_are_immutable_records_with_deterministic_grouping() {
        let bundle = EvidenceBundle::assemble(
            "BND-1".to_string(),
            "CHL-1".to_string(),
            vec![
                EvidenceBundleEntry::new(
                    "ENT-1".to_string(),
                    "EVI-1".to_string(),
                    "tenant-fixture".to_string(),
                    FIXTURE_DIGEST.to_string(),
                )
                .expect("entry"),
                EvidenceBundleEntry::new(
                    "ENT-2".to_string(),
                    "CMD-REF-1".to_string(),
                    "operator-fixture".to_string(),
                    FIXTURE_DIGEST.to_string(),
                )
                .expect("entry"),
            ],
        )
        .expect("valid bundle");
        let grouped = bundle.entries_by_namespace();
        assert_eq!(grouped.len(), 2);
        assert!(grouped.contains_key("tenant-fixture"));
        assert!(grouped.contains_key("operator-fixture"));
        // An empty bundle is refused (evidence or it does not exist).
        let err = EvidenceBundle::assemble(
            "BND-2".to_string(),
            "CHL-1".to_string(),
            Vec::new(),
        )
        .expect_err("empty bundle refused");
        assert_eq!(err.name(), "DISPUTE_EMPTY_REFERENCE");
    }

    #[test]
    fn resolutions_require_the_chain_linked_bundle() {
        let resolution = DisputeResolutionRecord::new(
            "RES-1".to_string(),
            "CHL-1".to_string(),
            "BND-1".to_string(),
            ResolutionKind::AdjustedByCounterparty,
            "operator-fixture".to_string(),
            "SYNTHETIC fixture note; no real dispute exists".to_string(),
        )
        .expect("valid resolution");
        assert_eq!(resolution.kind.as_str(), "adjusted-by-counterparty");
        // A resolution without a bundle reference is refused.
        let err = DisputeResolutionRecord::new(
            "RES-2".to_string(),
            "CHL-1".to_string(),
            "  ".to_string(),
            ResolutionKind::Upheld,
            "operator-fixture".to_string(),
            String::new(),
        )
        .expect_err("bundle-less resolution refused");
        assert_eq!(err.name(), "DISPUTE_RESOLUTION_WITHOUT_BUNDLE");
    }

    #[test]
    fn state_labels_are_stable() {
        assert_eq!(DisputeState::Opened.as_str(), "opened");
        assert_eq!(DisputeState::EvidenceAssembled.as_str(), "evidence-assembled");
        assert_eq!(DisputeState::Submitted.as_str(), "submitted");
        assert_eq!(DisputeState::Resolved.as_str(), "resolved");
        assert_eq!(DisputeState::Withdrawn.as_str(), "withdrawn");
        assert_eq!(ResolutionKind::Upheld.as_str(), "upheld");
        assert_eq!(ResolutionKind::Rejected.as_str(), "rejected");
    }
}
