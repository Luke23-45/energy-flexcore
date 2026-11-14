// P8.05 — The independent validation harness (the validator that does not
//         trust the researcher). Ledger `ledger/phase-08-advanced-
//         coordination/ledger_phase_8.md` §P8.05; report trace §13.1–§13.3,
//         §19.2(5)(9), §12.5, §22.3; decision record:
//         `ledger/phase-08-advanced-coordination/05_06_harness_baselines.md`.
//
// WHAT THIS IS: the harness that checks every research module's output
// against shape, provenance, freshness, and PHYSICS — independently of the
// research code that produced it. The exit gate's "independently validated"
// exists to prevent exactly one failure mode: the researcher validating
// their own output (ledger P8.05 risk). The design rules that follow from
// that:
//
// 1. THE HARNESS NEVER RE-IMPLEMENTS PHYSICS. The physical re-validation
//    stage consumes the platform's EXISTING independent gates as an
//    injected port ([`IndependentFeasibilityGate`]), whose verdict type
//    MIRRORS the `FeasibilityGateVerdict` surface of
//    `crates/flexcore-application/src/command_gates.rs` (P4.12, §12.5) by
//    citation — Passed / LimitViolated / FailClosed, with the model hash
//    traveling on the pass. This crate does not depend on
//    flexcore-application (composition placement, see `Cargo.toml`); the
//    mirror is recorded as the alignment seam, the same discipline the
//    kernel/host vocabulary mirror uses (P7.10/11 §6).
// 2. THE RESEARCHER SUPPLIES NO VERDICT. The proposal type
//    ([`ResearchProposal`]) has NO field a research module could use to
//    assert its own feasibility, optimality, or safety: diagnostics are
//    prose for humans, never gate input. A proposal whose diagnostics
//    claim "feasible" is still re-validated by the gate like any other.
// 3. THE GATE CANNOT BE BYPASSED. [`ValidatedProposal`] carries a private
//    seal field; the ONLY public path that returns one is
//    [`ValidationHarness::validate`], which refuses on ANY failed stage
//    and never forwards the proposal on refusal. There is no "validate
//    later", no "provisional pass", no API that marks a proposal validated
//    without the injected gate saying so.
// 4. FIXED STAGE ORDER, ONE NAMED REFUSAL PER FAILURE CLASS (the P6.15/
//    P7.36 named-refusal discipline, cited): (1) envelope shape →
//    (2) closed result vocabulary (§13.4) → (3) provenance completeness
//    (§9.2 + seed honesty + the P8.32 artifact seam) → (4) validity window
//    against the INJECTED clock (the P8.34 seam) → (5) input-snapshot
//    freshness → (6) physical re-validation LAST (a proposal refused by
//    any earlier stage never reaches the gate — cheap refusals first, and
//    a stale/malformed proposal never consumes a feasibility attempt).
//    Fail-closed everywhere: a seam error is a refusal, never a pass
//    (the command-gates seam convention, P4.11–P4.13).
// 5. SEPARATE OWNERSHIP (P8.05 step 3): this module lives outside every
//    research worker (Python, `python/workers/`), is reviewed by someone
//    who did not write the modules it validates, and stamps EVERY
//    validation result with the harness version ([`HARNESS_VERSION`] —
//    P8.05 step 4). The Python research worker cannot import this crate:
//    the ADR-004 process boundary IS the no-self-validation guarantee.
//
// P8.32 SEAM (recorded, not implemented here): provenance completeness
// requires a model-artifact reference, but artifact SIGNING and
// verification-at-load do not exist yet — P8.32 owns them. When they
// land, the signing check composes into stage (3) as an additional
// injected port; the proposal's `model_artifact_ref` field is the field
// the signature will bind to.
//
// P8.34 SEAM (recorded, not implemented here): stage (4) enforces the
// validity window and snapshot freshness NOW (window inverted → refusal;
// not-yet-valid / expired against the injected clock → refusal;
// superseded/unknown snapshot → refusal). P8.34 extends the same checks to
// the wire contract and the gate-at-time-of-use discipline; the harness
// side does not change shape when that lands.
//
// Control status: research proposals are DATA. Nothing here authorizes,
// issues, approves, or dispatches anything (ADR-010). An accepted proposal
// enters the protected validation → feasibility → authorization path as a
// candidate; it gains NO authority from having been validated (§13.4).
// Canonical units MW/MVAr/MWh; `P > 0` = export at the point of
// connection; SOC a fraction in [0, 1] (§10.2). Determinism: fixed stage
// order, sorted field iteration, injected clock only — `SystemTime::now()`
// appears nowhere in this module.

use serde::Serialize;

/// The harness version stamped on every validation result (P8.05 step 4).
/// A harness change is a NEW version (the P8.06 frozen-version discipline,
/// applied to the harness itself): never a silent edit of what "validated"
/// meant.
pub const HARNESS_VERSION: &str = "flexcore-research/validation.v1";

/// The recorded harness statement (the repo's statement pattern — pinned by
/// tests so what IS and IS NOT claimed stays reviewable).
pub const VALIDATION_STATEMENT: &str = "the independent validation harness consumes \
     research proposals as DATA and returns either a named refusal or a validated \
     proposal; it never re-implements physics (physical re-validation runs through the \
     injected platform feasibility gate), never accepts a researcher-supplied verdict \
     (the proposal type has no field for one), and never returns a validated proposal \
     without the gate (the constructor is crate-private). A validated proposal is a \
     candidate for the protected validation → feasibility → authorization path and \
     gains NO authority from having been proposed or validated (ADR-010, §13.4). No \
     safety, certification, or field-authority claim is made.";

// ---------------------------------------------------------------------------
// The proposal (the kernel proposal-envelope shape, mirrored by citation)
// ---------------------------------------------------------------------------

/// The kernel alignment seam (recorded — do not drift silently): this crate
/// MIRRORS the shape of `crates/flexcore-kernel/src/plugin_path_protection.rs::
/// PluginProposalEnvelope` (proposal id, producer plugin id + version,
/// payload, sequence — ADR-010's typed candidate) and EXTENDS it with the
/// research fields the ledger asks for (result kind, §9.2 provenance, the
/// validity window). The kernel is a composition crate and this crate does
/// not depend on it (the P7.10/11 kernel-vocabulary-mirror discipline,
/// §6: if the kernel shape changes, the mirror update is deliberate,
/// fail-closed in both directions until then — pinned by tests here).
pub const KERNEL_PROPOSAL_ENVELOPE_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/plugin_path_protection.rs::PluginProposalEnvelope \
     (proposal_id, producer_plugin_id, producer_plugin_version, payload, sequence) \
     extended with result_kind, provenance, and validity; kernel unchanged";

/// The closed §13.4 result vocabulary of a research output (report §13.4,
/// PLAN.md §4 "Proposal (all modules)"). A research worker returns one of
/// EXACTLY these kinds — never a dispatch decision, never an authorization
/// artifact. The vocabulary is CLOSED: `parse` accepts nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchResultKind {
    /// A proposed schedule (time-indexed setpoints).
    ProposedSchedule,
    /// A candidate setpoint vector (one operating point).
    CandidateSetpointVector,
    /// A ranking/score (e.g. constraint screening, contingency ranking).
    Ranking,
    /// A forecast distribution.
    ForecastDistribution,
    /// An uncertainty estimate.
    UncertaintyEstimate,
    /// Diagnostics only (residuals, termination evidence, scores).
    Diagnostics,
}

impl ResearchResultKind {
    /// The exact §13.4 kind string (the on-the-wire and in-memory
    /// vocabularies are identical — the §12.6 `SolveStatus::as_str`
    /// discipline, applied here).
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchResultKind::ProposedSchedule => "proposed_schedule",
            ResearchResultKind::CandidateSetpointVector => "candidate_setpoint_vector",
            ResearchResultKind::Ranking => "ranking",
            ResearchResultKind::ForecastDistribution => "forecast_distribution",
            ResearchResultKind::UncertaintyEstimate => "uncertainty_estimate",
            ResearchResultKind::Diagnostics => "diagnostics",
        }
    }

    /// Parses the exact kind string. There is no lossy parsing: any other
    /// string is `None`, because the vocabulary is closed.
    pub fn parse(text: &str) -> Option<ResearchResultKind> {
        let kind = match text {
            "proposed_schedule" => ResearchResultKind::ProposedSchedule,
            "candidate_setpoint_vector" => ResearchResultKind::CandidateSetpointVector,
            "ranking" => ResearchResultKind::Ranking,
            "forecast_distribution" => ResearchResultKind::ForecastDistribution,
            "uncertainty_estimate" => ResearchResultKind::UncertaintyEstimate,
            "diagnostics" => ResearchResultKind::Diagnostics,
            _ => return None,
        };
        Some(kind)
    }
}

/// The §9.2 provenance + reproducibility block a research proposal carries
/// (mirrors `worker_api.proto::ResultProvenance` — worker/model/solver
/// version, input snapshot id, termination status, diagnostics — plus the
/// P7.42 determinism fields and the P8.32 model-artifact seam).
///
/// DESIGN NOTE (ADR-010): there is deliberately NO feasibility, optimality,
/// safety, or authorization field on this type OR on the proposal. A
/// research module cannot assert its own verdict because no field exists
/// that could carry one; `diagnostics` is human-facing prose that the
/// harness never interprets as a verdict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResearchProvenance {
    /// The worker implementation version (proto `ResultProvenance.worker_version`).
    pub worker_version: String,
    /// The domain model version (proto `ResultProvenance.model_version`).
    pub model_version: String,
    /// The solver/algorithm version (proto `ResultProvenance.solver_version`).
    pub solver_version: String,
    /// The input snapshot id the proposal was computed against
    /// (proto `ResultProvenance.input_snapshot_id`; §9.2).
    pub input_snapshot_id: String,
    /// Termination status from the worker's own vocabulary
    /// (proto `ResultProvenance.termination_status`; "ok" alone is not
    /// sufficient — the proto's own note).
    pub termination_status: String,
    /// Human-readable diagnostics (§9.2: warnings, unmodelled constraints,
    /// assumptions). NEVER interpreted by the harness as a verdict.
    pub diagnostics: Vec<String>,
    /// The PRNG seed the worker ran under (proto `DeterminismMetadata.seed`);
    /// `None` is legal only when `seed_supported` is `false` (the honest
    /// admission, P7.42).
    pub seed: Option<u64>,
    /// Whether the worker's algorithm supports seeding at all
    /// (proto `DeterminismMetadata.seed_supported`).
    pub seed_supported: bool,
    /// Whether the run was deterministic (proto
    /// `DeterminismMetadata.deterministic`). Recorded on the report; `false`
    /// is an honest admission, not a refusal by itself.
    pub deterministic: bool,
    /// The model-artifact reference (P8.32 seam): the artifact whose
    /// signature P8.32 will verify at load. REQUIRED for ML/RL outputs —
    /// required for every research output here, because the completeness
    /// stage cannot know which module produced the proposal and must not
    /// guess (fail-closed).
    pub model_artifact_ref: String,
}

/// The validity window of a research proposal, in INJECTED-CLOCK
/// milliseconds (§14.3 field 15 by analogy; the P8.34 seam — mirrors the
/// kernel `PluginRequestEnvelope` window semantics: executable only while
/// `valid_from_ms <= now < valid_until_ms`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResearchValidityWindow {
    /// Window opens (inclusive), UTC millis.
    pub valid_from_ms: i64,
    /// Window closes (exclusive), UTC millis.
    pub valid_until_ms: i64,
}

/// A research proposal: the typed CANDIDATE every research module's output
/// is recorded as (the kernel proposal-envelope shape, mirrored — see
/// [`KERNEL_PROPOSAL_ENVELOPE_ALIGNMENT_SEAM`] — plus the research
/// extensions). A proposal is INPUT to the harness and, if validated, to
/// the protected platform path; it authorizes nothing (ADR-010).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResearchProposal {
    /// The proposal id (unique within the producer's proposal stream; the
    /// kernel `PluginProposalEnvelope.proposal_id` discipline).
    pub proposal_id: String,
    /// The producing plugin's id (provenance).
    pub producer_plugin_id: String,
    /// The producing plugin's version (provenance; §9.2).
    pub producer_plugin_version: String,
    /// The producer's monotonic sequence (the kernel discipline: starts at
    /// 1; sequence 0 is unsequenced).
    pub sequence: u64,
    /// The closed §13.4 result kind of `payload`.
    pub result_kind: ResearchResultKind,
    /// The proposal payload (opaque JSON here; the payload schema is the
    /// per-`payload_type` agreement of the worker contract — this harness
    /// checks shape/size, never interprets domain semantics).
    pub payload: serde_json::Value,
    /// The §9.2 + P8.32 provenance block (completeness-checked by the
    /// harness; never carrying a researcher verdict).
    pub provenance: ResearchProvenance,
    /// The validity window (checked against the injected clock).
    pub validity: ResearchValidityWindow,
}

/// Fail-closed payload cap on one proposal (mirrors the kernel
/// `MAX_REQUEST_PAYLOAD_BYTES` posture: an unbounded payload is a
/// denial-of-service vector before any host limit could apply). Same value
/// as the kernel's cap — the alignment seam covers limits too.
pub const MAX_PROPOSAL_PAYLOAD_BYTES: usize = 1024 * 1024;

impl ResearchProposal {
    /// Constructs a proposal, enforcing the SHAPE half of stage (1)/(2) at
    /// construction (the kernel `PluginProposalEnvelope::new` discipline:
    /// self-validity refused early, with named reasons). The full stage
    /// pipeline still re-checks everything — construction validation is a
    /// fast-fail convenience, never the gate.
    pub fn new(
        proposal_id: &str,
        producer_plugin_id: &str,
        producer_plugin_version: &str,
        sequence: u64,
        result_kind: ResearchResultKind,
        payload: serde_json::Value,
        provenance: ResearchProvenance,
        validity: ResearchValidityWindow,
    ) -> Result<ResearchProposal, Refusal> {
        let proposal = ResearchProposal {
            proposal_id: proposal_id.to_string(),
            producer_plugin_id: producer_plugin_id.to_string(),
            producer_plugin_version: producer_plugin_version.to_string(),
            sequence,
            result_kind,
            payload,
            provenance,
            validity,
        };
        check_envelope_shape(&proposal)?;
        check_result_vocabulary(proposal.result_kind)?;
        Ok(proposal)
    }
}

// ---------------------------------------------------------------------------
// The injected ports (the composition seams)
// ---------------------------------------------------------------------------

/// The physical re-validation verdict, MIRRORING the
/// `FeasibilityGateVerdict` surface of
/// `crates/flexcore-application/src/command_gates.rs` (P4.12; §12.5) by
/// citation: Passed (with the model hash the evidence binds to) /
/// LimitViolated (constraint named) / FailClosed (reason named). The
/// alignment seam is recorded; this crate does not depend on
/// flexcore-application (see `Cargo.toml`).
pub const FEASIBILITY_VERDICT_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-application/src/command_gates.rs::FeasibilityGateVerdict \
     (Passed{attempt_id, model_id, model_content_hash, margins, coverage} / \
     LimitViolated{...} / FailClosed{...}); the composition root implements the \
     port over the real gate";

/// The §12.5 network feasibility runner seam (P4.12 step 1), as the harness
/// consumes it: the composition root implements this port over the
/// platform's EXISTING feasibility pipeline. The harness NEVER re-implements
/// the physics — power flow, capability, SOC/ramp/duration, and command
/// invariants (§14.3) are the gate's obligation; the harness's obligation is
/// to REFUSE anything the gate does not pass, fail-closed on gate errors.
///
/// Fail-closed convention (the command-gates seam rule): an `Err` from this
/// port is a REFUSAL, never a pass.
pub trait IndependentFeasibilityGate {
    /// Re-validates the proposal physically. Called ONLY after stages 1–5
    /// passed (a malformed or stale proposal never consumes a gate
    /// attempt).
    fn revalidate(&self, proposal: &ResearchProposal) -> Result<IndependentVerdict, GateSeamError>;
}

/// The physical verdict (see [`FEASIBILITY_VERDICT_ALIGNMENT_SEAM`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum IndependentVerdict {
    /// The proposal passed the physical re-validation within declared scope
    /// and tolerances; the model hash travels (the P4.12 evidence binding).
    Passed {
        /// The attempt identifier (the gate's own evidence id).
        attempt_id: String,
        /// The model identifier the check evaluated against.
        model_id: String,
        /// THE MODEL HASH the evidence binds to (a model change invalidates
        /// the evidence).
        model_content_hash: String,
        /// Margin/diagnostics summary (§12.5 step 7).
        margins: String,
        /// Coverage statement/limitations (§12.5 step 6).
        coverage: String,
    },
    /// A network limit is violated; `constraint` names it.
    LimitViolated {
        /// The attempt identifier.
        attempt_id: String,
        /// The model identifier.
        model_id: String,
        /// The model hash.
        model_content_hash: String,
        /// The violated constraint (named, never a bare boolean).
        constraint: String,
    },
    /// Validator/model/infrastructure failure — fail-closed (§12.5 closing
    /// rule; §17.5 invariant 9).
    FailClosed {
        /// The attempt identifier, when the gate got far enough to record
        /// one.
        attempt_id: Option<String>,
        /// The fail-closed reason.
        reason: String,
    },
}

/// The freshness of the input snapshot a proposal was computed against
/// (the P8.34 seam: a proposal computed against old conditions cannot look
/// current). Mirrors the `ModelVersionGateStatus` posture of the command
/// gates (P4.03) — Active / NotActive / Unknown — adapted to snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SnapshotFreshness {
    /// The snapshot is the current one.
    Current,
    /// A newer snapshot replaced it; the replacing snapshot id travels.
    Superseded { by_snapshot_id: String },
    /// The snapshot is unknown in scope (unknown is never fresh — the
    /// P4.11 "unknown is never usable" rule).
    Unknown,
}

/// The snapshot-freshness source (injected; the composition root
/// implements it over the platform's snapshot registry).
pub trait SnapshotFreshnessSource {
    fn freshness(&self, snapshot_id: &str) -> SnapshotFreshness;
}

/// The harness clock (injected; every window comparison reads THIS clock —
/// `SystemTime::now()` appears nowhere in this module; the P7.36
/// injected-clock discipline, cited).
pub trait HarnessClock {
    /// The current instant in UTC millis.
    fn now_millis(&self) -> i64;
}

/// A non-verdict seam failure (fail-closed: it becomes a refusal, never a
/// pass). Mirrors `GateSeamError` of the command gates by citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateSeamError {
    /// Which seam failed (`feasibility_gate`, `snapshot_source`, `clock`).
    pub seam: &'static str,
    /// The recorded failure detail.
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Refusals — one named refusal per failure class
// ---------------------------------------------------------------------------

/// Every harness refusal is a DISTINCT named refusal
/// ([`Refusal::name`]) — never a warning, never a silent acceptance, never
/// one combined "valid" boolean (the P6.15 named-refusal discipline,
/// cited; the kernel `PluginPathError::name` pattern, mirrored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Refusal {
    /// Stage 1: the proposal envelope is malformed (empty or oversized id,
    /// empty producer identity, sequence 0, oversized payload).
    ProposalEnvelopeInvalid { reason: String },
    /// Stage 2: the result kind is not in the closed §13.4 vocabulary.
    ResultVocabularyUnknown { attempted: String },
    /// Stage 3: a required provenance field is missing/empty; `field`
    /// names it.
    ProvenanceIncomplete { field: &'static str },
    /// Stage 3: a seed is required but the worker claims seed support
    /// (the P7.42 honesty rule: no seed with `seed_supported = true` is a
    /// reproducibility lie).
    SeedPolicyInconsistent,
    /// Stage 3: the termination status is the bare "ok" the proto forbids
    /// ("ok" alone is not sufficient — `worker_api.proto`
    /// `ResultProvenance.termination_status` note).
    TerminationStatusInsufficient,
    /// Stage 4: the window has not opened yet (injected clock).
    ProposalNotYetValid { valid_from_ms: i64 },
    /// Stage 4: the window has closed — expired proposals are refused,
    /// never forwarded (the kernel `RequestExpired` posture; P8.34).
    ProposalExpired { valid_until_ms: i64 },
    /// Stage 5: the input snapshot was superseded — the proposal was
    /// computed against old conditions (P8.34).
    SnapshotSuperseded { by_snapshot_id: String },
    /// Stage 5: the input snapshot is unknown in scope (never fresh).
    SnapshotUnknown,
    /// Stage 6: physical re-validation found a violated limit; the
    /// constraint is named (the gate's verdict travels verbatim).
    PhysicalRevalidationLimitViolated { constraint: String },
    /// Stage 6: physical re-validation failed closed; the reason is named
    /// (§12.5 closing rule; §17.5 invariant 9).
    PhysicalRevalidationFailClosed { reason: String },
    /// Stage 6: the injected feasibility gate itself failed (seam error) —
    /// fail-closed, never a pass.
    PhysicalRevalidationSeamFailed { seam: &'static str, detail: String },
}

impl Refusal {
    /// The stable machine-readable refusal name (the named-refusal table
    /// key; pinned by tests).
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::ProposalEnvelopeInvalid { .. } => "PROPOSAL_ENVELOPE_INVALID",
            Refusal::ResultVocabularyUnknown { .. } => "RESULT_VOCABULARY_UNKNOWN",
            Refusal::ProvenanceIncomplete { .. } => "PROVENANCE_INCOMPLETE",
            Refusal::SeedPolicyInconsistent => "SEED_POLICY_INCONSISTENT",
            Refusal::TerminationStatusInsufficient => "TERMINATION_STATUS_INSUFFICIENT",
            Refusal::ProposalNotYetValid { .. } => "PROPOSAL_NOT_YET_VALID",
            Refusal::ProposalExpired { .. } => "PROPOSAL_EXPIRED",
            Refusal::SnapshotSuperseded { .. } => "SNAPSHOT_SUPERSEDED",
            Refusal::SnapshotUnknown => "SNAPSHOT_UNKNOWN",
            Refusal::PhysicalRevalidationLimitViolated { .. } => {
                "PHYSICAL_REVALIDATION_LIMIT_VIOLATED"
            }
            Refusal::PhysicalRevalidationFailClosed { .. } => {
                "PHYSICAL_REVALIDATION_FAIL_CLOSED"
            }
            Refusal::PhysicalRevalidationSeamFailed { .. } => {
                "PHYSICAL_REVALIDATION_SEAM_FAILED"
            }
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::ProposalEnvelopeInvalid { reason } => {
                write!(f, "the research proposal envelope is invalid: {reason}")
            }
            Refusal::ResultVocabularyUnknown { attempted } => write!(
                f,
                "result kind {attempted:?} is not in the closed §13.4 vocabulary; refused"
            ),
            Refusal::ProvenanceIncomplete { field } => {
                write!(f, "provenance field '{field}' is missing or empty; refused")
            }
            Refusal::SeedPolicyInconsistent => write!(
                f, "no seed recorded while the worker claims seed support; refused (P7.42 \
                     honesty rule — reproducibility metadata must not lie)"
            ),
            Refusal::TerminationStatusInsufficient => write!(
                f, "termination status is the bare \"ok\" the contract forbids; refused"
            ),
            Refusal::ProposalNotYetValid { valid_from_ms } => write!(
                f, "proposal validity window opens at {valid_from_ms} (injected clock); \
                     refused until then"
            ),
            Refusal::ProposalExpired { valid_until_ms } => write!(
                f, "proposal expired at {valid_until_ms} (injected clock); refused, never \
                     forwarded"
            ),
            Refusal::SnapshotSuperseded { by_snapshot_id } => write!(
                f, "the input snapshot was superseded by {by_snapshot_id}; the proposal was \
                     computed against old conditions; refused"
            ),
            Refusal::SnapshotUnknown => write!(
                f, "the input snapshot is unknown in scope; unknown is never fresh; refused"
            ),
            Refusal::PhysicalRevalidationLimitViolated { constraint } => write!(
                f, "physical re-validation found a violated limit: {constraint}; refused"
            ),
            Refusal::PhysicalRevalidationFailClosed { reason } => write!(
                f, "physical re-validation failed closed: {reason}; refused"
            ),
            Refusal::PhysicalRevalidationSeamFailed { seam, detail } => write!(
                f, "the feasibility seam '{seam}' failed: {detail}; fail-closed, refused"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

// ---------------------------------------------------------------------------
// Stage checks (each a named refusal; the first failure wins)
// ---------------------------------------------------------------------------

/// Hard identity cap mirroring the kernel's 256-character id bounds.
const MAX_ID_CHARS: usize = 256;

/// Stage 1 — envelope self-validity (the kernel `check_self` discipline,
/// mirrored).
fn check_envelope_shape(proposal: &ResearchProposal) -> Result<(), Refusal> {
    if proposal.proposal_id.trim().is_empty() || proposal.proposal_id.len() > MAX_ID_CHARS {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the proposal id is empty or longer than 256 characters".to_string(),
        });
    }
    if proposal.producer_plugin_id.trim().is_empty() {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the producer plugin id is empty".to_string(),
        });
    }
    if proposal.producer_plugin_version.trim().is_empty() {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the producer plugin version is empty".to_string(),
        });
    }
    if proposal.sequence == 0 {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the proposal sequence is 0 (sequences start at 1)".to_string(),
        });
    }
    if proposal.validity.valid_from_ms >= proposal.validity.valid_until_ms {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the validity window is empty or inverted (valid_from must be strictly \
                     before valid_until)"
                .to_string(),
        });
    }
    if proposal.payload.to_string().len() > MAX_PROPOSAL_PAYLOAD_BYTES {
        return Err(Refusal::ProposalEnvelopeInvalid {
            reason: "the proposal payload exceeds the payload cap".to_string(),
        });
    }
    Ok(())
}

/// Stage 2 — the closed §13.4 result vocabulary.
fn check_result_vocabulary(kind: ResearchResultKind) -> Result<(), Refusal> {
    // The enum is the vocabulary; a constructed kind is always in it. The
    // string round-trip is re-checked so the wire vocabulary cannot drift
    // from the enum (the §12.6 as_str/parse discipline).
    let text = kind.as_str();
    if ResearchResultKind::parse(text) != Some(kind) {
        return Err(Refusal::ResultVocabularyUnknown {
            attempted: text.to_string(),
        });
    }
    Ok(())
}

/// Stage 3 — provenance completeness (§9.2 + seed honesty + the P8.32
/// artifact seam). Every required field is named on failure; silence is
/// never legal (the §12.6 completeness rule).
fn check_provenance(provenance: &ResearchProvenance) -> Result<(), Refusal> {
    for (field, value) in [
        ("worker_version", provenance.worker_version.as_str()),
        ("model_version", provenance.model_version.as_str()),
        ("solver_version", provenance.solver_version.as_str()),
        ("input_snapshot_id", provenance.input_snapshot_id.as_str()),
        ("model_artifact_ref", provenance.model_artifact_ref.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(Refusal::ProvenanceIncomplete { field });
        }
    }
    if provenance.termination_status.trim().is_empty() {
        return Err(Refusal::ProvenanceIncomplete {
            field: "termination_status",
        });
    }
    if provenance.termination_status.trim() == "ok" {
        return Err(Refusal::TerminationStatusInsufficient);
    }
    // Seed honesty (P7.42): a worker that supports seeding must record the
    // seed it ran under; a worker without seeding must admit
    // `seed_supported = false`. No other combination is a lie — both
    // honest shapes pass.
    if provenance.seed.is_none() && provenance.seed_supported {
        return Err(Refusal::SeedPolicyInconsistent);
    }
    Ok(())
}

/// Stage 4 — the validity window against the INJECTED clock (the P8.34
/// semantics: `valid_from <= now < valid_until`; expired is refused, never
/// forwarded).
fn check_window(proposal: &ResearchProposal, now_millis: i64) -> Result<(), Refusal> {
    if now_millis < proposal.validity.valid_from_ms {
        return Err(Refusal::ProposalNotYetValid {
            valid_from_ms: proposal.validity.valid_from_ms,
        });
    }
    if now_millis >= proposal.validity.valid_until_ms {
        return Err(Refusal::ProposalExpired {
            valid_until_ms: proposal.validity.valid_until_ms,
        });
    }
    Ok(())
}

/// Stage 5 — snapshot freshness (unknown is never fresh; superseded is
/// refused with the replacing id named).
fn check_snapshot(
    snapshots: &dyn SnapshotFreshnessSource,
    snapshot_id: &str,
) -> Result<(), Refusal> {
    match snapshots.freshness(snapshot_id) {
        SnapshotFreshness::Current => Ok(()),
        SnapshotFreshness::Superseded { by_snapshot_id } => {
            Err(Refusal::SnapshotSuperseded { by_snapshot_id })
        }
        SnapshotFreshness::Unknown => Err(Refusal::SnapshotUnknown),
    }
}

// ---------------------------------------------------------------------------
// The validated proposal and the harness
// ---------------------------------------------------------------------------

/// A proposal that passed EVERY stage. THE BYPASS PROOF (P8.05 acceptance:
/// "no API returns a validated proposal without the gate"): this type
/// carries a PRIVATE seal field, so external code (including tests) cannot
/// construct it with a struct literal — the ONLY value ever produced is
/// the one [`ValidationHarness::validate`] returns after the injected gate
/// passed. There is no other public constructor, no `Default`, no
/// deserialization path (`serde::Deserialize` is deliberately NOT derived
/// here while `Serialize` is — a validated proposal cannot be smuggled in
/// from JSON).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidatedProposal {
    /// The proposal id that was validated.
    pub proposal_id: String,
    /// The §13.4 result kind of the validated payload.
    pub result_kind: ResearchResultKind,
    /// The validated payload (data for the protected path — a candidate,
    /// never an authority; ADR-010).
    pub payload: serde_json::Value,
    /// THE MODEL HASH the feasibility evidence binds to (the P4.12
    /// evidence-binding hook: a model change invalidates the evidence).
    pub model_content_hash: String,
    /// The feasibility evidence reference, in the recorded
    /// `feasibility:attempt/<id>:model/<mid>:hash/<hash>` format of the
    /// command gates (P4.12).
    pub evidence_reference: String,
    /// The harness version that validated this proposal (P8.05 step 4).
    pub harness_version: String,
    /// Whether the producing worker ran deterministically (recorded from
    /// the proposal's provenance; replay comparison keys on it — P7.42).
    pub producer_deterministic: bool,
    /// The private seal: no struct-literal construction outside this
    /// crate, therefore no validated proposal without the gate.
    seal: (),
}

/// The report of one validation run (the P8.05 step 4 record: the harness
/// version travels with EVERY validation result — refusal or pass).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidationReport {
    /// The harness version that produced this report.
    pub harness_version: String,
    /// The proposal id (echoed; never the payload — reports are log-safe).
    pub proposal_id: String,
    /// The §13.4 result kind.
    pub result_kind: ResearchResultKind,
    /// The stages that PASSED, in fixed order (the first failing stage
    /// ends the run; the refusal names the class).
    pub stages_passed: Vec<&'static str>,
    /// The physical verdict on a full pass (the gate's evidence travels).
    pub verdict: Option<IndependentVerdict>,
    /// The validated proposal on a full pass.
    pub validated: Option<ValidatedProposal>,
}

impl ValidationReport {
    /// True when the run produced a validated proposal.
    pub fn is_validated(&self) -> bool {
        self.validated.is_some()
    }
}

/// The stage names in the FIXED evaluation order (pinned by tests — the
/// order is the contract: cheap refusals first, the gate LAST, so a
/// malformed or stale proposal never consumes a feasibility attempt).
pub const STAGE_ORDER: [&str; 6] = [
    "envelope_shape",
    "result_vocabulary",
    "provenance_completeness",
    "validity_window",
    "snapshot_freshness",
    "physical_revalidation",
];

/// The independent validation harness: one struct per composition; all
/// collaborators injected. It consumes results and NEVER researcher-
/// supplied verdicts (there is no input channel for one — see
/// [`ResearchProvenance`]).
pub struct ValidationHarness<'a> {
    /// The injected §12.5 physical re-validation gate (the platform's
    /// existing feasibility pipeline; the harness never re-implements it).
    pub gate: &'a dyn IndependentFeasibilityGate,
    /// The injected snapshot-freshness source.
    pub snapshots: &'a dyn SnapshotFreshnessSource,
    /// The injected clock (every window comparison).
    pub clock: &'a dyn HarnessClock,
}

impl<'a> ValidationHarness<'a> {
    /// Constructs the harness over its injected collaborators.
    pub fn new(
        gate: &'a dyn IndependentFeasibilityGate,
        snapshots: &'a dyn SnapshotFreshnessSource,
        clock: &'a dyn HarnessClock,
    ) -> ValidationHarness<'a> {
        ValidationHarness {
            gate,
            snapshots,
            clock,
        }
    }

    /// Runs the FIXED stage pipeline. Returns the report on a full pass or
    /// the named [`Refusal`] of the FIRST failing stage. A refusal is
    /// terminal for this proposal: nothing is forwarded (P8.05: a proposal
    /// failing ANY stage → named refusal, never forwarded).
    pub fn validate(&self, proposal: &ResearchProposal) -> Result<ValidationReport, Refusal> {
        let mut stages_passed: Vec<&'static str> = Vec::with_capacity(STAGE_ORDER.len());

        // Stage 1 — envelope shape.
        check_envelope_shape(proposal)?;
        stages_passed.push(STAGE_ORDER[0]);

        // Stage 2 — the closed §13.4 result vocabulary.
        check_result_vocabulary(proposal.result_kind)?;
        stages_passed.push(STAGE_ORDER[1]);

        // Stage 3 — provenance completeness (§9.2 + seed honesty + P8.32
        // artifact reference).
        check_provenance(&proposal.provenance)?;
        stages_passed.push(STAGE_ORDER[2]);

        // Stage 4 — validity window against the injected clock.
        let now = self.clock.now_millis();
        check_window(proposal, now)?;
        stages_passed.push(STAGE_ORDER[3]);

        // Stage 5 — snapshot freshness.
        check_snapshot(self.snapshots, &proposal.provenance.input_snapshot_id)?;
        stages_passed.push(STAGE_ORDER[4]);

        // Stage 6 — PHYSICAL RE-VALIDATION through the injected gate,
        // LAST. Fail-closed on every non-Passed verdict and on every seam
        // error. The researcher's diagnostics are never consulted here.
        let verdict = self
            .gate
            .revalidate(proposal)
            .map_err(|error| Refusal::PhysicalRevalidationSeamFailed {
                seam: error.seam,
                detail: error.detail,
            })?;
        let validated = match verdict {
            IndependentVerdict::Passed {
                attempt_id,
                model_id,
                model_content_hash,
                margins: _,
                coverage: _,
            } => {
                let reference = format!(
                    "feasibility:attempt/{attempt_id}:model/{model_id}:hash/{model_content_hash}"
                );
                ValidatedProposal {
                    proposal_id: proposal.proposal_id.clone(),
                    result_kind: proposal.result_kind,
                    payload: proposal.payload.clone(),
                    model_content_hash,
                    evidence_reference: reference,
                    harness_version: HARNESS_VERSION,
                    producer_deterministic: proposal.provenance.deterministic,
                    seal: (),
                }
            }
            IndependentVerdict::LimitViolated { constraint, .. } => {
                return Err(Refusal::PhysicalRevalidationLimitViolated { constraint });
            }
            IndependentVerdict::FailClosed { reason, .. } => {
                return Err(Refusal::PhysicalRevalidationFailClosed { reason });
            }
        };
        stages_passed.push(STAGE_ORDER[5]);

        Ok(ValidationReport {
            harness_version: HARNESS_VERSION,
            proposal_id: proposal.proposal_id.clone(),
            result_kind: proposal.result_kind,
            stages_passed,
            verdict: Some(verdict),
            validated: Some(validated),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is either
// hand-computed from the rules above or a literal copied from this module's
// own constants — nothing is fabricated).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A fixed injected clock: the test supplies the instant; `now` never
    /// comes from a wall clock (the injected-clock discipline, tested by
    /// construction — no `SystemTime` appears anywhere in this module).
    struct FixedClock {
        now: i64,
    }

    impl HarnessClock for FixedClock {
        fn now_millis(&self) -> i64 {
            self.now
        }
    }

    /// A fixed snapshot source answering one freshness for every id.
    struct FixedSnapshots {
        fresh: SnapshotFreshness,
    }

    impl SnapshotFreshnessSource for FixedSnapshots {
        fn freshness(&self, _snapshot_id: &str) -> SnapshotFreshness {
            self.fresh.clone()
        }
    }

    /// A gate spy: returns the programmed verdict AND counts how many
    /// times the physical gate was entered. The cheap-refusals-first
    /// property is pinned by asserting `calls == 0` after any earlier
    /// stage refused.
    struct GateSpy {
        verdict: Result<IndependentVerdict, GateSeamError>,
        calls: Cell<usize>,
    }

    impl GateSpy {
        fn passing() -> GateSpy {
            GateSpy {
                verdict: Ok(IndependentVerdict::Passed {
                    attempt_id: "AT-1".to_string(),
                    model_id: "ieee13".to_string(),
                    model_content_hash: "abc123".to_string(),
                    margins: "no violations".to_string(),
                    coverage: "declared scope only".to_string(),
                }),
                calls: Cell::new(0),
            }
        }

        fn with(verdict: Result<IndependentVerdict, GateSeamError>) -> GateSpy {
            GateSpy {
                verdict,
                calls: Cell::new(0),
            }
        }
    }

    impl IndependentFeasibilityGate for GateSpy {
        fn revalidate(
            &self,
            _proposal: &ResearchProposal,
        ) -> Result<IndependentVerdict, GateSeamError> {
            self.calls.set(self.calls.get() + 1);
            self.verdict.clone()
        }
    }

    /// The reference provenance: every field filled, honest seed pair
    /// (Some(7) + seed_supported true), non-bare termination status.
    fn reference_provenance() -> ResearchProvenance {
        ResearchProvenance {
            worker_version: "research-worker/0.1.0".to_string(),
            model_version: "grid-model/2026.11".to_string(),
            solver_version: "highspy 1.15.1".to_string(),
            input_snapshot_id: "snapshot-2026-11-13-a".to_string(),
            termination_status: "converged".to_string(),
            diagnostics: Vec::new(),
            seed: Some(7),
            seed_supported: true,
            deterministic: true,
            model_artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
        }
    }

    /// The reference window: [1_000, 2_000) UTC millis — known-answer
    /// boundary values for the window tests.
    fn reference_window() -> ResearchValidityWindow {
        ResearchValidityWindow {
            valid_from_ms: 1_000,
            valid_until_ms: 2_000,
        }
    }

    /// A proposal that passes stages 1–3 by construction.
    fn reference_proposal() -> ResearchProposal {
        ResearchProposal::new(
            "prop-1",
            "plugin/research-admm",
            "0.1.0",
            1,
            ResearchResultKind::ProposedSchedule,
            serde_json::json!({ "setpoints": [1.5, 2.5] }),
            reference_provenance(),
            reference_window(),
        )
        .expect("the reference proposal is shape-valid")
    }

    fn harness<'a>(
        gate: &'a GateSpy,
        snapshots: &'a FixedSnapshots,
        clock: &'a FixedClock,
    ) -> ValidationHarness<'a> {
        ValidationHarness::new(gate, snapshots, clock)
    }

    /// The all-collaborators-passing fixture: gate Passed, snapshots
    /// Current, clock at `now`. Each test owns its collaborators (no
    /// shared state, no leaks).
    fn passing_fixture(now: i64) -> (GateSpy, FixedSnapshots, FixedClock) {
        (
            GateSpy::passing(),
            FixedSnapshots {
                fresh: SnapshotFreshness::Current,
            },
            FixedClock { now },
        )
    }

    // --- constants and vocabularies (known answers) -----------------------

    #[test]
    fn stage_order_is_the_fixed_contract() {
        assert_eq!(
            STAGE_ORDER,
            [
                "envelope_shape",
                "result_vocabulary",
                "provenance_completeness",
                "validity_window",
                "snapshot_freshness",
                "physical_revalidation",
            ]
        );
    }

    #[test]
    fn harness_and_statement_constants_are_the_recorded_strings() {
        assert_eq!(HARNESS_VERSION, "flexcore-research/validation.v1");
        // The crate version stamp lives at the crate root (lib.rs).
        assert_eq!(crate::CRATE_VERSION, "flexcore-research.v1");
        assert!(VALIDATION_STATEMENT.contains("never re-implements physics"));
        assert!(VALIDATION_STATEMENT.contains("NO authority"));
        assert!(VALIDATION_STATEMENT.contains("No safety, certification, or field-authority claim"));
    }

    #[test]
    fn result_kind_vocabulary_is_closed_and_round_trips() {
        let expected: [(&str, ResearchResultKind); 6] = [
            ("proposed_schedule", ResearchResultKind::ProposedSchedule),
            (
                "candidate_setpoint_vector",
                ResearchResultKind::CandidateSetpointVector,
            ),
            ("ranking", ResearchResultKind::Ranking),
            (
                "forecast_distribution",
                ResearchResultKind::ForecastDistribution,
            ),
            (
                "uncertainty_estimate",
                ResearchResultKind::UncertaintyEstimate,
            ),
            ("diagnostics", ResearchResultKind::Diagnostics),
        ];
        for (text, kind) in expected {
            assert_eq!(kind.as_str(), text);
            assert_eq!(ResearchResultKind::parse(text), Some(kind));
        }
        // The vocabulary is CLOSED: no dispatch/authorization word parses.
        for intruder in [
            "dispatch",
            "authorization_token",
            "approved_command",
            "executed",
            "",
        ] {
            assert_eq!(ResearchResultKind::parse(intruder), None);
        }
    }

    #[test]
    fn result_kind_serializes_snake_case() {
        let value = serde_json::to_value(ResearchResultKind::ProposedSchedule).unwrap();
        assert_eq!(value, serde_json::json!("proposed_schedule"));
        let value = serde_json::to_value(ResearchResultKind::UncertaintyEstimate).unwrap();
        assert_eq!(value, serde_json::json!("uncertainty_estimate"));
    }

    #[test]
    fn every_refusal_has_its_known_answer_name() {
        let cases: Vec<(Refusal, &'static str)> = vec![
            (
                Refusal::ProposalEnvelopeInvalid {
                    reason: "x".to_string(),
                },
                "PROPOSAL_ENVELOPE_INVALID",
            ),
            (
                Refusal::ResultVocabularyUnknown {
                    attempted: "x".to_string(),
                },
                "RESULT_VOCABULARY_UNKNOWN",
            ),
            (
                Refusal::ProvenanceIncomplete { field: "f" },
                "PROVENANCE_INCOMPLETE",
            ),
            (Refusal::SeedPolicyInconsistent, "SEED_POLICY_INCONSISTENT"),
            (
                Refusal::TerminationStatusInsufficient,
                "TERMINATION_STATUS_INSUFFICIENT",
            ),
            (
                Refusal::ProposalNotYetValid { valid_from_ms: 1 },
                "PROPOSAL_NOT_YET_VALID",
            ),
            (
                Refusal::ProposalExpired { valid_until_ms: 1 },
                "PROPOSAL_EXPIRED",
            ),
            (
                Refusal::SnapshotSuperseded {
                    by_snapshot_id: "s".to_string(),
                },
                "SNAPSHOT_SUPERSEDED",
            ),
            (Refusal::SnapshotUnknown, "SNAPSHOT_UNKNOWN"),
            (
                Refusal::PhysicalRevalidationLimitViolated {
                    constraint: "c".to_string(),
                },
                "PHYSICAL_REVALIDATION_LIMIT_VIOLATED",
            ),
            (
                Refusal::PhysicalRevalidationFailClosed {
                    reason: "r".to_string(),
                },
                "PHYSICAL_REVALIDATION_FAIL_CLOSED",
            ),
            (
                Refusal::PhysicalRevalidationSeamFailed {
                    seam: "s",
                    detail: "d".to_string(),
                },
                "PHYSICAL_REVALIDATION_SEAM_FAILED",
            ),
        ];
        for (refusal, name) in cases {
            assert_eq!(refusal.name(), name);
            // Every refusal renders human-readable prose, never empty.
            assert!(!refusal.to_string().is_empty());
        }
    }

    // --- stage 1: envelope shape ------------------------------------------

    #[test]
    fn envelope_refuses_empty_id_sequence_zero_and_inverted_window() {
        let cases: [Result<ResearchProposal, Refusal>; 4] = [
            ResearchProposal::new(
                "", // empty proposal id
                "plugin/x",
                "1.0",
                1,
                ResearchResultKind::Ranking,
                serde_json::json!({}),
                reference_provenance(),
                reference_window(),
            ),
            ResearchProposal::new(
                "prop-2",
                "plugin/x",
                "1.0",
                0, // sequence 0 is unsequenced — refused
                ResearchResultKind::Ranking,
                serde_json::json!({}),
                reference_provenance(),
                reference_window(),
            ),
            ResearchProposal::new(
                "prop-3",
                "plugin/x",
                "1.0",
                1,
                ResearchResultKind::Ranking,
                serde_json::json!({}),
                reference_provenance(),
                ResearchValidityWindow {
                    valid_from_ms: 2_000,
                    valid_until_ms: 1_000, // inverted
                },
            ),
            ResearchProposal::new(
                "prop-4",
                "", // empty producer identity
                "1.0",
                1,
                ResearchResultKind::Ranking,
                serde_json::json!({}),
                reference_provenance(),
                reference_window(),
            ),
        ];
        for case in cases {
            let refusal = case.expect_err("a malformed envelope must be refused");
            assert_eq!(refusal.name(), "PROPOSAL_ENVELOPE_INVALID");
        }
    }

    #[test]
    fn envelope_refuses_oversized_payload() {
        let oversized = "x".repeat(MAX_PROPOSAL_PAYLOAD_BYTES + 1);
        let refusal = ResearchProposal::new(
            "prop-big",
            "plugin/x",
            "1.0",
            1,
            ResearchResultKind::Diagnostics,
            serde_json::json!({ "blob": oversized }),
            reference_provenance(),
            reference_window(),
        )
        .expect_err("an over-cap payload must be refused");
        assert_eq!(refusal.name(), "PROPOSAL_ENVELOPE_INVALID");
    }

    // --- stage 3: provenance completeness ---------------------------------

    #[test]
    fn provenance_completeness_names_the_first_missing_field() {
        let mut provenance = reference_provenance();
        provenance.model_version = "  ".to_string();
        provenance.model_artifact_ref = String::new();
        let refusal = check_provenance(&provenance).expect_err("blank provenance is refused");
        // FIRST failure wins, in the fixed field order.
        assert_eq!(
            refusal,
            Refusal::ProvenanceIncomplete {
                field: "model_version"
            }
        );
        provenance.model_version = "grid-model/2026.11".to_string();
        let refusal = check_provenance(&provenance).expect_err("blank artifact ref is refused");
        assert_eq!(
            refusal,
            Refusal::ProvenanceIncomplete {
                field: "model_artifact_ref"
            }
        );
    }

    #[test]
    fn bare_ok_termination_status_is_refused() {
        let mut provenance = reference_provenance();
        provenance.termination_status = "ok".to_string();
        let refusal = check_provenance(&provenance).expect_err("bare \"ok\" is refused");
        assert_eq!(refusal.name(), "TERMINATION_STATUS_INSUFFICIENT");
    }

    #[test]
    fn seed_honesty_is_enforced_both_ways() {
        // A worker claiming seed support without a seed: a reproducibility
        // lie, refused (P7.42 honesty rule).
        let mut lying = reference_provenance();
        lying.seed = None;
        assert_eq!(
            check_provenance(&lying).expect_err("seed lie refused").name(),
            "SEED_POLICY_INCONSISTENT"
        );
        // The honest admission (seed_supported = false, no seed) passes.
        let mut honest = reference_provenance();
        honest.seed = None;
        honest.seed_supported = false;
        honest.deterministic = false;
        check_provenance(&honest).expect("the honest admission passes");
    }

    // --- stages 4–5: window and snapshot, with known-answer boundaries ----

    #[test]
    fn window_boundaries_are_known_answers() {
        // Window is [1_000, 2_000): now == valid_from is INSIDE (opens
        // inclusive); now == valid_until is EXPIRED (closes exclusive);
        // one millis before either boundary gives the other named refusal.
        let inside = (1_000, "accepted");
        let last_valid = (1_999, "accepted");
        let expired = (2_000, "PROPOSAL_EXPIRED");
        let not_yet = (999, "PROPOSAL_NOT_YET_VALID");
        for (now, expectation) in [inside, last_valid, expired, not_yet] {
            let (gate, snapshots, clock) = passing_fixture(now);
            let h = harness(&gate, &snapshots, &clock);
            let proposal = reference_proposal();
            let outcome = h.validate(&proposal);
            if expectation == "accepted" {
                assert!(
                    outcome.is_ok(),
                    "now={now} must be accepted, got {outcome:?}"
                );
            } else {
                let refusal = outcome.expect_err("outside the window is refused");
                assert_eq!(refusal.name(), expectation, "now={now}");
            }
        }
    }

    #[test]
    fn snapshot_freshness_refusals_name_the_replacement() {
        // Superseded: the replacing snapshot id travels on the refusal.
        let gate = GateSpy::passing();
        let superseded = FixedSnapshots {
            fresh: SnapshotFreshness::Superseded {
                by_snapshot_id: "snapshot-2026-11-13-b".to_string(),
            },
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &superseded, &clock);
        let refusal = h
            .validate(&reference_proposal())
            .expect_err("a superseded snapshot is refused");
        assert_eq!(
            refusal,
            Refusal::SnapshotSuperseded {
                by_snapshot_id: "snapshot-2026-11-13-b".to_string()
            }
        );
        assert_eq!(gate.calls.get(), 0, "the gate is never consumed");

        // Unknown is never fresh (the P4.11 rule, mirrored).
        let gate = GateSpy::passing();
        let unknown = FixedSnapshots {
            fresh: SnapshotFreshness::Unknown,
        };
        let h = harness(&gate, &unknown, &clock);
        let refusal = h
            .validate(&reference_proposal())
            .expect_err("an unknown snapshot is refused");
        assert_eq!(refusal.name(), "SNAPSHOT_UNKNOWN");
        assert_eq!(gate.calls.get(), 0);
    }

    // --- stage 6: the physical gate (the bypass proof) ---------------------

    #[test]
    fn gate_pass_produces_a_sealed_proposal_with_known_answer_evidence() {
        let gate = GateSpy::passing();
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let report = h.validate(&reference_proposal()).expect("full pass");
        assert!(report.is_validated());
        assert_eq!(report.stages_passed, STAGE_ORDER.to_vec());
        assert_eq!(report.harness_version, HARNESS_VERSION);
        let validated = report.validated.as_ref().unwrap();
        assert_eq!(validated.proposal_id, "prop-1");
        assert_eq!(validated.model_content_hash, "abc123");
        // Hand-computed from the recorded evidence-reference format.
        assert_eq!(
            validated.evidence_reference,
            "feasibility:attempt/AT-1:model/ieee13:hash/abc123"
        );
        assert_eq!(validated.harness_version, HARNESS_VERSION);
        assert_eq!(validated.producer_deterministic, true);
        assert_eq!(gate.calls.get(), 1, "exactly one gate attempt");
    }

    #[test]
    fn gate_limit_violation_refuses_with_the_named_constraint() {
        let gate = GateSpy::with(Ok(IndependentVerdict::LimitViolated {
            attempt_id: "AT-2".to_string(),
            model_id: "ieee13".to_string(),
            model_content_hash: "abc123".to_string(),
            constraint: "branch-7 current above rating".to_string(),
        }));
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let refusal = h
            .validate(&reference_proposal())
            .expect_err("a limit violation is refused");
        assert_eq!(
            refusal,
            Refusal::PhysicalRevalidationLimitViolated {
                constraint: "branch-7 current above rating".to_string()
            }
        );
        assert_eq!(gate.calls.get(), 1);
    }

    #[test]
    fn gate_fail_closed_refuses_never_passes() {
        let gate = GateSpy::with(Ok(IndependentVerdict::FailClosed {
            attempt_id: Some("AT-3".to_string()),
            reason: "power-flow did not converge".to_string(),
        }));
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let refusal = h
            .validate(&reference_proposal())
            .expect_err("fail-closed is a refusal");
        assert_eq!(
            refusal,
            Refusal::PhysicalRevalidationFailClosed {
                reason: "power-flow did not converge".to_string()
            }
        );
    }

    #[test]
    fn gate_seam_error_is_fail_closed_never_a_pass() {
        let gate = GateSpy::with(Err(GateSeamError {
            seam: "feasibility_gate",
            detail: "gate port not wired".to_string(),
        }));
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let refusal = h
            .validate(&reference_proposal())
            .expect_err("a seam error is a refusal");
        assert_eq!(
            refusal,
            Refusal::PhysicalRevalidationSeamFailed {
                seam: "feasibility_gate",
                detail: "gate port not wired".to_string()
            }
        );
    }

    #[test]
    fn cheap_refusals_never_consume_a_gate_attempt() {
        // A proposal failing stage 3 (provenance) and one failing stage 4
        // (expired window): in both cases the gate spy must record ZERO
        // calls — a malformed or stale proposal never reaches physics.
        let mut bad_provenance = reference_provenance();
        bad_provenance.worker_version = String::new();
        let malformed = ResearchProposal::new(
            "prop-5",
            "plugin/x",
            "1.0",
            1,
            ResearchResultKind::Ranking,
            serde_json::json!({}),
            bad_provenance,
            reference_window(),
        )
        .expect("shape-valid");
        let gate = GateSpy::passing();
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        assert_eq!(
            h.validate(&malformed).expect_err("refused").name(),
            "PROVENANCE_INCOMPLETE"
        );
        assert_eq!(gate.calls.get(), 0, "no gate attempt on a stage-3 refusal");

        let gate = GateSpy::passing();
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 2_500 }; // past the window
        let h = harness(&gate, &snapshots, &clock);
        assert_eq!(
            h.validate(&reference_proposal())
                .expect_err("refused")
                .name(),
            "PROPOSAL_EXPIRED"
        );
        assert_eq!(gate.calls.get(), 0, "no gate attempt on an expired proposal");
    }

    #[test]
    fn the_researcher_supplies_no_verdict_diagnostics_are_not_gate_input() {
        // Diagnostics CLAIMING feasibility change nothing: the harness has
        // no field to read a verdict from, and the gate still decides.
        let mut boasting = reference_provenance();
        boasting.diagnostics = vec!["feasible: yes, safe: yes".to_string()];
        let proposal = ResearchProposal::new(
            "prop-6",
            "plugin/research-marl",
            "0.1.0",
            1,
            ResearchResultKind::CandidateSetpointVector,
            serde_json::json!({ "setpoints": [0.0] }),
            boasting,
            reference_window(),
        )
        .expect("shape-valid");
        let gate = GateSpy::with(Ok(IndependentVerdict::FailClosed {
            attempt_id: None,
            reason: "gate unavailable".to_string(),
        }));
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let refusal = h
            .validate(&proposal)
            .expect_err("a researcher's self-declared verdict is not a verdict");
        assert_eq!(
            refusal,
            Refusal::PhysicalRevalidationFailClosed {
                reason: "gate unavailable".to_string()
            }
        );
    }

    #[test]
    fn the_gate_is_physically_the_last_stage() {
        // stages_passed on a full pass is the FIXED order with the gate
        // LAST — the ordering is the contract (cheap refusals first).
        let gate = GateSpy::passing();
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Current,
        };
        let clock = FixedClock { now: 1_500 };
        let h = harness(&gate, &snapshots, &clock);
        let report = h.validate(&reference_proposal()).expect("full pass");
        let last = report.stages_passed.last().copied();
        assert_eq!(last, Some("physical_revalidation"));
    }

    #[test]
    fn max_proposal_payload_cap_matches_the_kernel_posture() {
        // Same value as the kernel's cap (the recorded alignment).
        assert_eq!(MAX_PROPOSAL_PAYLOAD_BYTES, 1024 * 1024);
    }

    #[test]
    fn kernel_alignment_seams_are_recorded_verbatim() {
        assert!(KERNEL_PROPOSAL_ENVELOPE_ALIGNMENT_SEAM
            .contains("PluginProposalEnvelope"));
        assert!(FEASIBILITY_VERDICT_ALIGNMENT_SEAM.contains("FeasibilityGateVerdict"));
        assert!(FEASIBILITY_VERDICT_ALIGNMENT_SEAM.contains("command_gates.rs"));
    }
}
