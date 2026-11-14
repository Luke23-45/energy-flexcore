# P8.15 — The MARL evaluation harness (pure, stdlib-only): safety-filter
#         statistics and per-case §19.4 MARL metrics over INJECTED data.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. The ledger P8.15 aggregation code: pure stdlib
#    functions that compute the safety-filter statistics (activation rate,
#    infeasibility rate, and the outcome of EVERY infeasibility — ledger
#    step 3) and the per-case §19.4 MARL metric rows (constraint-violation
#    frequency, return, distribution-shift deltas, per-metric baseline
#    comparison — ledger step 2) from INJECTED shadow-log entries and
#    INJECTED consequence records. Every function is additive over the
#    P8.14 worker: it imports the sibling pure modules only, changes
#    NOTHING in `marl_worker.py` / `safety_filter.py` / `shadow_log.py`,
#    and performs NO I/O of its own.
# 2. NOTHING HERE HAS EVER RUN, AND NO MEASURED CELL EXISTS. The module
#    was WRITTEN, NEVER RUN (Phase 8 loop bans: nothing installed, built,
#    or executed; no pytest). The hand-computed expectations in
#    `run_self_tests` are derivations from constructed inputs with the
#    arithmetic STATED in each test (e.g. 1 activation in 5 actions is
#    exactly 1/5 = 0.2) — they pin the metric DEFINITIONS and are NOT a
#    benchmark measurement, NOT a seed result, and NOT evidence about any
#    case. Every measured cell in the P8.15 record
#    (`ledger/phase-08-advanced-coordination/15_marl_evaluation.md`) is
#    EMPTY until the human/lab gate fills it via the recorded commands.
# 3. THE HARNESS NEVER COMPUTES A VIOLATION OR A RETURN. Whether a
#    decision violated a constraint, and what return the policy earned,
#    are facts about a RUN — they come from the human/lab environment
#    outputs and enter ONLY through `build_case_consequence`, which
#    validates and shapes the record without ever deriving one. A log
#    slice that still carries `consequence_status: "not_simulated"` is
#    refused with the named error
#    VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED: an EMPTY
#    consequence cannot be silently turned into a frequency of zero
#    (zero observed violations from a finite test set is a TEST OUTCOME,
#    never a safety guarantee — §13.3, X-4; stated in plain language in
#    the P8.15 evaluation note).
# 4. NO VERDICT WITHOUT A PRE-REGISTERED THRESHOLD (X-8; §19.4). The
#    comparison function computes deltas and, per metric separately,
#    win/tie/loss — ONLY when the caller injects the pre-registered
#    tolerances (THRESHOLD_NOT_PRE_REGISTERED otherwise, mirroring the
#    P8.09 harness discipline). The two metric verdicts are NEVER merged
#    into one headline number (P8.13 §2.6: constraint-violation frequency
#    is reported alongside return, never merged).
# 5. REPORTING ONLY — NEVER A DISPATCH INPUT (ADR-010; the ledger's
#    named P8.22 risk applies here too). Every output carries the fixed
#    reporting-only statement; the module namespace contains NO
#    dispatch/issue/authorize/approve/command-shaped callable (pinned by
#    a self-test); nothing this module computes can become a dispatch
#    input, and no function here returns anything ranking-shaped.
# 6. THE PI BASELINE HAS NO FROZEN ID (recorded contract gap). The
#    baseline-id vocabulary below is closed to the frozen P8.06 registry
#    (`05_06_harness_baselines.md` §3); the §13.3 "rule-based/PI"
#    baseline's PI member is NOT in the frozen registry, so a comparison
#    naming it is refused BASELINE_UNKNOWN — the gap is recorded as a
#    Phase 7/P8.06 follow-up, never invented here.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

# The sibling pure modules (stdlib-only themselves; additive imports ONLY
# — no existing marl_worker file is modified by this module). The closed
# vocabularies are single-sourced from them so they cannot drift.
from safety_filter import FILTER_OUTCOMES, REFUSING_OUTCOMES
import shadow_log

# The harness output schema tag (carried on every statistics object).
EVALUATION_HARNESS_SCHEMA = "flexcore-marl-eval/1"

# The closed case-family vocabulary (ledger P8.15 step 1). The concrete
# case ids are RECORD-AT-FIRST-RUN fields of the evaluation record — the
# suite is declared BEFORE any run and frozen at the first run.
CASE_FAMILIES: frozenset = frozenset(
    {
        "held_out_topology",
        "held_out_regime",
        "device_outage",
        "message_delay_loss",
        "state_estimate_uncertainty",
    }
)

# The closed baseline-id vocabulary — EXACTLY the frozen P8.06 registry
# (`05_06_harness_baselines.md` §3). The per-module mapping for MARL is
# the deterministic Tier-1 LP baseline AND the rule-based baseline (§13.3).
# A PI baseline has NO frozen id (header item 6 — recorded contract gap).
FROZEN_BASELINE_IDS: frozenset = frozenset(
    {
        "baseline/highs-tier1-lp",
        "baseline/clarabel-tier2-conic",
        "baseline/rule-based-priority-list",
    }
)

# The baselines the MARL per-module mapping names (deterministic
# optimization + rule-based, §13.3 via `05_06_harness_baselines.md` §3).
MARL_BASELINE_IDS: frozenset = frozenset(
    {
        "baseline/highs-tier1-lp",
        "baseline/rule-based-priority-list",
    }
)

# The closed consequence-record schema tag (the P8.15 fill-in contract:
# the shape the human/lab harness attaches as a log entry's
# `simulated_consequence` when `consequence_status` is
# "recorded_by_harness"). Declared here, additive over the P8.14 log
# schema (whose `simulated_consequence` field is an opaque object).
CASE_CONSEQUENCE_SCHEMA = "flexcore-marl-case-consequence/1"

# The marker recorded when a fact the statistics need is NOT in the P8.14
# log schema (recorded honesty, never a fabricated default) — see header
# item 6 of the P8.15 record for the follow-up.
NOT_RECORDED_IN_LOG_SCHEMA = "NOT_RECORDED_IN_LOG_SCHEMA"

REPORTING_ONLY_STATEMENT = (
    "evaluation reporting only — a diagnostic record over injected "
    "simulation/shadow data; it never enters the dispatch lifecycle and "
    "no function in this module produces a dispatch or ranking input "
    "(ADR-010)"
)


class EvaluationHarnessError(ValueError):
    """A refused evaluation input or an unrepresentable statistic.

    Named errors (message prefix): EVAL_LOG_EMPTY,
    VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED,
    CONSEQUENCE_SHAPE_INVALID, RETURN_UNIT_INCONSISTENT,
    CASE_FAMILY_UNKNOWN, BASELINE_UNKNOWN, BASELINE_STATS_INVALID,
    THRESHOLD_NOT_PRE_REGISTERED, TOLERANCE_INVALID,
    STATS_OBJECT_INVALID. A refusal is a RESULT, never a silent zero,
    never a fabricated figure.
    """


def _require_non_empty_log(entries: object) -> list:
    """Refuses an empty/None log slice with the named error (the first
    check of every statistic — a rate over an empty denominator would be
    a fabricated figure)."""
    if not isinstance(entries, list) or not entries:
        raise EvaluationHarnessError(
            "EVAL_LOG_EMPTY: a statistics request over an empty log "
            "slice is refused — an empty denominator cannot become a "
            "rate of zero (that would be a fabricated figure)"
        )
    return entries


# ---------------------------------------------------------------------------
# Ledger step 3 — the safety-filter statistics over the shadow log
# ---------------------------------------------------------------------------


def filter_statistics(entries: list) -> dict:
    """The P8.15 safety-filter statistics (pure, over INJECTED entries).

    Validates the log first (strictly-increasing 1-based sequences — a
    gap would hide an action), then computes, with the denominator the
    harness owns (`shadow_log.log_summary` carries COUNTS only by design):

      activation_rate   = filter_activation_count / total_actions
      infeasibility_rate = infeasibility_count    / total_actions
      refusal_rate      = refusal_count           / total_actions

    and the OUTCOME OF EVERY INFEASIBILITY (ledger step 3): one row per
    entry whose `infeasibility` is non-null, carrying the named outcome
    (checked against the closed vocabulary), the affected asset ids, the
    recorded flag, and the fallback facts — recorded honestly as
    NOT_RECORDED_IN_LOG_SCHEMA when the P8.14 log schema does not carry
    them (the recorded contract gap; never a fabricated default).
    """
    _require_non_empty_log(entries)
    shadow_log.validate_shadow_log(entries)
    summary = shadow_log.log_summary(entries)
    total = summary["total_actions"]

    infeasibility_outcomes: list = []
    for entry in entries:
        infeasibility = entry.get("infeasibility")
        if infeasibility is None:
            continue
        if not isinstance(infeasibility, dict):
            raise EvaluationHarnessError(
                "CONSEQUENCE_SHAPE_INVALID: shadow_log[%d].infeasibility "
                "must be an object when present" % entry["sequence"]
            )
        outcome = infeasibility.get("outcome")
        if outcome not in FILTER_OUTCOMES:
            raise EvaluationHarnessError(
                "CONSEQUENCE_SHAPE_INVALID: shadow_log[%d].infeasibility."
                "outcome %r is not in the closed vocabulary %s"
                % (entry["sequence"], outcome, sorted(FILTER_OUTCOMES))
            )
        infeasibility_outcomes.append(
            {
                "sequence": entry["sequence"],
                "request_id": entry["request_id"],
                "outcome": outcome,
                "assets": infeasibility.get("entries"),
                "recorded": infeasibility.get("recorded"),
                "fallback_attempted": infeasibility.get(
                    "fallback_attempted", NOT_RECORDED_IN_LOG_SCHEMA
                ),
                "fallback_viable": infeasibility.get(
                    "fallback_viable", NOT_RECORDED_IN_LOG_SCHEMA
                ),
                "consequence_status": entry["consequence_status"],
            }
        )

    return {
        "schema": EVALUATION_HARNESS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "total_actions": total,
        "counts": {
            "by_outcome": dict(summary["by_outcome"]),
            "filter_activation_count": summary["filter_activation_count"],
            "infeasibility_count": summary["infeasibility_count"],
            "refusal_count": summary["refusal_count"],
            "refusal_outcomes": {
                outcome: summary["by_outcome"].get(outcome, 0)
                for outcome in sorted(REFUSING_OUTCOMES)
            },
            "consequence_status_counts": dict(
                summary["consequence_status_counts"]
            ),
        },
        # Rates: exact division over the validated total; the arithmetic
        # is pinned by the hand-computed self-tests (stated fractions).
        "activation_rate": summary["filter_activation_count"] / total,
        "infeasibility_rate": summary["infeasibility_count"] / total,
        "refusal_rate": summary["refusal_count"] / total,
        "outcome_of_every_infeasibility": infeasibility_outcomes,
    }


# ---------------------------------------------------------------------------
# The consequence record (the P8.15 fill-in contract) and the per-case
# §19.4 metrics (ledger step 2)
# ---------------------------------------------------------------------------


def build_case_consequence(
    *,
    constraint_violation: bool,
    violation_kind: str | None,
    return_value: float,
    return_unit: str,
) -> dict:
    """Validates and shapes ONE decision's consequence record (pure).

    This is the shape the human/lab harness attaches as a log entry's
    `simulated_consequence` (with `consequence_status` set to
    "recorded_by_harness" — the value the P8.14 logger declares for the
    fill-in path). THE HARNESS NEVER COMPUTES A VIOLATION OR A RETURN
    (header item 3): both arrive from the run's environment outputs and
    are only validated here.

    Rules:
      - `constraint_violation` is a real bool (bool-excluded ints refused);
      - a violation MUST name a non-empty `violation_kind`; a
        non-violation MUST carry None (a kind without a violation would
        be a fabricated event);
      - `return_value` is a finite number (bool excluded) in the
        caller-declared `return_unit` (non-empty string — the unit is
        DECLARED, never assumed, per the canonical-units discipline);
      - SOC and power fields, where a consequence record elaborates them,
        follow the canonical units (SOC a fraction in [0, 1]; MW/MWh) and
        belong to the elaboration, not to this minimal record.
    """
    if not isinstance(constraint_violation, bool):
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: constraint_violation must be a "
            "bool (got %r)" % (constraint_violation,)
        )
    if constraint_violation:
        if not isinstance(violation_kind, str) or not violation_kind.strip():
            raise EvaluationHarnessError(
                "CONSEQUENCE_SHAPE_INVALID: a constraint_violation=True "
                "record MUST name a non-empty violation_kind (an unnamed "
                "violation is an unevaluable event)"
            )
    elif violation_kind is not None:
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: a constraint_violation=False "
            "record MUST carry violation_kind=None (a kind without a "
            "violation would be a fabricated event)"
        )
    if isinstance(return_value, bool) or not isinstance(return_value, (int, float)):
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: return_value must be a finite "
            "number (got %r)" % (return_value,)
        )
    if not _is_finite_number(return_value):
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: return_value must be finite"
        )
    if not isinstance(return_unit, str) or not return_unit.strip():
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: return_unit must be a non-empty "
            "string (the unit is declared, never assumed)"
        )
    return {
        "consequence_schema": CASE_CONSEQUENCE_SCHEMA,
        "constraint_violation": constraint_violation,
        "violation_kind": violation_kind,
        "return_value": float(return_value),
        "return_unit": return_unit,
        "source": (
            "recorded_by_harness — a fact about the run supplied by the "
            "human/lab environment outputs; this module validates and "
            "shapes it, it never computes a violation or a return"
        ),
    }


def _consequence_of(entry: dict, index: int) -> dict:
    """Extracts and shape-checks one entry's consequence record."""
    if entry.get("consequence_status") != "recorded_by_harness":
        raise EvaluationHarnessError(
            "VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED: "
            "shadow_log[%d] has consequence_status %r — an EMPTY "
            "consequence cannot become a frequency of zero; the human/lab "
            "gate records consequences first (zero observed violations "
            "from a finite test set is a test outcome, not a safety "
            "guarantee — §13.3)"
            % (entry.get("sequence"), entry.get("consequence_status"))
        )
    consequence = entry.get("simulated_consequence")
    if (
        not isinstance(consequence, dict)
        or consequence.get("consequence_schema") != CASE_CONSEQUENCE_SCHEMA
        or not isinstance(consequence.get("constraint_violation"), bool)
        or not isinstance(consequence.get("return_value"), (int, float))
        or isinstance(consequence.get("return_value"), bool)
        or not isinstance(consequence.get("return_unit"), str)
    ):
        raise EvaluationHarnessError(
            "CONSEQUENCE_SHAPE_INVALID: shadow_log[%d]."
            "simulated_consequence does not match %s — rebuild it with "
            "build_case_consequence instead of hand-rolling a dict"
            % (entry.get("sequence"), CASE_CONSEQUENCE_SCHEMA)
        )
    return consequence


def case_statistics(entries: list) -> dict:
    """The §19.4 per-case MARL metrics for ONE case's log slice (pure).

    The caller injects exactly the log entries of ONE case (the slice is
    a caller decision — the log schema carries no case-id field, a
    recorded contract gap). Every entry must carry a recorded consequence
    (a not_simulated slice is refused — header item 3), and every
    consequence must share one declared return unit.

      violation_frequency = violated_decisions / total_decisions
      mean_return         = sum(return_value)   / total_decisions

    Both are exact divisions over the injected slice; the arithmetic is
    pinned by hand-computed self-tests. The violation-kind multiset is
    reported alongside (never merged into the frequency — a frequency
    alone would hide WHAT was violated).
    """
    _require_non_empty_log(entries)
    shadow_log.validate_shadow_log(entries)
    consequences = [_consequence_of(entry, i) for i, entry in enumerate(entries)]

    units = {c["return_unit"] for c in consequences}
    if len(units) != 1:
        raise EvaluationHarnessError(
            "RETURN_UNIT_INCONSISTENT: the slice mixes return units %s — "
            "one case is reported in ONE declared unit"
            % sorted(units)
        )
    unit = units.pop()

    violated = sum(1 for c in consequences if c["constraint_violation"])
    total = len(consequences)
    kinds: dict = {}
    for c in consequences:
        if c["constraint_violation"]:
            kinds[c["violation_kind"]] = kinds.get(c["violation_kind"], 0) + 1

    return {
        "schema": EVALUATION_HARNESS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "total_decisions": total,
        "constraint_violation_count": violated,
        "violation_frequency": violated / total,
        "violation_kinds": kinds,
        "mean_return": sum(c["return_value"] for c in consequences) / total,
        "return_unit": unit,
        "per_decision": [
            {
                "sequence": entry["sequence"],
                "constraint_violation": c["constraint_violation"],
                "violation_kind": c["violation_kind"],
                "return_value": c["return_value"],
            }
            for entry, c in zip(entries, consequences)
        ],
    }


def distribution_shift_robustness(
    reference_stats: dict, shifted_stats: dict
) -> dict:
    """The §19.4 robustness-under-distribution-shift deltas (pure).

    Both inputs MUST be `case_statistics` outputs over the SAME metric
    definitions and ONE declared return unit. The output is the deltas
    ONLY — no verdict without a pre-registered threshold (header item 4):

      violation_frequency_delta = shifted - reference   (positive = MORE
                                  violations under shift — worse)
      mean_return_delta         = shifted - reference   (negative = worse)
    """
    for stats in (reference_stats, shifted_stats):
        if (
            not isinstance(stats, dict)
            or stats.get("schema") != EVALUATION_HARNESS_SCHEMA
            or "violation_frequency" not in stats
            or "mean_return" not in stats
        ):
            raise EvaluationHarnessError(
                "STATS_OBJECT_INVALID: distribution_shift_robustness "
                "requires two case_statistics outputs (schema %r)"
                % (EVALUATION_HARNESS_SCHEMA,)
            )
    if reference_stats["return_unit"] != shifted_stats["return_unit"]:
        raise EvaluationHarnessError(
            "RETURN_UNIT_INCONSISTENT: the reference slice (%r) and the "
            "shifted slice (%r) use different return units — the delta "
            "would be meaningless"
            % (reference_stats["return_unit"], shifted_stats["return_unit"])
        )
    return {
        "schema": EVALUATION_HARNESS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "violation_frequency_delta": (
            shifted_stats["violation_frequency"]
            - reference_stats["violation_frequency"]
        ),
        "mean_return_delta": (
            shifted_stats["mean_return"] - reference_stats["mean_return"]
        ),
        "return_unit": reference_stats["return_unit"],
        "reading": (
            "positive violation_frequency_delta = more violations under "
            "shift; negative mean_return_delta = worse return under "
            "shift; no verdict without the pre-registered threshold (X-8)"
        ),
    }


def compare_case_to_baselines(
    *,
    case_id: str,
    case_family: str,
    policy_stats: dict,
    baseline_stats_by_id: dict,
    pre_registered_tolerances: dict,
) -> list:
    """Per-baseline, PER-METRIC comparison rows for one case (pure).

    Rules (each pinned by a self-test or a named refusal):
      - `case_family` must be in the closed CASE_FAMILIES vocabulary
        (CASE_FAMILY_UNKNOWN);
      - every baseline id must be in the frozen P8.06 registry
        (BASELINE_UNKNOWN — the PI baseline has no frozen id, header
        item 6);
      - the pre-registered tolerances MUST be injected — the dict needs
        `violation_frequency_absolute_tolerance` and
        `return_absolute_tolerance`, finite numbers >= 0, bool excluded
        (THRESHOLD_NOT_PRE_REGISTERED — X-8: no verdict without a
        pre-registered threshold; the P8.09 named-refusal discipline);
      - the verdict is PER METRIC, never a merged headline (header
        item 4):
          violation frequency (lower is better):
            win  if policy - baseline <  -tol_vf
            tie  if |policy - baseline| <=  tol_vf
            loss otherwise
          return (higher is better):
            win  if policy - baseline >   tol_ret
            tie  if |policy - baseline| <= tol_ret
            loss otherwise.
    """
    if not isinstance(case_id, str) or not case_id.strip():
        raise EvaluationHarnessError(
            "CASE_FAMILY_UNKNOWN: case_id must be a non-empty string "
            "(a row without a case id is unattributable)"
        )
    if case_family not in CASE_FAMILIES:
        raise EvaluationHarnessError(
            "CASE_FAMILY_UNKNOWN: case_family %r is not in the closed "
            "vocabulary %s" % (case_family, sorted(CASE_FAMILIES))
        )
    if (
        not isinstance(policy_stats, dict)
        or policy_stats.get("schema") != EVALUATION_HARNESS_SCHEMA
        or "violation_frequency" not in policy_stats
        or "mean_return" not in policy_stats
    ):
        raise EvaluationHarnessError(
            "STATS_OBJECT_INVALID: policy_stats must be a "
            "case_statistics output (schema %r)"
            % (EVALUATION_HARNESS_SCHEMA,)
        )
    if not isinstance(baseline_stats_by_id, dict) or not baseline_stats_by_id:
        raise EvaluationHarnessError(
            "BASELINE_STATS_INVALID: baseline_stats_by_id must be a "
            "non-empty dict of {frozen baseline id: case_statistics}"
        )
    for baseline_id in baseline_stats_by_id:
        if baseline_id not in FROZEN_BASELINE_IDS:
            raise EvaluationHarnessError(
                "BASELINE_UNKNOWN: %r is not in the frozen P8.06 "
                "registry %s — a baseline that was not defined, run, and "
                "frozen cannot be compared against (the PI baseline has "
                "no frozen id: recorded Phase 7/P8.06 follow-up)"
                % (baseline_id, sorted(FROZEN_BASELINE_IDS))
            )

    if not isinstance(pre_registered_tolerances, dict):
        raise EvaluationHarnessError(
            "THRESHOLD_NOT_PRE_REGISTERED: pre_registered_tolerances "
            "must be a dict — no verdict exists without it (X-8)"
        )
    tolerances = {}
    for key in (
        "violation_frequency_absolute_tolerance",
        "return_absolute_tolerance",
    ):
        value = pre_registered_tolerances.get(key)
        if (
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not _is_finite_number(value)
            or value < 0
        ):
            raise EvaluationHarnessError(
                "THRESHOLD_NOT_PRE_REGISTERED: %r is missing or invalid "
                "(a finite number >= 0, pre-registered at the P8.06 "
                "freeze BEFORE any research number exists — X-8)" % key
            )
        tolerances[key] = float(value)
    tol_vf = tolerances["violation_frequency_absolute_tolerance"]
    tol_ret = tolerances["return_absolute_tolerance"]

    rows: list = []
    for baseline_id, baseline_stats in sorted(baseline_stats_by_id.items()):
        if (
            not isinstance(baseline_stats, dict)
            or baseline_stats.get("schema") != EVALUATION_HARNESS_SCHEMA
            or "violation_frequency" not in baseline_stats
            or "mean_return" not in baseline_stats
        ):
            raise EvaluationHarnessError(
                "STATS_OBJECT_INVALID: baseline_stats_by_id[%r] must be "
                "a case_statistics output" % (baseline_id,)
            )
        if baseline_stats.get("return_unit") != policy_stats.get("return_unit"):
            raise EvaluationHarnessError(
                "RETURN_UNIT_INCONSISTENT: baseline %r reports return in "
                "%r but the policy reports %r — same cases, same metric "
                "definitions, same units, or no comparison"
                % (
                    baseline_id,
                    baseline_stats.get("return_unit"),
                    policy_stats.get("return_unit"),
                )
            )
        vf_delta = policy_stats["violation_frequency"] - baseline_stats[
            "violation_frequency"
        ]
        return_delta = policy_stats["mean_return"] - baseline_stats["mean_return"]
        rows.append(
            {
                "case_id": case_id,
                "case_family": case_family,
                "baseline_id": baseline_id,
                "violation_frequency_delta": vf_delta,
                "violation_frequency_verdict": _verdict(
                    vf_delta, tol_vf, higher_is_better=False
                ),
                "mean_return_delta": return_delta,
                "return_verdict": _verdict(
                    return_delta, tol_ret, higher_is_better=True
                ),
                "merged_headline": None,
                "merged_note": (
                    "deliberately absent: the two metric verdicts are "
                    "NEVER merged into one headline number (P8.13 §2.6; "
                    "§19.4)"
                ),
            }
        )
    return rows


def _verdict(delta: float, tolerance: float, *, higher_is_better: bool) -> str:
    """The pre-registered-tolerance verdict for ONE metric delta."""
    if higher_is_better:
        if delta > tolerance:
            return "win"
        if delta >= -tolerance:
            return "tie"
        return "loss"
    if delta < -tolerance:
        return "win"
    if delta <= tolerance:
        return "tie"
    return "loss"


def _is_finite_number(value: object) -> bool:
    """True for a finite int/float (bools excluded by the callers)."""
    try:
        return float(value) == float(value) and float(value) not in (
            float("inf"),
            float("-inf"),
        )
    except (TypeError, ValueError):  # pragma: no cover — callers type-check first
        return False


# ---------------------------------------------------------------------------
# Hand-computed self-tests (NEVER RUN in this loop; every expectation is a
# hand derivation from constructed inputs, with the arithmetic STATED).
# Run instruction (human gate):
#   python apps/worker/python/marl_worker/evaluation_harness.py
# ---------------------------------------------------------------------------


def _log_entry(sequence: int, outcome: str, **overrides) -> dict:
    """Builds ONE canonical log entry through the REAL P8.14 builder
    (never a hand-rolled dict that could drift from the schema)."""
    base = dict(
        sequence=sequence,
        request_id="req-%04d" % sequence,
        input_snapshot_id="snapshot-RECORD",
        worker_mode="filter_probe",
        policy_version="RECORD-AT-FIRST-RUN",
        training_seed=0,
        environment_version="flexcore-marl-env/0.1.0",
        action_kind="setpoint_delta",
        proposed_permission="schedule.propose",
        safety_filter_outcome=outcome,
        per_entry_outcomes=[
            {"asset_id": "ess-1", "entry_outcome": "within_limits"}
        ],
        filter_activations=[],
        infeasibility=None,
    )
    base.update(overrides)
    return shadow_log.build_log_entry(**base)


def _with_consequence(entry: dict, *, violated: bool, kind, ret: float) -> dict:
    """Returns a copy of the entry whose consequence is RECORDED through
    the real builder (recorded_by_harness REQUIRES a consequence value)."""
    consequence = build_case_consequence(
        constraint_violation=violated,
        violation_kind=kind,
        return_value=ret,
        return_unit="declared-cost-unit",
    )
    rebuilt = shadow_log.build_log_entry(
        sequence=entry["sequence"],
        request_id=entry["request_id"],
        input_snapshot_id=entry["input_snapshot_id"],
        worker_mode=entry["worker_mode"],
        policy_version=entry["policy_version"],
        training_seed=entry["training_seed"],
        environment_version=entry["environment_version"],
        action_kind=entry["action_kind"],
        proposed_permission=entry["proposed_permission"],
        safety_filter_outcome=entry["safety_filter_outcome"],
        per_entry_outcomes=entry["per_entry_outcomes"],
        filter_activations=entry["filter_activations"],
        infeasibility=entry["infeasibility"],
        consequence_status="recorded_by_harness",
        simulated_consequence=consequence,
    )
    return rebuilt


def run_self_tests() -> list:
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: filter statistics — hand-computed rates ----------------
    # HAND CHECK: 5 entries with outcomes passed / clamped / refused-
    # permission / refused-nonfinite / infeasible. Activation count 1
    # (only entry 2 carries a clamp) -> activation_rate = 1/5 = 0.2.
    # Infeasibility count 1 -> infeasibility_rate = 1/5 = 0.2. Refusals
    # 3 (permission + nonfinite + infeasible) -> refusal_rate = 3/5 = 0.6.
    log = [
        _log_entry(1, "passed_within_limits"),
        _log_entry(
            2,
            "filtered_clamped_to_limits",
            filter_activations=[
                {"asset_id": "ess-1", "p_before_mw": 4.0, "p_filtered_mw": 3.5}
            ],
        ),
        _log_entry(3, "refused_permission_ceiling"),
        _log_entry(4, "refused_nonfinite_action"),
        _log_entry(
            5,
            "infeasible_no_safe_projection",
            infeasibility={
                "outcome": "infeasible_no_safe_projection",
                "entries": ["ess-1"],
                "recorded": True,
            },
        ),
    ]
    stats = filter_statistics(log)
    check(
        "filter_statistics_hand_computed_rates",
        stats["total_actions"] == 5
        and stats["counts"]["filter_activation_count"] == 1
        and stats["counts"]["infeasibility_count"] == 1
        and stats["counts"]["refusal_count"] == 3
        and stats["activation_rate"] == 0.2
        and stats["infeasibility_rate"] == 0.2
        and stats["refusal_rate"] == 0.6
        and stats["counts"]["refusal_outcomes"]
        == {
            "infeasible_no_safe_projection": 1,
            "refused_nonfinite_action": 1,
            "refused_permission_ceiling": 1,
        },
        "1/5 = 0.2 activation, 1/5 = 0.2 infeasibility, 3/5 = 0.6 refusal "
        "(stated fractions over the validated total)",
    )
    check(
        "outcome_of_every_infeasibility_listed",
        len(stats["outcome_of_every_infeasibility"]) == 1
        and stats["outcome_of_every_infeasibility"][0]["sequence"] == 5
        and stats["outcome_of_every_infeasibility"][0]["outcome"]
        == "infeasible_no_safe_projection"
        and stats["outcome_of_every_infeasibility"][0]["assets"] == ["ess-1"]
        and stats["outcome_of_every_infeasibility"][0]["fallback_attempted"]
        == NOT_RECORDED_IN_LOG_SCHEMA
        and stats["outcome_of_every_infeasibility"][0]["fallback_viable"]
        == NOT_RECORDED_IN_LOG_SCHEMA,
        "the ONE infeasibility is listed with its outcome; the fallback "
        "facts are honestly NOT_RECORDED_IN_LOG_SCHEMA (the P8.14 log "
        "schema does not carry them — recorded contract gap, never a "
        "fabricated default)",
    )
    check(
        "reporting_only_statement_present",
        stats["usage"] == REPORTING_ONLY_STATEMENT
        and stats["schema"] == EVALUATION_HARNESS_SCHEMA,
        "every statistics object carries the schema tag and the fixed "
        "reporting-only statement",
    )

    # ---- Test 2: fallback facts ARE carried when the log carries them --
    log2 = [
        _log_entry(
            1,
            "infeasible_no_safe_projection",
            infeasibility={
                "outcome": "infeasible_no_safe_projection",
                "entries": ["ess-2"],
                "recorded": True,
                "fallback_attempted": True,
                "fallback_viable": False,
            },
        )
    ]
    stats2 = filter_statistics(log2)
    row = stats2["outcome_of_every_infeasibility"][0]
    check(
        "fallback_facts_carried_when_recorded",
        row["fallback_attempted"] is True
        and row["fallback_viable"] is False
        and stats2["infeasibility_rate"] == 1.0,
        "1/1 = 1.0 infeasibility rate; the recorded fallback facts pass "
        "through verbatim (attempted=True, viable=False)",
    )

    # ---- Test 3: an infeasibility outcome outside the vocabulary -------
    def bad_infeasibility_outcome() -> None:
        filter_statistics(
            [
                _log_entry(
                    1,
                    "infeasible_no_safe_projection",
                    infeasibility={"outcome": "infeasible", "entries": []},
                )
            ]
        )

    _expects_error(
        results, bad_infeasibility_outcome, "infeasibility_outcome_vocabulary"
    )

    # ---- Test 4: the empty log is refused (no fabricated zero rate) ----
    def empty_log() -> None:
        filter_statistics([])

    _expects_error(results, empty_log, "EVAL_LOG_EMPTY")

    # ---- Test 5: a not_simulated slice cannot become a frequency -------
    def not_simulated_slice() -> None:
        case_statistics([_log_entry(1, "passed_within_limits")])

    _expects_error(
        results,
        not_simulated_slice,
        "VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED",
    )

    # ---- Test 6: case statistics — hand-computed frequency and return --
    # HAND CHECK: 4 decisions, violations [True, False, False, True] ->
    # frequency = 2/4 = 0.5. Returns 10.0 + 12.0 + 8.0 + 6.0 = 36.0 ->
    # mean = 36.0/4 = 9.0. Kinds: {"soc_bound": 1, "power_limit": 1}.
    case_log = [
        _with_consequence(
            _log_entry(1, "passed_within_limits"),
            violated=True,
            kind="soc_bound",
            ret=10.0,
        ),
        _with_consequence(
            _log_entry(2, "filtered_clamped_to_limits"),
            violated=False,
            kind=None,
            ret=12.0,
        ),
        _with_consequence(
            _log_entry(3, "passed_within_limits"),
            violated=False,
            kind=None,
            ret=8.0,
        ),
        _with_consequence(
            _log_entry(4, "passed_within_limits"),
            violated=True,
            kind="power_limit",
            ret=6.0,
        ),
    ]
    case_stats = case_statistics(case_log)
    check(
        "case_statistics_hand_computed",
        case_stats["total_decisions"] == 4
        and case_stats["constraint_violation_count"] == 2
        and case_stats["violation_frequency"] == 0.5
        and case_stats["violation_kinds"] == {"soc_bound": 1, "power_limit": 1}
        and case_stats["mean_return"] == 9.0
        and case_stats["return_unit"] == "declared-cost-unit",
        "frequency 2/4 = 0.5; mean return 36.0/4 = 9.0; kinds counted "
        "alongside (never merged into the frequency)",
    )

    # ---- Test 7: a violation without a kind is unbuildable -------------
    def unnamed_violation() -> None:
        build_case_consequence(
            constraint_violation=True,
            violation_kind=None,
            return_value=1.0,
            return_unit="declared-cost-unit",
        )

    _expects_error(results, unnamed_violation, "unnamed_violation_refused")

    def kind_without_violation() -> None:
        build_case_consequence(
            constraint_violation=False,
            violation_kind="soc_bound",
            return_value=1.0,
            return_unit="declared-cost-unit",
        )

    _expects_error(
        results, kind_without_violation, "kind_without_violation_refused"
    )

    def nonfinite_return() -> None:
        build_case_consequence(
            constraint_violation=False,
            violation_kind=None,
            return_value=float("inf"),
            return_unit="declared-cost-unit",
        )

    _expects_error(results, nonfinite_return, "nonfinite_return_refused")

    # ---- Test 8: distribution-shift deltas — hand-computed -------------
    # HAND CHECK: reference slice = decisions 1..4 above (frequency 0.5,
    # mean return 9.0). Shifted slice: 4 decisions, violations
    # [True, True, False, False] -> frequency 2/4 = 0.5; returns
    # [6.0, 6.0, 10.0, 10.0] -> sum 32.0 -> mean 8.0. Deltas:
    # frequency 0.5 - 0.5 = 0.0; return 8.0 - 9.0 = -1.0 (worse).
    shifted_log = [
        _with_consequence(
            _log_entry(1, "passed_within_limits"),
            violated=True,
            kind="soc_bound",
            ret=6.0,
        ),
        _with_consequence(
            _log_entry(2, "passed_within_limits"),
            violated=True,
            kind="soc_bound",
            ret=6.0,
        ),
        _with_consequence(
            _log_entry(3, "passed_within_limits"),
            violated=False,
            kind=None,
            ret=10.0,
        ),
        _with_consequence(
            _log_entry(4, "passed_within_limits"),
            violated=False,
            kind=None,
            ret=10.0,
        ),
    ]
    shifted_stats = case_statistics(shifted_log)
    robustness = distribution_shift_robustness(case_stats, shifted_stats)
    check(
        "distribution_shift_deltas_hand_computed",
        robustness["violation_frequency_delta"] == 0.0
        and robustness["mean_return_delta"] == -1.0,
        "frequency delta 0.5 - 0.5 = 0.0; return delta 8.0 - 9.0 = -1.0 "
        "(worse under shift); deltas only — no verdict without the "
        "pre-registered threshold",
    )

    # ---- Test 9: per-metric comparison — hand-computed verdicts --------
    # HAND CHECK (tolerances pre-registered 0.05 frequency / 0.5 return):
    # policy (frequency 0.0, return 9.0) vs
    #   baseline/highs-tier1-lp (frequency 0.25, return 8.0):
    #     frequency delta 0.0 - 0.25 = -0.25 < -0.05 -> win;
    #     return delta 9.0 - 8.0 = 1.0 > 0.5 -> win.
    #   baseline/rule-based-priority-list (frequency 0.0, return 8.5):
    #     frequency delta 0.0 -> |0.0| <= 0.05 -> tie;
    #     return delta 9.0 - 8.5 = 0.5 -> |0.5| <= 0.5 -> tie (the
    #     boundary IS the tie, exactly as the rule states).
    highs_stats = case_statistics(
        [
            _with_consequence(
                _log_entry(1, "passed_within_limits"),
                violated=True,
                kind="power_limit",
                ret=8.0,
            ),
            _with_consequence(
                _log_entry(2, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.0,
            ),
            _with_consequence(
                _log_entry(3, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.0,
            ),
            _with_consequence(
                _log_entry(4, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.0,
            ),
        ]
    )
    rule_stats = case_statistics(
        [
            _with_consequence(
                _log_entry(1, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.5,
            ),
            _with_consequence(
                _log_entry(2, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.5,
            ),
            _with_consequence(
                _log_entry(3, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.5,
            ),
            _with_consequence(
                _log_entry(4, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.5,
            ),
        ]
    )
    # HAND CHECK for the constructed baseline slices: highs has
    # violations [True, F, F, F] -> frequency 1/4 = 0.25; returns all
    # 8.0 -> mean 8.0. Rule-based has no violation -> frequency 0.0;
    # returns all 8.5 -> mean 8.5.
    check(
        "constructed_baseline_slices_hand_computed",
        highs_stats["violation_frequency"] == 0.25
        and highs_stats["mean_return"] == 8.0
        and rule_stats["violation_frequency"] == 0.0
        and rule_stats["mean_return"] == 8.5,
        "highs slice: 1/4 = 0.25 frequency, mean 8.0; rule slice: 0/4 = "
        "0.0 frequency, mean 8.5",
    )
    zero_violation_slice = case_statistics(
        [
            _with_consequence(
                _log_entry(i, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=ret,
            )
            for i, ret in ((1, 10.0), (2, 12.0), (3, 8.0), (4, 6.0))
        ]
    )
    rows = compare_case_to_baselines(
        case_id="RECORD-AT-FIRST-RUN",
        case_family="held_out_regime",
        policy_stats=zero_violation_slice,
        baseline_stats_by_id={
            "baseline/highs-tier1-lp": highs_stats,
            "baseline/rule-based-priority-list": rule_stats,
        },
        pre_registered_tolerances={
            "violation_frequency_absolute_tolerance": 0.05,
            "return_absolute_tolerance": 0.5,
        },
    )
    check(
        "per_metric_verdicts_hand_computed",
        len(rows) == 2
        and rows[0]["baseline_id"] == "baseline/highs-tier1-lp"
        and rows[0]["violation_frequency_delta"] == -0.25
        and rows[0]["violation_frequency_verdict"] == "win"
        and rows[0]["mean_return_delta"] == 1.0
        and rows[0]["return_verdict"] == "win"
        and rows[1]["baseline_id"] == "baseline/rule-based-priority-list"
        and rows[1]["violation_frequency_delta"] == 0.0
        and rows[1]["violation_frequency_verdict"] == "tie"
        and rows[1]["mean_return_delta"] == 0.5
        and rows[1]["return_verdict"] == "tie"
        and all(row["merged_headline"] is None for row in rows),
        "wins and ties per metric, hand-computed; NO merged headline "
        "exists (the field is deliberately None)",
    )

    # ---- Test 10: the named refusals ------------------------------------
    def unknown_baseline() -> None:
        compare_case_to_baselines(
            case_id="c",
            case_family="held_out_regime",
            policy_stats=zero_violation_slice,
            baseline_stats_by_id={"baseline/pi-undeclared": highs_stats},
            pre_registered_tolerances={
                "violation_frequency_absolute_tolerance": 0.05,
                "return_absolute_tolerance": 0.5,
            },
        )

    _expects_error(results, unknown_baseline, "BASELINE_UNKNOWN")

    def missing_tolerances() -> None:
        compare_case_to_baselines(
            case_id="c",
            case_family="held_out_regime",
            policy_stats=zero_violation_slice,
            baseline_stats_by_id={"baseline/highs-tier1-lp": highs_stats},
            pre_registered_tolerances={},
        )

    _expects_error(
        results, missing_tolerances, "THRESHOLD_NOT_PRE_REGISTERED"
    )

    def unknown_family() -> None:
        compare_case_to_baselines(
            case_id="c",
            case_family="training_regime",
            policy_stats=zero_violation_slice,
            baseline_stats_by_id={"baseline/highs-tier1-lp": highs_stats},
            pre_registered_tolerances={
                "violation_frequency_absolute_tolerance": 0.05,
                "return_absolute_tolerance": 0.5,
            },
        )

    _expects_error(results, unknown_family, "CASE_FAMILY_UNKNOWN")

    def mixed_units() -> None:
        compare_case_to_baselines(
            case_id="c",
            case_family="device_outage",
            policy_stats=zero_violation_slice,
            baseline_stats_by_id={
                "baseline/highs-tier1-lp": dict(
                    highs_stats, return_unit="other-unit"
                )
            },
            pre_registered_tolerances={
                "violation_frequency_absolute_tolerance": 0.05,
                "return_absolute_tolerance": 0.5,
            },
        )

    _expects_error(results, mixed_units, "RETURN_UNIT_INCONSISTENT")

    # ---- Test 11: no dispatch-shaped callable exists in this module ----
    forbidden_tokens = (
        "dispatch",
        "issue",
        "authorize",
        "approve",
        "command",
        "rank_dispatch",
    )
    offenders = [
        name
        for name, obj in sorted(globals().items())
        if callable(obj)
        and not name.startswith("_")
        and any(token in name.lower() for token in forbidden_tokens)
    ]
    check(
        "no_dispatch_shaped_callable",
        offenders == [],
        "the module namespace contains no dispatch/issue/authorize/"
        "approve/command-shaped callable — the reporting-only boundary "
        "holds by construction (ADR-010)",
    )

    return results


def _expects_error(results: list, fn, name: str) -> None:
    try:
        fn()
    except EvaluationHarnessError:
        results.append(
            {
                "name": name,
                "passed": True,
                "detail": "raised EvaluationHarnessError as expected",
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of EvaluationHarnessError" % error,
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
