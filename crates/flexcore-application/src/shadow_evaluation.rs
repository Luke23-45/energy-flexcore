// P4.31 — Shadow evaluation (§21.4)
//         (report §19.2 layer 9, §19.1, §21.4; ledger P4.31 steps 3–4;
//         decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md`).
//
// Shadow mode (§21.4): proposed actions are computed and compared against
// measured behaviour WITHOUT issuing commands. This module is the Phase 4
// comparison METHOD for dispatch proposals: each proposed command's
// requested value is compared against the simulator's BASELINE (no-command)
// behaviour at the same instants, within a configured tolerance — and the
// result is a report, never a dispatch input.
//
// ## The seam statement
//
// flexcore-application may depend only on flexcore-contracts, flexcore-
// domain, flexcore-ports, flexcore-events, and flexcore-optimization-api
// (tools/check_dependency_rule.py) — it CANNOT name flexcore-simulator.
// The simulator's baseline behaviour is therefore reached through the
// [`ShadowBaselineSource`] port declared HERE, implemented by the
// composition root over the Phase 2 simulator crate (the same seam pattern
// as `dispatch_simulator.rs`'s `SimulatorDispatchTransport`). The port is
// READ-ONLY: it has exactly one method (`baseline_sample`) — there is no
// deliver, no issue, no write of any kind, and the structural test in
// `tests/shadow_evaluation.rs` pins the port shape and scans this source
// for forbidden write-path identifiers.
//
// ## The comparison method (recorded once)
//
// For each requested comparison instant, the baseline sample at that
// instant is pulled through the port and classified:
//
// 1. no baseline sample → `NotComparable` (fail-closed — a MISSING
//    baseline is never read as agreement, the §10.4/§17.5-invariant-9
//    discipline applied to shadow);
// 2. a baseline sample whose quantity differs from the requested quantity
//    → `NotCompatible` quantity mismatch (fail-closed, never coerced —
//    the P4.16 observation-mismatch rule);
// 3. a baseline sample older than the configured maximum age →
//    `NotComparable` stale baseline (the §10.4 STALE discipline;
//    staleness is judged against the SUPPLIED controlled-clock `now`,
//    never a wall clock);
// 4. a baseline sample observed AFTER `now` (a future-stamped sample —
//    clock skew, §17.4) → `NotComparable`, never "close enough";
// 5. `|baseline − requested| ≤ tolerance` → `Agreement` (the delta is
//    recorded, not just the boolean);
// 6. otherwise → `Divergence` (baseline, requested, and delta recorded).
//
// The overall verdict is the CONSERVATIVE composition: any
// not-comparable entry makes the report `NotComparable`; else any
// divergence makes it `Divergence`; else `Agreement`. A single
// divergent or missing sample is never averaged away.
//
// ## What shadow agreement IS and IS NOT (the limits, carried on every report)
//
// [`SHADOW_LIMITS_STATEMENT`] travels on every [`ShadowReport`] as a
// field — the report type cannot be constructed without it (the field is
// populated from the constant; the test pins it). In short: shadow
// agreement is NOT a safety claim; shadow results are NEVER evidence for
// field dispatch; a shadow report is not a §14.3 evidence reference and
// is not attachable to the authorization gate. The simulator baseline is
// a synthetic behaviour profile — a stated assumption, never a validated
// physical model (P2.15; the recorded local-protection boundary of
// `29_safety_invariant_matrix.md` applies: the simulator models NO local
// protection). Nothing is issued by this module — it has no
// command-delivery path ([`NOTHING_ISSUED_STATEMENT`]).
//
// ## Control status
//
// The entire 0.x line is simulation-only (§21.4). Shadow evaluation is a
// read-only observation discipline; FlexCore performs no primary frequency
// response, synthetic inertia, relay protection, inverter inner-loop
// control, or sub-cycle control (§14.5), and this module performs no
// control at all — it only compares and reports.

use flexcore_domain::ids::{AssetId, CommandId};
use flexcore_domain::tenant_context::TenantContext;

use crate::dispatch_simulator::{SimulatorTargetValue, TrackedQuantity};

/// The comparison method statement (see the module header). Recorded so
/// tests and reviewers can pin what the method is.
pub const SHADOW_METHOD_STATEMENT: &str = "shadow evaluation compares each proposed command's \
     requested value against the simulator's BASELINE (no-command) behaviour at the same \
     instants, within a configured absolute tolerance in the quantity's canonical unit \
     (§21.4); the comparison is read-only over the ShadowBaselineSource port — no command is \
     issued to any target — and the baseline is the Phase 2 synthetic behaviour profile, a \
     stated assumption, never a validated physical model";

/// The limits statement carried on EVERY shadow report (the ledger P4.31
/// risk note: "shadow results presented as evidence for field dispatch" —
/// the mitigation is that the report states its limits in its header).
pub const SHADOW_LIMITS_STATEMENT: &str = "shadow agreement is NOT a safety claim: agreement \
     with a synthetic simulator baseline shows only that the proposal is consistent with the \
     modelled behaviour profile; shadow results are NEVER evidence for field dispatch, are \
     not §14.3 evidence references, and are not attachable to the authorization gate; the \
     entire 0.x line is simulation-only and operator-approved field control remains a v1.0.0 \
     matter (§21.4)";

/// The nothing-issued statement: what this module can and cannot reach.
pub const NOTHING_ISSUED_STATEMENT: &str = "nothing is issued: shadow evaluation reads the \
     simulator baseline through a read-only, observe-only port and has NO command-delivery \
     path; no device, gateway, protocol adapter, or field endpoint is reachable from this \
     module";

/// The evidence status of every shadow report (never a §14.3 evidence
/// reference — see [`SHADOW_LIMITS_STATEMENT`]).
pub const SHADOW_EVIDENCE_STATUS: &str = "not_evidence_for_field_dispatch";

// --- The baseline seam -----------------------------------------------------------------

/// One baseline observation pulled from the simulator's no-command run:
/// what the asset would have done WITHOUT the proposed command. The
/// composition root implements [`ShadowBaselineSource`] over the Phase 2
/// simulator (deterministic scenario replay at the supplied instant).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowBaselineSample {
    /// Which quantity the baseline observation carries.
    pub quantity: TrackedQuantity,
    /// The baseline value (canonical units).
    pub value: f64,
    /// The baseline observation instant (scenario/controlled clock, UTC
    /// millis). A sample stamped after the comparison instant is a clock
    /// skew and is fail-closed, never "close enough".
    pub observed_at_unix_millis: i64,
}

/// The read-only baseline port (the ONLY integration point with the
/// simulator). Deliberately NARROW: one observe-only method — there is no
/// deliver, no issue, no write of any kind. Tenant scoping is the
/// implementation's obligation (P0.16 rule 5), matching every other store
/// seam.
pub trait ShadowBaselineSource {
    /// Pulls the baseline sample for the asset at the supplied (controlled)
    /// instant, or `None` when the baseline run holds no sample there
    /// (absence is a verdict input — never silently skipped).
    fn baseline_sample(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
        at_unix_millis: i64,
    ) -> Result<Option<ShadowBaselineSample>, ShadowEvaluationError>;
}

// --- Configuration -----------------------------------------------------------------------

/// The shadow comparison configuration (deployment settings; never
/// hard-coded thresholds — the P4.16 criterion discipline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowComparisonConfig {
    /// The absolute agreement tolerance in the quantity's canonical unit
    /// (MW / MVAr / fraction). Finite and strictly positive.
    pub tolerance: f64,
    /// The maximum age of a baseline sample relative to the comparison
    /// `now` (milliseconds; strictly positive). A sample older than this
    /// is a stale baseline — `NotComparable`, never agreement.
    pub baseline_max_age_millis: i64,
}

impl ShadowComparisonConfig {
    /// Validates and builds the configuration.
    pub fn new(tolerance: f64, baseline_max_age_millis: i64) -> Result<Self, ShadowEvaluationError> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(ShadowEvaluationError::InvalidConfiguration {
                detail: format!(
                    "the shadow comparison tolerance must be finite and positive, got {tolerance}"
                ),
            });
        }
        if baseline_max_age_millis <= 0 {
            return Err(ShadowEvaluationError::InvalidConfiguration {
                detail: format!(
                    "the baseline maximum age must be positive, got {baseline_max_age_millis}"
                ),
            });
        }
        Ok(Self {
            tolerance,
            baseline_max_age_millis,
        })
    }
}

// --- Errors -------------------------------------------------------------------------------

/// The named errors of shadow evaluation. Every variant is a refusal —
/// the evaluation never degrades a failure into a comparison result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowEvaluationError {
    /// The configuration or the input instants are invalid.
    InvalidConfiguration {
        /// The recorded detail.
        detail: String,
    },
    /// The baseline source seam failed (opaque code + human-safe message —
    /// the infrastructure-error discipline; no internals).
    BaselineSourceFailed {
        /// The opaque failure code.
        code: String,
        /// The human-safe message.
        message: String,
    },
}

impl std::fmt::Display for ShadowEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShadowEvaluationError::InvalidConfiguration { detail } => write!(f, "{detail}"),
            ShadowEvaluationError::BaselineSourceFailed { code, message } => {
                write!(f, "shadow baseline source failed ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ShadowEvaluationError {}

// --- The comparison results -----------------------------------------------------------------

/// Why one comparison instant is NOT comparable. Every variant is
/// fail-closed: a not-comparable entry is never read as agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotComparableReason {
    /// The baseline run holds no sample at the instant (absence — the
    /// §10.4 MISSING discipline applied to shadow).
    NoBaseline,
    /// The baseline sample is older than the configured maximum age
    /// (§10.4 STALE).
    StaleBaseline {
        /// The stale sample's instant.
        observed_at_unix_millis: i64,
        /// The configured maximum age.
        max_age_millis: i64,
    },
    /// The baseline sample is stamped AFTER the comparison instant — a
    /// clock skew (§17.4), never "close enough".
    BaselineFromTheFuture {
        /// The sample's future-stamped instant.
        observed_at_unix_millis: i64,
        /// The comparison instant.
        now_unix_millis: i64,
    },
    /// The baseline sample carries a DIFFERENT quantity than the proposed
    /// command targets — never coerced (the P4.16 mismatch rule).
    QuantityMismatch {
        /// The baseline sample's quantity label.
        baseline_quantity: &'static str,
        /// The requested quantity label.
        requested_quantity: &'static str,
    },
}

/// One instant's comparison outcome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowComparisonOutcome {
    /// `|baseline − requested| ≤ tolerance` — the delta is recorded, not
    /// just the boolean.
    Agreement {
        /// The baseline value.
        baseline: f64,
        /// The absolute delta `|baseline − requested|`.
        delta: f64,
    },
    /// The delta exceeds the tolerance — recorded with all three values.
    Divergence {
        /// The baseline value.
        baseline: f64,
        /// The requested value.
        requested: f64,
        /// The absolute delta `|baseline − requested|`.
        delta: f64,
    },
    /// The instant is not comparable (fail-closed — see
    /// [`NotComparableReason`]).
    NotComparable(NotComparableReason),
}

/// One comparison instant's entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowComparisonEntry {
    /// The compared instant (controlled clock, UTC millis).
    pub at_unix_millis: i64,
    /// The outcome at that instant.
    pub outcome: ShadowComparisonOutcome,
}

/// The overall verdict — the CONSERVATIVE composition (any
/// not-comparable entry makes the whole report not comparable; else any
/// divergence makes it divergent; else agreement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowOverallVerdict {
    /// Every instant agreed within tolerance.
    Agreement,
    /// At least one instant diverged beyond tolerance (and none were
    /// not comparable).
    Divergence,
    /// At least one instant was not comparable (fail-closed).
    NotComparable,
}

impl ShadowOverallVerdict {
    /// The stable recorded text (report consumers, Phase 5 display).
    pub fn as_str(self) -> &'static str {
        match self {
            ShadowOverallVerdict::Agreement => "agreement",
            ShadowOverallVerdict::Divergence => "divergence",
            ShadowOverallVerdict::NotComparable => "not_comparable",
        }
    }
}

// --- The report -----------------------------------------------------------------------------

/// The shadow evaluation report (P4.31 step 3). The header/limits
/// statements TRAVEL ON THE TYPE — a report cannot be constructed without
/// them (all three fields are populated from the module constants), and
/// [`ShadowReport::evidence_status`] always reports
/// [`SHADOW_EVIDENCE_STATUS`]. This type is NOT a §14.3 evidence
/// reference: it carries no method that produces one, and the structural
/// test pins that.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowReport {
    /// The evaluated (but NEVER issued) command.
    pub command_id: CommandId,
    /// The asset the proposal targets.
    pub asset_id: AssetId,
    /// The requested quantity label (the frozen
    /// [`SimulatorTargetValue::quantity`] vocabulary).
    pub requested_quantity: &'static str,
    /// The requested value (canonical units).
    pub requested_value: f64,
    /// One entry per requested comparison instant, in input order.
    pub entries: Vec<ShadowComparisonEntry>,
    /// The conservative overall verdict.
    pub overall: ShadowOverallVerdict,
    /// The evaluation instant (controlled clock, UTC millis).
    pub evaluated_at_unix_millis: i64,
    /// THE LIMITS STATEMENT — always [`SHADOW_LIMITS_STATEMENT`]. Shadow
    /// agreement is not a safety claim; shadow results are never evidence
    /// for field dispatch.
    pub limits_statement: &'static str,
    /// The comparison-method statement — always
    /// [`SHADOW_METHOD_STATEMENT`].
    pub method_statement: &'static str,
    /// The nothing-issued statement — always [`NOTHING_ISSUED_STATEMENT`].
    pub issued_statement: &'static str,
}

impl ShadowReport {
    /// The number of agreement entries.
    pub fn agreement_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.outcome, ShadowComparisonOutcome::Agreement { .. }))
            .count()
    }

    /// The number of divergence entries.
    pub fn divergence_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.outcome, ShadowComparisonOutcome::Divergence { .. }))
            .count()
    }

    /// The number of not-comparable entries.
    pub fn not_comparable_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.outcome, ShadowComparisonOutcome::NotComparable(_)))
            .count()
    }

    /// The evidence status of this report — ALWAYS
    /// [`SHADOW_EVIDENCE_STATUS`]. There is no code path that turns a
    /// shadow report into dispatch evidence.
    pub fn evidence_status(&self) -> &'static str {
        SHADOW_EVIDENCE_STATUS
    }
}

// --- The evaluation input and the pure driver -------------------------------------------------

/// Everything one shadow evaluation needs. `now_unix_millis` and every
/// entry of `instants_unix_millis` come from the caller's CONTROLLED
/// clock — never a wall clock; a comparison instant after `now` is
/// rejected as invalid configuration (comparing against a future instant
/// would be speculation, not evaluation).
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowComparisonInput {
    /// The proposed (never issued) command.
    pub command_id: CommandId,
    /// The asset the proposal targets.
    pub asset_id: AssetId,
    /// The requested target (canonical units).
    pub target: SimulatorTargetValue,
    /// The comparison instants (controlled-clock millis; each must be
    /// `<= now`; the list must be non-empty).
    pub instants_unix_millis: Vec<i64>,
    /// The controlled-clock `now` the staleness and future-stamp checks
    /// are judged against.
    pub now_unix_millis: i64,
    /// The configured tolerance and baseline-max-age settings.
    pub config: ShadowComparisonConfig,
}

/// Evaluates the shadow comparison for one proposed command (P4.31 step
/// 3). READ-ONLY over the baseline port; NOTHING is issued (the module
/// has no delivery path). Deterministic: the same input and the same
/// baseline sequence produce the same report (the in-suite double-run
/// assertion pins this).
pub fn evaluate_shadow_comparison(
    ctx: &TenantContext,
    baseline: &dyn ShadowBaselineSource,
    input: ShadowComparisonInput,
) -> Result<ShadowReport, ShadowEvaluationError> {
    if input.instants_unix_millis.is_empty() {
        return Err(ShadowEvaluationError::InvalidConfiguration {
            detail: "at least one comparison instant is required".to_string(),
        });
    }
    if let Some(&instant) = input
        .instants_unix_millis
        .iter()
        .find(|&&instant| instant > input.now_unix_millis)
    {
        return Err(ShadowEvaluationError::InvalidConfiguration {
            detail: format!(
                "comparison instant {instant} is after the controlled now {} — comparing \
                 against a future instant is speculation, not evaluation",
                input.now_unix_millis
            ),
        });
    }

    let requested_quantity = input.target.quantity();
    let requested_value = input.target.value();
    let mut entries = Vec::with_capacity(input.instants_unix_millis.len());
    for &instant in &input.instants_unix_millis {
        let sample = baseline
            .baseline_sample(ctx, input.asset_id, instant)
            .map_err(|error| ShadowEvaluationError::BaselineSourceFailed {
                code: "shadow_baseline_source".to_string(),
                message: error.to_string(),
            })?;
        let outcome = match sample {
            None => ShadowComparisonOutcome::NotComparable(NotComparableReason::NoBaseline),
            Some(sample) => {
                if sample.quantity != input.target.quantity() {
                    ShadowComparisonOutcome::NotComparable(
                        NotComparableReason::QuantityMismatch {
                            baseline_quantity: sample.quantity.label(),
                            requested_quantity,
                        },
                    )
                } else if sample.observed_at_unix_millis > input.now_unix_millis {
                    ShadowComparisonOutcome::NotComparable(
                        NotComparableReason::BaselineFromTheFuture {
                            observed_at_unix_millis: sample.observed_at_unix_millis,
                            now_unix_millis: input.now_unix_millis,
                        },
                    )
                } else if input.now_unix_millis - sample.observed_at_unix_millis
                    > input.config.baseline_max_age_millis
                {
                    ShadowComparisonOutcome::NotComparable(NotComparableReason::StaleBaseline {
                        observed_at_unix_millis: sample.observed_at_unix_millis,
                        max_age_millis: input.config.baseline_max_age_millis,
                    })
                } else {
                    let delta = (sample.value - requested_value).abs();
                    if delta <= input.config.tolerance {
                        ShadowComparisonOutcome::Agreement {
                            baseline: sample.value,
                            delta,
                        }
                    } else {
                        ShadowComparisonOutcome::Divergence {
                            baseline: sample.value,
                            requested: requested_value,
                            delta,
                        }
                    }
                }
            }
        };
        entries.push(ShadowComparisonEntry {
            at_unix_millis: instant,
            outcome,
        });
    }

    let overall = if entries
        .iter()
        .any(|e| matches!(e.outcome, ShadowComparisonOutcome::NotComparable(_)))
    {
        ShadowOverallVerdict::NotComparable
    } else if entries
        .iter()
        .any(|e| matches!(e.outcome, ShadowComparisonOutcome::Divergence { .. }))
    {
        ShadowOverallVerdict::Divergence
    } else {
        ShadowOverallVerdict::Agreement
    };

    Ok(ShadowReport {
        command_id: input.command_id,
        asset_id: input.asset_id,
        requested_quantity,
        requested_value,
        entries,
        overall,
        evaluated_at_unix_millis: input.now_unix_millis,
        limits_statement: SHADOW_LIMITS_STATEMENT,
        method_statement: SHADOW_METHOD_STATEMENT,
        issued_statement: NOTHING_ISSUED_STATEMENT,
    })
}
