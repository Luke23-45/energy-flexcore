# P8.13/P8.14 — The MARL safety filter (pure, stdlib-only): the closed
#               permission-ceiling set, the physical-limit projection, and
#               the named infeasible-filter outcome.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS FILTER IS — AND WHAT IT IS NOT. This is the STATIC
#    limit/permission filter defined by the P8.13 design (§3/§4): a pure
#    projection over (a) the CLOSED permission-ceiling set and (b) the
#    declared per-asset physical limits (P bounds in MW, ramp limits in
#    MW per decision interval). It is a SUBSET of the P8.13 safety layer,
#    NOT the control-barrier-function layer: the CBF derivation (model,
#    relative degree) is RECORDED AS NOT YET DERIVED in
#    `ledger/phase-08-advanced-coordination/13_marl_design.md` §3, and
#    TRAINING IS BLOCKED until a power-systems reviewer signs that
#    derivation. This filter does NOT cover voltage, branch-current, or
#    SOC-dynamic constraints — those are exactly the constraints the
#    design warns are NOT simple dynamic state constraints (voltage and
#    branch current are functions of the algebraic power-flow solution;
#    their relative degree w.r.t. inverter setpoint inputs is generally
#    NOT 1, and a naive clamp on them would be unsound). No safety
#    guarantee of any kind is made or implied by this file.
# 2. THE PERMISSION CEILING IS LOAD-BEARING AND CLOSED. `PERMISSION_CEILING`
#    mirrors `plugins/profiles/sim-lab.profile.json` ("permission-ceiling":
#    telemetry.read, asset.read, schedule.propose) — the profile JSON is
#    the DEFINITION OF RECORD and the two MUST move together (a test in
#    tests/test_safety_filter.py pins the equality by reading the JSON).
#    An action whose `proposed_permission` is outside the ceiling is
#    REFUSED with the named outcome `refused_permission_ceiling` — never
#    projected, never passed through.
# 3. THE OUTCOME VOCABULARY IS LOAD-BEARING AND CLOSED. `FILTER_OUTCOMES`
#    is the closed value set of the P8.04 result key
#    `safety_filter_outcome` (the MARL payload
#    `flexcore.research.marl_shadow.v1`); every filter result carries one
#    of exactly these five values, and nothing outside the set can be
#    produced. A policy action that would violate a physical limit or a
#    platform permission ceiling therefore ALWAYS produces a named
#    outcome — the "a shadow log that records actions but not filter
#    outcomes" failure mode named in ledger P8.14 is structurally
#    impossible through this module.
# 4. THE INFEASIBLE-FILTER OUTCOME IS DEFINED AND PROVABLE. When the
#    feasible interval
#        [max(p_min, p_current - ramp), min(p_max, p_current + ramp)]
#    is EMPTY, no safe setpoint exists for the entry — the filter is
#    INFEASIBLE, and the P8.13 §4 fallback (hold-current-setpoint,
#    re-filtered) is attempted and RECORDED. The fallback is PROVABLY
#    infeasible in exactly that case: the hold-current action
#    (p_proposed = p_current) passes the interval check iff
#    p_current ∈ [p_min, p_max], and p_current ∈ [p_min, p_max] makes the
#    primary interval NON-empty (it contains p_current itself, since
#    ramp >= 0). So: primary interval empty => hold-current outside
#    [p_min, p_max] => fallback infeasible. The recorded failure is the
#    ONLY output in that case — never an unfiltered action passed through
#    silently (ledger P8.13 step 3; P8.14 acceptance "filter infeasibility
#    is logged, never silent"). The derivation above is stated here and
#    pinned by hand-computed self-tests (§ run_self_tests), not just
#    asserted.
# 5. STDLIB-ONLY, WRITTEN-NEVER-RUN. No framework, no numpy, no torch, no
#    ray/rllib — the module imports only `math`. It was NEVER executed in
#    this loop (Phase 8 bans); the hand-computed expectations in
#    `run_self_tests` are derivations from the constructed inputs, never
#    observed outputs, and the pytest mirror in tests/test_safety_filter.py
#    was written and never run either.
# 6. UNITS (§10.2 canonical): power entries are MW; P > 0 means EXPORT at
#    the point of connection (the sign is a caller-side declaration carried
#    by the field names, enforced nowhere here — this filter projects
#    magnitudes against declared limits, never re-interprets signs). SOC,
#    where declared, is a FRACTION in [0, 1] at the API boundary and is
#    validated as an INPUT only (never clamped — see item 1).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

# The schema tag of the filter report (validated by the caller; fail-closed
# on mismatch like every other protocol tag in this worker family).
FILTER_REPORT_SCHEMA = "flexcore-marl-safety-filter/1"

# The CLOSED permission-ceiling set, mirrored verbatim from
# `plugins/profiles/sim-lab.profile.json` -> "permission-ceiling". The
# profile file is the definition of record; this constant and that file
# MUST move together (pinned by tests/test_safety_filter.py, which reads
# the JSON). NOTE: the marl research profile
# (`plugins/profiles/research-marl.profile.json`) withholds even
# `schedule.propose` (a DEFAULT decision recorded there) — this constant
# is the sim-lab composition ceiling the P8.13 design froze for the
# filter's arithmetic; a deployment whose profile withholds a permission
# refuses at the platform permission layer BEFORE this filter is reached.
PERMISSION_CEILING: frozenset = frozenset(
    {"telemetry.read", "asset.read", "schedule.propose"}
)

# The ONLY ceiling permission a proposed action may consume. telemetry.read
# and asset.read are observation-side permissions (the request's input data
# provenance), never action-side: an action asking for anything but
# schedule.propose is structurally refused.
ACTION_PROPOSAL_PERMISSION = "schedule.propose"

# The closed action kinds. `hold_current_setpoint` is the P8.13 §4 fallback
# kind; it is also a legal primary action kind (an agent may propose to
# hold). A discrete on/off action kind is deliberately ABSENT: a
# connect/disconnect is a device-command shape, and the worker's action
# surface carries setpoint PROPOSALS only (P8.14 boundary; ADR-010).
ACTION_KINDS: frozenset = frozenset(
    {"setpoint_delta", "hold_current_setpoint"}
)

FALLBACK_ACTION_KIND = "hold_current_setpoint"

# Per-entry outcomes (the filter report's per-asset vocabulary; closed).
ENTRY_OUTCOMES: frozenset = frozenset(
    {
        "within_limits",
        "clamped_to_limit",
        "refused_nonfinite_action",
        "infeasible_no_safe_projection",
    }
)

# The closed AGGREGATE outcome vocabulary — the closed value set of the
# P8.04 result key `safety_filter_outcome` (payload
# `flexcore.research.marl_shadow.v1`). Five values, exactly; the worker's
# envelope and the P8.15 filter statistics both consume this vocabulary.
FILTER_OUTCOMES: frozenset = frozenset(
    {
        "passed_within_limits",
        "filtered_clamped_to_limits",
        "refused_permission_ceiling",
        "refused_nonfinite_action",
        "infeasible_no_safe_projection",
    }
)

# The outcomes under which NO proposal is emitted (the action is refused;
# the envelope is a recorded failure, exit 0 — a refusal is a result, not
# a crash).
REFUSING_OUTCOMES: frozenset = frozenset(
    {
        "refused_permission_ceiling",
        "refused_nonfinite_action",
        "infeasible_no_safe_projection",
    }
)


class FilterInputError(ValueError):
    """A malformed filter input (limits, SOC, action structure) — refused
    BEFORE any projection is attempted. A structural refusal is never a
    filter outcome: outcomes classify ACTIONS, errors classify MALFORMED
    INPUTS, and the two are never conflated."""


# ---------------------------------------------------------------------------
# Pure input validation (no I/O, no framework, no platform state)
# ---------------------------------------------------------------------------


def validate_limits(limits: object, where: str) -> dict:
    """Validates one asset's declared physical limits; returns normalized.

    `limits` = {"p_min_mw": finite, "p_max_mw": finite,
                "ramp_mw_per_interval": finite >= 0}.
    p_min > p_max is a malformed limit (refused — an inverted interval is
    a declaration error, never an infeasible filter outcome, which is a
    property of a VALID declaration). Ramp < 0 is refused (a negative ramp
    would widen the interval — nonsense). Units: MW.
    """
    if not isinstance(limits, dict):
        raise FilterInputError("%s.limits must be an object" % where)
    values = {}
    for field in ("p_min_mw", "p_max_mw", "ramp_mw_per_interval"):
        value = limits.get(field)
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise FilterInputError(
                "%s.limits.%s must be a finite number" % (where, field)
            )
        if not math.isfinite(float(value)):
            raise FilterInputError(
                "%s.limits.%s must be finite" % (where, field)
            )
        values[field] = float(value)
    if values["p_min_mw"] > values["p_max_mw"]:
        raise FilterInputError(
            "%s.limits: p_min_mw (%r) > p_max_mw (%r) is an inverted "
            "interval — a malformed declaration, refused" %
            (where, values["p_min_mw"], values["p_max_mw"])
        )
    if values["ramp_mw_per_interval"] < 0.0:
        raise FilterInputError(
            "%s.limits.ramp_mw_per_interval must be >= 0" % where
        )
    return values


def validate_soc_fraction(value: object, where: str) -> float | None:
    """Validates an optional SOC declaration (a FRACTION in [0, 1]).

    Absent (None) returns None. Present: finite float in [0, 1]; outside
    is a MALFORMED INPUT (the state declaration is invalid), refused — an
    out-of-range SOC state is never silently accepted as filter input and
    never clamped (clamping a state would falsify the observation record).
    """
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise FilterInputError("%s.soc_fraction must be a number or null" % where)
    value = float(value)
    if not math.isfinite(value):
        raise FilterInputError("%s.soc_fraction must be finite" % where)
    if not (0.0 <= value <= 1.0):
        raise FilterInputError(
            "%s.soc_fraction %r is outside [0, 1] (SOC is a fraction at "
            "the API boundary) — malformed input, refused" % (where, value)
        )
    return value


def validate_action(action: object) -> dict:
    """Structural validation of a proposed action (pure; no projection).

    `action` = {"action_kind": in ACTION_KINDS,
                "proposed_permission": == ACTION_PROPOSAL_PERMISSION,
                "assets": [ {"asset_id": non-empty str,
                             "p_current_mw": number,
                             "p_proposed_mw": number,
                             "soc_fraction": optional number in [0,1],
                             "limits": {...}}, ... ]}.

    Numbers are checked for TYPE here and for FINITENESS inside the
    projection (a non-finite PROPOSED value is a filter outcome —
    `refused_nonfinite_action` — not an input error, because the action is
    well-formed and the policy simply emitted a non-finite setpoint). A
    non-finite p_current_mw, by contrast, is a malformed INPUT (the current
    state cannot be non-finite) and refuses the whole action before any
    projection. The asset list must be non-empty (a no-asset action is
    meaningless, not safe).
    """
    if not isinstance(action, dict):
        raise FilterInputError("action must be an object")
    kind = action.get("action_kind")
    if kind not in ACTION_KINDS:
        raise FilterInputError(
            "action.action_kind %r is not one of %s" % (kind, sorted(ACTION_KINDS))
        )
    permission = action.get("proposed_permission")
    if not isinstance(permission, str) or not permission.strip():
        raise FilterInputError("action.proposed_permission must be a non-empty string")
    assets = action.get("assets")
    if not isinstance(assets, list) or not assets:
        raise FilterInputError(
            "action.assets must be a non-empty list (a no-asset action is "
            "meaningless, never 'safely empty')"
        )
    normalized = []
    for index, asset in enumerate(assets):
        where = "action.assets[%d]" % index
        if not isinstance(asset, dict):
            raise FilterInputError("%s must be an object" % where)
        asset_id = asset.get("asset_id")
        if not isinstance(asset_id, str) or not asset_id.strip():
            raise FilterInputError("%s.asset_id must be a non-empty string" % where)
        p_current = asset.get("p_current_mw")
        if isinstance(p_current, bool) or not isinstance(p_current, (int, float)):
            raise FilterInputError("%s.p_current_mw must be a number" % where)
        if not math.isfinite(float(p_current)):
            raise FilterInputError(
                "%s.p_current_mw must be finite (the CURRENT state cannot "
                "be non-finite; a non-finite PROPOSED value is a filter "
                "outcome, not an input error)" % where
            )
        p_proposed = asset.get("p_proposed_mw")
        if isinstance(p_proposed, bool) or not isinstance(p_proposed, (int, float)):
            raise FilterInputError("%s.p_proposed_mw must be a number" % where)
        soc = validate_soc_fraction(asset.get("soc_fraction"), where)
        limits = validate_limits(asset.get("limits"), where)
        normalized.append(
            {
                "asset_id": asset_id,
                "p_current_mw": float(p_current),
                "p_proposed_mw": float(p_proposed),
                "soc_fraction": soc,
                "limits": limits,
            }
        )
    return {
        "action_kind": kind,
        "proposed_permission": permission,
        "assets": normalized,
    }


# ---------------------------------------------------------------------------
# The projection (pure; every outcome named; hand-computed in self-tests)
# ---------------------------------------------------------------------------


def feasible_interval(p_current_mw: float, limits: dict) -> tuple:
    """The ramp-and-limit feasible interval (pure; the filter's model).

    [max(p_min, p_current - ramp), min(p_max, p_current + ramp)] — the set
    of setpoints reachable in ONE decision interval from the current
    setpoint without violating the declared P bounds. THE MODEL THIS
    FILTER PROJECTS AGAINST is exactly this declared-limits model
    (P8.13 §3: the filter's MODEL and its SCOPE are stated, not assumed):
    a per-asset linear ramp model with hard P bounds. It is NOT the CBF
    layer, NOT a network model, and covers NO voltage/branch-current
    constraint (see header item 1).
    """
    p_min = limits["p_min_mw"]
    p_max = limits["p_max_mw"]
    ramp = limits["ramp_mw_per_interval"]
    return (max(p_min, p_current_mw - ramp), min(p_max, p_current_mw + ramp))


def filter_entry(asset: dict) -> dict:
    """Projects ONE asset's proposed setpoint against its limits (pure).

    Outcome rules (closed ENTRY_OUTCOMES; the arithmetic of each rule is
    hand-computed in `run_self_tests`):
      - non-finite p_proposed  -> `refused_nonfinite_action`,
                                  p_filtered_mw = None (nothing is
                                  projected from a NaN — there is no
                                  'nearest safe value' to a NaN);
      - feasible interval EMPTY -> `infeasible_no_safe_projection`,
                                   p_filtered_mw = None (no safe setpoint
                                   exists; the fallback is handled at the
                                   action level, where its provable failure
                                   is recorded — header item 4);
      - p_proposed INSIDE the interval -> `within_limits`,
                                  p_filtered_mw = p_proposed;
      - otherwise -> `clamped_to_limit`,
                                  p_filtered_mw = the interval endpoint the
                                  proposal is pulled to (the projection).
    """
    asset_id = asset["asset_id"]
    p_current = asset["p_current_mw"]
    p_proposed = asset["p_proposed_mw"]
    limits = asset["limits"]
    report = {
        "asset_id": asset_id,
        "p_current_mw": p_current,
        "p_proposed_mw": p_proposed,
        "p_filtered_mw": None,
        "allowed_min_mw": None,
        "allowed_max_mw": None,
        "entry_outcome": None,
    }
    if not math.isfinite(p_proposed):
        report["entry_outcome"] = "refused_nonfinite_action"
        report["diagnostic"] = (
            "proposed setpoint is not finite (%s); no projection exists "
            "from a non-finite value — refused, never coerced" %
            ("NaN" if math.isnan(p_proposed) else "infinity")
        )
        return report
    lo, hi = feasible_interval(p_current, limits)
    report["allowed_min_mw"] = lo
    report["allowed_max_mw"] = hi
    if lo > hi:
        report["entry_outcome"] = "infeasible_no_safe_projection"
        report["diagnostic"] = (
            "the feasible interval [max(p_min, p_current - ramp), "
            "min(p_max, p_current + ramp)] = [%r, %r] is EMPTY: no "
            "setpoint is reachable within one interval without violating "
            "the declared limits (the current setpoint %r is outside "
            "[p_min, p_max] and the ramp is too small to reach the "
            "bounds); the hold-current fallback is attempted at the "
            "action level and is provably infeasible (13_marl_design.md "
            "section 4)" % (lo, hi, p_current)
        )
        return report
    filtered = min(max(p_proposed, lo), hi)
    report["p_filtered_mw"] = filtered
    if filtered == p_proposed:
        report["entry_outcome"] = "within_limits"
    else:
        report["entry_outcome"] = "clamped_to_limit"
        report["diagnostic"] = (
            "proposed %r MW projected to %r MW (feasible interval "
            "[%r, %r] MW from p_current %r MW)" %
            (p_proposed, filtered, lo, hi, p_current)
        )
    return report


def _fallback_entries(validated_assets: list) -> list:
    """Builds the P8.13 §4 fallback action's entries: hold-current for
    every asset (p_proposed = p_current, kind hold_current_setpoint)."""
    fallback = []
    for asset in validated_assets:
        entry = dict(asset)
        entry["p_proposed_mw"] = asset["p_current_mw"]
        fallback.append(entry)
    return fallback


def filter_action(action: object) -> dict:
    """The filter over ONE proposed action (pure; the P8.13 §3/§4 logic).

    Gate order:
      1. structural validation (FilterInputError on malformed input —
         refused, never classified as an outcome);
      2. the PERMISSION-CEILING gate: `proposed_permission` must be exactly
         `schedule.propose` (the only ceiling permission an action may
         consume). Anything else — a dispatch/control-shaped permission,
         an unknown permission — is the named aggregate outcome
         `refused_permission_ceiling`, with NO projection attempted (a
         permission ceiling is not projectable: no clamping turns a
         forbidden action kind into an allowed one);
      3. per-entry projection (`filter_entry`); per-entry outcomes and
         activations recorded;
      4. the aggregate outcome (closed FILTER_OUTCOMES), the fallback
         attempt + its PROVABLE failure recorded for the infeasible case,
         and the full report returned.

    The report schema is `flexcore-marl-safety-filter/1`; the aggregate
    `action_outcome` is the closed value that travels as the P8.04 result
    key `safety_filter_outcome`.
    """
    validated = validate_action(action)
    report = {
        "report_schema": FILTER_REPORT_SCHEMA,
        "action_kind": validated["action_kind"],
        "proposed_permission": validated["proposed_permission"],
        "action_outcome": None,  # the closed aggregate (FILTER_OUTCOMES)
        "entries": [],
        "filter_activations": [],
        "infeasibility": None,
        "fallback": None,
        "permission_ceiling": sorted(PERMISSION_CEILING),
    }

    # Gate 2: the permission ceiling (non-projectable — refuse whole).
    if validated["proposed_permission"] != ACTION_PROPOSAL_PERMISSION:
        report["action_outcome"] = "refused_permission_ceiling"
        report["diagnostic"] = (
            "proposed_permission %r is outside the closed permission "
            "ceiling %s (only %r may be consumed by an action; the "
            "ceiling mirrors plugins/profiles/sim-lab.profile.json). "
            "Refused with no projection attempted: no clamping turns a "
            "forbidden permission into an allowed one"
            % (
                validated["proposed_permission"],
                sorted(PERMISSION_CEILING),
                ACTION_PROPOSAL_PERMISSION,
            )
        )
        return report

    # Gate 3: per-entry projection.
    entries = [filter_entry(asset) for asset in validated["assets"]]
    report["entries"] = entries
    report["filter_activations"] = [
        {
            "asset_id": entry["asset_id"],
            "p_proposed_mw": entry["p_proposed_mw"],
            "p_filtered_mw": entry["p_filtered_mw"],
            "allowed_min_mw": entry["allowed_min_mw"],
            "allowed_max_mw": entry["allowed_max_mw"],
        }
        for entry in entries
        if entry["entry_outcome"] == "clamped_to_limit"
    ]

    entry_outcomes = [entry["entry_outcome"] for entry in entries]
    if "infeasible_no_safe_projection" in entry_outcomes:
        # Gate 4a: the filter is INFEASIBLE for at least one entry. The
        # P8.13 section-4 fallback (hold-current, re-filtered) is
        # attempted and RECORDED — and is PROVABLY infeasible here
        # (header item 4): the hold-current entry re-filtered hits the
        # same empty interval. The fallback is re-run THROUGH the same
        # projection (never assumed), its outcome recorded, and the
        # whole action refused: the only honest output is the recorded
        # failure, never an unfiltered pass-through.
        fallback_entries = [filter_entry(asset) for asset in _fallback_entries(validated["assets"])]
        fallback_outcomes = [entry["entry_outcome"] for entry in fallback_entries]
        fallback_viable = all(
            outcome in ("within_limits", "clamped_to_limit")
            for outcome in fallback_outcomes
        )
        report["fallback"] = {
            "action_kind": FALLBACK_ACTION_KIND,
            "proposed_permission": ACTION_PROPOSAL_PERMISSION,
            "attempted": True,
            "viable": fallback_viable,
            "entries": fallback_entries,
            "provable_note": (
                "primary interval empty => p_current outside [p_min, "
                "p_max] with ramp too small to reach the bounds => the "
                "hold-current entry (p_proposed = p_current) violates "
                "the same bounds => fallback infeasible (derivation in "
                "the module header, item 4)"
            ),
        }
        report["infeasibility"] = {
            "outcome": "infeasible_no_safe_projection",
            "entries": [
                entry["asset_id"]
                for entry in entries
                if entry["entry_outcome"] == "infeasible_no_safe_projection"
            ],
            "recorded": True,
        }
        report["action_outcome"] = "infeasible_no_safe_projection"
        return report

    if "refused_nonfinite_action" in entry_outcomes:
        # Gate 4b: a non-finite proposed setpoint. No fallback is
        # auto-emitted here: a non-finite proposal is a policy defect
        # recorded as the named refusal (the P8.15 evaluation counts it);
        # the current state itself was validated finite (structural
        # validation), so the refusal is attributable to the action, not
        # the state. Never coerced, never passed through.
        report["action_outcome"] = "refused_nonfinite_action"
        return report

    if any(outcome == "clamped_to_limit" for outcome in entry_outcomes):
        report["action_outcome"] = "filtered_clamped_to_limits"
        return report

    report["action_outcome"] = "passed_within_limits"
    return report


# ---------------------------------------------------------------------------
# Hand-computed self-tests (NEVER RUN in this loop; every expectation is a
# hand derivation from constructed inputs, stated in the comments).
# Run instruction (human gate):
#   python apps/worker/python/marl_worker/safety_filter.py
# (the pytest mirror is tests/test_safety_filter.py).
# ---------------------------------------------------------------------------


def _limits(p_min: float, p_max: float, ramp: float) -> dict:
    return {
        "p_min_mw": p_min,
        "p_max_mw": p_max,
        "ramp_mw_per_interval": ramp,
    }


def _action(kind: str, permission: str, entries: list) -> dict:
    return {
        "action_kind": kind,
        "proposed_permission": permission,
        "assets": entries,
    }


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. NEVER EXECUTED in this
    loop; the expectations are hand derivations (arithmetic stated), never
    observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    base_limits = _limits(0.0, 5.0, 1.5)  # p in [0, 5] MW, ramp 1.5 MW

    # ---- Test 1: inside the interval -> within_limits -------------------
    # HAND CHECK: p_current = 2.0, ramp = 1.5, limits [0, 5].
    #   interval = [max(0.0, 2.0 - 1.5), min(5.0, 2.0 + 1.5)] = [0.5, 3.5].
    #   proposed 2.5 is inside [0.5, 3.5] -> within_limits, filtered 2.5.
    entry = filter_entry(
        {"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": 2.5,
         "soc_fraction": 0.5, "limits": base_limits}
    )
    check(
        "entry_inside_interval_within_limits",
        entry["entry_outcome"] == "within_limits"
        and entry["p_filtered_mw"] == 2.5
        and entry["allowed_min_mw"] == 0.5
        and entry["allowed_max_mw"] == 3.5,
        "interval [0.5, 3.5] = [max(0, 2.0-1.5), min(5.0, 2.0+1.5)]; "
        "proposed 2.5 inside -> within_limits, filtered 2.5",
    )

    # ---- Test 2: above the interval -> clamped DOWN to p_current + ramp -
    # HAND CHECK: proposed 7.0 > hi = 3.5 -> clamped to 3.5
    #   (= min(5.0, 2.0 + 1.5): the RAMP binds before the P bound).
    entry = filter_entry(
        {"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": 7.0,
         "soc_fraction": None, "limits": base_limits}
    )
    check(
        "entry_above_clamped_to_ramp_bound",
        entry["entry_outcome"] == "clamped_to_limit"
        and entry["p_filtered_mw"] == 3.5,
        "proposed 7.0 > hi 3.5 = min(5.0, 2.0+1.5); ramp binds -> "
        "clamped to 3.5, activation recorded",
    )

    # ---- Test 3: below the interval -> clamped UP to p_current - ramp ---
    # HAND CHECK: proposed -1.0 < lo = 0.5 -> clamped to 0.5
    #   (= max(0.0, 2.0 - 1.5): the ramp binds before p_min = 0.0).
    entry = filter_entry(
        {"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": -1.0,
         "soc_fraction": None, "limits": base_limits}
    )
    check(
        "entry_below_clamped_to_ramp_floor",
        entry["entry_outcome"] == "clamped_to_limit"
        and entry["p_filtered_mw"] == 0.5,
        "proposed -1.0 < lo 0.5 = max(0.0, 2.0-1.5); ramp binds -> "
        "clamped to 0.5",
    )

    # ---- Test 4: P bound binds when the ramp would allow more -----------
    # HAND CHECK: p_current = 4.0, ramp = 1.5 -> hi = min(5.0, 5.5) = 5.0
    #   (the P bound binds before the ramp); proposed 5.4 clamps to 5.0.
    entry = filter_entry(
        {"asset_id": "ess-1", "p_current_mw": 4.0, "p_proposed_mw": 5.4,
         "soc_fraction": None, "limits": base_limits}
    )
    check(
        "entry_p_bound_binds",
        entry["entry_outcome"] == "clamped_to_limit"
        and entry["p_filtered_mw"] == 5.0
        and entry["allowed_max_mw"] == 5.0,
        "hi = min(5.0, 4.0+1.5) = 5.0: the P bound binds; 5.4 -> 5.0",
    )

    # ---- Test 5: EMPTY interval -> infeasible; fallback provably dead ---
    # HAND CHECK: p_current = 3.0 outside [p_min, p_max] = [1.0, 2.0],
    #   ramp 0.5: interval = [max(1.0, 3.0-0.5), min(2.0, 3.0+0.5)]
    #            = [2.5, 2.0] -> EMPTY (lo > hi).
    #   Fallback hold-current (p_proposed = 3.0) re-filtered: same interval
    #   [2.5, 2.0], and 3.0 is outside it -> fallback_infeasible recorded.
    infeasible_action = _action(
        "setpoint_delta",
        "schedule.propose",
        [{"asset_id": "ess-1", "p_current_mw": 3.0, "p_proposed_mw": 3.0,
          "soc_fraction": None, "limits": _limits(1.0, 2.0, 0.5)}],
    )
    report = filter_action(infeasible_action)
    fallback_ok = (
        report["action_outcome"] == "infeasible_no_safe_projection"
        and report["entries"][0]["allowed_min_mw"] == 2.5
        and report["entries"][0]["allowed_max_mw"] == 2.0
        and report["entries"][0]["p_filtered_mw"] is None
        and report["infeasibility"] is not None
        and report["infeasibility"]["recorded"] is True
        and report["fallback"] is not None
        and report["fallback"]["attempted"] is True
        and report["fallback"]["viable"] is False
    )
    check(
        "empty_interval_infeasible_fallback_attempted_and_dead",
        fallback_ok,
        "interval [max(1.0, 3.0-0.5), min(2.0, 3.0+0.5)] = [2.5, 2.0] "
        "EMPTY -> infeasible_no_safe_projection; fallback attempted, "
        "recorded NOT viable (3.0 outside [1.0, 2.0])",
    )

    # ---- Test 6: the provable fallback property (second construction) ---
    # HAND CHECK: p_current = -1.0 below p_min = 0.0, ramp 0.2:
    #   interval = [max(0.0, -1.2), min(4.0, -0.8)] = [0.0, -0.8] -> EMPTY.
    #   Hold-current (-1.0) is below p_min = 0.0 -> fallback infeasible.
    report = filter_action(
        _action(
            "setpoint_delta",
            "schedule.propose",
            [{"asset_id": "pv-1", "p_current_mw": -1.0, "p_proposed_mw": 1.0,
              "soc_fraction": None, "limits": _limits(0.0, 4.0, 0.2)}],
        )
    )
    check(
        "fallback_property_second_construction",
        report["action_outcome"] == "infeasible_no_safe_projection"
        and report["fallback"]["viable"] is False,
        "interval [max(0.0, -1.2), min(4.0, -0.8)] = [0.0, -0.8] EMPTY; "
        "hold-current -1.0 < p_min 0.0 -> fallback infeasible (the "
        "header item 4 derivation, second instance)",
    )

    # ---- Test 7: non-finite proposal -> named refusal, no projection ----
    # HAND CHECK: json.loads accepts NaN, so a NaN proposal can arrive;
    # the filter refuses it with refused_nonfinite_action and
    # p_filtered_mw = None (no 'nearest safe value' to a NaN).
    report = filter_action(
        _action(
            "setpoint_delta",
            "schedule.propose",
            [{"asset_id": "ess-1", "p_current_mw": 2.0,
              "p_proposed_mw": float("nan"),
              "soc_fraction": None, "limits": base_limits}],
        )
    )
    check(
        "nonfinite_proposal_refused_named",
        report["action_outcome"] == "refused_nonfinite_action"
        and report["entries"][0]["entry_outcome"] == "refused_nonfinite_action"
        and report["entries"][0]["p_filtered_mw"] is None,
        "NaN proposal -> refused_nonfinite_action; p_filtered_mw None "
        "(never coerced to an interval endpoint)",
    )

    # ---- Test 8: permission outside the ceiling -> refused, no entries --
    # HAND CHECK: "dispatch.issue" is not in {telemetry.read, asset.read,
    # schedule.propose} -> refused_permission_ceiling, zero entries, no
    # projection attempted (a permission is not projectable).
    report = filter_action(
        _action(
            "setpoint_delta",
            "dispatch.issue",
            [{"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": 2.5,
              "soc_fraction": None, "limits": base_limits}],
        )
    )
    check(
        "permission_outside_ceiling_refused_no_projection",
        report["action_outcome"] == "refused_permission_ceiling"
        and report["entries"] == []
        and report["filter_activations"] == [],
        "'dispatch.issue' not in the ceiling -> refused wholesale; no "
        "entry is projected (no clamp turns a forbidden permission into "
        "an allowed one)",
    )

    # ---- Test 9: aggregate mapping (clamped + within -> filtered_clamped)
    # HAND CHECK: entry A (2.0 -> 7.0, ramp 1.5) clamps to 3.5; entry B
    #   (1.0 -> 1.2) is inside [max(0,-0.5), min(5,2.5)] = [0.0, 2.5] ->
    #   within. Aggregate = filtered_clamped_to_limits; ONE activation.
    report = filter_action(
        _action(
            "setpoint_delta",
            "schedule.propose",
            [
                {"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": 7.0,
                 "soc_fraction": None, "limits": base_limits},
                {"asset_id": "ess-2", "p_current_mw": 1.0, "p_proposed_mw": 1.2,
                 "soc_fraction": None, "limits": base_limits},
            ],
        )
    )
    check(
        "aggregate_mixed_clamped_and_within",
        report["action_outcome"] == "filtered_clamped_to_limits"
        and len(report["filter_activations"]) == 1
        and report["filter_activations"][0]["asset_id"] == "ess-1"
        and report["filter_activations"][0]["p_filtered_mw"] == 3.5,
        "ess-1 clamps 7.0 -> 3.5; ess-2 within -> aggregate "
        "filtered_clamped_to_limits with exactly one activation",
    )

    # ---- Test 10: fully clean action -> passed_within_limits ------------
    report = filter_action(
        _action(
            "hold_current_setpoint",
            "schedule.propose",
            [{"asset_id": "ess-1", "p_current_mw": 2.0, "p_proposed_mw": 2.0,
              "soc_fraction": None, "limits": base_limits}],
        )
    )
    check(
        "aggregate_clean_hold_passes",
        report["action_outcome"] == "passed_within_limits"
        and report["filter_activations"] == [],
        "hold-current 2.0 inside [0.5, 3.5] -> passed_within_limits, "
        "zero activations",
    )

    # ---- Test 11: malformed inputs are ERRORS, never outcomes -----------
    def inverted_limits() -> None:
        validate_limits(_limits(5.0, 1.0, 1.0), "test")

    _expects_filter_input_error(results, inverted_limits, "inverted_limits_refused")

    def negative_ramp() -> None:
        validate_limits(_limits(0.0, 5.0, -1.0), "test")

    _expects_filter_input_error(results, negative_ramp, "negative_ramp_refused")

    def soc_out_of_range() -> None:
        validate_soc_fraction(1.5, "test")

    _expects_filter_input_error(results, soc_out_of_range, "soc_above_one_refused")

    def soc_negative() -> None:
        validate_soc_fraction(-0.1, "test")

    _expects_filter_input_error(results, soc_negative, "soc_negative_refused")

    def soc_boundaries_ok() -> None:
        validate_soc_fraction(0.0, "test")
        validate_soc_fraction(1.0, "test")

    try:
        soc_boundaries_ok()
        soc_ok = True
    except FilterInputError:
        soc_ok = False
    check(
        "soc_boundary_values_accepted",
        soc_ok,
        "SOC 0.0 and 1.0 (closed-interval endpoints) accepted",
    )

    def nonfinite_current_state() -> None:
        validate_action(
            _action(
                "setpoint_delta",
                "schedule.propose",
                [{"asset_id": "ess-1", "p_current_mw": float("nan"),
                  "p_proposed_mw": 1.0, "soc_fraction": None,
                  "limits": base_limits}],
            )
        )

    _expects_filter_input_error(
        results, nonfinite_current_state, "nonfinite_current_state_refused"
    )

    def unknown_action_kind() -> None:
        validate_action(
            _action(
                "connect_device",  # a device-command shape: outside the set
                "schedule.propose",
                [{"asset_id": "ess-1", "p_current_mw": 2.0,
                  "p_proposed_mw": 2.0, "soc_fraction": None,
                  "limits": base_limits}],
            )
        )

    _expects_filter_input_error(
        results, unknown_action_kind, "device_command_action_kind_refused"
    )

    def empty_asset_list() -> None:
        validate_action(_action("setpoint_delta", "schedule.propose", []))

    _expects_filter_input_error(
        results, empty_asset_list, "empty_asset_list_refused"
    )

    # ---- Test 12: closed vocabularies (no value outside the sets) -------
    check(
        "filter_outcome_vocabulary_closed",
        FILTER_OUTCOMES == frozenset(
            {
                "passed_within_limits",
                "filtered_clamped_to_limits",
                "refused_permission_ceiling",
                "refused_nonfinite_action",
                "infeasible_no_safe_projection",
            }
        )
        and REFUSING_OUTCOMES <= FILTER_OUTCOMES
        and set(ENTRY_OUTCOMES)
        <= {"within_limits", "clamped_to_limit", "refused_nonfinite_action",
            "infeasible_no_safe_projection"},
        "the aggregate vocabulary is exactly the five named outcomes; "
        "refusals are a subset; entry outcomes are the closed four",
    )

    # ---- Test 13: every filter_action result carries a closed outcome ---
    # HAND CHECK: each constructed action above produced an action_outcome
    # inside FILTER_OUTCOMES — assert it directly over fresh runs.
    probes = []
    for constructed in (
        infeasible_action,
        _action("setpoint_delta", "dispatch.issue", [
            {"asset_id": "a", "p_current_mw": 2.0, "p_proposed_mw": 2.5,
             "soc_fraction": None, "limits": base_limits}]),
        _action("setpoint_delta", "schedule.propose", [
            {"asset_id": "a", "p_current_mw": 2.0, "p_proposed_mw": float("inf"),
             "soc_fraction": None, "limits": base_limits}]),
    ):
        probes.append(filter_action(constructed)["action_outcome"])
    check(
        "every_probe_outcome_in_closed_vocabulary",
        all(outcome in FILTER_OUTCOMES for outcome in probes),
        "infeasible / permission / nonfinite probes all name closed "
        "outcomes (no silent path exists)",
    )

    return results


def _expects_filter_input_error(results: list, fn, name: str) -> None:
    """Self-test helper: `fn` must raise FilterInputError and nothing else."""
    try:
        fn()
    except FilterInputError:
        results.append(
            {
                "name": name,
                "passed": True,
                "detail": "raised FilterInputError as expected",
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of FilterInputError" % error,
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
