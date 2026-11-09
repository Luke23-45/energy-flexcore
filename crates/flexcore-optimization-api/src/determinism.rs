// P3.19 — Deterministic-replay identity machinery (ledger P3.19 step 4;
// report §19.2 layer 5, §7.4 determinism declaration).
//
// Policy (stated once, verbatim, as [`DETERMINISM_POLICY`]): the same
// problem IR + the same pinned solver/provider versions + the same
// configuration must produce the SAME schedule — byte-identical, or
// tolerance-identical with the tolerance STATED. This module provides:
//
// - [`ReplayIdentity`] — the canonical serialization of (problem IR, config
//   digest, pinned solver version, provider version, thread count) plus its
//   stable identity digest (the same FNV-1a 64 convention as the config
//   digest, `crate::config::fnv1a64`).
// - [`compare_replay_records`] — the replay-identity check comparing two
//   solve records field by field, either byte-identical (`tolerance = 0.0`)
//   or tolerance-identical with the tolerance recorded in the comparison.
//   Measured fields (wall-clock runtime, the solver-log excerpt) are
//   excluded, and the exclusion is recorded in every comparison.
// - [`ReplayHarness`] — the harness P3.26's scenario-regression calls over
//   REAL solves: it solves the same problem twice through the port and
//   verifies replay identity. The tests in `tests/determinism.rs` run the
//   harness over the deterministic Tier 1 double (scripted backend) and the
//   offline identity of the P3.18 conic fixtures; the real-solver replay
//   runs are the P3.26 scenario-regression's job (decision note
//   17_optimizer_port.md §6 item 6, 18_clarabel_tier2.md §7 item 8).
//
// Solver-neutrality: this module names no solver. Versions arrive through
// the port's capability declaration or as parameters. (Open item recorded in
// the decision note: parallel-MIP determinism at the pinned Tier 1 version
// is NOT assumed — RESEARCH.md §6 task 1; the fixed thread count is pinned
// in the config, and the harness flags non-identity rather than assuming.)

use crate::config::{fnv1a64, SolverConfig};
use crate::port::{Optimizer, OptimizerError};
use crate::problem::OptimizationProblem;
use crate::result::{
    Diagnostics, DualDiagnostics, OptimizationResult, PrimalDiagnostics, ResidualReport,
};
use serde::{Deserialize, Serialize};

/// The determinism policy, stated once (ledger P3.19 step 4).
pub const DETERMINISM_POLICY: &str = "Same problem IR + same pinned solver and \
     provider versions + same configuration (including the fixed thread count) \
     must produce byte-identical schedules, or tolerance-identical schedules \
     with the tolerance STATED in the comparison. The configuration carries no \
     random seed: both pinned provider paths are deterministic given fixed \
     options and a fixed thread count; the config digest stands in for \
     'seed/config'. Parallel-MIP determinism is NOT assumed (RESEARCH.md §6 \
     task 1); any non-identity is reported, never smoothed over.";

/// The stated tolerance for REAL-solver replays (P3.26): values equal within
/// `1e-9` absolute are treated as identical, and the tolerance is recorded on
/// every comparison. The pinned solver's own feasibility/optimality
/// tolerances are at the 1e-7..1e-8 level, so a replay that is deterministic
/// run-to-run reproduces values to full f64 precision; `1e-9` absorbs only
/// serialization rounding. Offline double replays (scripted outcomes) use
/// `0.0` — byte-identical.
pub const DEFAULT_REPLAY_TOLERANCE: f64 = 1e-9;

/// The replay identity of one solve path (ledger P3.19 step 4): the canonical
/// serialization of (problem IR, config digest + verifiable canonical config,
/// pinned solver version, provider version, thread count). Two replays with
/// the same [`ReplayIdentity::identity_digest`] ran the same formulation
/// under the same configuration and the same pinned versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayIdentity {
    /// Digest of the canonical serialization of the problem IR
    /// ([`OptimizationProblem`] serializes deterministically — derived serde
    /// with a fixed field order).
    pub problem_ir_digest: String,
    /// Digest of the solver configuration
    /// ([`SolverConfig::config_digest`]).
    pub config_digest: String,
    /// The verifiable canonical JSON of the configuration (not a blob —
    /// every configured value is readable; see
    /// [`SolverConfig::canonical_json`]).
    pub config_canonical_json: String,
    /// The pinned solver version declared by the provider.
    pub solver_version: String,
    /// The provider version declared by the provider.
    pub provider_version: String,
    /// The fixed thread count used (recorded determinism default 1).
    pub thread_count: u32,
}

impl ReplayIdentity {
    /// Assembles the replay identity for one solve path from the problem,
    /// the configuration, and the pinned solver/provider versions.
    pub fn new(
        problem: &OptimizationProblem,
        config: &SolverConfig,
        solver_version: &str,
        provider_version: &str,
    ) -> Self {
        Self {
            problem_ir_digest: problem_ir_digest(problem),
            config_digest: config.config_digest(),
            config_canonical_json: config.canonical_json(),
            solver_version: solver_version.to_string(),
            provider_version: provider_version.to_string(),
            thread_count: config.thread_count,
        }
    }

    /// The stable identity digest: FNV-1a 64 over the canonical serde
    /// serialization of this struct (same convention as the config digest).
    pub fn identity_digest(&self) -> String {
        match serde_json::to_vec(self) {
            Ok(bytes) => format!("fnv1a64:{:016x}", fnv1a64(&bytes)),
            // Serialization of this struct cannot fail (no maps, no
            // non-string keys); the guard keeps the function total.
            Err(_) => "fnv1a64:serialization_unavailable".to_string(),
        }
    }
}

/// Digest of the canonical serialization of the problem IR (FNV-1a 64 over
/// the derived serde bytes — deterministic field order).
pub fn problem_ir_digest(problem: &OptimizationProblem) -> String {
    match serde_json::to_vec(problem) {
        Ok(bytes) => format!("fnv1a64:{:016x}", fnv1a64(&bytes)),
        // The IR serializes (no maps, no non-string keys); the guard keeps
        // the function total with a detectable sentinel.
        Err(_) => "fnv1a64:serialization_unavailable".to_string(),
    }
}

/// The outcome of a replay-identity comparison: whether the two records are
/// identical (at the stated tolerance) and, when not, exactly which fields
/// differ. `identical == false` is a determinism FINDING, never smoothed
/// over — the differences are the diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayComparison {
    /// `true` when the two records are identical at the stated tolerance.
    pub identical: bool,
    /// The tolerance this comparison was evaluated at (`0.0` =
    /// byte-identical requirement).
    pub tolerance: f64,
    /// One entry per differing field (empty when identical).
    pub differences: Vec<String>,
    /// The recorded comparison scope: which fields are compared and which
    /// measured fields are excluded (stated, per the ledger requirement).
    pub scope_note: String,
}

/// The comparison scope, stated on every comparison (ledger P3.19 step 4:
/// "byte-identical (or tolerance-identical, with the tolerance stated)").
pub const COMPARISON_SCOPE_NOTE: &str = "compared: status, problem class, \
     solver/provider identity and versions, objective value and units (within \
     tolerance), primal and dual diagnostics (within tolerance), residuals \
     (within tolerance), infeasibility evidence, warnings, determinism record, \
     candidate id, input snapshot id, grid model version, assumptions; \
     EXCLUDED (measured per run): wall-clock runtime and the solver-log \
     excerpt (resources).";

/// Compares two solve records for replay identity at the stated tolerance
/// (`0.0` = byte-identical). Differences are reported per field; nothing is
/// silently normalized.
pub fn compare_replay_records(
    baseline: &OptimizationResult,
    replay: &OptimizationResult,
    tolerance: f64,
) -> ReplayComparison {
    let mut differences: Vec<String> = Vec::new();

    if baseline.status != replay.status {
        push_difference(
            &mut differences,
            "status",
            format!("{:?} vs {:?}", baseline.status, replay.status),
        );
    }
    if baseline.problem_class != replay.problem_class {
        push_difference(
            &mut differences,
            "problem_class",
            format!(
                "{:?} vs {:?}",
                baseline.problem_class, replay.problem_class
            ),
        );
    }
    // Compared as Option<&str> so required and optional identification
    // fields share one comparison (None on one side and Some on the other
    // is a difference).
    let text_of = |value: &String| Some(value.as_str());
    for (field, a, b) in [
        ("solver_name", text_of(&baseline.solver_name), text_of(&replay.solver_name)),
        (
            "solver_version",
            text_of(&baseline.solver_version),
            text_of(&replay.solver_version),
        ),
        ("provider_id", text_of(&baseline.provider_id), text_of(&replay.provider_id)),
        (
            "provider_version",
            text_of(&baseline.provider_version),
            text_of(&replay.provider_version),
        ),
        (
            "objective_units",
            baseline.objective_units.as_deref(),
            replay.objective_units.as_deref(),
        ),
        (
            "input_snapshot_id",
            baseline.input_snapshot_id.as_deref(),
            replay.input_snapshot_id.as_deref(),
        ),
        (
            "grid_model_version",
            baseline.grid_model_version.as_deref(),
            replay.grid_model_version.as_deref(),
        ),
    ] {
        if a != b {
            push_difference(&mut differences, field, format!("{a:?} vs {b:?}"));
        }
    }
    if baseline.candidate_id != replay.candidate_id {
        push_difference(
            &mut differences,
            "candidate_id",
            format!("{:?} vs {:?}", baseline.candidate_id, replay.candidate_id),
        );
    }
    if baseline.uncertainty_and_assumptions != replay.uncertainty_and_assumptions {
        push_difference(
            &mut differences,
            "uncertainty_and_assumptions",
            "declared assumptions differ".to_string(),
        );
    }

    // Objective value: within tolerance when both present; presence must
    // match.
    match (baseline.objective_value, replay.objective_value) {
        (Some(a), Some(b)) => {
            if !within_tolerance(a, b, tolerance) {
                push_difference(&mut differences, "objective_value", format!("{a} vs {b}"));
            }
        }
        (None, None) => {}
        (a, b) => push_difference(
            &mut differences,
            "objective_value",
            format!("{a:?} vs {b:?} (presence)"),
        ),
    }

    compare_primal(&baseline.primal, &replay.primal, tolerance, &mut differences);
    compare_dual(&baseline.dual, &replay.dual, tolerance, &mut differences);
    compare_residuals(
        &baseline.residuals,
        &replay.residuals,
        tolerance,
        &mut differences,
    );

    if baseline.infeasibility_evidence != replay.infeasibility_evidence {
        push_difference(
            &mut differences,
            "infeasibility_evidence",
            "evidence (or not-available reason) differs".to_string(),
        );
    }
    if baseline.warnings != replay.warnings {
        push_difference(&mut differences, "warnings", "warning lists differ".to_string());
    }
    if baseline.determinism != replay.determinism {
        push_difference(
            &mut differences,
            "determinism",
            "determinism records differ (pinned version, thread count, config \
             digest, or time limit)"
                .to_string(),
        );
    }

    ReplayComparison {
        identical: differences.is_empty(),
        tolerance,
        differences,
        scope_note: COMPARISON_SCOPE_NOTE.to_string(),
    }
}

/// Records one replay difference (`field: detail`).
fn push_difference(differences: &mut Vec<String>, field: &str, detail: String) {
    differences.push(format!("{field}: {detail}"));
}

fn within_tolerance(a: f64, b: f64, tolerance: f64) -> bool {
    if tolerance == 0.0 {
        a == b
    } else {
        (a - b).abs() <= tolerance
    }
}

fn compare_primal(
    baseline: &Diagnostics<PrimalDiagnostics>,
    replay: &Diagnostics<PrimalDiagnostics>,
    tolerance: f64,
    differences: &mut Vec<String>,
) {
    match (baseline, replay) {
        (Diagnostics::Available(a), Diagnostics::Available(b)) => {
            if a.values.len() != b.values.len() {
                differences.push(format!(
                    "primal: {} values vs {} values",
                    a.values.len(),
                    b.values.len()
                ));
                return;
            }
            for (index, (va, vb)) in a.values.iter().zip(b.values.iter()).enumerate() {
                if va.name != vb.name || va.unit != vb.unit {
                    differences.push(format!(
                        "primal[{index}]: identity ({} {}, {} {}) differs",
                        va.name, va.unit, vb.name, vb.unit
                    ));
                }
                if !within_tolerance(va.value, vb.value, tolerance) {
                    differences.push(format!(
                        "primal[{}] ({}): {} vs {}",
                        index, va.name, va.value, vb.value
                    ));
                }
            }
        }
        (Diagnostics::NotAvailable { reason: ra }, Diagnostics::NotAvailable { reason: rb }) => {
            if ra != rb {
                differences.push(format!("primal not-available reason: {ra:?} vs {rb:?}"));
            }
        }
        (a, b) => differences.push(format!(
            "primal: availability differs ({:?} vs {:?})",
            a.is_available(),
            b.is_available()
        )),
    }
}

fn compare_dual(
    baseline: &Diagnostics<DualDiagnostics>,
    replay: &Diagnostics<DualDiagnostics>,
    tolerance: f64,
    differences: &mut Vec<String>,
) {
    match (baseline, replay) {
        (Diagnostics::Available(a), Diagnostics::Available(b)) => {
            if a.reduced_costs.len() != b.reduced_costs.len() {
                differences.push(format!(
                    "dual: {} reduced costs vs {}",
                    a.reduced_costs.len(),
                    b.reduced_costs.len()
                ));
            } else {
                for (index, (va, vb)) in a
                    .reduced_costs
                    .iter()
                    .zip(b.reduced_costs.iter())
                    .enumerate()
                {
                    if !within_tolerance(*va, *vb, tolerance) {
                        differences.push(format!(
                            "dual reduced_cost[{index}]: {va} vs {vb}"
                        ));
                    }
                }
            }
            if a.row_duals.len() != b.row_duals.len() {
                differences.push(format!(
                    "dual: {} row duals vs {}",
                    a.row_duals.len(),
                    b.row_duals.len()
                ));
            } else {
                for (index, (va, vb)) in a.row_duals.iter().zip(b.row_duals.iter()).enumerate() {
                    if !within_tolerance(*va, *vb, tolerance) {
                        differences.push(format!("dual row_dual[{index}]: {va} vs {vb}"));
                    }
                }
            }
        }
        (Diagnostics::NotAvailable { reason: ra }, Diagnostics::NotAvailable { reason: rb }) => {
            if ra != rb {
                differences.push(format!("dual not-available reason: {ra:?} vs {rb:?}"));
            }
        }
        (a, b) => differences.push(format!(
            "dual: availability differs ({:?} vs {:?})",
            a.is_available(),
            b.is_available()
        )),
    }
}

fn compare_residuals(
    baseline: &Diagnostics<ResidualReport>,
    replay: &Diagnostics<ResidualReport>,
    tolerance: f64,
    differences: &mut Vec<String>,
) {
    let compare_option = |field: &str, a: Option<f64>, b: Option<f64>| {
        match (a, b) {
            (Some(va), Some(vb)) => {
                if !within_tolerance(va, vb, tolerance) {
                    differences.push(format!("residual {field}: {va} vs {vb}"));
                }
            }
            (None, None) => {}
            (va, vb) => differences.push(format!(
                "residual {field}: presence differs ({va:?} vs {vb:?})"
            )),
        }
    };
    match (baseline, replay) {
        (Diagnostics::Available(a), Diagnostics::Available(b)) => {
            compare_option(
                "max_primal_violation",
                a.max_primal_violation,
                b.max_primal_violation,
            );
            compare_option(
                "max_dual_violation",
                a.max_dual_violation,
                b.max_dual_violation,
            );
            if a.iteration_count != b.iteration_count {
                differences.push(format!(
                    "residual iteration_count: {:?} vs {:?}",
                    a.iteration_count, b.iteration_count
                ));
            }
        }
        (Diagnostics::NotAvailable { reason: ra }, Diagnostics::NotAvailable { reason: rb }) => {
            if ra != rb {
                differences.push(format!("residual not-available reason: {ra:?} vs {rb:?}"));
            }
        }
        (a, b) => differences.push(format!(
            "residuals: availability differs ({:?} vs {:?})",
            a.is_available(),
            b.is_available()
        )),
    }
}

/// The replay verification of one harness run: the replay identity of the
/// path plus the record comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayVerification {
    /// The replay identity both solves ran under.
    pub identity: ReplayIdentity,
    /// The record comparison of the two solves.
    pub comparison: ReplayComparison,
}

/// The deterministic-replay harness (ledger P3.19 step 4; P3.26 calls this
/// over real solves). Solves the SAME problem twice through the port under
/// the SAME configuration and verifies the two result records are identical
/// at the harness's stated tolerance. `byte_identical()` is the strict
/// mode for offline/double replays; `with_tolerance(DEFAULT_REPLAY_TOLERANCE)`
/// is the stated-tolerance mode for real-solver replays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayHarness {
    tolerance: f64,
}

impl ReplayHarness {
    /// Byte-identical mode (tolerance `0.0`): every compared value must be
    /// exactly equal.
    pub fn byte_identical() -> Self {
        Self { tolerance: 0.0 }
    }

    /// Tolerance-identical mode with the tolerance STATED (recorded on the
    /// comparison).
    pub fn with_tolerance(tolerance: f64) -> Self {
        Self { tolerance }
    }

    /// The harness's stated tolerance.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Runs the replay: two solves through the port, identity assembly, and
    /// the record comparison. Solve errors (malformed problem, invalid
    /// config, backend failure) propagate — a replay over a failing path is
    /// not a determinism finding but an infrastructure outcome.
    pub fn verify_replay(
        &self,
        optimizer: &dyn Optimizer,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> Result<ReplayVerification, OptimizerError> {
        let capabilities = optimizer.capabilities();
        let identity = ReplayIdentity::new(
            problem,
            config,
            &capabilities.solver_version,
            &capabilities.provider_version,
        );
        let first = optimizer.solve(problem, config)?;
        let second = optimizer.solve(problem, config)?;
        let comparison = compare_replay_records(&first, &second, self.tolerance);
        Ok(ReplayVerification {
            identity,
            comparison,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    #[test]
    fn replay_identity_is_stable_and_discriminating() {
        let problem = fixture::reference_lp();
        let config = SolverConfig::default();
        let identity = ReplayIdentity::new(&problem, &config, "1.0.0", "0.1.0");
        let again = ReplayIdentity::new(&problem, &config, "1.0.0", "0.1.0");
        assert_eq!(identity, again);
        assert_eq!(identity.identity_digest(), again.identity_digest());
        assert!(identity.identity_digest().starts_with("fnv1a64:"));
        assert!(identity.config_canonical_json.contains("\"thread_count\":1"));

        // Any change to the identity parts changes the digest.
        let mut config_other = SolverConfig::default();
        config_other.thread_count = 4;
        assert_ne!(
            identity.identity_digest(),
            ReplayIdentity::new(&problem, &config_other, "1.0.0", "0.1.0").identity_digest()
        );
        assert_ne!(
            identity.identity_digest(),
            ReplayIdentity::new(&problem, &config, "2.0.0", "0.1.0").identity_digest()
        );
        assert_ne!(
            identity.identity_digest(),
            ReplayIdentity::new(&problem, &config, "1.0.0", "0.2.0").identity_digest()
        );
        assert_ne!(
            identity.identity_digest(),
            ReplayIdentity::new(&fixture::reference_qp(), &config, "1.0.0", "0.1.0")
                .identity_digest()
        );
    }

    #[test]
    fn problem_ir_digest_is_stable_and_discriminating() {
        let a = problem_ir_digest(&fixture::reference_lp());
        let b = problem_ir_digest(&fixture::reference_lp());
        assert_eq!(a, b);
        assert_ne!(a, problem_ir_digest(&fixture::reference_qp()));
    }
}
