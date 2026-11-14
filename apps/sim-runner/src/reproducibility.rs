// P5.19 — The replay reproducibility checker (§24 step 10, §19.2 layer 5;
//         ledger P5.19).
//
// The verification method (report §24 step 10; ledger P5.19 objective):
// the SAME scenario + the SAME versions + the SAME seeds produce the SAME
// results WITHIN A STATED, PRE-REGISTERED tolerance. This module is the
// checker: it consumes the per-leg comparisons the P5.18 replay-run path
// produces (`replay_record.rs`) and a pre-registered tolerance table, and
// emits a verdict with NAMED differences.
//
// ## The two comparison regimes (decided once, D-P5.19-1, DEFAULT)
//
// - **Byte-identical** — the scripted deterministic legs (telemetry from
//   the seeded Phase 2 simulator; the whole `ScenarioRunRecord` on the
//   scripted grid backend). f64 equality is EXACT here (bit-identical
//   determinism, P2.13 §3): anything else IS a replay break and must
//   surface as a mismatch, never be rounded into plausibility.
// - **Tolerance-bounded** — floating-point legs (a `worker_process` grid
//   backend, whose pandapower power flow may legitimately differ in the
//   last ulp across platforms/BLAS builds): numeric values compare within
//   the pre-registered RELATIVE tolerance of 1e-9 on MW/MVAr/MWh/pu values
//   (DEFAULT), with an absolute floor of 1e-12 so near-zero values cannot
//   hide a real deviation. The tolerance is registered BEFORE the runs
//   (ledger P5.19 risk note: "tolerance fitted after the runs" is the
//   failure mode); the table below is that pre-registration.
//
// ## What is NEVER a reproducibility criterion
//
// Runtimes, iteration counts, ingest times, and hardware figures are
// conditions, not reproducibility properties (the P2.13 §3 rule that
// excludes `ingest_time` by record shape; the P4.35 §0 rule that
// conditions travel with every figure but are never compared as
// correctness). A run that differs ONLY in its recorded durations is
// reproducible.
//
// ## The nondeterminism-source inventory (ledger P5.19 requirement)
//
// EVERY known nondeterminism source must be listed and neutralized. The
// inventory ([`NONDETERMINISM_CONTROLS`]) names each source and HOW the
// run paths neutralize it. The thread count is already pinned at 1 in the
// solver configuration (the P3.26 pin block,
// `scenarios/synthetic-portfolio/scenarios-phase3/`:
// `solver_pins.thread_count = 1`; `DEFAULT_REPLAY_TOLERANCE` 1e-9) — this
// module inherits and cites that pin rather than re-pinning it.
//
// ## The proof obligation is the human's
//
// The double-run execution itself is NOT performed by the authoring loop
// (nothing is built or run — the Phase 5 loop bans hold). The checker, the
// pre-registered verification plan (the exact commands in the decision
// record `18_20_replay_library.md`), and the unit tests below are written;
// the P5.19 acceptance (two clean runs match within the pre-stated
// tolerances; the negative control reports its difference) is UNRUN and
// belongs to the human build/test pass.
//
// Control status: pure comparison logic over synthetic simulation records;
// no solver call, no I/O, no dispatch path, no control authority, no
// safety implication.

use serde::Serialize;

use crate::replay::ReplayRecord;
use crate::replay::ReplayValue;

// ---------------------------------------------------------------------------
// The pre-registered tolerance table (D-P5.19-1, DEFAULT)
// ---------------------------------------------------------------------------

/// The DEFAULT relative tolerance for floating-point legs (on MW / MVAr /
/// MWh / per-unit values). Registered BEFORE any run; a change is a
/// recorded decision, never a post-hoc fit.
pub const DEFAULT_VALUE_RELATIVE_TOLERANCE: f64 = 1e-9;

/// The DEFAULT absolute floor for floating-point legs: a difference below
/// the floor is within tolerance even where the relative form would not be
/// meaningful (near-zero values). Registered BEFORE any run.
pub const DEFAULT_VALUE_ABSOLUTE_FLOOR: f64 = 1e-12;

/// The verdict vocabulary of the reproducibility checker.
pub const VERDICT_IDENTICAL: &str = "identical";
pub const VERDICT_WITHIN_TOLERANCE: &str = "within_tolerance";
pub const VERDICT_MISMATCH: &str = "mismatch";

/// One entry of the pre-registered tolerance table: HOW each output type
/// is compared, and at what tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ToleranceEntry {
    /// The output type (the leg / record class the rule applies to).
    pub output_type: &'static str,
    /// The comparison rule (`byte_identical` or `relative_1e-9`).
    pub rule: &'static str,
    /// The relative tolerance, when the rule is tolerance-bounded.
    pub relative: Option<f64>,
    /// The absolute floor, when the rule is tolerance-bounded.
    pub absolute_floor: Option<f64>,
}

/// The pre-registered tolerance table (the P5.19 step-1 requirement:
/// "tolerance stated per output type BEFORE the runs").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ToleranceTable {
    pub entries: &'static [ToleranceEntry],
}

impl ToleranceTable {
    /// The DEFAULT table (pre-registered; every entry carries the rule the
    /// P5.18/P5.19 comparison applies).
    pub fn default_table() -> Self {
        Self {
            entries: DEFAULT_TOLERANCE_ENTRIES,
        }
    }
}

/// The DEFAULT table's entries, in comparison order.
pub const DEFAULT_TOLERANCE_ENTRIES: &[ToleranceEntry] = &[
    ToleranceEntry {
        output_type: "telemetry_records (scripted deterministic legs)",
        rule: "byte_identical",
        relative: None,
        absolute_floor: None,
    },
    ToleranceEntry {
        output_type: "scenario_run_record (scripted grid backend)",
        rule: "byte_identical",
        relative: None,
        absolute_floor: None,
    },
    ToleranceEntry {
        output_type: "scenario_run_record (worker_process grid backend, numeric fields)",
        rule: "relative_1e-9",
        relative: Some(DEFAULT_VALUE_RELATIVE_TOLERANCE),
        absolute_floor: Some(DEFAULT_VALUE_ABSOLUTE_FLOOR),
    },
    ToleranceEntry {
        output_type: "telemetry_records (worker_process legs, numeric fields)",
        rule: "relative_1e-9",
        relative: Some(DEFAULT_VALUE_RELATIVE_TOLERANCE),
        absolute_floor: Some(DEFAULT_VALUE_ABSOLUTE_FLOOR),
    },
    ToleranceEntry {
        output_type: "objective values (real-solve legs, VERIFY-BUILD)",
        rule: "relative_1e-9",
        relative: Some(DEFAULT_VALUE_RELATIVE_TOLERANCE),
        absolute_floor: Some(DEFAULT_VALUE_ABSOLUTE_FLOOR),
    },
    ToleranceEntry {
        output_type: "runtimes / iteration counts / ingest times / hardware",
        rule: "never_compared (conditions, not reproducibility properties)",
        relative: None,
        absolute_floor: None,
    },
];

// ---------------------------------------------------------------------------
// The nondeterminism-source inventory (listed + neutralized, ledger P5.19)
// ---------------------------------------------------------------------------

/// One (source, neutralization) entry of the nondeterminism inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NondeterminismControl {
    /// The nondeterminism source, named.
    pub source: &'static str,
    /// HOW the run paths neutralize it (the recorded mechanism + its
    /// authority).
    pub neutralization: &'static str,
}

/// The full inventory. A NEW nondeterminism source reaching a run path
/// must be ADDED here with its neutralization — an unlisted source is a
/// defect, not an oversight.
pub const NONDETERMINISM_CONTROLS: &[NondeterminismControl] = &[
    NondeterminismControl {
        source: "wall_clock",
        neutralization: "no wall clock is read in any run path: every instant is the \
             scenario clock via the controlled RunClock, advanced together with the \
             simulator pump position (P0.09 §5; the P4.16 controlled-clock discipline). \
             The wall-clock `ingest_time` is excluded from comparison BY RECORD SHAPE \
             (P2.13 §3) — the ReplayRecord structurally cannot carry it.",
    },
    NondeterminismControl {
        source: "hash_map_iteration_order",
        neutralization: "every comparison order is deterministic: the replay comparison \
             sorts both sides on the frozen identity tuple before matching \
             (compare.rs); run-record maps are BTreeMap (quality accounting, \
             setpoints, asset buses); asset identities derive in scenario-file order. \
             No output depends on HashMap iteration order.",
    },
    NondeterminismControl {
        source: "thread_count",
        neutralization: "pinned thread_count = 1 in the solver configuration (the P3.26 \
             pin block, `scenarios/synthetic-portfolio/scenarios-phase3/` \
             `solver_pins.thread_count = 1`; the replay tolerance 1e-9 there is the \
             same DEFAULT this module registers). The sim-runner composition itself is \
             single-threaded in-process; the scenario record's \
             `nondeterminism_controls.thread_count` MUST equal 1 and is validated \
             fail-closed.",
    },
    NondeterminismControl {
        source: "ambient_entropy",
        neutralization: "all run identities are deterministic FNV-1a-64 derivations \
             (stand_in_asset_id, deterministic_uuid — no wall clock, no ambient \
             entropy, no iteration-order dependence); the master seed is the only \
             randomness source and is pinned in the scenario record. No uuid-v4, no \
             OS entropy enters a run.",
    },
    NondeterminismControl {
        source: "floating_point_backend",
        neutralization: "the scripted grid backend is a recorded fixture and is \
             byte-stable; a `worker_process` backend leg is tolerance-bounded \
             (relative 1e-9, floor 1e-12, DEFAULT) and the backend pin (pandapower \
             3.5.6) plus the platform are recorded per run — a backend or platform \
             change is a VERSION change, not a reproducibility property.",
    },
    NondeterminismControl {
        source: "helics_federation_execution",
        neutralization: "the HELICS leg is OPT-IN (P5.16): the default scenario runs \
             WITHOUT HELICS (direct adapters). A federation run records its config, \
             versions, and execution outcomes in the run record; a co-sim leg is \
             reproducibility-checked only against a run with the SAME federation \
             config and versions, and its failures are fail-closed, never retried.",
    },
];

// ---------------------------------------------------------------------------
// The value comparison (the tolerance arithmetic)
// ---------------------------------------------------------------------------

/// Whether two floating-point values are within the pre-registered
/// tolerance: the RELATIVE form `|a - b| <= relative * max(|a|, |b|)`, OR
/// the absolute floor `|a - b| <= floor` (so near-zero values cannot hide
/// a deviation, and a floor-scale difference cannot fail a relative test
/// at a large magnitude). Non-finite values NEVER compare equal unless
/// bit-identical (a NaN in a record is a defect that must surface).
pub fn values_within_tolerance(a: f64, b: f64, relative: f64, floor: f64) -> bool {
    if a == b {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        // A NaN never equals a NaN in this arithmetic; an infinity paired
        // with a finite value is a real difference.
        return false;
    }
    let diff = (a - b).abs();
    if diff <= floor {
        return true;
    }
    let scale = a.abs().max(b.abs());
    diff <= relative * scale
}

// ---------------------------------------------------------------------------
// The telemetry-record tolerance comparison (P5.19 checker core)
// ---------------------------------------------------------------------------

/// One numeric deviation the tolerance comparison found, named per record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToleranceDiff {
    /// Whether the deviation is OUTSIDE the tolerance (a mismatch) or
    /// inside it (recorded, not a failure).
    pub outside_tolerance: bool,
    /// The record identity the deviation is at.
    pub asset_id: String,
    pub event_time_unix_ms: i64,
    pub quantity: String,
    pub source_sequence: u64,
    /// The reference value (as rendered text; the frozen value form).
    pub reference: String,
    /// The candidate value.
    pub candidate: String,
    /// The absolute difference (rendered; `not_comparable` for
    /// non-numeric or non-finite pairs).
    pub deviation: String,
}

/// The outcome of one tolerance-bounded telemetry comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToleranceComparison {
    /// Records matched exactly.
    pub exact_matches: usize,
    /// Records matched within the tolerance (numeric deviations inside the
    /// pre-registered bound — recorded, never silent).
    pub within_tolerance_matches: usize,
    /// Deviations outside the tolerance.
    pub mismatches: Vec<ToleranceDiff>,
}

impl ToleranceComparison {
    /// Whether the comparison passes at the stated tolerance.
    pub fn is_pass(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// Compares two telemetry record sets within the pre-registered tolerance.
/// The identity/multiset discipline is the P2.24 comparison's
/// (`compare::compare`): records are matched on the frozen identity tuple;
/// a record present on one side only is a mismatch (never absorbed by a
/// count); non-numeric fields (quality, unit tags) compare exactly — a
/// tolerance applies to NUMBERS, never to a quality state or a sign
/// convention.
pub fn compare_records_within_tolerance(
    reference: &[ReplayRecord],
    candidate: &[ReplayRecord],
    relative: f64,
    floor: f64,
) -> ToleranceComparison {
    let exact = crate::compare::compare(reference, candidate);
    // Start from the exact comparison: every exact mismatch is either a
    // tolerance-within numeric deviation or a real mismatch. One-sided
    // records and quality differences stay mismatches regardless.
    let mut out = ToleranceComparison {
        exact_matches: exact.match_count,
        within_tolerance_matches: 0,
        mismatches: Vec::new(),
    };
    for diff in &exact.mismatches {
        let (reference, candidate) = match (&diff.reference, &diff.candidate) {
            (Some(r), Some(c)) => (r, c),
            // One-sided records: identity differences are never within a
            // numeric tolerance.
            _ => {
                out.mismatches.push(ToleranceDiff {
                    outside_tolerance: true,
                    asset_id: diff.asset_id.clone(),
                    event_time_unix_ms: diff.event_time_unix_ms,
                    quantity: diff.quantity.clone(),
                    source_sequence: diff
                        .reference
                        .as_ref()
                        .or(diff.candidate.as_ref())
                        .map(|record| record.source_sequence)
                        .unwrap_or(0),
                    reference: diff
                        .reference
                        .as_ref()
                        .map(|record| value_line(&record.value))
                        .unwrap_or_else(|| "<missing>".to_string()),
                    candidate: diff
                        .candidate
                        .as_ref()
                        .map(|record| value_line(&record.value))
                        .unwrap_or_else(|| "<missing>".to_string()),
                    deviation: "record present on one side only".to_string(),
                });
                continue;
            }
        };
        if diff.field == crate::compare::DiffField::Value {
            if let (Some(a), Some(b)) = (numeric_value(&reference.value), numeric_value(&candidate.value)) {
                let delta = if a == b {
                    0.0
                } else if !a.is_finite() || !b.is_finite() {
                    f64::NAN
                } else {
                    (a - b).abs()
                };
                if values_within_tolerance(a, b, relative, floor) {
                    out.within_tolerance_matches += 1;
                    out.mismatches.push(ToleranceDiff {
                        outside_tolerance: false,
                        asset_id: diff.asset_id.clone(),
                        event_time_unix_ms: diff.event_time_unix_ms,
                        quantity: diff.quantity.clone(),
                        source_sequence: reference.source_sequence,
                        reference: value_line(&reference.value),
                        candidate: value_line(&candidate.value),
                        deviation: if delta.is_nan() {
                            "not_comparable (non-finite)".to_string()
                        } else {
                            format!("{delta:.3e}")
                        },
                    });
                    continue;
                }
            }
        }
        out.mismatches.push(ToleranceDiff {
            outside_tolerance: true,
            asset_id: diff.asset_id.clone(),
            event_time_unix_ms: diff.event_time_unix_ms,
            quantity: diff.quantity.clone(),
            source_sequence: reference.source_sequence,
            reference: value_line(&reference.value),
            candidate: value_line(&candidate.value),
            deviation: if diff.field == crate::compare::DiffField::Quality {
                "quality state differs (never tolerance-bounded)".to_string()
            } else {
                "outside the pre-registered tolerance".to_string()
            },
        });
    }
    out
}

/// The numeric value of a replay value, when it has one (power MW,
/// reactive MVAr, fraction, energy MWh). Flags and states have none.
fn numeric_value(value: &ReplayValue) -> Option<f64> {
    match value {
        ReplayValue::Power { mw } => Some(*mw),
        ReplayValue::Reactive { mvar, .. } => Some(*mvar),
        ReplayValue::Fraction { value } => Some(*value),
        ReplayValue::Energy { mwh } => Some(*mwh),
        ReplayValue::Flag { .. } | ReplayValue::State { .. } => None,
    }
}

fn value_line(value: &ReplayValue) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unrenderable>".to_string())
}

// ---------------------------------------------------------------------------
// The structural (run-record) comparison
// ---------------------------------------------------------------------------

/// One structural difference between two run-record JSON documents.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructuralDiff {
    /// `mismatch` (a real difference) or `within_tolerance` (a numeric
    /// deviation inside the pre-registered bound).
    pub kind: &'static str,
    /// The JSON path the difference is at (e.g. `commands[0].final_state`).
    pub path: String,
    /// The reference rendering.
    pub reference: String,
    /// The candidate rendering.
    pub candidate: String,
}

/// Recursively diffs two JSON documents, naming every differing path.
/// Numbers compare within the stated tolerance (relative + absolute
/// floor); everything else compares exactly. Array length differences and
/// missing keys are mismatches at the named path. `out` is capped so a
/// pathological document cannot flood the record.
pub fn structural_diff(
    reference: &serde_json::Value,
    candidate: &serde_json::Value,
    path: &str,
    relative: f64,
    floor: f64,
    out: &mut Vec<StructuralDiff>,
) {
    const DIFF_LIMIT: usize = 200;
    if out.len() >= DIFF_LIMIT {
        return;
    }
    match (reference, candidate) {
        (serde_json::Value::Object(reference), serde_json::Value::Object(candidate)) => {
            // Deterministic order: sorted keys (serde_json's default map is
            // ordered) — never HashMap order.
            let keys: std::collections::BTreeSet<&String> =
                reference.keys().chain(candidate.keys()).collect();
            for key in keys {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match (reference.get(key), candidate.get(key)) {
                    (Some(reference), Some(candidate)) => {
                        structural_diff(reference, candidate, &child_path, relative, floor, out)
                    }
                    (Some(reference), None) => out.push(StructuralDiff {
                        kind: "mismatch",
                        path: child_path,
                        reference: reference.to_string(),
                        candidate: "<missing>".to_string(),
                    }),
                    (None, Some(candidate)) => out.push(StructuralDiff {
                        kind: "mismatch",
                        path: child_path,
                        reference: "<missing>".to_string(),
                        candidate: candidate.to_string(),
                    }),
                    (None, None) => unreachable!("key came from one of the maps"),
                }
                if out.len() >= DIFF_LIMIT {
                    return;
                }
            }
        }
        (serde_json::Value::Array(reference), serde_json::Value::Array(candidate)) => {
            if reference.len() != candidate.len() {
                out.push(StructuralDiff {
                    kind: "mismatch",
                    path: path.to_string(),
                    reference: format!("array of {} elements", reference.len()),
                    candidate: format!("array of {} elements", candidate.len()),
                });
                return;
            }
            for (index, (reference, candidate)) in
                reference.iter().zip(candidate.iter()).enumerate()
            {
                structural_diff(
                    reference,
                    candidate,
                    &format!("{path}[{index}]"),
                    relative,
                    floor,
                    out,
                );
                if out.len() >= DIFF_LIMIT {
                    return;
                }
            }
        }
        (
            serde_json::Value::Number(reference),
            serde_json::Value::Number(candidate),
        ) => {
            let reference = reference.as_f64();
            let candidate = candidate.as_f64();
            match (reference, candidate) {
                (Some(reference), Some(candidate)) => {
                    if reference == candidate {
                        return;
                    }
                    if values_within_tolerance(reference, candidate, relative, floor) {
                        out.push(StructuralDiff {
                            kind: "within_tolerance",
                            path: path.to_string(),
                            reference: reference.to_string(),
                            candidate: candidate.to_string(),
                        });
                    } else {
                        out.push(StructuralDiff {
                            kind: "mismatch",
                            path: path.to_string(),
                            reference: reference.to_string(),
                            candidate: candidate.to_string(),
                        });
                    }
                }
                _ => {
                    if reference != candidate {
                        out.push(StructuralDiff {
                            kind: "mismatch",
                            path: path.to_string(),
                            reference: reference.to_string(),
                            candidate: candidate.to_string(),
                        });
                    }
                }
            }
        }
        _ => {
            if reference != candidate {
                out.push(StructuralDiff {
                    kind: "mismatch",
                    path: path.to_string(),
                    reference: reference.to_string(),
                    candidate: candidate.to_string(),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The checker verdict
// ---------------------------------------------------------------------------

/// The reproducibility verdict over a set of compared legs (the P5.19
/// output the Simulation Lab displays as the reproducibility verdict,
/// P5.08).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReproducibilityVerdict {
    /// `identical` / `within_tolerance` / `mismatch`.
    pub verdict: String,
    /// The named differences (mismatches first; within-tolerance
    /// deviations recorded, never silent).
    pub diffs: Vec<String>,
    /// The recorded statement of what the verdict means.
    pub statement: String,
}

/// Builds the verdict from per-leg outcomes. A leg is
/// `(leg_name, mismatch_count, within_tolerance_count, named_diffs)`.
/// ANY mismatch ⇒ `mismatch`; otherwise any within-tolerance deviation ⇒
/// `within_tolerance`; otherwise `identical`.
pub fn check_reproducibility(
    legs: &[(&str, usize, usize, Vec<String>)],
    tolerance_reference: &str,
) -> ReproducibilityVerdict {
    let mut total_mismatches = 0usize;
    let mut total_within = 0usize;
    let mut diffs = Vec::new();
    for (leg, mismatches, within, named) in legs {
        total_mismatches += mismatches;
        total_within += within;
        for diff in named {
            diffs.push(format!("[{leg}] {diff}"));
        }
    }
    let (verdict, statement) = if total_mismatches > 0 {
        (
            VERDICT_MISMATCH,
            "the replay is NOT reproducible at the pre-registered tolerance: at least one \
             leg reported a difference outside the stated bound (or a structural / \
             identity difference, which no tolerance covers) — the named diffs are the \
             finding, never something to fit the tolerance around"
                .to_string(),
        )
    } else if total_within > 0 {
        (
            VERDICT_WITHIN_TOLERANCE,
            format!(
                "the replay is reproducible WITHIN the pre-registered tolerance \
                 ({tolerance_reference}): numeric deviations inside the stated bound were \
                 recorded, not silently absorbed"
            ),
        )
    } else {
        (
            VERDICT_IDENTICAL,
            format!(
                "the replay is byte-identical on every compared leg ({tolerance_reference})"
            ),
        )
    };
    ReproducibilityVerdict {
        verdict: verdict.to_string(),
        diffs,
        statement,
    }
}

// ---------------------------------------------------------------------------
// Unit tests (the suite also lives in tests/reproducibility_p5_19.rs; the
// in-crate tests carry the arithmetic core)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_table_is_pre_registered_with_the_stated_rules() {
        let table = ToleranceTable::default_table();
        assert!(table.entries.len() >= 5);
        // The scripted legs are byte-identical.
        assert_eq!(table.entries[0].rule, "byte_identical");
        assert_eq!(table.entries[1].rule, "byte_identical");
        // The floating-point legs carry 1e-9 relative / 1e-12 floor.
        assert_eq!(
            table.entries[2].relative,
            Some(DEFAULT_VALUE_RELATIVE_TOLERANCE)
        );
        assert_eq!(
            table.entries[2].absolute_floor,
            Some(DEFAULT_VALUE_ABSOLUTE_FLOOR)
        );
        // Conditions are never compared as correctness.
        assert!(table.entries[table.entries.len() - 1]
            .rule
            .starts_with("never_compared"));
    }

    #[test]
    fn the_tolerance_arithmetic_matches_the_registered_rule() {
        let relative = DEFAULT_VALUE_RELATIVE_TOLERANCE;
        let floor = DEFAULT_VALUE_ABSOLUTE_FLOOR;
        // Exact equality always passes.
        assert!(values_within_tolerance(1.0, 1.0, relative, floor));
        // A floor-scale difference passes even at a large magnitude.
        assert!(values_within_tolerance(100.0, 100.0 + 1e-13, relative, floor));
        // A 1e-12 relative deviation passes (inside 1e-9).
        assert!(values_within_tolerance(1.0, 1.0 + 1e-12, relative, floor));
        // A 1e-6 relative deviation fails (outside 1e-9) — the negative
        // control territory.
        assert!(!values_within_tolerance(1.0, 1.0 + 1e-6, relative, floor));
        // Zero vs a small non-zero value: the floor decides, and the floor
        // is small enough that a real deviation still fails.
        assert!(values_within_tolerance(0.0, 1e-13, relative, floor));
        assert!(!values_within_tolerance(0.0, 1e-6, relative, floor));
        // Non-finite values never compare equal unless bit-identical.
        assert!(!values_within_tolerance(f64::NAN, f64::NAN, relative, floor));
        assert!(!values_within_tolerance(f64::INFINITY, 1.0, relative, floor));
    }

    #[test]
    fn the_nondeterminism_inventory_lists_and_neutralizes_every_source() {
        let sources: Vec<&str> = NONDETERMINISM_CONTROLS
            .iter()
            .map(|control| control.source)
            .collect();
        // The ledger names wall-clock, HashMap iteration order, and thread
        // count explicitly; all three must be present with a stated
        // neutralization.
        for required in ["wall_clock", "hash_map_iteration_order", "thread_count"] {
            assert!(sources.contains(&required), "missing source {required}");
        }
        for control in NONDETERMINISM_CONTROLS {
            assert!(
                control.neutralization.len() > 20,
                "source '{}' has no real neutralization statement",
                control.source
            );
        }
        // The thread-count control cites the existing solver pin.
        let thread = NONDETERMINISM_CONTROLS
            .iter()
            .find(|control| control.source == "thread_count")
            .expect("thread_count is listed");
        assert!(thread.neutralization.contains("thread_count = 1"));
    }

    #[test]
    fn the_checker_verdict_ranks_mismatch_over_tolerance_over_identical() {
        let identical = check_reproducibility(
            &[("leg_a", 0, 0, Vec::new()), ("leg_b", 0, 0, Vec::new())],
            "relative 1e-9",
        );
        assert_eq!(identical.verdict, VERDICT_IDENTICAL);
        let within = check_reproducibility(
            &[("leg_a", 0, 2, vec!["1e-12 MW at seq 3".to_string()])],
            "relative 1e-9",
        );
        assert_eq!(within.verdict, VERDICT_WITHIN_TOLERANCE);
        assert_eq!(within.diffs.len(), 1);
        assert!(within.diffs[0].starts_with("[leg_a]"));
        let mismatch = check_reproducibility(
            &[
                ("leg_a", 0, 2, Vec::new()),
                ("leg_b", 1, 0, vec!["value at seq 9".to_string()]),
            ],
            "relative 1e-9",
        );
        assert_eq!(mismatch.verdict, VERDICT_MISMATCH);
        assert!(mismatch.statement.contains("NOT reproducible"));
    }

    #[test]
    fn the_structural_diff_names_paths_and_respects_the_tolerance() {
        use serde_json::json;
        let reference = json!({
            "commands": [{"final_state": "COMPLETED", "target_p_mw": 1.0}],
            "records_ingested": 144,
        });
        // Within tolerance numerically: no mismatch, one within-tolerance.
        let mut out = Vec::new();
        structural_diff(
            &reference,
            &json!({
                "commands": [{"final_state": "COMPLETED", "target_p_mw": 1.0 + 1e-12}],
                "records_ingested": 144,
            }),
            "",
            DEFAULT_VALUE_RELATIVE_TOLERANCE,
            DEFAULT_VALUE_ABSOLUTE_FLOOR,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "within_tolerance");
        assert!(out[0].path.contains("target_p_mw"));
        // A changed text field is a mismatch with the named path.
        let mut out = Vec::new();
        structural_diff(
            &reference,
            &json!({
                "commands": [{"final_state": "FAILED", "target_p_mw": 1.0}],
                "records_ingested": 144,
            }),
            "",
            DEFAULT_VALUE_RELATIVE_TOLERANCE,
            DEFAULT_VALUE_ABSOLUTE_FLOOR,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "mismatch");
        assert_eq!(out[0].path, "commands[0].final_state");
        // An array length difference is a mismatch at the array path.
        let mut out = Vec::new();
        structural_diff(
            &reference,
            &json!({"commands": [], "records_ingested": 144}),
            "",
            DEFAULT_VALUE_RELATIVE_TOLERANCE,
            DEFAULT_VALUE_ABSOLUTE_FLOOR,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "commands");
    }
}
