// P4.06 — Feasibility-gate pipeline, step 5: bus-voltage, branch
// current/thermal, transformer-loading, and generator/inverter equipment
// limit verification WITH margins (report §12.5 step 5; ledger P4.06).
// Decision record:
// `ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md`
// (decisions D-P4.06-1..3).
//
// ```text
// step 5 (this module)   verify the converged power-flow result against the
//                        configured limit set — EVERY limit, EVERY violation
//                        named, EVERY margin recorded
// ```
//
// DECIDED HERE (recorded in the decision note):
// - **D-P4.06-1 — limit ownership:** limits are CONFIGURATION carried per
//   network model, supplied by the caller (composition root) — the voltage
//   bands and loading fractions in [`NetworkLimits`] plus the generator /
//   inverter apparent-power capability in [`GeneratorLimitSet`]. For the
//   0.x synthetic feeders the values derive from the fixture definitions;
//   the source is recorded PER LIMIT on every check record. No limit is
//   hard-coded in the check logic; the one numeric constant in this module
//   ([`PHYSICAL_RATING_FRACTION`]) is the model-data-derived definition of
//   "physical rating", not a configured limit.
// - **D-P4.06-2 — margins are evidence:** every check records the measured
//   value, the limit, the margin in the limit's own units, AND the margin
//   as a fraction of the limit where meaningful. P4.35's runtime and
//   diagnostics measurement consumes these records; the §12.5 step 7
//   diagnostics (worst margin, binding constraint, per-element loading
//   percentages) are computed from them.
// - **D-P4.06-3 — boundary convention, decided once:** `value == limit` is
//   WITHIN limits (boundary-inclusive; margin 0). A violation requires the
//   measured value STRICTLY beyond the limit. Tested on both sides. This
//   matches the crate's existing conventions (`age == limit` not stale,
//   P2.08/P3.13; the port-level `evaluate_limits` strict comparisons).
//
// MULTI-VIOLATION DESIGN: the report's fixtures violate one limit at a
// time, which makes "the named constraint" ambiguous if a candidate
// violates two limits at once. This verifier therefore names EVERY violated
// constraint ([`LimitCheckReport::violations`], one named constraint per
// failed check) — a single-violation fixture yields exactly one name; the
// type supports any number.
//
// FAIL-CLOSED SHAPE: a non-converged run, an incomplete result, an invalid
// limit configuration, or a candidate injection at a bus with no declared
// apparent-power capability NEVER produces a within-limits report — the
// report status is INCONCLUSIVE or FAIL-CLOSED (the P4.08 triggers
// `POWER_FLOW_NON_CONVERGENCE` / `CAPABILITY_DATA_MISSING` /
// `VALIDATOR_FAILED`), never a pass (§17.5 invariant 9).
//
// SOLVER-CLASS DISCIPLINE: this module CHECKS limits against a converged
// AC power-flow result. It does not optimize, does not propose re-dispatch
// (a constraining PROPOSAL is P4.07's evidence, and the gate never
// optimizes — §12.5), and never runs an AC-OPF.
//
// Control status: simulation-only 0.x line; a within-limits report means
// the candidate satisfied the DECLARED model and limits within stated
// tolerances — not that it is safe in the field, certified, or approved.

use crate::fail_closed::{fail_closed_outcome, FailClosedOutcome, FailClosedTrigger};
use crate::port::{NetworkLimits, PowerFlowResult, PowerFlowStatus};
use crate::scope::ScopeDeclaration;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recorded statements
// ---------------------------------------------------------------------------

/// The boundary convention (D-P4.06-3), decided once and recorded.
pub const BOUNDARY_CONVENTION: &str = "Boundary convention (D-P4.06-3): \
     value == limit is WITHIN limits (boundary-inclusive; margin 0). A \
     violation requires the measured value strictly beyond the limit. \
     Tested on both sides of every limit category.";

/// The limit-configuration-source statement (D-P4.06-1; ledger CONTEXT
/// open question 1).
pub const LIMIT_SOURCE_STATEMENT: &str = "Limit ownership (D-P4.06-1): the \
     limit set is caller-supplied CONFIGURATION per network model — \
     voltage bands and loading fractions via `NetworkLimits`, generator / \
     inverter apparent-power capability via `GeneratorLimitSet` (§11 \
     capability-derived). For the 0.x synthetic feeders the values derive \
     from the fixture definitions. The source is recorded per limit on \
     every check record; no limit is hard-coded in the check logic.";

/// The margin-evidence statement (D-P4.06-2).
pub const MARGIN_STATEMENT: &str = "Margins are evidence (D-P4.06-2): every \
     check records the measured value, the limit, the margin in the \
     limit's own units, and the margin as a fraction of the limit where \
     meaningful. The §12.5 step 7 diagnostics and the P4.35 runtime / \
     diagnostics measurement consume these records.";

/// The definition of a HARD limit violation (the D-P4.07-1 routing input):
/// a branch or transformer loading measured strictly above the equipment's
/// PHYSICAL rating. `loading_fraction` is relative to the rated current /
/// rated MVA declared in the model artefact (model data, not
/// configuration), so "the physical rating" is fraction 1.0 BY DEFINITION
/// of the loading base — this constant records that definition; it is not
/// a configured limit.
pub const PHYSICAL_RATING_FRACTION: f64 = 1.0;

/// The hard-limit statement accompanying [`PHYSICAL_RATING_FRACTION`].
pub const HARD_LIMIT_STATEMENT: &str = "A branch/transformer loading strictly \
     above 1.0 of the model-declared physical rating is a HARD limit \
     violation (the equipment rating is exceeded). The configured \
     `max_*_loading_fraction` is the OPERATIONAL limit at or below it; \
     hard and operational violations route differently (D-P4.07-1).";

// ---------------------------------------------------------------------------
// Limit categories, directions, records
// ---------------------------------------------------------------------------

/// The category of one limit check (§12.5 step 5: "voltage, current/thermal,
/// transformer, generator, and relevant equipment limits").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitCategory {
    /// Bus voltage below the configured lower band (per-unit of the bus
    /// nominal voltage — the base is stated per model, D-P4.06-1).
    BusVoltageLower,
    /// Bus voltage above the configured upper band (per-unit).
    BusVoltageUpper,
    /// Branch current/thermal loading above the configured operational
    /// limit (fraction of the model-declared rated current).
    BranchThermal,
    /// Transformer loading above the configured operational limit (fraction
    /// of the model-declared rated MVA).
    TransformerLoading,
    /// Generator / inverter apparent power above the capability-declared
    /// S_max (the §11 capability-derived equipment limit; the S_max cone).
    GeneratorApparentPower,
}

impl LimitCategory {
    /// The frozen machine-readable category code.
    pub const fn code(self) -> &'static str {
        match self {
            LimitCategory::BusVoltageLower => "bus_voltage_lower",
            LimitCategory::BusVoltageUpper => "bus_voltage_upper",
            LimitCategory::BranchThermal => "branch_thermal",
            LimitCategory::TransformerLoading => "transformer_loading",
            LimitCategory::GeneratorApparentPower => "generator_apparent_power",
        }
    }
}

/// The direction of one limit: the measured value must be at most (`AtMost`)
/// or at least (`AtLeast`) the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitDirection {
    /// `measured <= limit` is within limits.
    AtMost,
    /// `measured >= limit` is within limits.
    AtLeast,
}

/// One limit check record: the measured value, the limit, the margin (in
/// the limit's own units), the margin as a fraction of the limit, the
/// recorded configuration source, and the pass/fail outcome (D-P4.06-2).
/// Margins are EVIDENCE — they travel on the report and feed the §12.5
/// step 7 diagnostics and P4.35's measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitCheckRecord {
    /// The limit category.
    pub category: LimitCategory,
    /// The checked element (bus / branch / transformer id; for the
    /// generator check, the bus the aggregated injection applies at).
    pub element_id: String,
    /// The measured value, in the limit's own units (per-unit voltage;
    /// fraction of rating; MVA).
    pub measured: f64,
    /// The configured limit, in the same units.
    pub limit: f64,
    /// Which side the limit binds on.
    pub direction: LimitDirection,
    /// The margin in the limit's own units: `limit − measured` for
    /// [`LimitDirection::AtMost`], `measured − limit` for
    /// [`LimitDirection::AtLeast`]. Positive = slack; zero = at the
    /// boundary (within limits, D-P4.06-3); negative = violated.
    pub margin: f64,
    /// The margin as a fraction of the limit (`margin / limit`), where
    /// meaningful. Every limit this module checks is strictly positive
    /// (validated configuration), so this is always `Some` in practice;
    /// the `Option` records the "where meaningful" rule honestly.
    pub margin_fraction: Option<f64>,
    /// Whether the check passed (boundary-inclusive, D-P4.06-3).
    pub passed: bool,
    /// The recorded configuration source of the limit (D-P4.06-1).
    pub source: String,
}

impl LimitCheckRecord {
    fn new(
        category: LimitCategory,
        element_id: String,
        measured: f64,
        limit: f64,
        direction: LimitDirection,
        source: String,
    ) -> Self {
        let margin = match direction {
            LimitDirection::AtMost => limit - measured,
            LimitDirection::AtLeast => measured - limit,
        };
        let margin_fraction = if limit != 0.0 {
            Some(margin / limit)
        } else {
            None
        };
        let passed = match direction {
            LimitDirection::AtMost => measured <= limit,
            LimitDirection::AtLeast => measured >= limit,
        };
        Self {
            category,
            element_id,
            measured,
            limit,
            direction,
            margin,
            margin_fraction,
            passed,
            source,
        }
    }

    /// The frozen constraint name: `<category code>:<element id>`. This is
    /// the name the routing (P4.07), the diagnostics, and the P4.37
    /// consumption contract cite — every violated constraint is named, and
    /// a multi-violation fixture yields every name (D-P4.06-2 companion
    /// rule).
    pub fn constraint_name(&self) -> String {
        format!("{}:{}", self.category.code(), self.element_id)
    }
}

/// Whether the check is a HARD violation: the measured loading is strictly
/// above the model-declared physical rating ([`PHYSICAL_RATING_FRACTION`],
/// [`HARD_LIMIT_STATEMENT`]). Only the loading categories have a physical
/// rating; voltage and capability violations are never hard (they route
/// per the D-P4.07-1 rule, P4.07).
pub fn is_hard_violation(check: &LimitCheckRecord) -> bool {
    matches!(
        check.category,
        LimitCategory::BranchThermal | LimitCategory::TransformerLoading
    ) && check.measured > PHYSICAL_RATING_FRACTION
}

// ---------------------------------------------------------------------------
// Generator / inverter equipment limits (§11 capability-derived)
// ---------------------------------------------------------------------------

/// One declared generator / inverter apparent-power limit (the S_max of the
/// §11 capability model, configuration-carried per bus — D-P4.06-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorLimit {
    /// The bus the equipment injects at (the aggregation key).
    pub bus_id: String,
    /// The declared apparent-power maximum in MVA (strictly positive).
    pub s_max_mva: f64,
    /// The recorded source of the value (e.g. the capability model
    /// reference); carried verbatim onto every check record's `source`.
    pub source_note: String,
}

impl GeneratorLimit {
    fn validate(&self) -> Result<(), String> {
        if self.bus_id.trim().is_empty() {
            return Err("generator limit bus_id is empty".to_string());
        }
        if !self.s_max_mva.is_finite() || self.s_max_mva <= 0.0 {
            return Err(format!(
                "generator limit for bus '{}' has a non-positive or non-finite S_max",
                self.bus_id
            ));
        }
        if self.source_note.trim().is_empty() {
            return Err(format!(
                "generator limit for bus '{}' has no recorded source (D-P4.06-1: the \
                 source is recorded per limit)",
                self.bus_id
            ));
        }
        Ok(())
    }
}

/// The declared generator / inverter apparent-power limit set for one
/// network model (configuration, D-P4.06-1). A candidate injection at a
/// bus WITHOUT a declared limit is fail-closed — unknown is never
/// unconstrained (the P3.02 rule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeneratorLimitSet {
    /// One limit per equipment bus; bus ids must be unique.
    pub limits: Vec<GeneratorLimit>,
}

impl GeneratorLimitSet {
    /// Validates the set: every entry valid, bus ids unique.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen: Vec<&str> = Vec::new();
        for limit in &self.limits {
            limit.validate()?;
            if seen.contains(&limit.bus_id.as_str()) {
                return Err(format!(
                    "duplicate generator limit for bus '{}'",
                    limit.bus_id
                ));
            }
            seen.push(limit.bus_id.as_str());
        }
        Ok(())
    }

    /// The declared S_max for a bus, if any.
    pub fn s_max_for(&self, bus_id: &str) -> Option<f64> {
        self.limits
            .iter()
            .find(|limit| limit.bus_id == bus_id)
            .map(|limit| limit.s_max_mva)
    }
}

// ---------------------------------------------------------------------------
// Scope-declared limit gaps (§12.5 step 6 companion)
// ---------------------------------------------------------------------------

/// One limit the DECLARED SCOPE cannot verify (from the adapter's
/// `not_modelled` limitation record). A limit gap is recorded, never
/// silently skipped: a violation the model cannot see must not be
/// mistaken for a pass (§12.5 step 6; P4.07 carries these onto the
/// verdict evidence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeLimitGap {
    /// The scope's limitation label.
    pub label: String,
    /// The scope's limitation statement, with the limit-check framing.
    pub statement: String,
}

// ---------------------------------------------------------------------------
// The report: status, checks, violations, diagnostics
// ---------------------------------------------------------------------------

/// The outcome status of the step-5 limit check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum LimitCheckStatus {
    /// Every checked limit passed (boundary-inclusive). The report still
    /// carries the scope limit gaps — an unverifiable limit is not a pass.
    WithinLimits,
    /// At least one limit was violated; EVERY violated constraint is named
    /// in [`LimitCheckReport::violations`].
    Violated,
    /// No limit verdict is possible (non-convergence, incomplete result).
    /// Fail-closed: never a pass, never a rejection-by-limit (P4.07 owns
    /// the verdict category).
    Inconclusive { reason: String },
    /// A fail-closed trigger fired before a limit verdict could exist
    /// (invalid configuration, missing capability data). Never a pass.
    FailClosed {
        trigger: FailClosedTrigger,
        reason: String,
    },
}

/// The per-element loading percentage record (§12.5 step 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementLoadingPercent {
    /// The element id (branch or transformer).
    pub element_id: String,
    /// `branch` or `transformer`.
    pub equipment: String,
    /// Loading as a percentage of the model-declared rating.
    pub loading_percent: f64,
}

/// The worst margin within one category (§12.5 step 7: "worst-case margin").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryWorstMargin {
    /// The category.
    pub category: LimitCategory,
    /// The element carrying the worst (smallest) margin in the category.
    pub element_id: String,
    /// The worst margin, in the category's limit units.
    pub margin: f64,
    /// The worst margin as a fraction of the limit.
    pub margin_fraction: Option<f64>,
}

/// The binding constraint: the check with the smallest margin FRACTION
/// across all categories. Margin fractions are dimensionless, so they are
/// the one comparable currency across per-unit voltages and fractions of
/// rating (recorded rationale; D-P4.06-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingConstraint {
    /// The binding check's category.
    pub category: LimitCategory,
    /// The binding element.
    pub element_id: String,
    /// Its margin fraction (the smallest across all checks).
    pub margin_fraction: f64,
    /// The recorded statement naming the binding constraint.
    pub statement: String,
}

/// The §12.5 step 7 diagnostics computed from the check records (P4.06
/// step 3; finalised onto the verdict evidence by P4.07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MarginDiagnostics {
    /// Per-element loading percentages (branches, then transformers).
    pub per_element_loading_percent: Vec<ElementLoadingPercent>,
    /// The worst margin within each checked category, in fixed category
    /// order.
    pub worst_margin_by_category: Vec<CategoryWorstMargin>,
    /// The binding constraint (smallest margin fraction across all
    /// categories), if any check was recorded.
    pub binding_constraint: Option<BindingConstraint>,
}

/// The complete step-5 limit-check report. Every check appears in
/// [`LimitCheckReport::checks`]; the failed subset is repeated in
/// [`LimitCheckReport::violations`] with every violated constraint named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitCheckReport {
    /// The outcome status.
    pub status: LimitCheckStatus,
    /// Every limit check, in check order (voltages lower+upper per bus,
    /// branches, transformers, generators).
    pub checks: Vec<LimitCheckRecord>,
    /// The failed checks — EVERY violated constraint, named. Empty exactly
    /// when the status is `WithinLimits` (or no checks ran because the
    /// status is inconclusive/fail-closed).
    pub violations: Vec<LimitCheckRecord>,
    /// Limits the declared scope cannot verify (recorded, never silently
    /// skipped).
    pub scope_limit_gaps: Vec<ScopeLimitGap>,
    /// The §12.5 step 7 diagnostics.
    pub diagnostics: MarginDiagnostics,
}

impl LimitCheckReport {
    /// The constraint names of every violated constraint, in check order.
    pub fn violated_constraint_names(&self) -> Vec<String> {
        self.violations
            .iter()
            .map(|check| check.constraint_name())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The step-5 check
// ---------------------------------------------------------------------------

/// Verifies the converged power-flow result against the configured limit
/// set (§12.5 step 5; ledger P4.06). Provider-independent and pure: the
/// SAME result with the SAME configuration yields the SAME report,
/// regardless of which adapter produced the result (the P4.02 contract
/// discipline).
///
/// Check order (pinned by the tests): per bus lower voltage, per bus upper
/// voltage, per branch thermal loading, per transformer loading, per
/// generator/inverter apparent power (injections aggregated per bus).
pub fn check_limits(
    result: &PowerFlowResult,
    limits: &NetworkLimits,
    generators: &GeneratorLimitSet,
    scope: &ScopeDeclaration,
) -> LimitCheckReport {
    // Configuration is validated at this boundary: an invalid limit set
    // never produces a within-limits report (fail-closed; the same
    // validation the P4.05 boundary runs).
    let config_error = match limits.validate() {
        Err(error) => Some(error.to_string()),
        Ok(()) => generators.validate().err(),
    };
    if let Some(error) = config_error {
        let outcome: FailClosedOutcome = fail_closed_outcome(
            FailClosedTrigger::ValidatorFailed,
            &format!("invalid limit configuration: {error}"),
        );
        return LimitCheckReport {
            status: LimitCheckStatus::FailClosed {
                trigger: outcome.trigger,
                reason: outcome.reason,
            },
            checks: Vec::new(),
            violations: Vec::new(),
            scope_limit_gaps: scope_gaps(scope),
            diagnostics: MarginDiagnostics::default(),
        };
    }

    // No converged result → no limit verdict is possible. Fail-closed
    // inconclusive: never a pass, never a rejection-by-limit (§12.5 step 6;
    // §17.5 invariant 9).
    if result.status != PowerFlowStatus::Converged {
        return LimitCheckReport {
            status: LimitCheckStatus::Inconclusive {
                reason: format!(
                    "the power flow did not produce a converged result (status {}); \
                     no limit verdict is possible — never a pass, never a \
                     rejection-by-limit (retry policy NONE, D-P4.05-2)",
                    result.status.as_str()
                ),
            },
            checks: Vec::new(),
            violations: Vec::new(),
            scope_limit_gaps: scope_gaps(scope),
            diagnostics: MarginDiagnostics::default(),
        };
    }
    if result.validate_complete().is_err() {
        return LimitCheckReport {
            status: LimitCheckStatus::Inconclusive {
                reason: "the power-flow result is incomplete (provider bug); no \
                         limit verdict is published (fail-closed)"
                    .to_string(),
            },
            checks: Vec::new(),
            violations: Vec::new(),
            scope_limit_gaps: scope_gaps(scope),
            diagnostics: MarginDiagnostics::default(),
        };
    }

    let mut checks: Vec<LimitCheckRecord> = Vec::new();

    // Bus voltages, per unit of the bus nominal voltage (the base is stated
    // per model; the conversion is the translation layer's tested contract).
    for voltage in &result.bus_voltages {
        checks.push(LimitCheckRecord::new(
            LimitCategory::BusVoltageLower,
            voltage.bus_id.clone(),
            voltage.vm_pu,
            limits.voltage_min_pu,
            LimitDirection::AtLeast,
            "config:NetworkLimits.voltage_min_pu (per-unit of the bus nominal \
             voltage; D-P4.06-1)"
                .to_string(),
        ));
        checks.push(LimitCheckRecord::new(
            LimitCategory::BusVoltageUpper,
            voltage.bus_id.clone(),
            voltage.vm_pu,
            limits.voltage_max_pu,
            LimitDirection::AtMost,
            "config:NetworkLimits.voltage_max_pu (per-unit of the bus nominal \
             voltage; D-P4.06-1)"
                .to_string(),
        ));
    }

    // Branch current/thermal loading, fraction of the model-declared rated
    // current.
    for branch in &result.branch_loadings {
        checks.push(LimitCheckRecord::new(
            LimitCategory::BranchThermal,
            branch.branch_id.clone(),
            branch.loading_fraction,
            limits.max_branch_loading_fraction,
            LimitDirection::AtMost,
            "config:NetworkLimits.max_branch_loading_fraction (fraction of the \
             model-declared rated current; D-P4.06-1)"
                .to_string(),
        ));
    }

    // Transformer loading, fraction of the model-declared rated MVA.
    for trafo in &result.transformer_loadings {
        checks.push(LimitCheckRecord::new(
            LimitCategory::TransformerLoading,
            trafo.transformer_id.clone(),
            trafo.loading_fraction,
            limits.max_transformer_loading_fraction,
            LimitDirection::AtMost,
            "config:NetworkLimits.max_transformer_loading_fraction (fraction of \
             the model-declared rated MVA; D-P4.06-1)"
                .to_string(),
        ));
    }

    // Generator / inverter apparent power: the candidate injections are
    // aggregated per bus and compared against the capability-declared S_max
    // (the §11 equipment limit). A bus WITHOUT a declared S_max is
    // fail-closed — unknown is never unconstrained (the P3.02 rule).
    let mut aggregated: Vec<(String, f64, f64)> = Vec::new();
    for injection in &result.applied_injections {
        if let Some(entry) = aggregated
            .iter_mut()
            .find(|(bus, _, _)| bus == &injection.bus_id)
        {
            entry.1 += injection.active_power_mw;
            entry.2 += injection.reactive_power_mvar;
        } else {
            aggregated.push((
                injection.bus_id.clone(),
                injection.active_power_mw,
                injection.reactive_power_mvar,
            ));
        }
    }
    for (bus_id, p_mw, q_mvar) in aggregated {
        let s_max = match generators.s_max_for(&bus_id) {
            Some(s_max) => s_max,
            None => {
                let outcome = fail_closed_outcome(
                    FailClosedTrigger::CapabilityDataMissing,
                    &format!(
                        "candidate injection at bus '{bus_id}' has no declared \
                         apparent-power limit (S_max); unknown is never unconstrained"
                    ),
                );
                return LimitCheckReport {
                    status: LimitCheckStatus::FailClosed {
                        trigger: outcome.trigger,
                        reason: outcome.reason,
                    },
                    checks,
                    violations: Vec::new(),
                    scope_limit_gaps: scope_gaps(scope),
                    diagnostics: MarginDiagnostics::default(),
                };
            }
        };
        // Apparent power of the aggregate: S = sqrt(P² + Q²). The
        // reactive sign convention does not affect the magnitude.
        let s_mva = (p_mw * p_mw + q_mvar * q_mvar).sqrt();
        checks.push(LimitCheckRecord::new(
            LimitCategory::GeneratorApparentPower,
            bus_id.clone(),
            s_mva,
            s_max,
            LimitDirection::AtMost,
            format!(
                "capability:GeneratorLimitSet bus '{bus_id}' S_max = {s_max} MVA \
                 (§11 capability-derived; source: {})",
                generators
                    .limits
                    .iter()
                    .find(|limit| limit.bus_id == bus_id)
                    .map(|limit| limit.source_note.as_str())
                    .unwrap_or("")
            ),
        ));
    }

    let violations: Vec<LimitCheckRecord> = checks
        .iter()
        .filter(|check| !check.passed)
        .cloned()
        .collect();
    let status = if violations.is_empty() {
        LimitCheckStatus::WithinLimits
    } else {
        LimitCheckStatus::Violated
    };

    LimitCheckReport {
        status,
        diagnostics: build_diagnostics(&checks, result),
        scope_limit_gaps: scope_gaps(scope),
        checks,
        violations,
    }
}

/// The scope's unverifiable-limit gaps (recorded, never silently skipped).
fn scope_gaps(scope: &ScopeDeclaration) -> Vec<ScopeLimitGap> {
    scope
        .not_modelled
        .iter()
        .map(|not_modelled| ScopeLimitGap {
            label: not_modelled.label.clone(),
            statement: format!(
                "the declared scope does NOT model '{}'; limits in this class \
                 cannot be verified by this evaluation and a violation here \
                 cannot be seen — never mistaken for a pass (§12.5 step 6)",
                not_modelled.label
            ),
        })
        .collect()
}

/// Builds the §12.5 step 7 diagnostics from the check records.
fn build_diagnostics(checks: &[LimitCheckRecord], result: &PowerFlowResult) -> MarginDiagnostics {
    // Per-element loading percentages (branches, then transformers).
    let mut per_element: Vec<ElementLoadingPercent> = result
        .branch_loadings
        .iter()
        .map(|branch| ElementLoadingPercent {
            element_id: branch.branch_id.clone(),
            equipment: "branch".to_string(),
            loading_percent: branch.loading_fraction * 100.0,
        })
        .collect();
    per_element.extend(result.transformer_loadings.iter().map(|trafo| {
        ElementLoadingPercent {
            element_id: trafo.transformer_id.clone(),
            equipment: "transformer".to_string(),
            loading_percent: trafo.loading_fraction * 100.0,
        }
    }));

    // Worst margin per category, in fixed category order.
    let category_order = [
        LimitCategory::BusVoltageLower,
        LimitCategory::BusVoltageUpper,
        LimitCategory::BranchThermal,
        LimitCategory::TransformerLoading,
        LimitCategory::GeneratorApparentPower,
    ];
    let mut worst_by_category = Vec::new();
    for category in category_order {
        let mut worst: Option<&LimitCheckRecord> = None;
        for check in checks.iter().filter(|check| check.category == category) {
            let is_worse = match worst {
                None => true,
                Some(current) => check.margin < current.margin,
            };
            if is_worse {
                worst = Some(check);
            }
        }
        if let Some(worst) = worst {
            worst_by_category.push(CategoryWorstMargin {
                category,
                element_id: worst.element_id.clone(),
                margin: worst.margin,
                margin_fraction: worst.margin_fraction,
            });
        }
    }

    // The binding constraint: the smallest margin fraction across ALL
    // checks (fractions are dimensionless — the comparable currency across
    // categories; D-P4.06-2).
    let mut binding: Option<BindingConstraint> = None;
    for check in checks {
        let Some(fraction) = check.margin_fraction else {
            continue;
        };
        let is_binding = match &binding {
            None => true,
            Some(current) => fraction < current.margin_fraction,
        };
        if is_binding {
            binding = Some(BindingConstraint {
                category: check.category,
                element_id: check.element_id.clone(),
                margin_fraction: fraction,
                statement: format!(
                    "binding constraint {} at margin fraction {fraction:.6} (the \
                     smallest margin fraction across all categories)",
                    check.constraint_name()
                ),
            });
        }
    }

    MarginDiagnostics {
        per_element_loading_percent: per_element,
        worst_margin_by_category: worst_by_category,
        binding_constraint: binding,
    }
}

// ---------------------------------------------------------------------------
// In-crate unit tests (the integration tests live in tests/limits.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_is_inclusive_on_both_directions() {
        let at_most = LimitCheckRecord::new(
            LimitCategory::BranchThermal,
            "line".to_string(),
            0.9,
            0.9,
            LimitDirection::AtMost,
            "test".to_string(),
        );
        assert!(at_most.passed);
        assert!((at_most.margin - 0.0).abs() < 1.0e-12);

        let at_least = LimitCheckRecord::new(
            LimitCategory::BusVoltageLower,
            "bus".to_string(),
            0.9,
            0.9,
            LimitDirection::AtLeast,
            "test".to_string(),
        );
        assert!(at_least.passed);
        assert!((at_least.margin - 0.0).abs() < 1.0e-12);
    }

    #[test]
    fn hard_violation_is_loading_strictly_above_the_physical_rating() {
        let hard = LimitCheckRecord::new(
            LimitCategory::BranchThermal,
            "line".to_string(),
            1.05,
            0.9,
            LimitDirection::AtMost,
            "test".to_string(),
        );
        assert!(is_hard_violation(&hard));
        let operational = LimitCheckRecord::new(
            LimitCategory::BranchThermal,
            "line".to_string(),
            0.95,
            0.9,
            LimitDirection::AtMost,
            "test".to_string(),
        );
        assert!(!is_hard_violation(&operational));
        let voltage = LimitCheckRecord::new(
            LimitCategory::BusVoltageUpper,
            "bus".to_string(),
            1.4,
            1.1,
            LimitDirection::AtMost,
            "test".to_string(),
        );
        assert!(!is_hard_violation(&voltage));
    }
}
