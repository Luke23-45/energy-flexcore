# P8.12 — The GNN evaluation protocol: PURE metric definitions and protocol
#         rules computed on INJECTED data (no framework, no I/O, no torch).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. WHAT THIS MODULE IS: the P8.12 evaluation PROTOCOL — pure, stdlib-only
#    functions that define and compute the metrics the evaluation record's
#    tables are filled from, over data INJECTED by the caller: per-run
#    timings and outcomes (from the human/lab runs), the true
#    active-constraint/contingency sets (from the exact solves, recorded
#    per case), the frozen P8.06 baseline artifacts' timings, and the
#    P8.35 training-data manifest's topology lists. Like
#    `../andes_worker/evaluation_harness.py`, it imports NO simulator and
#    NO framework, and it reuses the worker's own pure functions
#    (`rank_candidates`, `evaluate_topology_scope`) — one definition of
#    the ranking order and the scope semantics, never a parallel one.
# 2. TEST ARITHMETIC IS NOT AN EXPERIMENT RESULT. Every expected value in
#    `run_self_tests()` is HAND-COMPUTED arithmetic over toy inputs,
#    stated in the test comments (e.g. solver 2.0 s + validation 0.5 s =
#    2.6 s end-to-end when inference took 0.1 s). That pins the metric
#    definitions. It is NOT a benchmark measurement, NOT a seed result,
#    and NOT evidence about any held-out topology. NO accuracy, latency,
#    speedup, success-rate, ranking-quality, or miss-cost figure from a
#    real run exists anywhere in this module, its tests, or its record:
#    every MEASURED cell is EMPTY until the human/lab run fills it
#    (ENTRY_NOTE boundary 4). (There is nothing to measure even in
#    principle today: the torch/PyG pair is a recorded DECISION-POINT —
#    no framework exists in this repository, so no inference output
#    exists, so no end-to-end run exists.)
# 3. THE HEADLINE RULE (§13.1; the ledger's named risk, enforced in
#    code): the headline metric is END-TO-END FEASIBLE SOLVE TIME —
#    inference + solver + validation up to a FEASIBLE, INDEPENDENTLY
#    VALIDATED solution — never model inference latency. A run whose
#    validation component is not recorded is REFUSED
#    (`VALIDATION_COMPONENT_MISSING`): a table without validation times
#    is a table that cannot prove its headline is not inference latency.
#    An infeasible or validation-failed run has NO end-to-end feasible
#    time — it is a recorded FAILURE (`NOT_A_SUCCESS`), never averaged
#    in as a fast success: a warm start that leads to an infeasible
#    solution fails the end-to-end metric, and if the solver fixed it,
#    that is the solver's success, not the GNN's.
# 4. WARM-START HONESTY (carried VERBATIM from the worker's boundary
#    statements, P8.10 §2): a warm start changes only the exact solver's
#    starting point: the solver still solves the problem to its own
#    termination criteria — the GNN does not replace the solver, and a
#    warm start affects SPEED of an exact solve, never its optimality.
#    NOTHING in this module measures, asserts, or scores optimality; the
#    speedup metrics compare TIMES of successful validated solves only.
#    Solver-class discipline (AGENTS.md rule 4): HiGHS is LP/QP/MIP,
#    never SOCP; Clarabel is convex conic, never MISOCP; AC power flow is
#    not AC-OPF.
# 5. NO THRESHOLD IS SET HERE (X-8; `02_research_scope_freeze.md` §5): the
#    falsifiable questions (P8.02 M2a/M2b) and their metrics are frozen;
#    the numeric success thresholds (minimum speedup, maximum miss rate,
#    miss-cost budget) are pre-registered in writing after the model,
#    service, benchmark, and operational risk are defined — at P8.06
#    freeze time at the earliest, never as universal targets, and never
#    inside this module. The functions compute raw metrics; the record's
#    falsification verdicts are the human evaluator's, against the
#    pre-registered thresholds.
# 6. HELD-OUT TOPOLOGY PROTOCOL (P8.10 §4, carried UNCONFIRMED): the
#    hold-out assignment below is the recorded RECOMMENDATION over the
#    §19.3 library — training families lib-01…lib-05, held-out families
#    lib-06/lib-07/lib-08 plus at least one structurally unseen family —
#    and it is UNCONFIRMED until the P8.35 training-data manifest exists
#    and the reviewer's overlap check passes (this module's
#    `check_heldout_overlap` is that check, in code). The leakage
#    controls (topology, normalization, regime — P8.10 §5) are
#    `verify_leakage_controls`' declared-record check; enforcement lives
#    at P8.35 ingestion and in the training pipeline, not here.
# 7. SCOPE-SEMANTICS GAP (recorded, not invented — Phase 7 follow-up):
#    the P8.11 worker FLAGS any topology outside the manifest's trained
#    list and returns NULL outputs. Held-out topologies are by definition
#    outside the trained list, so the worker AS IMPLEMENTED refuses them
#    (self-test 9 pins this behaviour — it is the contract-correct
#    refusal, never silently bypassed). The P8.12 transfer experiment
#    therefore REQUIRES a recorded scope-extension mechanism (an explicit
#    evaluation-scope manifest version, or an equivalent recorded P8.04
#    contract extension) BEFORE any held-out run can produce outputs.
#    Until that follow-up lands, every held-out cell stays EMPTY for
#    this reason TOO — recorded in `12_gnn_evaluation.md` §7, never
#    worked around here.
# 8. CONTROL BOUNDARY (ADR-010): evaluation metrics are DATA. Nothing
#    here authorizes, issues, or dispatches anything; no safety claim;
#    no field authority. Canonical units: MW/MVAr/MVA/MWh; SOC a fraction
#    in [0,1]; times in seconds (wall-clock measurements belong to the
#    human/lab run — this module only computes over the recorded values).
#
# Honest status: WRITTEN, NEVER EXECUTED. No python invocation ran this
# module (the loop bans forbid running it); the hand-computed self-tests
# and the pytest mirror (`tests/test_gnn_evaluation.py`) are written, not
# run. The human build/test gate owns the run evidence.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

# The worker's pure functions and constants are the SINGLE definition of
# the ranking order, the scope semantics, and the DECISION-POINT status —
# imported, never duplicated (same directory; the recorded seam). The
# module-level import is PURE: gnn_inference_worker's torch/PyG imports
# are guarded INSIDE its functions, so nothing is executed here.
from gnn_inference_worker import BOUNDARY_STATEMENTS, GNN_PIN_STATUS, rank_candidates

# The evaluation strategies (P8.06 baseline mapping: GNN warm-start vs the
# cold-start Tier 1 solver + the rule-based record's non-GNN heuristic
# warm start) are defined HERE, once, and pinned by tests.
EVALUATION_STRATEGIES: frozenset = frozenset(
    {"cold_start", "heuristic_warm_start", "gnn_warm_start"}
)

EVALUATION_PROTOCOL_ID = "flexcore-gnn-evaluation/1"
EVALUATION_VERSION = "flexcore-gnn-evaluation/0.1.0"

# The warm-start honesty statement, VERBATIM from the worker's boundary
# statements (statement[0]) — pinned by tests.
SOLVE_TIME_DISCIPLINE: str = BOUNDARY_STATEMENTS[0]

# The headline rule, VERBATIM (§13.1; ledger P8.12) — pinned by tests.
HEADLINE_RULE: str = (
    "the headline metric is END-TO-END FEASIBLE SOLVE TIME (inference + "
    "solver + validation up to a feasible, independently validated "
    "solution), never model inference latency (report 13.1; ledger P8.12)"
)

# The held-out protocol status, VERBATIM (P8.10 §4) — pinned by tests.
HOLDOUT_STATUS: str = (
    "RECOMMENDATION — UNCONFIRMED until the P8.35 training-data manifest "
    "exists: training families lib-01..lib-05; held-out families "
    "lib-06/lib-07/lib-08 plus at least one structurally unseen family; "
    "held-out operating regimes = the P8.19 research-case extensions. "
    "The reviewer's overlap check (check_heldout_overlap against the "
    "P8.35 manifest) is the evidence, never this constant"
)

# The pre-registration status, VERBATIM (X-8; 02_research_scope_freeze.md
# §5) — pinned by tests.
PRE_REGISTRATION_STATUS: str = (
    "the numeric success thresholds (minimum speedup, maximum miss rate, "
    "miss-cost budget) are NOT SET in this module or its record: they are "
    "pre-registered in writing after the model, service, benchmark, and "
    "operational risk are defined (P8.06 freeze time at the earliest), "
    "never as a universal target, and never after research numbers are "
    "known"
)

# The measured-cells status, VERBATIM (ENTRY_NOTE boundary 4) — pinned by
# tests.
MEASURED_CELLS_STATUS: str = (
    "every measured cell in the P8.12 evaluation record is EMPTY until "
    "the human/lab run fills it: no accuracy, latency, speedup, "
    "success-rate, ranking-quality, or miss-cost figure from a real run "
    "exists in this module, its tests, or its record; the tests' "
    "hand-computed toy arithmetic pins the metric definitions and is NOT "
    "a measurement"
)


class EvaluationError(ValueError):
    """A protocol refusal — the metric cannot be computed honestly from
    the injected data. Named in the message; never absorbed, never
    guessed around."""


# ---------------------------------------------------------------------------
# Role (a) — end-to-end feasible solve time (the headline, §13.1)
# ---------------------------------------------------------------------------


def _finite_nonnegative(run: dict, field: str, where: str) -> float:
    value = run.get(field)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvaluationError(
            "%s.%s must be a number (got %r)" % (where, field, value)
        )
    if not math.isfinite(value):
        raise EvaluationError("%s.%s must be finite (got %r)" % (where, field, value))
    if value < 0:
        raise EvaluationError(
            "%s.%s must be >= 0 (got %r); a negative timing is a recording "
            "error, never clamped" % (where, field, value)
        )
    return float(value)


def end_to_end_feasible_time(run: dict) -> float:
    """One successful run's END-TO-END FEASIBLE SOLVE TIME, seconds.

    `run` is one injected per-run record:
      {"strategy": "cold_start" | "heuristic_warm_start" | "gnn_warm_start",
       "topology_id": str,
       "solver_time_s": float,        # the exact solver's wall clock
       "validation_time_s": float,    # the P8.05 independent validation
       "inference_time_s": float,     # recorded for every GNN run
       "feasible": bool,              # the SOLVER's outcome
       "validation_passed": bool}     # the P8.05 harness's outcome

    Returns inference + solver + validation for runs that reached a
    FEASIBLE, INDEPENDENTLY VALIDATED solution. Refusals (named):
      - unknown strategy (`UNKNOWN_STRATEGY`);
      - missing/non-finite/negative component (`INVALID_COMPONENT`);
      - a run that did not reach a feasible+validated solution
        (`NOT_A_SUCCESS` — a failure is recorded as a failure, never
        averaged in as a fast success; if the solver fixed an infeasible
        warm start, that is the solver's success, not the GNN's);
      - a missing validation component (`VALIDATION_COMPONENT_MISSING`)
        — the guard that keeps the headline end-to-end and never
        inference latency (HONESTY HEADER item 3).
    """
    strategy = run.get("strategy")
    if strategy not in EVALUATION_STRATEGIES:
        raise EvaluationError(
            "run.strategy %r is not in the closed evaluation strategy set "
            "%s" % (strategy, sorted(EVALUATION_STRATEGIES))
        )
    where = "run[%s]" % run.get("topology_id", "?")
    solver = _finite_nonnegative(run, "solver_time_s", where)
    validation = _finite_nonnegative(run, "validation_time_s", where)
    if not (run.get("feasible") is True and run.get("validation_passed") is True):
        raise EvaluationError(
            "%s did not reach a feasible, independently validated solution "
            "(feasible=%r, validation_passed=%r): it has NO end-to-end "
            "feasible time — it is a recorded FAILURE (NOT_A_SUCCESS); if "
            "the solver fixed an infeasible warm start, that is the "
            "solver's success, not the GNN's" % (where, run.get("feasible"),
                                                 run.get("validation_passed"))
        )
    inference = 0.0
    if strategy == "gnn_warm_start":
        # A GNN run RECORDS its inference component (reported separately,
        # never presented as the headline); the end-to-end time includes
        # it because the clock runs until the feasible validated solution.
        inference = _finite_nonnegative(run, "inference_time_s", where)
    return inference + solver + validation


def feasible_success_rate(runs: list) -> float:
    """The feasible-solver success rate over one strategy's injected runs:
    the fraction that reached a feasible, independently validated
    solution. A warm start must not reduce this (P8.02 M2a) — the metric
    is computed here; the verdict is the pre-registered threshold's."""
    if not runs:
        raise EvaluationError(
            "feasible_success_rate over an empty run set is refused: an "
            "empty aggregate would read as a zero rate"
        )
    successes = 0
    for index, run in enumerate(runs):
        if run.get("strategy") not in EVALUATION_STRATEGIES:
            raise EvaluationError("runs[%d].strategy is unknown" % index)
        if run.get("feasible") is True and run.get("validation_passed") is True:
            successes += 1
    return successes / len(runs)


def speedup(baseline_time_s: float, treatment_time_s: float) -> float:
    """The paired speedup `baseline / treatment` over ONE topology, from
    END-TO-END times of two successful validated solves. Refuses
    non-finite or non-positive times (a zero/negative time is a recording
    error, never a division artifact). A speedup is a TIME ratio — it
    carries NO optimality claim (HONESTY HEADER item 4)."""
    for name, value in (("baseline_time_s", baseline_time_s),
                        ("treatment_time_s", treatment_time_s)):
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise EvaluationError("%s must be a number (got %r)" % (name, value))
        if not math.isfinite(value) or value <= 0.0:
            raise EvaluationError(
                "%s must be finite and > 0 (got %r); a speedup over a "
                "non-positive time is a division artifact, never computed"
                % (name, value)
            )
    return float(baseline_time_s) / float(treatment_time_s)


def paired_speedups(runs: list, baseline_strategy: str, treatment_strategy: str) -> dict:
    """Pairs the two strategies' runs BY TOPOLOGY (exact topology_id match)
    and computes the per-topology speedup where BOTH runs succeeded.

    Returns {"speedups": {topology_id: ratio}, "treatment_failures":
    [topology_id...], "unpaired_treatment_successes": [topology_id...]}:
    a topology where the treatment failed is a RECORDED FAILURE (it can
    never appear in the speedup table — a failure is not a fast solve),
    and a treatment success with no baseline pair on the same topology is
    listed unpaired, never silently dropped (the ledger's no-silent-drop
    rule)."""
    if baseline_strategy not in EVALUATION_STRATEGIES:
        raise EvaluationError("baseline_strategy %r is unknown" % baseline_strategy)
    if treatment_strategy not in EVALUATION_STRATEGIES:
        raise EvaluationError("treatment_strategy %r is unknown" % treatment_strategy)
    if baseline_strategy == treatment_strategy:
        raise EvaluationError(
            "the baseline and treatment strategies are the same; a speedup "
            "of a strategy against itself is not an experiment"
        )

    def succeeded(run: dict) -> bool:
        return run.get("feasible") is True and run.get("validation_passed") is True

    baseline: dict = {}
    treatment: dict = {}
    for run in runs:
        strategy = run.get("strategy")
        topology_id = run.get("topology_id")
        if not isinstance(topology_id, str) or not topology_id.strip():
            raise EvaluationError(
                "every run needs a non-empty topology_id to pair by"
            )
        if strategy == baseline_strategy:
            if topology_id in baseline:
                raise EvaluationError(
                    "duplicate %s run for topology %r; pairing needs one "
                    "run per topology per strategy" % (baseline_strategy, topology_id)
                )
            baseline[topology_id] = run
        elif strategy == treatment_strategy:
            if topology_id in treatment:
                raise EvaluationError(
                    "duplicate %s run for topology %r; pairing needs one "
                    "run per topology per strategy" % (treatment_strategy, topology_id)
                )
            treatment[topology_id] = run

    speedups: dict = {}
    treatment_failures: list = []
    unpaired: list = []
    for topology_id, run in treatment.items():
        if not succeeded(run):
            treatment_failures.append(topology_id)
            continue
        if topology_id not in baseline:
            unpaired.append(topology_id)
            continue
        baseline_run = baseline[topology_id]
        if not succeeded(baseline_run):
            # The BASELINE failed on this topology: no paired time exists.
            unpaired.append(topology_id)
            continue
        # Component validation with the named refusals, then the ratio of
        # the two end-to-end times (one arithmetic path, never a parallel
        # one).
        speedups[topology_id] = speedup(
            end_to_end_feasible_time(baseline_run),
            end_to_end_feasible_time(run),
        )
    treatment_failures.sort()
    unpaired.sort()
    return {
        "speedups": speedups,
        "treatment_failures": treatment_failures,
        "unpaired_treatment_successes": unpaired,
    }


# ---------------------------------------------------------------------------
# Role (b) — screening ranking quality (P8.02 M2b's falsifiable question)
# ---------------------------------------------------------------------------


def screening_metrics(scored: list, true_active: set, screen_depth: int,
                      miss_costs: dict) -> dict:
    """Ranking quality of ONE screening run against the TRUE active set.

    `scored` is the worker-shaped candidate list
    [{"candidate_id": str, "score": float, "uncertainty": float}]; it is
    ranked with the worker's OWN deterministic order
    (`gnn_inference_worker.rank_candidates` — descending score,
    candidate_id-ascending tie-break: one definition, never a parallel
    one). `true_active` is the recorded ground truth from the exact
    solves. The top `screen_depth` candidates are screened IN as
    "active"; the rest are screened OUT.

    Returns {"ranking": [ids in order], "screened_in", "misses",
    "false_alarms", "hits", "precision_at_screen_depth", "miss_rate",
    "miss_cost"}:
      - misses = active candidates SCREENED OUT — the cost of a missed
        constraint is a named P8.12 metric (P8.02 M2b), and every miss
        needs a RECORDED per-candidate cost in `miss_costs`
        (`MISS_COST_UNRECORDED` otherwise — a fabricated zero would make
        the headline metric look free);
      - miss_rate = |misses| / |true_active| (refused when the true
        active set is empty — a screening experiment with no active
        constraint is not a screening test);
      - precision_at_screen_depth = |hits| / screen_depth.
    No threshold is applied (X-8): the miss rate and miss cost are raw
    metrics; the falsification verdict is the human evaluator's against
    the pre-registered thresholds.
    """
    ranked = rank_candidates(scored)
    ranking_ids = [entry["candidate_id"] for entry in ranked]
    if len(set(ranking_ids)) != len(ranking_ids):
        raise EvaluationError("duplicate candidate_id in the ranking")
    if not isinstance(true_active, set):
        raise EvaluationError("true_active must be a set of candidate ids")
    unknown = sorted(true_active - set(ranking_ids))
    if unknown:
        raise EvaluationError(
            "true_active names candidates absent from the scored list: %s "
            "— the ground truth and the ranking must cover the same "
            "candidate set" % unknown
        )
    if isinstance(screen_depth, bool) or not isinstance(screen_depth, int):
        raise EvaluationError("screen_depth must be an integer")
    if not (0 <= screen_depth <= len(ranking_ids)):
        raise EvaluationError(
            "screen_depth %d is outside [0, %d]" % (screen_depth, len(ranking_ids))
        )
    if not true_active:
        raise EvaluationError(
            "the true active set is empty: a screening experiment with no "
            "active constraint is not a screening test (EMPTY_TRUE_ACTIVE)"
        )
    screened_in = set(ranking_ids[:screen_depth])
    hits = screened_in & true_active
    false_alarms = screened_in - true_active
    misses = true_active - screened_in
    if not isinstance(miss_costs, dict):
        raise EvaluationError("miss_costs must be a mapping candidate_id -> cost")
    missing_costs = sorted(misses - set(miss_costs))
    if missing_costs:
        raise EvaluationError(
            "no recorded miss cost for %s: the cost of a missed constraint "
            "is a pre-registered metric and every candidate needs its "
            "RECORDED cost — never a fabricated zero "
            "(MISS_COST_UNRECORDED)" % missing_costs
        )
    for candidate_id, cost in miss_costs.items():
        if isinstance(cost, bool) or not isinstance(cost, (int, float)) \
                or not math.isfinite(cost) or cost < 0:
            raise EvaluationError(
                "miss_costs[%r] must be a finite number >= 0 (got %r)"
                % (candidate_id, cost)
            )
    miss_cost_total = sum(float(miss_costs[candidate_id]) for candidate_id in misses)
    return {
        "ranking": ranking_ids,
        "screened_in": sorted(screened_in),
        "hits": sorted(hits),
        "false_alarms": sorted(false_alarms),
        "misses": sorted(misses),
        "precision_at_screen_depth": len(hits) / screen_depth if screen_depth else 0.0,
        "miss_rate": len(misses) / len(true_active),
        "miss_cost": miss_cost_total,
    }


# ---------------------------------------------------------------------------
# The held-out topology protocol and the leakage controls (P8.10 §4–§5)
# ---------------------------------------------------------------------------


def check_heldout_overlap(trained_topology_ids: list, heldout_topology_ids: list) -> dict:
    """The reviewer's overlap check, in code (the ledger P8.10/P8.12
    verification method): any topology id BOTH trained on and claimed
    held-out is LEAKAGE — the generalization claim is invalid for it.

    Exact string equality (the worker's scope semantics); returns
    {"overlap": sorted list, "leakage": bool, "finding": str}. A non-empty
    overlap is a FINDING against the evaluation, never absorbed."""
    overlap = sorted(set(trained_topology_ids) & set(heldout_topology_ids))
    return {
        "overlap": overlap,
        "leakage": bool(overlap),
        "finding": (
            "LEAKAGE: %s appear in BOTH the trained and held-out lists — "
            "the generalization claim is invalid for them" % overlap
            if overlap
            else "no overlap between the trained and held-out topology "
                 "lists (exact string equality)"
        ),
    }


def verify_leakage_controls(dataset_record: dict) -> list:
    """Checks the P8.10 §5 leakage-control declarations of one
    training-dataset record (the P8.35 manifest's shape):

      - "normalization_split" == "training_only" (control 2: test/held-out
        cases are normalized with the FROZEN training statistics — a test
        case never contributes to a statistic);
      - "regime_seeds_disjoint_from_holdout" is True (control 3);
      - "dataset_version" non-empty (the versioning discipline);
      - "heldout_topology_overlap_checked" is True (control 1 — the
        check_heldout_overlap result is recorded).

    Returns the list of FINDINGS (empty = every control is declared);
    enforcement lives at P8.35 ingestion and in the training pipeline —
    this check is the evaluation-time audit that the declarations exist
    and are not silently violated."""
    findings: list = []
    if not isinstance(dataset_record, dict):
        return ["dataset_record is not a mapping"]
    if dataset_record.get("normalization_split") != "training_only":
        findings.append(
            "normalization_split %r is not 'training_only': test/held-out "
            "data must never contribute to normalization statistics "
            "(P8.10 leakage control 2)" % (dataset_record.get("normalization_split"),)
        )
    if dataset_record.get("regime_seeds_disjoint_from_holdout") is not True:
        findings.append(
            "regime_seeds_disjoint_from_holdout is not True: held-out "
            "operating-regime seeds must be disjoint from and recorded "
            "against the training seeds (P8.10 leakage control 3)"
        )
    version = dataset_record.get("dataset_version")
    if not isinstance(version, str) or not version.strip():
        findings.append(
            "dataset_version is missing or empty: the dataset version must "
            "be linked to the model artifact (P8.10 §5; P8.35 step 4)"
        )
    if dataset_record.get("heldout_topology_overlap_checked") is not True:
        findings.append(
            "heldout_topology_overlap_checked is not True: the trained/"
            "held-out overlap check must be run and recorded "
            "(P8.10 leakage control 1)"
        )
    return findings


# ---------------------------------------------------------------------------
# Hand-computed self-tests (WRITTEN, NEVER EXECUTED — the loop bans forbid
# running them; the human build/test gate owns the run. Every expected
# value is HAND-COMPUTED arithmetic over toy inputs, stated in the
# comments. THIS IS TEST ARITHMETIC PINNING THE METRIC DEFINITIONS — NOT a
# benchmark measurement, NOT a seed result, NOT evidence about any
# topology; every measured cell stays EMPTY: ENTRY_NOTE boundary 4.)
# ---------------------------------------------------------------------------

def run_self_tests() -> list:
    results: list = []

    def check(name: str, condition: bool, detail: str = "") -> None:
        results.append((name, "PASS" if condition else "FAIL", detail))

    # ── 1. End-to-end feasible time (test arithmetic) ────────────────────
    # cold run: solver 2.0 s + validation 0.5 s = 2.5 s (no inference).
    cold = {
        "strategy": "cold_start",
        "topology_id": "t-cold",
        "solver_time_s": 2.0,
        "validation_time_s": 0.5,
        "feasible": True,
        "validation_passed": True,
    }
    check("e2e cold 2.0 + 0.5 = 2.5", end_to_end_feasible_time(cold) == 2.5)
    # GNN run: inference 0.1 + solver 2.0 + validation 0.5 = 2.6 s — the
    # inference component is INCLUDED in end-to-end and REPORTED
    # separately; the headline is the sum, never the 0.1 s alone.
    gnn = dict(cold, strategy="gnn_warm_start", topology_id="t-gnn",
               inference_time_s=0.1)
    check("e2e gnn 0.1 + 2.0 + 0.5 = 2.6", end_to_end_feasible_time(gnn) == 2.6)

    # ── 2. NOT_A_SUCCESS: a validation-failed run has NO e2e time ────────
    failed = dict(gnn, feasible=False)
    try:
        end_to_end_feasible_time(failed)
        check("infeasible run refused", False, "no refusal raised")
    except EvaluationError as error:
        check("infeasible run refused", "NOT_A_SUCCESS" in str(error))

    solver_fixed = dict(gnn, feasible=True, validation_passed=False)
    try:
        end_to_end_feasible_time(solver_fixed)
        check("validation-failed run refused", False, "no refusal raised")
    except EvaluationError as error:
        check("validation-failed run refused", "NOT_A_SUCCESS" in str(error))

    # ── 3. VALIDATION_COMPONENT_MISSING: the headline guard ──────────────
    no_validation = dict(gnn)
    del no_validation["validation_time_s"]
    try:
        end_to_end_feasible_time(no_validation)
        check("missing validation component refused", False, "no refusal raised")
    except EvaluationError as error:
        check("missing validation component refused",
              "must be a number" in str(error))

    # ── 4. Speedup arithmetic and refusals (test arithmetic) ─────────────
    # cold 10.0 s e2e vs gnn 4.0 s e2e → 10.0 / 4.0 = 2.5x. TEST ARITHMETIC.
    check("speedup 10/4 = 2.5", speedup(10.0, 4.0) == 2.5)
    check("speedup 4/10 < 1", speedup(4.0, 10.0) == 0.4)
    try:
        speedup(0.0, 4.0)
        check("zero baseline time refused", False)
    except EvaluationError as error:
        check("zero baseline time refused", "must be finite and > 0" in str(error))

    # ── 5. Feasible-solver success rate (test arithmetic) ────────────────
    # Four runs, two succeeded (cold, gnn) → 2/4 = 0.5; the two failures
    # (one infeasible, one validation-failed) count as failures.
    four_runs = [
        dict(cold),
        dict(gnn),
        dict(gnn, topology_id="t-gnn2", feasible=False),
        dict(cold, topology_id="t-cold2", validation_passed=False),
    ]
    check("success rate 2/4 = 0.5", feasible_success_rate(four_runs) == 0.5)
    try:
        feasible_success_rate([])
        check("empty rate refused", False)
    except EvaluationError as error:
        check("empty rate refused", "empty run set" in str(error))

    # ── 6. Paired speedups (test arithmetic) ─────────────────────────────
    # cold: t1 10.0, t2 8.0; gnn: t1 4.0 (→ 2.5x), t2 FAILED, t3 2.0
    # (success but no cold pair → unpaired, listed). Hand-summed:
    # speedups {"t1": 2.5}; failures ["t2"]; unpaired ["t3"].
    paired_runs = [
        dict(cold, topology_id="t1", solver_time_s=9.5),   # 9.5 + 0.5 = 10.0
        dict(cold, topology_id="t2", solver_time_s=7.5),   # 7.5 + 0.5 = 8.0
        dict(gnn, topology_id="t1", solver_time_s=3.4),    # 0.1 + 3.4 + 0.5 = 4.0
        dict(gnn, topology_id="t2", feasible=False),
        dict(gnn, topology_id="t3", solver_time_s=1.4),    # 0.1 + 1.4 + 0.5 = 2.0
    ]
    paired = paired_speedups(paired_runs, "cold_start", "gnn_warm_start")
    check("paired speedups t1 = 10/4 = 2.5",
          paired["speedups"].get("t1") == 2.5)
    check("t2 recorded as a treatment failure",
          paired["treatment_failures"] == ["t2"])
    check("t3 listed unpaired, never dropped",
          paired["unpaired_treatment_successes"] == ["t3"])

    # ── 7. Screening metrics (test arithmetic) ───────────────────────────
    # Five candidates; scores chosen so the deterministic order is
    # c1..c5; true active {c1, c3}; screen depth 2 → screened in
    # {c1, c2}: hits {c1}, false alarms {c2}, misses {c3}.
    # precision@2 = 1/2 = 0.5; miss rate = 1/2 = 0.5; miss cost = 7.0
    # (the recorded cost of c3). TEST ARITHMETIC.
    scored = [
        {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.1},
        {"candidate_id": "c2", "score": 0.8, "uncertainty": 0.2},
        {"candidate_id": "c3", "score": 0.7, "uncertainty": 0.3},
        {"candidate_id": "c4", "score": 0.6, "uncertainty": 0.2},
        {"candidate_id": "c5", "score": 0.5, "uncertainty": 0.4},
    ]
    metrics = screening_metrics(scored, {"c1", "c3"}, 2, {"c3": 7.0, "c1": 3.0})
    check("screening ranking order c1..c5",
          metrics["ranking"] == ["c1", "c2", "c3", "c4", "c5"])
    check("screening misses = {c3}", metrics["misses"] == ["c3"])
    check("screening false alarms = {c2}", metrics["false_alarms"] == ["c2"])
    check("precision@2 = 0.5", metrics["precision_at_screen_depth"] == 0.5)
    check("miss rate = 0.5", metrics["miss_rate"] == 0.5)
    check("miss cost = 7.0", metrics["miss_cost"] == 7.0)
    # A missed candidate without a RECORDED cost is refused (never a
    # fabricated zero).
    try:
        screening_metrics(scored, {"c1", "c3"}, 2, {"c1": 3.0})
        check("unrecorded miss cost refused", False)
    except EvaluationError as error:
        check("unrecorded miss cost refused", "MISS_COST_UNRECORDED" in str(error))
    # An empty true active set is refused.
    try:
        screening_metrics(scored, set(), 2, {})
        check("empty true active refused", False)
    except EvaluationError as error:
        check("empty true active refused", "EMPTY_TRUE_ACTIVE" in str(error))

    # ── 8. Held-out overlap / leakage controls ───────────────────────────
    # trained {t1, t2}, held-out {t2, t3} → overlap {t2} → LEAKAGE.
    overlap = check_heldout_overlap(["t1", "t2"], ["t2", "t3"])
    check("overlap finds t2", overlap["overlap"] == ["t2"] and overlap["leakage"])
    clean = check_heldout_overlap(["t1"], ["t2"])
    check("disjoint lists pass", not clean["leakage"])
    # A complete record → no findings; each missing control → a finding.
    complete = {
        "normalization_split": "training_only",
        "regime_seeds_disjoint_from_holdout": True,
        "dataset_version": "dataset-v1",
        "heldout_topology_overlap_checked": True,
    }
    check("complete leakage record: no findings",
          verify_leakage_controls(complete) == [])
    check("missing controls produce 4 findings",
          len(verify_leakage_controls({})) == 4)

    # ── 9. The scope-semantics gap, pinned (HONESTY HEADER item 7) ───────
    # The worker (contract-correctly) FLAGS a held-out topology: held-out
    # ids are by definition outside the manifest's trained list, so the
    # as-implemented worker refuses them — the transfer experiment needs
    # the recorded Phase 7 follow-up BEFORE any held-out run.
    from gnn_inference_worker import evaluate_topology_scope
    scope = evaluate_topology_scope("heldout-t6", ["train-t1", "train-t2"],
                                    "balanced-radial-feeder")
    check("held-out topology is flagged out of scope (contract-correct)",
          scope["in_scope"] is False)

    # ── 10. The carried statements, VERBATIM ─────────────────────────────
    check("warm-start statement carried verbatim (speed, never optimality)",
          "affects SPEED of an exact solve, never its optimality"
          in SOLVE_TIME_DISCIPLINE)
    check("headline rule carried verbatim",
          "never model inference latency" in HEADLINE_RULE)
    check("hold-out status carried UNCONFIRMED",
          HOLDOUT_STATUS.startswith("RECOMMENDATION — UNCONFIRMED")
          and "lib-06" in HOLDOUT_STATUS)
    check("no threshold set here (X-8)",
          "NOT SET" in PRE_REGISTRATION_STATUS
          and "never as a universal target" in PRE_REGISTRATION_STATUS)
    check("measured-cells status carried (ENTRY_NOTE boundary 4)",
          "EMPTY until" in MEASURED_CELLS_STATUS and "NOT a" in MEASURED_CELLS_STATUS)
    check("DECISION-POINT carried verbatim (P8.03 §4)",
          GNN_PIN_STATUS.startswith("DECISION-POINT OPEN"))

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    failures = [entry for entry in run_self_tests() if entry[1] != "PASS"]
    for name, status, detail in run_self_tests():
        print("%s %s%s" % (status, name, (" — " + detail) if detail else ""))
    if failures:
        raise SystemExit(1)
    print("all hand-computed self-tests PASS (test arithmetic, not "
          "measurements)")
