// FIM-F14.02 / FIM-F14.03 / FIM-F14.04 (wave 5, Agent B) — The DOE
// constraint wiring: the composition layer that turns the feed-declared
// dynamic operating envelopes (the FIM-F14.01 seam in
// `doe_constraint_source`) into optimizer CONSTRAINT DATA at the point
// where the scheduler/optimizer assembles per-connection-point limits.
//
// Feature: F-14 "Dynamic operating envelopes from live DSO feeds"
// (docs/releases/final-implementation-before-publication.md §5, FIM-F14).
// Owning phase record: P14.15 (`crates/flexcore-domain/src/doe_envelope.rs`
// — the domain consumption interface, CONSUMED here, never restated) and
// decision record `docs/phases-11-20/phase-14-protocol-device-connectivity/
// records/63_p14_15_doe_consumption_interface.md`.
//
// HONESTY STATUS (FIM-F14.06):
//
// - **Implemented-under-FIM-F14, WRITTEN-UNRUN.** No cargo command was
//   executed in this loop; the human build/test gate owns the first run.
// - **Live DSO feed interop is UNPROVEN.** No DOE was received from any
//   operator or DNSP; wiring a real feed is a **HUMAN LAB GATE**
//   (`doe_constraint_source::DOE_FEED_LAB_GATE_STATEMENT`). Everything
//   below is composition over the EXISTING domain types over CONSTRUCTED
//   declared data.
// - **ADR-010 posture (constraint data, never commands).** An envelope
//   resolved here is optimizer CONSTRAINT DATA only. Nothing here issues,
//   extends, approves, or modifies a DOE; nothing here talks to a device;
//   a curtailment DECISION is a command-lifecycle outcome (the Phase 4
//   gates), never an effect of this module — the curtailment RECORD only
//   observes a declared reduction, exactly as the domain module frames it.
//
// THE OVERRIDE RULE (FIM-F14.02, stated once —
// [`DOE_CONSTRAINT_WIRING_STATEMENT`]):
//
//   Where the scheduler/optimizer assembles per-connection-point limits,
//   the in-force envelope's `ImportExportLimits` override the
//   `StaticConnectionLimit` as constraint data; when the envelope lapses,
//   the static limit governs again. The override is a bounds replacement
//   on the DECLARED connection-limit seat only
//   ([`ConnectionBoundsSeat::StaticLimit`]); it never widens a bound
//   seated at another declared authority (fail-closed, named refusal),
//   never changes the declared problem class, and never touches the
//   solver or its classes (the solver-class discipline, AGENTS.md rule 4 —
//   absolute, is upstream's and untouched here).
//
//   The optimizer-IR application
//   ([`apply_connection_point_bounds`]) writes the in-force MW pair into
//   an `ActivePowerMW`-declared variable of the EXISTING
//   `flexcore_optimization_api::problem::OptimizationProblem` IR — no
//   solver, provider, backend, or problem-IR type is modified, and no new
//   problem class exists. The per-asset mapping (one connection point may
//   host several assets) stays the composition root's declared business;
//   this module is per-connection-point, the domain granularity.
//
// THE LAPSE / REVERSION TELEMETRY (FIM-F14.03, stated once —
// [`DOE_REVERSION_TELEMETRY_STATEMENT`]):
//
//   The evaluation itself is the DOMAIN's
//   (`evaluate_envelope` — the deterministic `NotYetInEffect` → `Expired`
//   → `Stale` precedence, the SAPN static-limit fallback [S-159], the
//   EDGE Workstream 3 staleness budget [S-158]; all UNVERIFIED semantics
//   per record 63 §4 — consumed by citation, never re-decided here). The
//   wiring adds only the OBSERVATION: a typed record carrying the FROZEN
//   six-state quality vocabulary (`flexcore_contracts::quality`) —
//   `STALE` for an envelope past expiry or past its declared cadence
//   budget and for a feed loss with a last known envelope (the frozen
//   `COMMUNICATION_LOSS` mapping REUSED via `map_source_state`), `MISSING`
//   when no envelope value was received. The frozen 26-event vocabulary
//   has no DOE-reversion event and none is invented (P0.14); when the
//   composition root maps a connection point onto an asset telemetry
//   record, the EXISTING `TelemetryQualityChanged` event is the declared
//   carrier. Observations are emitted per evaluation (the composition is
//   stateless and pure — no clock is read, the instant is injected);
//   duplicate suppression is the existing P2.09 dedup machinery's
//   business, not a new one invented here.
//
// THE BREACH / CURTAILMENT RECORDING (FIM-F14.04):
//
//   A dispatch that would exceed the in-force bounds is recorded through
//   the EXISTING domain record surfaces (`EnvelopeBreachRecord`,
//   `CurtailmentRecord`) — the envelope version/identity fields of the
//   record carry the IN-FORCE envelope's id (`None` = the static limit
//   was breached, the honest no-DOE representation). The records are
//   operator-attention data consumed by the P11 taxonomy — never
//   protection signals, never dispatch decisions.
//
// UNITS (frozen): canonical MW at the boundary —
// [`DOE_UNIT_PASS_THROUGH_STATEMENT`]. The optimizer bounds pair is
// exactly `[-import_limit_mw, +export_limit_mw]` (the `P > 0` = export
// sign rule owned by the domain module — delegated to
// `effective_envelope_bounds_mw`, never restated). No conversion, no
// scaling, no unit reinterpretation happens anywhere in this module.

use flexcore_contracts::quality::{map_source_state, SourceQualityState, TelemetryQuality};
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::doe_envelope::{
    effective_envelope_bounds_mw, evaluate_envelope, BreachDirection, ConnectionPointId,
    CurtailmentRecord, DoeError, DynamicOperatingEnvelope, EffectiveEnvelope, EnvelopeBreachRecord,
    ImportExportLimits, LapseReason, StaticConnectionLimit,
};
use flexcore_domain::ids::SiteId;
use flexcore_optimization_api::problem::{OptimizationProblem, VariableUnit};

use crate::doe_constraint_source::{DoeEnvelopeSource, DoeSourceError};

// --- Recorded statements -------------------------------------------------------

/// The override rule, stated once (FIM-F14.02; pinned by tests).
pub const DOE_CONSTRAINT_WIRING_STATEMENT: &str = "Where the scheduler or \
     optimizer assembles per-connection-point limits, the in-force \
     envelope's ImportExportLimits override the StaticConnectionLimit as \
     CONSTRAINT DATA; when the envelope lapses, the static limit governs \
     again. The override replaces bounds on the declared connection-limit \
     seat only, never widens a bound seated at another declared authority, \
     never changes the declared problem class, and never touches the \
     solver or its classes.";

/// The ADR-010 posture, restated for the DOE constraint path (pinned by
/// tests).
pub const DOE_ADR010_STATEMENT: &str = "A dynamic operating envelope is \
     CONSTRAINT DATA, never a command (ADR-010): resolving bounds, \
     applying them to a problem variable, or clipping a proposed setpoint \
     to them issues nothing, approves nothing, and talks to no device. A \
     curtailment decision is a command-lifecycle outcome (the Phase 4 \
     gates); the curtailment record only observes a declared reduction.";

/// The MW pass-through rule (pinned by tests).
pub const DOE_UNIT_PASS_THROUGH_STATEMENT: &str = "Envelope limits are \
     canonical MW at the boundary and pass through UNCHANGED: the \
     optimizer bounds pair is exactly [-import_limit_mw, +export_limit_mw] \
     (P > 0 = export, the frozen sign rule owned by \
     flexcore_domain::doe_envelope). No conversion, scaling, or unit \
     reinterpretation happens in the wiring; bounds are applied only to \
     ActivePowerMW-declared variables and any other unit is a named \
     refusal.";

/// The reversion-telemetry rule (FIM-F14.03; pinned by tests).
pub const DOE_REVERSION_TELEMETRY_STATEMENT: &str = "A reversion to the \
     static limit is recorded as a typed observation carrying the FROZEN \
     six-state telemetry quality vocabulary: STALE for an envelope past \
     expiry or past its declared cadence budget (freshness_limit_seconds) \
     and for a feed loss with a last known envelope (the frozen \
     COMMUNICATION_LOSS source-state mapping reused), MISSING when no \
     envelope value was received. The frozen 26-event vocabulary has no \
     DOE-reversion event and none is invented (P0.14); when the \
     composition root maps a connection point onto an asset telemetry \
     record, the existing TelemetryQualityChanged event is the declared \
     carrier. Observations are emitted per evaluation; duplicate \
     suppression is the existing P2.09 dedup machinery's business.";

/// The fail-closed posture of the whole wiring (pinned by tests).
pub const DOE_FAIL_CLOSED_STATEMENT: &str = "The static connection limit is \
     MANDATORY declared data on every evaluation: a point without a \
     declared static limit is refused, a feed error answers with the \
     static limit (never synthetic envelope data, never an unconstrained \
     default), and a lapsed envelope's fallback is the static pair the \
     domain evaluation returned.";

/// The FIM-F14.06 honesty statement for this module (pinned by tests).
pub const FIM_F14_HONESTY_STATEMENT: &str = "Implemented under FIM-F14 \
     (wave 5, Agent B): WRITTEN-UNRUN — no cargo command was executed and \
     the human build/test gate owns the first run. Live DSO feed interop \
     is UNPROVEN: no DOE was received from any operator or DNSP and no \
     wire protocol is implemented; wiring a real feed is a HUMAN LAB GATE \
     (doe_constraint_source::DOE_FEED_LAB_GATE_STATEMENT). ADR-010 \
     posture: the envelope is constraint data, never a command.";

/// The prefix a DOE id carries when it is recorded in a problem-provenance
/// envelope-version field ([`doe_envelope_version_field`]): the
/// `ProblemProvenance::envelope_version` field is the P3.13
/// flexibility-envelope version; the DOE id is a DIFFERENT envelope kind
/// and the prefix keeps the two from ever being conflated (a declared
/// composition choice, never a silent overwrite of the field's meaning).
pub const DOE_ENVELOPE_VERSION_PREFIX: &str = "doe:";

// --- Errors ----------------------------------------------------------------------

/// Why the constraint wiring refused an operation. Named, never absorbed;
/// every refusal is fail-closed (the static limit or the existing bounds
/// stay in force — never an unconstrained default).
#[derive(Debug, Clone, PartialEq)]
pub enum DoeWiringError {
    /// The declared static-limit list named the same connection point
    /// twice (a reviewed declaration, not a bag — the group-scope rule's
    /// spirit applied to the static list).
    DuplicateStaticLimit {
        /// The duplicated connection point.
        connection_point: String,
    },
    /// The named problem variable does not exist exactly once
    /// (`matches` 0 = unknown, > 1 = duplicated in the unvalidated input).
    UnknownOrDuplicateVariable {
        /// The variable name sought.
        name: String,
        /// How many variables matched.
        matches: usize,
    },
    /// The named variable is not an `ActivePowerMW` variable. DOE bounds
    /// are MW and are NEVER converted to another unit — the application is
    /// refused by name.
    VariableUnitMismatch {
        /// The variable name.
        name: String,
        /// The declared unit the variable carries.
        found: &'static str,
    },
    /// Applying the in-force bounds to a variable seated at another
    /// declared authority would WIDEN it — refused by name (fail-closed:
    /// constraint composition may only narrow, never loosen).
    WouldWidenExistingBound {
        /// The variable name.
        name: String,
        /// The existing (lower, upper) bounds.
        existing: (Option<f64>, Option<f64>),
        /// The applied (lower, upper) bounds that were refused.
        applied: (f64, f64),
    },
    /// The seat was declared `Unset` but the variable already carries
    /// bounds — the declaration and the problem disagree, which is a
    /// caller bookkeeping defect, never silently overwritten.
    SeatConflict {
        /// The variable name.
        name: String,
    },
    /// A value that must be finite was NaN or infinite.
    NonFiniteValue {
        /// What was non-finite.
        context: &'static str,
    },
    /// The domain evaluation refused the inputs (the `DoeError` travels
    /// unchanged — never reinterpreted).
    DomainEvaluation(DoeError),
    /// The evaluation instant fell outside the wiring's Unix-ms domain.
    TimestampOutOfRange {
        /// The offending value (i128 Unix ms).
        value: i128,
    },
}

impl DoeWiringError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            DoeWiringError::DuplicateStaticLimit { .. } => "DOE_WIRING_DUPLICATE_STATIC_LIMIT",
            DoeWiringError::UnknownOrDuplicateVariable { .. } => {
                "DOE_WIRING_UNKNOWN_OR_DUPLICATE_VARIABLE"
            }
            DoeWiringError::VariableUnitMismatch { .. } => "DOE_WIRING_VARIABLE_UNIT_MISMATCH",
            DoeWiringError::WouldWidenExistingBound { .. } => {
                "DOE_WIRING_WOULD_WIDEN_EXISTING_BOUND"
            }
            DoeWiringError::SeatConflict { .. } => "DOE_WIRING_SEAT_CONFLICT",
            DoeWiringError::NonFiniteValue { .. } => "DOE_WIRING_NON_FINITE_VALUE",
            DoeWiringError::DomainEvaluation(_) => "DOE_WIRING_DOMAIN_EVALUATION",
            DoeWiringError::TimestampOutOfRange { .. } => "DOE_WIRING_TIMESTAMP_OUT_OF_RANGE",
        }
    }
}

impl std::fmt::Display for DoeWiringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoeWiringError::DuplicateStaticLimit { connection_point } => write!(
                f,
                "the static-limit list declares connection point '{connection_point}' twice; \
                 a declared list is reviewed data, not a bag"
            ),
            DoeWiringError::UnknownOrDuplicateVariable { name, matches } => write!(
                f,
                "the problem carries {matches} variable(s) named '{name}' (exactly one required)"
            ),
            DoeWiringError::VariableUnitMismatch { name, found } => write!(
                f,
                "variable '{name}' is declared '{found}'; DOE bounds are MW and are never \
                 converted — declare the variable ActivePowerMW"
            ),
            DoeWiringError::WouldWidenExistingBound { name, existing, applied } => write!(
                f,
                "applying DOE bounds {:?} to variable '{name}' would widen its existing \
                 declared bounds {:?}; constraint composition may only narrow, never loosen",
                applied, existing
            ),
            DoeWiringError::SeatConflict { name } => write!(
                f,
                "variable '{name}' was declared seat-unset but already carries bounds; \
                 the declaration and the problem disagree"
            ),
            DoeWiringError::NonFiniteValue { context } => {
                write!(f, "DOE wiring field '{context}' must be finite")
            }
            DoeWiringError::DomainEvaluation(err) => {
                write!(f, "the domain DOE evaluation refused the inputs: {err}")
            }
            DoeWiringError::TimestampOutOfRange { value } => {
                write!(f, "DOE wiring instant {value} is outside the Unix-ms domain")
            }
        }
    }
}

impl std::error::Error for DoeWiringError {}

// --- The resolved constraint -----------------------------------------------------

/// Why the STATIC limit governs a connection point at an evaluation
/// instant (the wiring-level vocabulary; the domain `LapseReason` travels
/// unchanged in the `Lapsed` variant — no parallel lapse taxonomy is
/// invented).
#[derive(Debug, Clone, PartialEq)]
pub enum StaticGovernanceReason {
    /// The feed declares no envelope for the point — a NORMAL state
    /// (most points have no DOE program); the feed seam's declared
    /// identity travels so the governing source is never silent.
    NoEnvelopeDeclared {
        /// The declared identity of the feed source consulted.
        source_identity: String,
    },
    /// The feed read failed (fail-closed: the static limit answers, the
    /// opaque code + message travel — no raw wire text beyond what the
    /// port boundary already guarantees opaque).
    FeedUnavailable {
        /// The declared identity of the feed source consulted.
        source_identity: String,
        /// The opaque adapter error code.
        error_code: String,
        /// The opaque adapter message (the P1.08 boundary rule).
        error_message: String,
    },
    /// A declared envelope has lapsed (the domain lapse reason travels
    /// unchanged: `NotYetInEffect`, `Expired`, or `Stale`).
    Lapsed {
        /// The domain lapse reason (deterministic precedence owned by
        /// `evaluate_envelope`).
        reason: LapseReason,
        /// The lapsed envelope's id (the version/identity fields
        /// FIM-F14.04 records travel from here too).
        envelope_id: String,
    },
}

impl StaticGovernanceReason {
    /// The canonical name of the reason (records and observations).
    pub fn name(&self) -> &'static str {
        match self {
            StaticGovernanceReason::NoEnvelopeDeclared { .. } => "no_envelope_declared",
            StaticGovernanceReason::FeedUnavailable { .. } => "feed_unavailable",
            StaticGovernanceReason::Lapsed { reason, .. } => reason.name(),
        }
    }
}

impl std::fmt::Display for StaticGovernanceReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StaticGovernanceReason::NoEnvelopeDeclared { source_identity } => write!(
                f,
                "no envelope declared by feed source '{source_identity}' — the static \
                 limit is the whole truth"
            ),
            StaticGovernanceReason::FeedUnavailable {
                source_identity,
                error_code,
                error_message,
            } => write!(
                f,
                "feed source '{source_identity}' unavailable ({code}): {message} — \
                 fail-closed to the static limit",
                code = error_code,
                message = error_message
            ),
            StaticGovernanceReason::Lapsed { reason, envelope_id } => write!(
                f,
                "envelope '{envelope_id}' lapsed ({}) — the static limit governs",
                reason.name()
            ),
        }
    }
}

/// What governs a connection point at one evaluation instant: the dynamic
/// envelope in force (the full envelope travels — the version/identity
/// fields FIM-F14.04 records come from here) or the static limit with the
/// named reason.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvelopeConstraintState {
    /// The dynamic envelope is in force (the domain evaluation said
    /// `Dynamic`).
    Dynamic {
        /// The in-force envelope (id, scope, limits, timestamps, cadence,
        /// issuer, provenance — the existing domain record).
        envelope: DynamicOperatingEnvelope,
    },
    /// The static limit governs, for the named reason.
    Static {
        /// Why the static limit governs.
        reason: StaticGovernanceReason,
    },
}

/// A typed REVERSION observation (FIM-F14.03): the fact that, at one
/// evaluation instant, a connection point's dynamic governance ended and
/// the static limit governs. The quality state is from the FROZEN
/// six-state vocabulary (`STALE` / `MISSING` per
/// [`DOE_REVERSION_TELEMETRY_STATEMENT`]); no event is emitted here and
/// none is invented — the record is the operator-view data the existing
/// telemetry/event surfaces map onto when the composition root declares
/// the connection-point → asset mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct DoeReversionObservation {
    /// The connection point whose governance ended.
    pub connection_point: ConnectionPointId,
    /// The envelope whose governance ended (`None` when no envelope was
    /// ever in force — absence or feed loss with nothing before).
    pub ended_envelope_id: Option<String>,
    /// The declared identity of the feed source that produced the state
    /// (the seam is never silent — the unwired default names itself).
    pub source_identity: String,
    /// The named static-governance reason.
    pub reason: StaticGovernanceReason,
    /// The frozen quality state of the envelope value at this evaluation
    /// (`STALE` or `MISSING`; never `GOOD` — a reversion is never silently
    /// converted into a normal reading, the non-silencing rule).
    pub quality: TelemetryQuality,
    /// Whether this evaluation is a TRANSITION from a previously in-force
    /// envelope (the caller-declared previous state) — `false` for a
    /// continuing or initial static state.
    pub is_reversion: bool,
    /// The observation instant (Unix ms — the injected evaluation instant;
    /// no clock is read).
    pub observed_at_unix_ms: i64,
    /// The site context when the composition root declares the
    /// connection-point → site mapping (declared, never inferred) — the
    /// hook for mapping onto the existing `TelemetryQualityChanged` event
    /// shape.
    pub site: Option<SiteId>,
}

/// The constraint data for ONE connection point at ONE evaluation
/// instant: the in-force limits, the optimizer bounds pair, the state
/// (dynamic-with-version or static-with-reason), and the reversion
/// observations this evaluation produced. Produced ONLY by
/// [`resolve_connection_point_constraint`] /
/// [`resolve_connection_point_constraints`] — every field is resolved
/// data, never guessed.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPointConstraint {
    /// The connection point.
    pub connection_point: ConnectionPointId,
    /// The limits IN FORCE (the dynamic pair when the envelope is in
    /// force, else the static fallback). Canonical MW.
    pub limits: ImportExportLimits,
    /// The optimizer bounds pair `(p_min_mw, p_max_mw)` =
    /// `[-import_limit, +export_limit]` (the frozen sign rule, delegated
    /// to the domain function — pass-through MW, never converted).
    pub bounds_mw: (f64, f64),
    /// What governs (dynamic-with-version or static-with-reason).
    pub state: EnvelopeConstraintState,
    /// The reversion observations THIS evaluation produced (empty when
    /// the dynamic envelope is in force or the state is a normal
    /// pre-issue posture).
    pub observations: Vec<DoeReversionObservation>,
}

impl ResolvedPointConstraint {
    /// Whether a dynamic envelope is in force.
    pub fn is_dynamic(&self) -> bool {
        matches!(self.state, EnvelopeConstraintState::Dynamic { .. })
    }

    /// The in-force envelope's id, when a dynamic envelope governs
    /// (`None` = the static limit governs — the honest no-DOE
    /// representation the breach records carry).
    pub fn in_force_envelope_id(&self) -> Option<&str> {
        match &self.state {
            EnvelopeConstraintState::Dynamic { envelope } => Some(envelope.envelope_id.as_str()),
            EnvelopeConstraintState::Static { .. } => None,
        }
    }

    /// The problem-provenance envelope-version field for this constraint
    /// (`Some("doe:<id>")` when a DOE is in force, `None` when the static
    /// limit governs). The prefix disambiguates the DOE id from the P3.13
    /// flexibility-envelope version the field was defined for
    /// ([`DOE_ENVELOPE_VERSION_PREFIX`]).
    pub fn doe_envelope_version_field(&self) -> Option<String> {
        self.in_force_envelope_id()
            .map(|id| format!("{DOE_ENVELOPE_VERSION_PREFIX}{id}"))
    }

    /// The `(lower, upper)` bounds for an `OptimizationProblem` variable
    /// (`VariableUnit::ActivePowerMW`): always `Some` on both sides — the
    /// static fallback is mandatory declared data, so a resolved point is
    /// never unconstrained (fail-closed by construction).
    pub fn variable_bounds_mw(&self) -> (Option<f64>, Option<f64>) {
        (Some(self.bounds_mw.0), Some(self.bounds_mw.1))
    }
}

/// The caller-declared previous state for one connection point: the
/// envelope id that was in force at the previous evaluation (only points
/// with an envelope in force are listed; entries for points not in the
/// static-limit list are ignored; the first match per point wins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointPreviousEnvelope {
    /// The connection point (the declared spelling).
    pub connection_point: String,
    /// The envelope id that was in force.
    pub envelope_id: String,
}

// --- Resolution (FIM-F14.02 + FIM-F14.03) ----------------------------------------

/// The MW bounds pair for an in-force limits value, DELEGATED to the
/// domain function (the frozen sign rule has exactly one home —
/// `flexcore_domain::doe_envelope::effective_envelope_bounds_mw`; the
/// `Dynamic` variant is a pure carrier of the limits here).
fn bounds_of(limits: ImportExportLimits) -> (f64, f64) {
    effective_envelope_bounds_mw(&EffectiveEnvelope::Dynamic { limits })
}

/// Resolves the constraint data for ONE connection point at ONE injected
/// evaluation instant (FIM-F14.02 + FIM-F14.03).
///
/// Pure function of (source, static limit, now, previous state) — no
/// clock is read, no I/O, no state is mutated. The evaluation of the
/// envelope is the DOMAIN's (`evaluate_envelope`); this function adds the
/// feed read (the FIM-F14.01 seam), the fail-closed static answers, and
/// the reversion observations.
///
/// `previous_in_force` is the caller-declared envelope id that was in
/// force at the previous evaluation (`None` = nothing was in force /
/// unknown); it decides only the observation's `is_reversion` flag and
/// the `ended_envelope_id` attribution — never the constraint outcome.
pub fn resolve_connection_point_constraint(
    source: &dyn DoeEnvelopeSource,
    static_limit: &StaticConnectionLimit,
    now: &EventTime,
    previous_in_force: Option<&str>,
    site: Option<SiteId>,
) -> Result<ResolvedPointConstraint, DoeWiringError> {
    let now_ms = i64::try_from(now.unix_millis())
        .map_err(|_| DoeWiringError::TimestampOutOfRange { value: now.unix_millis() })?;
    let point = static_limit.connection_point.clone();
    let source_identity = source.source_identity().to_string();

    // The static limits are mandatory declared data and must be finite
    // (the domain constructor validated them at construction; a
    // hand-assembled static limit is refused here rather than evaluated
    // into NaN bounds).
    if !static_limit.limits.import_limit_mw.is_finite()
        || !static_limit.limits.export_limit_mw.is_finite()
    {
        return Err(DoeWiringError::NonFiniteValue {
            context: "static connection limits",
        });
    }

    // Read the feed's CURRENT declared envelope (the FIM-F14.01 seam).
    let declared: Result<Option<DynamicOperatingEnvelope>, DoeSourceError> =
        source.current_envelope(&static_limit.connection_point);

    let (state, limits, observations) = match declared {
        // Feed error: fail closed to the static limit, LOUDLY (the frozen
        // quality vocabulary: a communication loss with a last known
        // envelope is STALE — the frozen source-state mapping REUSED;
        // nothing ever received is MISSING).
        Err(err) => {
            let (error_code, error_message) = match &err {
                DoeSourceError::FeedUnavailable { code, message } => {
                    (code.clone(), message.clone())
                }
            };
            let reason = StaticGovernanceReason::FeedUnavailable {
                source_identity: source_identity.clone(),
                error_code,
                error_message,
            };
            let quality = if previous_in_force.is_some() {
                map_source_state(SourceQualityState::CommunicationLoss)
            } else {
                TelemetryQuality::Missing
            };
            let observation = DoeReversionObservation {
                connection_point: point.clone(),
                ended_envelope_id: previous_in_force.map(|id| id.to_string()),
                source_identity: source_identity.clone(),
                reason: reason.clone(),
                quality,
                is_reversion: previous_in_force.is_some(),
                observed_at_unix_ms: now_ms,
                site,
            };
            (
                EnvelopeConstraintState::Static { reason },
                static_limit.limits,
                vec![observation],
            )
        }
        // Feed-absent: the static limit is the whole truth — a NORMAL
        // state, recorded as a MISSING-quality observation naming the
        // source (the unwired seam is visible, never silent).
        Ok(None) => {
            let reason = StaticGovernanceReason::NoEnvelopeDeclared {
                source_identity: source_identity.clone(),
            };
            let observation = DoeReversionObservation {
                connection_point: point.clone(),
                ended_envelope_id: previous_in_force.map(|id| id.to_string()),
                source_identity: source_identity.clone(),
                reason: reason.clone(),
                quality: TelemetryQuality::Missing,
                is_reversion: previous_in_force.is_some(),
                observed_at_unix_ms: now_ms,
                site,
            };
            (
                EnvelopeConstraintState::Static { reason },
                static_limit.limits,
                vec![observation],
            )
        }
        Ok(Some(envelope)) => {
            // The evaluation is the DOMAIN's — the deterministic lapse
            // precedence and the mandatory fallback are consumed, never
            // re-decided here.
            let effective = evaluate_envelope(&envelope, static_limit, now)
                .map_err(DoeWiringError::DomainEvaluation)?;
            match effective {
                EffectiveEnvelope::Dynamic { limits } => (
                    EnvelopeConstraintState::Dynamic { envelope },
                    limits,
                    Vec::new(),
                ),
                EffectiveEnvelope::Lapsed { reason, fallback } => {
                    // Pre-issue is a scheduled posture, not a data-quality
                    // fact: no observation (the state names the reason).
                    // Past expiry or past the cadence budget: the last
                    // known envelope's governance ended — STALE.
                    let observation = match reason {
                        LapseReason::NotYetInEffect => None,
                        LapseReason::Expired | LapseReason::Stale => {
                            Some(DoeReversionObservation {
                                connection_point: point.clone(),
                                ended_envelope_id: Some(envelope.envelope_id.clone()),
                                source_identity: source_identity.clone(),
                                reason: StaticGovernanceReason::Lapsed {
                                    reason,
                                    envelope_id: envelope.envelope_id.clone(),
                                },
                                quality: TelemetryQuality::Stale,
                                is_reversion: previous_in_force.is_some(),
                                observed_at_unix_ms: now_ms,
                                site,
                            })
                        }
                    };
                    let observations = match observation {
                        Some(obs) => vec![obs],
                        None => Vec::new(),
                    };
                    (
                        EnvelopeConstraintState::Static {
                            reason: StaticGovernanceReason::Lapsed {
                                reason,
                                envelope_id: envelope.envelope_id.clone(),
                            },
                        },
                        fallback,
                        observations,
                    )
                }
            }
        }
    };

    let bounds_mw = bounds_of(limits);
    Ok(ResolvedPointConstraint {
        connection_point: point,
        limits,
        bounds_mw,
        state,
        observations,
    })
}

/// Resolves the constraint data for a DECLARED list of connection points
/// (the point where the scheduler/optimizer assembles the per-connection
/// -point limits). The list order is the caller's declared order and is
/// preserved; a duplicated connection point is a named refusal. Entries
/// of `previous_in_force` for points not in the list are ignored; the
/// first match per point wins.
pub fn resolve_connection_point_constraints(
    source: &dyn DoeEnvelopeSource,
    static_limits: &[StaticConnectionLimit],
    now: &EventTime,
    previous_in_force: &[PointPreviousEnvelope],
    site: Option<SiteId>,
) -> Result<Vec<ResolvedPointConstraint>, DoeWiringError> {
    let mut seen: Vec<&str> = Vec::with_capacity(static_limits.len());
    for limit in static_limits {
        let id = limit.connection_point.as_str();
        if seen.contains(&id) {
            return Err(DoeWiringError::DuplicateStaticLimit {
                connection_point: id.to_string(),
            });
        }
        seen.push(id);
    }
    let mut resolved = Vec::with_capacity(static_limits.len());
    for limit in static_limits {
        let previous = previous_in_force
            .iter()
            .find(|p| p.connection_point.as_str() == limit.connection_point.as_str())
            .map(|p| p.envelope_id.as_str());
        resolved.push(resolve_connection_point_constraint(
            source, limit, now, previous, site,
        )?);
    }
    Ok(resolved)
}

// --- Dispatch checking and curtailment (FIM-F14.04) ------------------------------

/// The verdict of a proposed dispatch against the in-force constraint
/// data. A verdict is a RECORD — never a dispatch decision and never a
/// command (ADR-010): the breach/curtailment records are operator- and
/// taxonomy-facing data about a PROPOSED value.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchEnvelopeVerdict {
    /// The proposed dispatch is inside the in-force bounds (a boundary
    /// value is NOT a violation — the frozen boundary convention).
    WithinBounds,
    /// The proposed dispatch would exceed the in-force bounds; the breach
    /// record carries the envelope version in force (`None` = the static
    /// limit was breached).
    Breach {
        /// The breach record (existing domain type).
        breach: EnvelopeBreachRecord,
    },
}

/// Checks one PROPOSED dispatch value (canonical MW, `P > 0` = export)
/// against the in-force constraint data. Pure; no clock is read (the
/// observation timestamp is caller-declared); nothing is issued.
pub fn check_dispatch_against_constraint(
    constraint: &ResolvedPointConstraint,
    proposed_p_mw: f64,
    observed_at_unix_ms: i64,
    site: Option<SiteId>,
) -> Result<DispatchEnvelopeVerdict, DoeWiringError> {
    if !proposed_p_mw.is_finite() {
        return Err(DoeWiringError::NonFiniteValue {
            context: "proposed dispatch",
        });
    }
    let (p_min, p_max) = constraint.bounds_mw;
    if !p_min.is_finite() || !p_max.is_finite() {
        return Err(DoeWiringError::NonFiniteValue {
            context: "in-force bounds",
        });
    }
    if proposed_p_mw >= p_min && proposed_p_mw <= p_max {
        return Ok(DispatchEnvelopeVerdict::WithinBounds);
    }
    let (direction, limit_mw) = if proposed_p_mw > p_max {
        (BreachDirection::Export, p_max)
    } else {
        (BreachDirection::Import, -p_min)
    };
    let observed = ActivePowerMW::new(proposed_p_mw).map_err(|_| {
        DoeWiringError::NonFiniteValue {
            context: "proposed dispatch",
        }
    })?;
    let breach = EnvelopeBreachRecord::new(
        constraint.in_force_envelope_id().map(|id| id.to_string()),
        constraint.connection_point.clone(),
        direction,
        observed,
        limit_mw,
        observed_at_unix_ms,
        site,
    )
    .map_err(DoeWiringError::DomainEvaluation)?;
    Ok(DispatchEnvelopeVerdict::Breach { breach })
}

/// The outcome of clipping a proposed dispatch to the in-force bounds as
/// CONSTRAINT DATA. The clip is a scheduling-side composition over
/// constraint data; the curtailment DECISION itself remains a
/// command-lifecycle outcome (ADR-010) — the record only observes the
/// declared reduction.
#[derive(Debug, Clone, PartialEq)]
pub enum CurtailmentOutcome {
    /// The proposed dispatch is inside the in-force bounds; nothing is
    /// curtailed and no record exists.
    WithinBounds,
    /// The proposed dispatch would exceed the in-force bounds; the
    /// clipped value stays inside and the curtailment record carries the
    /// envelope version in force (`None` = the static limit governs).
    Curtailed {
        /// The clipped setpoint (inside the in-force bounds; canonical
        /// MW, pass-through — the proposed value truncated to the bound,
        /// never scaled).
        clipped_p_mw: f64,
        /// The curtailment record (existing domain type).
        record: CurtailmentRecord,
    },
}

/// Clips one PROPOSED dispatch value to the in-force bounds (the
/// constraint-data clip the DOE-aware composition uses) and records the
/// declared reduction with the envelope version in force. Pure; nothing
/// is issued; the actual curtailment of a real asset is a
/// command-lifecycle outcome, never an effect of this function.
pub fn curtail_proposed_to_bounds(
    constraint: &ResolvedPointConstraint,
    proposed_p_mw: f64,
    recorded_at_unix_ms: i64,
    site: Option<SiteId>,
) -> Result<CurtailmentOutcome, DoeWiringError> {
    if !proposed_p_mw.is_finite() {
        return Err(DoeWiringError::NonFiniteValue {
            context: "proposed dispatch",
        });
    }
    let (p_min, p_max) = constraint.bounds_mw;
    if !p_min.is_finite() || !p_max.is_finite() {
        return Err(DoeWiringError::NonFiniteValue {
            context: "in-force bounds",
        });
    }
    if proposed_p_mw >= p_min && proposed_p_mw <= p_max {
        return Ok(CurtailmentOutcome::WithinBounds);
    }
    // The clip: truncate to the violated bound (a boundary value is not a
    // violation). Pass-through MW — no scaling, no conversion.
    let clipped = proposed_p_mw.max(p_min).min(p_max);
    let direction = if proposed_p_mw > p_max {
        BreachDirection::Export
    } else {
        BreachDirection::Import
    };
    let curtailed_mw = (proposed_p_mw - clipped).abs();
    let record = CurtailmentRecord::new(
        constraint.in_force_envelope_id().map(|id| id.to_string()),
        constraint.connection_point.clone(),
        curtailed_mw,
        direction,
        recorded_at_unix_ms,
        site,
    )
    .map_err(DoeWiringError::DomainEvaluation)?;
    Ok(CurtailmentOutcome::Curtailed {
        clipped_p_mw: clipped,
        record,
    })
}

// --- Optimizer-IR application (FIM-F14.02) ---------------------------------------

/// The declared authority a problem variable's current bounds come from —
/// the caller DECLARED it (declared context, never inferred from the
/// problem). The seat decides whether the in-force pair may replace the
/// existing bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionBoundsSeat {
    /// The variable carries the STATIC connection limit pair — the seat
    /// the DOE override governs: the in-force pair replaces it when a DOE
    /// comes into force, and the static pair replaces the DOE when it
    /// lapses (the reversion is the same override in the other
    /// direction).
    StaticLimit,
    /// The variable carries bounds from another declared authority (an
    /// asset nameplate, a device profile): the DOE may only NARROW them;
    /// a widening application is refused by name (fail-closed).
    OtherDeclaredAuthority,
    /// The variable carries no bounds yet; the in-force pair is applied.
    /// A conflict (bounds already present) is a named refusal.
    Unset,
}

/// Applies the in-force MW bounds of one resolved point to ONE problem
/// variable (FIM-F14.02): the point where the scheduler/optimizer
/// assembles per-connection-point limits in the EXISTING problem IR.
///
/// Rules (all fail-closed):
/// - the variable must exist exactly once;
/// - the variable must be declared `VariableUnit::ActivePowerMW` — MW
///   bounds are never converted to another unit;
/// - the in-force pair is written as `(Some(p_min), Some(p_max))` — the
///   resolved point is never unconstrained (the static fallback is
///   mandatory data);
/// - the seat decides replaceability ([`ConnectionBoundsSeat`]); a
///   widening write onto a foreign authority is refused by name.
///
/// The problem's declared class, solver routing, and every other field
/// are untouched. Callers wanting the solve's provenance to carry the
/// constraint record use [`doe_constraint_assumption_text`] (explicit,
/// caller-owned) and [`doe_envelope_version_field`].
pub fn apply_connection_point_bounds(
    problem: &mut OptimizationProblem,
    variable_name: &str,
    constraint: &ResolvedPointConstraint,
    seat: ConnectionBoundsSeat,
) -> Result<(), DoeWiringError> {
    let mut matches = 0usize;
    let mut target_index: Option<usize> = None;
    for (index, variable) in problem.variables.iter().enumerate() {
        if variable.name == variable_name {
            matches += 1;
            if matches == 1 {
                target_index = Some(index);
            }
        }
    }
    let index = target_index.ok_or_else(|| DoeWiringError::UnknownOrDuplicateVariable {
        name: variable_name.to_string(),
        matches,
    })?;
    if matches != 1 {
        return Err(DoeWiringError::UnknownOrDuplicateVariable {
            name: variable_name.to_string(),
            matches,
        });
    }
    let variable = &mut problem.variables[index];
    if variable.unit != VariableUnit::ActivePowerMW {
        return Err(DoeWiringError::VariableUnitMismatch {
            name: variable_name.to_string(),
            found: variable.unit.canonical_unit(),
        });
    }
    let (p_min, p_max) = constraint.bounds_mw;
    if !p_min.is_finite() || !p_max.is_finite() {
        return Err(DoeWiringError::NonFiniteValue {
            context: "in-force bounds",
        });
    }
    let existing = (variable.lower_bound, variable.upper_bound);
    match seat {
        ConnectionBoundsSeat::Unset => {
            if existing.0.is_some() || existing.1.is_some() {
                return Err(DoeWiringError::SeatConflict {
                    name: variable_name.to_string(),
                });
            }
        }
        ConnectionBoundsSeat::StaticLimit => {
            // The override seat: replace in either direction (the DOE
            // overrides the static pair; the static pair overrides the
            // DOE on lapse).
        }
        ConnectionBoundsSeat::OtherDeclaredAuthority => {
            let widens_lower = existing.0.is_some_and(|lower| p_min < lower);
            let widens_upper = existing.1.is_some_and(|upper| p_max > upper);
            if widens_lower || widens_upper {
                return Err(DoeWiringError::WouldWidenExistingBound {
                    name: variable_name.to_string(),
                    existing,
                    applied: (p_min, p_max),
                });
            }
        }
    }
    variable.lower_bound = Some(p_min);
    variable.upper_bound = Some(p_max);
    Ok(())
}

/// The declared-assumption text one resolved constraint contributes to a
/// problem's `declared_assumptions` (caller-owned, explicit — this module
/// never mutates a problem's assumptions as a side effect). Pinned by
/// tests.
pub fn doe_constraint_assumption_text(constraint: &ResolvedPointConstraint) -> String {
    match &constraint.state {
        EnvelopeConstraintState::Dynamic { envelope } => format!(
            "connection point '{}' bounds carry the IN-FORCE dynamic operating envelope \
             '{}' (issued {}, expires {}, issuer '{}') as optimizer CONSTRAINT DATA — \
             never a command (ADR-010)",
            constraint.connection_point,
            envelope.envelope_id,
            envelope.issued_at_unix_ms,
            envelope.expires_at_unix_ms,
            envelope.issuer,
        ),
        EnvelopeConstraintState::Static { reason } => format!(
            "connection point '{}' bounds carry the STATIC connection limit ({}) as \
             optimizer CONSTRAINT DATA — never a command (ADR-010)",
            constraint.connection_point,
            reason,
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the rules above over CONSTRUCTED inputs, or a
// literal copied from this module's constants. Deterministic and
// socket-free: no clock is read, no I/O, no threads.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::capability::Provenance;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::doe_envelope::{EnvelopeScope, UpdateCadence};
    use flexcore_optimization_api::problem::{
        Coefficient, Objective, ObjectiveSense, ProblemClass, ProblemProvenance, Variable,
        VariableKind,
    };

    const POINT: &str = "cp-fim14-wire-0001";
    const POINT_OTHER: &str = "cp-fim14-wire-0002";

    fn point() -> ConnectionPointId {
        ConnectionPointId::new(POINT.to_string()).expect("valid id")
    }

    fn other_point() -> ConnectionPointId {
        ConnectionPointId::new(POINT_OTHER.to_string()).expect("valid id")
    }

    /// The static limit of the fixture point: import 5.0 / export 3.0 MW.
    fn static_limit() -> StaticConnectionLimit {
        StaticConnectionLimit::new(
            point(),
            ImportExportLimits::new(5.0, 3.0).expect("valid limits"),
            "connection agreement (declared data, fixture)".to_string(),
            Provenance::Configured,
        )
        .expect("valid static limit")
    }

    /// A single-point envelope over the fixture point with the given
    /// limits, validity window, and latest-update instant (cadence 60 s
    /// x 2 intervals = a 120 s freshness budget).
    fn envelope(
        id: &str,
        import_mw: f64,
        export_mw: f64,
        issued_ms: i64,
        expires_ms: i64,
        latest_update_ms: i64,
    ) -> DynamicOperatingEnvelope {
        DynamicOperatingEnvelope::new(
            id.to_string(),
            EnvelopeScope::Single(point()),
            ImportExportLimits::new(import_mw, export_mw).expect("valid limits"),
            issued_ms,
            expires_ms,
            UpdateCadence {
                expected_interval_seconds: 60,
                max_intervals_before_stale: 2,
            },
            latest_update_ms,
            "dnsp-operator (declared data, synthetic)".to_string(),
            Provenance::Configured,
            "FIM-F14 test envelope (synthetic; never a real operator issuance)".to_string(),
        )
        .expect("valid envelope")
    }

    fn event(ms: i64) -> EventTime {
        EventTime::from_unix_millis(ms).expect("valid instant")
    }

    /// A test-only feed double that always errors (the deterministic,
    /// socket-free stand-in for a broken adapter; the shipped sources
    /// never error, so this is the only way to pin the feed-error path).
    struct FailingDoeSource;
    impl DoeEnvelopeSource for FailingDoeSource {
        fn current_envelope(
            &self,
            _connection_point: &ConnectionPointId,
        ) -> Result<Option<DynamicOperatingEnvelope>, DoeSourceError> {
            Err(DoeSourceError::FeedUnavailable {
                code: "E-FIXTURE".to_string(),
                message: "fixture feed failure".to_string(),
            })
        }
        fn source_identity(&self) -> &str {
            "failing-fixture-source"
        }
    }

    /// A minimal one-variable LP fixture for the optimizer-IR application
    /// tests.
    fn problem() -> OptimizationProblem {
        OptimizationProblem {
            name: "doe-wiring-fixture".to_string(),
            declared_class: ProblemClass::LinearProgram,
            variables: vec![Variable {
                name: "p_connection".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: None,
                upper_bound: None,
            }],
            rows: Vec::new(),
            conic_constraints: Vec::new(),
            objective: Objective {
                sense: ObjectiveSense::Minimize,
                linear_terms: vec![Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                }],
                quadratic_terms: Vec::new(),
                constant: 0.0,
                declared_units: "synthetic_cost_unit".to_string(),
            },
            declared_assumptions: Vec::new(),
            provenance: ProblemProvenance {
                candidate_id: FlexUuid::parse("0192a800-0000-7000-8000-0000000000f1")
                    .expect("fixture uuid"),
                input_snapshot_id: "snap-fim14".to_string(),
                data_timestamp: None,
                grid_model_version: None,
                objective_config_version: None,
                envelope_version: None,
                forecast_version: None,
                data_provenance_label: Some("synthetic".to_string()),
            },
        }
    }

    // -- statements ----------------------------------------------------------

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(DOE_CONSTRAINT_WIRING_STATEMENT.contains("override the StaticConnectionLimit"));
        assert!(DOE_CONSTRAINT_WIRING_STATEMENT.contains("CONSTRAINT DATA"));
        assert!(DOE_CONSTRAINT_WIRING_STATEMENT.contains("never touches the solver"));
        assert!(DOE_ADR010_STATEMENT.contains("never a command"));
        assert!(DOE_ADR010_STATEMENT.contains("command-lifecycle outcome"));
        assert!(DOE_UNIT_PASS_THROUGH_STATEMENT.contains("[-import_limit_mw, +export_limit_mw]"));
        assert!(DOE_UNIT_PASS_THROUGH_STATEMENT.contains("No conversion"));
        assert!(DOE_REVERSION_TELEMETRY_STATEMENT.contains("STALE"));
        assert!(DOE_REVERSION_TELEMETRY_STATEMENT.contains("MISSING"));
        assert!(DOE_REVERSION_TELEMETRY_STATEMENT.contains("26-event vocabulary"));
        assert!(DOE_REVERSION_TELEMETRY_STATEMENT.contains("none is invented"));
        assert!(DOE_REVERSION_TELEMETRY_STATEMENT.contains("freshness_limit_seconds"));
        assert!(DOE_FAIL_CLOSED_STATEMENT.contains("MANDATORY declared data"));
        assert!(DOE_FAIL_CLOSED_STATEMENT.contains("never an unconstrained default"));
        assert!(FIM_F14_HONESTY_STATEMENT.contains("WRITTEN-UNRUN"));
        assert!(FIM_F14_HONESTY_STATEMENT.contains("UNPROVEN"));
        assert!(FIM_F14_HONESTY_STATEMENT.contains("HUMAN LAB GATE"));
        assert!(FIM_F14_HONESTY_STATEMENT.contains("constraint data, never a command"));
        assert_eq!(DOE_ENVELOPE_VERSION_PREFIX, "doe:");
    }

    // -- (a) a live envelope constrains a dispatch the static limit would
    //        have allowed ------------------------------------------------------

    #[test]
    fn fim_f14_05_a_live_envelope_constrains_a_static_allowed_dispatch() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-a", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();

        // At 2_000 ms the envelope is inside its window and fresh (the
        // latest update arrived at issue; the 120 s budget is far away).
        let constraint = resolve_connection_point_constraint(
            &source,
            &static_limit,
            &event(2_000),
            None,
            None,
        )
        .expect("resolution");
        assert!(constraint.is_dynamic());
        assert_eq!(
            constraint.limits,
            ImportExportLimits::new(2.0, 1.0).expect("valid")
        );
        assert_eq!(constraint.bounds_mw, (-2.0, 1.0));
        assert_eq!(constraint.in_force_envelope_id(), Some("doe-fim14-a"));
        assert!(constraint.observations.is_empty(), "no reversion while in force");

        // The proposed export of +1.5 MW is INSIDE the static limit
        // (3.0 MW export) but OUTSIDE the in-force envelope (1.0 MW
        // export) — the live envelope constrains what the static limit
        // would have allowed, and the breach record carries the envelope
        // version in force.
        let verdict = check_dispatch_against_constraint(&constraint, 1.5, 2_000, None)
            .expect("the check");
        let DispatchEnvelopeVerdict::Breach { breach } = verdict else {
            panic!("the proposed dispatch must breach the in-force envelope");
        };
        assert_eq!(breach.envelope_id, Some("doe-fim14-a".to_string()));
        assert_eq!(breach.direction, BreachDirection::Export);
        assert_eq!(breach.limit_mw, 1.0);
        assert_eq!(breach.observed_mw.value(), 1.5);
        assert_eq!(breach.observed_at_unix_ms, 2_000);

        // The same dispatch checked against the STATIC limit alone (the
        // feed absent) is within bounds — proving the constraint came
        // from the live envelope, not from the static limit.
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(2_000), None, None)
                .expect("resolution");
        assert!(matches!(
            check_dispatch_against_constraint(&static_constraint, 1.5, 2_000, None)
                .expect("the check"),
            DispatchEnvelopeVerdict::WithinBounds
        ));
    }

    #[test]
    fn fim_f14_05_a_the_override_replaces_the_static_seat_in_the_problem_ir() {
        // The optimizer-IR level of (a): the variable carries the static
        // seat (-5, 3); the in-force DOE pair (-2, 1) replaces it.
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-a2", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();

        let mut problem = problem();
        let static_constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(1_000), None, None)
                .expect("static resolution");
        apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &static_constraint,
            ConnectionBoundsSeat::Unset,
        )
        .expect("the static pair is applied to the unset seat");
        assert_eq!(problem.variables[0].lower_bound, Some(-5.0));
        assert_eq!(problem.variables[0].upper_bound, Some(3.0));

        let live_constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("live resolution");
        apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &live_constraint,
            ConnectionBoundsSeat::StaticLimit,
        )
        .expect("the in-force DOE pair overrides the static seat");
        assert_eq!(problem.variables[0].lower_bound, Some(-2.0));
        assert_eq!(problem.variables[0].upper_bound, Some(1.0));

        // The problem still validates AS the declared LP (no class change,
        // no solver touched).
        problem
            .validate()
            .expect("the DOE bounds never change the declared class");
        assert_eq!(problem.declared_class, ProblemClass::LinearProgram);

        // On lapse the same seat carries the static pair back (the
        // reversion is the same override in the other direction).
        let lapsed_constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(600_000), Some("doe-fim14-a2"), None)
                .expect("lapsed resolution");
        apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &lapsed_constraint,
            ConnectionBoundsSeat::StaticLimit,
        )
        .expect("the static pair replaces the DOE pair on lapse");
        assert_eq!(problem.variables[0].lower_bound, Some(-5.0));
        assert_eq!(problem.variables[0].upper_bound, Some(3.0));
    }

    // -- (b) an expired envelope reverts to the static limit and the
    //        reversion is telemetered --------------------------------------

    #[test]
    fn fim_f14_05_b_expired_envelope_reverts_and_the_reversion_is_telemetered() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-b", 2.0, 1.0, 1_000, 2_000, 1_000));
        let static_limit = static_limit();

        // At 2_000 ms the envelope is exactly at its expiry: lapsed
        // (Expired), the static fallback governs, and — because the
        // caller declares the envelope WAS in force — the reversion is
        // telemetered with the frozen STALE quality state.
        let constraint = resolve_connection_point_constraint(
            &source,
            &static_limit,
            &event(2_000),
            Some("doe-fim14-b"),
            None,
        )
        .expect("resolution");
        let EnvelopeConstraintState::Static { reason } = &constraint.state else {
            panic!("an expired envelope must revert to the static limit");
        };
        let StaticGovernanceReason::Lapsed {
            reason: lapse,
            envelope_id,
        } = reason
        else {
            panic!("the reason must be the domain lapse reason");
        };
        assert_eq!(*lapse, LapseReason::Expired);
        assert_eq!(envelope_id, "doe-fim14-b");
        assert_eq!(
            constraint.limits,
            ImportExportLimits::new(5.0, 3.0).expect("valid static")
        );
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0), "the static bounds govern");

        assert_eq!(constraint.observations.len(), 1);
        let observation = &constraint.observations[0];
        assert!(observation.is_reversion, "a previously in-force envelope ended");
        assert_eq!(observation.ended_envelope_id, Some("doe-fim14-b".to_string()));
        assert_eq!(observation.quality, TelemetryQuality::Stale);
        assert_eq!(observation.source_identity, "in-memory-configured");
        assert_eq!(observation.observed_at_unix_ms, 2_000);
        assert_eq!(observation.reason.name(), "expired");
        assert_eq!(observation.connection_point.as_str(), POINT);

        // The expiry reversion also moves the optimizer seat back to the
        // static pair (the bounds the reverted problem carries).
        assert_eq!(constraint.variable_bounds_mw(), (Some(-5.0), Some(3.0)));
    }

    // -- (c) absent feed → static limit, fail-closed ------------------------

    #[test]
    fn fim_f14_05_c_absent_feed_fails_closed_to_the_static_limit() {
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_limit = static_limit();

        let constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(5_000), None, None)
                .expect("resolution");
        let EnvelopeConstraintState::Static { reason } = &constraint.state else {
            panic!("no envelope exists without a feed; the static limit governs");
        };
        let StaticGovernanceReason::NoEnvelopeDeclared { source_identity } = reason else {
            panic!("the reason must name the absent feed");
        };
        assert_eq!(source_identity, "unwired-feed-absent");
        assert_eq!(constraint.in_force_envelope_id(), None);
        assert_eq!(
            constraint.limits,
            ImportExportLimits::new(5.0, 3.0).expect("valid static")
        );
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0));
        assert_eq!(constraint.variable_bounds_mw(), (Some(-5.0), Some(3.0)));

        // The absence is a MISSING-quality observation naming the source
        // (the seam is never silent), and it is not a reversion while no
        // envelope was ever in force.
        assert_eq!(constraint.observations.len(), 1);
        let observation = &constraint.observations[0];
        assert_eq!(observation.quality, TelemetryQuality::Missing);
        assert!(!observation.is_reversion);
        assert_eq!(observation.ended_envelope_id, None);
        assert_eq!(observation.source_identity, "unwired-feed-absent");
        assert_eq!(observation.reason.name(), "no_envelope_declared");

        // With a previously in-force envelope the absence IS a reversion
        // (the governance ended at this evaluation) — still MISSING until
        // a value arrives again.
        let constraint = resolve_connection_point_constraint(
            &unwired,
            &static_limit,
            &event(5_000),
            Some("doe-earlier"),
            None,
        )
        .expect("resolution");
        assert_eq!(constraint.observations.len(), 1);
        let observation = &constraint.observations[0];
        assert!(observation.is_reversion);
        assert_eq!(observation.ended_envelope_id, Some("doe-earlier".to_string()));
        assert_eq!(observation.quality, TelemetryQuality::Missing);
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0), "fail-closed to the static pair");

        // A dispatch that the static limit allows is untouched by the
        // absent feed (no fabricated envelope, no invented constraint).
        assert!(matches!(
            check_dispatch_against_constraint(&constraint, 2.5, 5_000, None).expect("the check"),
            DispatchEnvelopeVerdict::WithinBounds
        ));
    }

    // -- (d) staleness per freshness_limit_seconds reverts ------------------

    #[test]
    fn fim_f14_05_d_staleness_per_freshness_limit_reverts() {
        // Cadence 60 s x 2 intervals = a 120 s freshness budget (the EDGE
        // Workstream 3 subject [S-158], domain-owned semantics). The
        // envelope's window runs to 600_000 ms, so ONLY the staleness
        // budget can lapse it here.
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-d", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();

        // Exactly at the budget: still in force (a boundary value is not
        // a violation — the frozen boundary convention the domain uses).
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(120_000), Some("doe-fim14-d"), None)
                .expect("resolution");
        assert!(constraint.is_dynamic());
        assert!(constraint.observations.is_empty());

        // One millisecond past the budget: stale INSIDE its window — the
        // static limit governs and the reversion is telemetered (STALE).
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(121_001), Some("doe-fim14-d"), None)
                .expect("resolution");
        let EnvelopeConstraintState::Static { reason } = &constraint.state else {
            panic!("an envelope past its freshness budget must revert");
        };
        let StaticGovernanceReason::Lapsed { reason: lapse, .. } = reason else {
            panic!("the reason must be the domain lapse reason");
        };
        assert_eq!(*lapse, LapseReason::Stale);
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0));
        assert_eq!(constraint.observations.len(), 1);
        let observation = &constraint.observations[0];
        assert!(observation.is_reversion);
        assert_eq!(observation.quality, TelemetryQuality::Stale);
        assert_eq!(observation.ended_envelope_id, Some("doe-fim14-d".to_string()));
        assert_eq!(observation.reason.name(), "stale");
    }

    // -- (e) no silent unit conversion (MW pass-through) --------------------

    #[test]
    fn fim_f14_05_e_no_silent_unit_conversion_mw_pass_through() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-e", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();

        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("resolution");

        // The bounds pair is EXACTLY [-import, +export] — bit-exact
        // pass-through of the declared MW values, no scaling.
        assert_eq!(constraint.bounds_mw.0, -2.0);
        assert_eq!(constraint.bounds_mw.1, 1.0);
        assert_eq!(constraint.variable_bounds_mw(), (Some(-2.0), Some(1.0)));

        // The problem-IR application refuses a non-MW variable instead of
        // converting the bounds to it.
        let mut problem = problem();
        problem.variables[0].unit = VariableUnit::ReactivePowerMVAr;
        let err = apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::Unset,
        )
        .expect_err("a non-MW variable is a named refusal, never a conversion");
        assert_eq!(err.name(), "DOE_WIRING_VARIABLE_UNIT_MISMATCH");
        let DoeWiringError::VariableUnitMismatch { found, .. } = err else {
            panic!("expected the unit mismatch");
        };
        assert_eq!(found, "MVAr");

        // On a correctly declared ActivePowerMW variable the applied
        // bounds are the EXACT pass-through pair.
        let mut problem = problem();
        apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::Unset,
        )
        .expect("the MW pair is applied unchanged");
        assert_eq!(problem.variables[0].lower_bound, Some(-2.0));
        assert_eq!(problem.variables[0].upper_bound, Some(1.0));

        // The dispatch check works on the SAME MW values with no
        // reinterpretation of the sign convention (P > 0 = export): the
        // boundary value itself is not a violation, the first MW past it
        // is.
        assert!(matches!(
            check_dispatch_against_constraint(&constraint, 1.0, 2_000, None).expect("the check"),
            DispatchEnvelopeVerdict::WithinBounds
        ));
        assert!(matches!(
            check_dispatch_against_constraint(&constraint, -2.0, 2_000, None).expect("the check"),
            DispatchEnvelopeVerdict::WithinBounds
        ));
        let verdict = check_dispatch_against_constraint(&constraint, -2.5, 2_000, None)
            .expect("the check");
        let DispatchEnvelopeVerdict::Breach { breach } = verdict else {
            panic!("an import past the import limit must breach");
        };
        assert_eq!(breach.direction, BreachDirection::Import);
        assert_eq!(breach.limit_mw, 2.0, "the import limit passes through unchanged");
        assert_eq!(breach.observed_mw.value(), -2.5, "the sign is carried, never reinterpreted");
    }

    // -- fail-closed feed errors ----------------------------------------------

    #[test]
    fn a_feed_error_fails_closed_to_the_static_limit_with_a_named_observation() {
        let static_limit = static_limit();
        let source = FailingDoeSource;

        // No previous envelope: the feed loss is MISSING (nothing was
        // ever received).
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("a feed error is not a wiring error — it fails closed");
        assert!(matches!(
            constraint.state,
            EnvelopeConstraintState::Static {
                reason: StaticGovernanceReason::FeedUnavailable { .. }
            }
        ));
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0));
        assert_eq!(constraint.observations.len(), 1);
        let observation = &constraint.observations[0];
        assert_eq!(observation.quality, TelemetryQuality::Missing);
        assert!(!observation.is_reversion);
        assert_eq!(observation.source_identity, "failing-fixture-source");
        assert_eq!(observation.reason.name(), "feed_unavailable");

        // With a last known envelope: the frozen COMMUNICATION_LOSS
        // source-state mapping (P0.08) is REUSED — STALE, never invented
        // anew.
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), Some("doe-last"), None)
                .expect("resolution");
        let observation = &constraint.observations[0];
        assert_eq!(observation.quality, TelemetryQuality::Stale);
        assert!(observation.is_reversion);
        assert_eq!(observation.ended_envelope_id, Some("doe-last".to_string()));
    }

    // -- the seat rules (fail-closed composition) ------------------------------

    #[test]
    fn the_other_declared_authority_seat_never_widens() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-seat", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("resolution");

        // A variable seated at another authority with a LOOSER bound pair
        // (-4.0, 2.0): the in-force DOE pair (-2.0, 1.0) narrows BOTH
        // sides — allowed, and the applied pair is what lands.
        let mut problem = problem();
        problem.variables[0].lower_bound = Some(-4.0);
        problem.variables[0].upper_bound = Some(2.0);
        apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::OtherDeclaredAuthority,
        )
        .expect("a narrowing application is allowed");
        assert_eq!(problem.variables[0].lower_bound, Some(-2.0));
        assert_eq!(problem.variables[0].upper_bound, Some(1.0));

        // A variable seated at another authority whose IMPORT side is
        // tighter (-0.5): the DOE pair would WIDEN the import side —
        // refused by name.
        let mut problem = problem();
        problem.variables[0].lower_bound = Some(-0.5);
        problem.variables[0].upper_bound = Some(4.0);
        let err = apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::OtherDeclaredAuthority,
        )
        .expect_err("a widening application onto a foreign authority is refused");
        assert_eq!(err.name(), "DOE_WIRING_WOULD_WIDEN_EXISTING_BOUND");

        // The EXPORT side alone tightening is refused the same way.
        let mut problem = problem();
        problem.variables[0].lower_bound = Some(-4.0);
        problem.variables[0].upper_bound = Some(0.5);
        let err = apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::OtherDeclaredAuthority,
        )
        .expect_err("a widening application onto a foreign authority is refused");
        assert_eq!(err.name(), "DOE_WIRING_WOULD_WIDEN_EXISTING_BOUND");

        // The Unset seat refuses a conflict instead of overwriting.
        let mut problem = problem();
        problem.variables[0].upper_bound = Some(0.5);
        let err = apply_connection_point_bounds(
            &mut problem,
            "p_connection",
            &constraint,
            ConnectionBoundsSeat::Unset,
        )
        .expect_err("an unset-seat conflict is refused");
        assert_eq!(err.name(), "DOE_WIRING_SEAT_CONFLICT");

        // An unknown variable is refused by name.
        let mut problem = problem();
        let err = apply_connection_point_bounds(
            &mut problem,
            "no_such_variable",
            &constraint,
            ConnectionBoundsSeat::Unset,
        )
        .expect_err("an unknown variable is refused");
        assert_eq!(err.name(), "DOE_WIRING_UNKNOWN_OR_DUPLICATE_VARIABLE");
    }

    // -- curtailment recording (FIM-F14.04) --------------------------------------

    #[test]
    fn the_curtailment_clip_records_the_reduction_with_the_in_force_envelope() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-c", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("resolution");

        // Proposed +1.5 MW export against the 1.0 MW export limit: the
        // clip is exactly the bound (pass-through, no scaling) and the
        // record carries the envelope version in force.
        let outcome = curtail_proposed_to_bounds(&constraint, 1.5, 2_000, None).expect("the clip");
        let CurtailmentOutcome::Curtailed {
            clipped_p_mw,
            record,
        } = outcome
        else {
            panic!("the proposed dispatch must be curtailed to the bound");
        };
        assert_eq!(clipped_p_mw, 1.0);
        assert_eq!(record.envelope_id, Some("doe-fim14-c".to_string()));
        assert_eq!(record.curtailed_mw, 0.5);
        assert_eq!(record.direction, BreachDirection::Export);
        assert_eq!(record.recorded_at_unix_ms, 2_000);

        // Inside the bounds nothing is curtailed and no record exists.
        assert!(matches!(
            curtail_proposed_to_bounds(&constraint, 0.25, 2_000, None).expect("the clip"),
            CurtailmentOutcome::WithinBounds
        ));

        // Import side: proposed -3.0 MW against the 2.0 MW import limit.
        let outcome = curtail_proposed_to_bounds(&constraint, -3.0, 2_000, None).expect("the clip");
        let CurtailmentOutcome::Curtailed {
            clipped_p_mw,
            record,
        } = outcome
        else {
            panic!("the proposed import must be curtailed to the bound");
        };
        assert_eq!(clipped_p_mw, -2.0);
        assert_eq!(record.direction, BreachDirection::Import);
        assert_eq!(record.curtailed_mw, 1.0);

        // The static-governing constraint records the same clip with NO
        // envelope id (the honest no-DOE representation).
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(2_000), None, None)
                .expect("resolution");
        let outcome =
            curtail_proposed_to_bounds(&static_constraint, 3.5, 2_000, None).expect("the clip");
        let CurtailmentOutcome::Curtailed { record, .. } = outcome else {
            panic!("the proposed export must be curtailed to the static bound");
        };
        assert!(record.envelope_id.is_none());
        assert_eq!(record.curtailed_mw, 0.5);
    }

    // -- multi-point resolution -----------------------------------------------------

    #[test]
    fn multi_point_resolution_preserves_the_declared_order_and_refuses_duplicates() {
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_limit = static_limit();
        let other_limit = StaticConnectionLimit::new(
            other_point(),
            ImportExportLimits::new(4.0, 2.0).expect("valid limits"),
            "connection agreement (declared data, fixture)".to_string(),
            Provenance::Configured,
        )
        .expect("valid static limit");

        let previous = vec![PointPreviousEnvelope {
            connection_point: POINT.to_string(),
            envelope_id: "doe-earlier".to_string(),
        }];
        let resolved = resolve_connection_point_constraints(
            &unwired,
            &[static_limit.clone(), other_limit],
            &event(5_000),
            &previous,
            None,
        )
        .expect("resolution");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].connection_point.as_str(), POINT);
        assert_eq!(resolved[1].connection_point.as_str(), POINT_OTHER);
        assert_eq!(resolved[0].bounds_mw, (-5.0, 3.0));
        assert_eq!(resolved[1].bounds_mw, (-4.0, 2.0));
        // The previous-state entry matched ONLY its own point.
        assert!(resolved[0].observations[0].is_reversion);
        assert!(!resolved[1].observations[0].is_reversion);

        // A duplicated point in the declared list is a named refusal.
        let err = resolve_connection_point_constraints(
            &unwired,
            &[static_limit.clone(), static_limit],
            &event(5_000),
            &[],
            None,
        )
        .expect_err("a duplicated static limit is refused");
        assert_eq!(err.name(), "DOE_WIRING_DUPLICATE_STATIC_LIMIT");
    }

    // -- provenance helpers ------------------------------------------------------

    #[test]
    fn the_provenance_helpers_carry_the_version_and_the_assumption() {
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-p", 2.0, 1.0, 1_000, 600_000, 1_000));
        let static_limit = static_limit();

        let live =
            resolve_connection_point_constraint(&source, &static_limit, &event(2_000), None, None)
                .expect("resolution");
        assert_eq!(
            live.doe_envelope_version_field(),
            Some("doe:doe-fim14-p".to_string()),
            "the DOE id travels prefixed, never conflated with the P3.13 envelope version"
        );
        let assumption = doe_constraint_assumption_text(&live);
        assert!(assumption.contains("doe-fim14-p"));
        assert!(assumption.contains("IN-FORCE"));
        assert!(assumption.contains("CONSTRAINT DATA"));
        assert!(assumption.contains("never a command"));

        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(2_000), None, None)
                .expect("resolution");
        assert_eq!(static_constraint.doe_envelope_version_field(), None);
        let assumption = doe_constraint_assumption_text(&static_constraint);
        assert!(assumption.contains("STATIC connection limit"));
        assert!(assumption.contains("no envelope declared"));
    }

    // -- error rendering -----------------------------------------------------------

    #[test]
    fn every_wiring_error_renders_non_empty_prose() {
        let errors = [
            DoeWiringError::DuplicateStaticLimit {
                connection_point: "cp-x".to_string(),
            },
            DoeWiringError::UnknownOrDuplicateVariable {
                name: "p".to_string(),
                matches: 2,
            },
            DoeWiringError::VariableUnitMismatch {
                name: "p".to_string(),
                found: "MVAr",
            },
            DoeWiringError::WouldWidenExistingBound {
                name: "p".to_string(),
                existing: (Some(-1.0), Some(1.0)),
                applied: (-2.0, 2.0),
            },
            DoeWiringError::SeatConflict {
                name: "p".to_string(),
            },
            DoeWiringError::NonFiniteValue {
                context: "fixture",
            },
            DoeWiringError::DomainEvaluation(DoeError::ZeroStalenessBudget),
            DoeWiringError::TimestampOutOfRange { value: 1 },
        ];
        for error in &errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn a_non_finite_dispatch_is_refused_never_classified() {
        let unwired = crate::doe_constraint_source::UnwiredDoeEnvelopeSource;
        let static_limit = static_limit();
        let constraint =
            resolve_connection_point_constraint(&unwired, &static_limit, &event(2_000), None, None)
                .expect("resolution");
        let err = check_dispatch_against_constraint(&constraint, f64::NAN, 2_000, None)
            .expect_err("NaN is refused");
        assert_eq!(err.name(), "DOE_WIRING_NON_FINITE_VALUE");
        let err = curtail_proposed_to_bounds(&constraint, f64::NAN, 2_000, None)
            .expect_err("NaN is refused");
        assert_eq!(err.name(), "DOE_WIRING_NON_FINITE_VALUE");
    }

    #[test]
    fn a_not_yet_in_effect_envelope_is_a_scheduled_state_without_an_observation() {
        // Pre-issue: the static limit governs by schedule, no data-quality
        // fact exists, and no reversion observation fires (the state
        // names the reason — the seam stays visible without noise).
        let mut source = crate::doe_constraint_source::InMemoryDoeEnvelopeSource::new();
        source.declare(envelope("doe-fim14-f", 2.0, 1.0, 5_000, 9_000, 5_000));
        let static_limit = static_limit();
        let constraint =
            resolve_connection_point_constraint(&source, &static_limit, &event(4_999), Some("doe-earlier"), None)
                .expect("resolution");
        let EnvelopeConstraintState::Static { reason } = &constraint.state else {
            panic!("a pre-issue envelope must leave the static limit governing");
        };
        let StaticGovernanceReason::Lapsed { reason: lapse, .. } = reason else {
            panic!("the reason must be the domain lapse reason");
        };
        assert_eq!(*lapse, LapseReason::NotYetInEffect);
        assert!(constraint.observations.is_empty());
        assert_eq!(constraint.bounds_mw, (-5.0, 3.0));
    }
}
