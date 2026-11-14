# P8.39 — Co-simulation and dynamic cross-checks: PURE agreement checkers
#         over INJECTED result data (no simulator, no I/O, no ANDES, no
#         HELICS, no pandapower).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. WHAT THIS MODULE IS: pure, stdlib-only checkers that compare two
#    INJECTED result documents (or federate records) and report whether
#    they AGREE within a PRE-REGISTERED tolerance. Four checkers:
#      - endpoint_agreement:       the endpoint values of a steady-state
#                                  schedule vs the end-of-window endpoint
#                                  values of a dynamic simulation, on an
#                                  IDENTICAL time base (mismatched time
#                                  bases are REFUSED, never interpolated);
#      - vector_agreement:         ANDES power-flow vs pandapower
#                                  power-flow per-bus/branch vectors
#                                  (voltages, flows) on an IDENTICAL index
#                                  base in an IDENTICAL order (a reordered
#                                  or partially different index list is
#                                  REFUSED, never silently aligned);
#      - federate_time_step_check: declared per-federate HELICS time steps
#                                  — a mismatch is DETECTED and reported,
#                                  and any co-simulation interaction claim
#                                  built on the mismatched federation is
#                                  refused;
#      - granted_time_check:       granted-time exit reports — final
#                                  granted times must agree across
#                                  federates, and a federate that stops
#                                  before the scenario end is recorded as
#                                  a DROPOUT, never silently treated as
#                                  "no change".
# 2. WHAT A CROSS-CHECK IS (the design, fixed before any run — see the
#    decision record 39_cosim_crosschecks.md): a cross-check between two
#    model classes is AGREEMENT EVIDENCE ON THE OVERLAPPING DOMAIN ONLY.
#    It is never a validation of either model against reality, never a
#    safety case, and never a statement about the real grid (§19.1).
#    Agreement within tolerance says the two DECLARED models agree where
#    their domains overlap; disagreement beyond tolerance says they
#    disagree there — both outcomes are recorded as observations, and a
#    difference beyond tolerance is EXPLAINED (model conventions, model
#    scope), never absorbed by widening the tolerance (X-8).
# 3. STRUCTURAL SEPARATION OF DYNAMIC BLOCKS (X-5) IS ENFORCED IN CODE:
#    every compared series lives inside its own per-model-class block
#    (`series_blocks`, each entry declaring `model_class` from the closed
#    vocabulary below); the `comparison` block carries DELTAS ONLY —
#    never values, never a joint series. `assert_structural_separation`
#    refuses any document that carries a merged/averaged/blended series
#    key, any comparison block that carries values, and any series block
#    without a declared model class. Steady-state and dynamic results are
#    NEVER averaged or merged — the merge is unrepresentable, not merely
#    discouraged.
# 4. HELICS IS TRANSPORT, NOT PHYSICS (§19.1): the two HELICS checkers
#    report transport-level facts about the COORDINATOR (declared time
#    steps, granted times, completion) — they are never power-system
#    results, and "the federation synchronized" says nothing about grid
#    behaviour.
# 5. NO TOLERANCE IS BAKED IN (X-8): every comparison REQUIRES the caller
#    to inject the pre-registered tolerance; a `None` tolerance is refused
#    with THRESHOLD_NOT_PRE_REGISTERED. The tolerance VALUES are
#    registered in the decision record BEFORE the first run (dated
#    amendment), never fitted after seeing a difference. This module
#    computes deltas and verdicts; it never emits a success/failure claim
#    beyond "within / beyond the injected pre-registered tolerance".
# 6. UNITS: power MW/MVAr, voltage pu (solver representation), time
#    seconds (§10.2 canonical units). `P > 0` = export at the point of
#    connection; the SIGN CONVENTION is DECLARED per series and a
#    comparison across different declared conventions is refused
#    (§10.2: reactive-power sign declared per grid model and device
#    profile, never assumed).
# 7. WRITTEN, NEVER EXECUTED. No function in this module has ever run in
#    this loop (the loop bans execution; ANDES and HELICS were never
#    installed). The self-tests below carry HAND-COMPUTED expected values
#    — each expected figure is derived in a comment from the constructed
#    input, never read back from the code under test. The first execution
#    belongs to the human build/test pass. A self-test's toy arithmetic
#    pins the checker DEFINITIONS; it is NOT a cross-check measurement
#    and fills no result cell.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

PROTOCOL_ID = "flexcore-cosim-crosscheck/1"

MODULE_VERSION = "flexcore-cosim-crosscheck/0.1.0"

# The closed model-class vocabulary (X-5): every compared series declares
# exactly one class; the classes' results live in separate blocks and are
# never merged or averaged.
MODEL_CLASSES: tuple = ("steady_state", "dynamic")

# The closed quantity vocabulary with its canonical units (§10.2). A
# series/vector declaring any other quantity is refused, never guessed.
ALLOWED_QUANTITIES: frozenset = frozenset({"voltage_pu", "p_mw", "q_mvar"})

# Keys that must NEVER appear anywhere in a cross-check record: they would
# express a merged/averaged/blended series across model classes — exactly
# what X-5 forbids. `assert_structural_separation` refuses them.
FORBIDDEN_MERGE_KEYS: frozenset = frozenset(
    {
        "merged",
        "merged_series",
        "averaged",
        "average",
        "average_series",
        "mean_series",
        "blended",
        "combined_series",
        "weighted_blend",
    }
)

# Keys the comparison block must never carry: the comparison block carries
# DELTAS ONLY. A value inside the comparison block would be a joint series
# across model classes (X-5).
FORBIDDEN_COMPARISON_KEYS: frozenset = frozenset(
    {"values", "value", "series", "series_values", "samples"}
)

# Keys that must never appear anywhere in a cross-check record — the
# in-code proposal-only boundary (ADR-010; P8.04 posture, mirroring the
# P8.16 worker's forbidden-key discipline). A cross-check record is
# diagnostics; its response type cannot express an authorization token or
# a device command.
FORBIDDEN_RESULT_KEYS: frozenset = frozenset(
    {
        "authorization_token",
        "authorization",
        "auth_token",
        "command",
        "command_id",
        "command_ids",
        "issue",
        "issued",
        "dispatch",
        "dispatchable",
        "approval",
        "approval_token",
    }
)

# The model-class boundary statements carried VERBATIM in every record
# built by `build_crosscheck_record` (§19.1; X-5; ADR-010).
BOUNDARY_STATEMENTS: tuple = (
    "A cross-check between two model classes is AGREEMENT EVIDENCE ON THE "
    "OVERLAPPING DOMAIN ONLY — it is never a validation of either model "
    "against reality, never a safety case, and never a statement about the "
    "real grid (report 19.1).",
    "Steady-state and dynamic results are structurally separate (X-5): "
    "they are never averaged, merged, or blended; the comparison block "
    "carries deltas only, every series lives in its own declared-class "
    "block, and this separation is enforced in code "
    "(assert_structural_separation).",
    "A steady-state power flow is NOT transient-stability evidence (X-5): "
    "the ANDES-vs-pandapower comparison is a steady-state consistency "
    "check only and says nothing about frequency nadir, ROCOF, or "
    "transient response.",
    "HELICS is TRANSPORT, not physics (report 19.1): the time-step and "
    "granted-time checks are statements about the co-simulation "
    "coordinator's synchronization, never power-system results.",
    "Every tolerance is PRE-REGISTERED before any run (X-8) and injected "
    "by the caller; this module refuses an unregistered comparison and "
    "never fits a tolerance to an observed difference.",
    "Cross-check records are research diagnostics/proposals only "
    "(ADR-010): no authorization token exists in this response type, and "
    "nothing here can be issued to a device.",
)

WRITTEN_UNRUN_NOTE = (
    "WRITTEN-UNRUN module: every checker below is pure stdlib over "
    "INJECTED data; ANDES and HELICS were never installed in this loop, "
    "no federation ran, no power flow ran, and no function here was ever "
    "executed. The ANDES/HELICS runs that fill the decision record's "
    "EMPTY cells are human/lab gate inputs."
)


class CrossCheckError(ValueError):
    """A cross-check's injected inputs violate a recorded rule (fail-closed).

    Raised BEFORE any agreement figure is produced: a malformed or
    unregistered comparison can never yield a verdict, because a verdict
    computed from malformed input (or without a pre-registered tolerance)
    would be indistinguishable from a real agreement result. The message
    begins with the NAMED refusal code.
    """


# ---------------------------------------------------------------------------
# Shared fail-closed validation
# ---------------------------------------------------------------------------


def _validate_scalar(name: str, value) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise CrossCheckError("%s must be a finite real number" % name)


def _validate_nonempty_str(name: str, value) -> None:
    if not isinstance(value, str) or not value:
        raise CrossCheckError("%s must be a non-empty string" % name)


def _validate_tolerance(tolerance) -> float:
    """X-8 gate: a comparison does not exist without a pre-registered
    tolerance, and the tolerance is a non-negative finite number."""
    if tolerance is None:
        raise CrossCheckError(
            "THRESHOLD_NOT_PRE_REGISTERED: tolerance is None — every "
            "cross-check tolerance is pre-registered BEFORE any run (X-8) "
            "and injected by the caller; a comparison without one is "
            "refused, never run with a default, and never fitted after "
            "seeing a difference"
        )
    if isinstance(tolerance, bool) or not isinstance(tolerance, (int, float)) or not math.isfinite(tolerance):
        raise CrossCheckError("TOLERANCE_INVALID: tolerance must be a finite real number")
    if tolerance < 0.0:
        raise CrossCheckError("TOLERANCE_INVALID: tolerance must be non-negative")
    return float(tolerance)


def _validate_series(times: list, values: list, tag: str) -> None:
    """Fail-closed series validation for one injected series.

    Requires: equal, non-empty lengths; at least two samples; every time
    and value a finite (non-boolean) real number; times STRICTLY
    increasing (a repeated timestamp cannot be ordered).
    """
    if not isinstance(times, (list, tuple)) or not isinstance(values, (list, tuple)):
        raise CrossCheckError("%s: times and values must be sequences of numbers" % tag)
    if len(times) != len(values):
        raise CrossCheckError(
            "%s: times and values must have equal length (%d vs %d)"
            % (tag, len(times), len(values))
        )
    if len(times) < 2:
        raise CrossCheckError("%s: a comparison needs at least two samples" % tag)
    previous_time = None
    for index, (t, v) in enumerate(zip(times, values)):
        if isinstance(t, bool) or not isinstance(t, (int, float)) or not math.isfinite(t):
            raise CrossCheckError("%s: times[%d] is not a finite real number" % (tag, index))
        if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
            raise CrossCheckError("%s: values[%d] is not a finite real number" % (tag, index))
        if previous_time is not None and t <= previous_time:
            raise CrossCheckError(
                "%s: times must be strictly increasing (times[%d]=%r follows %r)"
                % (tag, index, t, previous_time)
            )
        previous_time = t


def _require_shared_time_base(times_a: list, times_b: list) -> None:
    """Refuse mismatched time bases — never interpolate.

    The two series are compared ONLY on an identical sample grid (exact
    equality of every timestamp). Resampling or interpolation is NOT
    provided: interpolating one model's result onto the other's grid would
    FABRICATE samples that neither model produced, and an agreement
    computed on fabricated samples would be agreement about nothing.
    """
    if len(times_a) != len(times_b):
        raise CrossCheckError(
            "TIME_BASE_MISMATCH: the two series have different lengths "
            "(%d vs %d samples); comparison is refused — never padded, "
            "resampled, or interpolated" % (len(times_a), len(times_b))
        )
    for index, (ta, tb) in enumerate(zip(times_a, times_b)):
        if ta != tb:
            raise CrossCheckError(
                "TIME_BASE_MISMATCH: times differ at sample %d (%r vs %r); "
                "comparison is refused — the checkers compare only on an "
                "IDENTICAL sample grid, never interpolated (interpolation "
                "would fabricate samples neither model produced)"
                % (index, ta, tb)
            )


def _validate_class_and_quantity(
    class_a: str, class_b: str, quantity: str
) -> None:
    if class_a not in MODEL_CLASSES or class_b not in MODEL_CLASSES:
        raise CrossCheckError(
            "MODEL_CLASS_UNKNOWN: every compared series declares its model "
            "class from the closed vocabulary %s (got %r and %r); an "
            "undeclared class would make the X-5 structural separation "
            "unenforceable" % (list(MODEL_CLASSES), class_a, class_b)
        )
    if quantity not in ALLOWED_QUANTITIES:
        raise CrossCheckError(
            "QUANTITY_UNKNOWN: quantity %r is not one of %s (units are "
            "declared, never assumed — §10.2)" % (quantity, sorted(ALLOWED_QUANTITIES))
        )


# ---------------------------------------------------------------------------
# CC-4 — endpoint agreement (steady-state schedule vs dynamic endpoints)
# ---------------------------------------------------------------------------


def endpoint_agreement(
    times_a: list,
    values_a: list,
    times_b: list,
    values_b: list,
    *,
    tolerance,
    label_a: str,
    label_b: str,
    class_a: str,
    class_b: str,
    quantity: str,
    sign_convention_a: str,
    sign_convention_b: str,
) -> dict:
    """Endpoint-agreement check between two injected series.

    The overlapping domain: the SHARED sample times. At each shared time,
    delta = |value_b − value_a|; the verdict is
    max_abs_delta <= tolerance with the PRE-REGISTERED tolerance.

    The OVERLAPPING DOMAIN ONLY discipline: agreement is asserted at the
    shared samples and about nothing else. A steady-state schedule says
    nothing about a dynamic trace BETWEEN its endpoints, and a dynamic
    trace says nothing about the steady state between schedule intervals —
    a verdict here is agreement evidence on the shared samples, never a
    validation of either series against reality (§19.1).

    Structural separation (X-5): the returned record keeps each series in
    its own declared-class block under `series_blocks`; the `comparison`
    block carries PER-SAMPLE ABSOLUTE DELTAS and the max only — never
    values, never a joint series, never an average.
    """
    _validate_class_and_quantity(class_a, class_b, quantity)
    _validate_nonempty_str("label_a", label_a)
    _validate_nonempty_str("label_b", label_b)
    _validate_nonempty_str("sign_convention_a", sign_convention_a)
    _validate_nonempty_str("sign_convention_b", sign_convention_b)
    if sign_convention_a != sign_convention_b:
        raise CrossCheckError(
            "SIGN_CONVENTION_MISMATCH: the two series declare different "
            "sign conventions (%r vs %r) — a delta between "
            "differently-signed values is meaningless (§10.2: the sign "
            "is declared per grid model and device profile, never "
            "assumed; `P > 0` = export at the point of connection)"
            % (sign_convention_a, sign_convention_b)
        )
    sign_convention = sign_convention_a
    _validate_series(times_a, values_a, "series_a (%s)" % label_a)
    _validate_series(times_b, values_b, "series_b (%s)" % label_b)
    _require_shared_time_base(times_a, times_b)
    registered_tolerance = _validate_tolerance(tolerance)
    deltas = [abs(vb - va) for va, vb in zip(values_a, values_b)]
    max_delta = max(deltas)
    max_index = deltas.index(max_delta)
    return {
        "check": "endpoint_agreement",
        "quantity": quantity,
        "sign_convention": sign_convention,
        "comparison_rule": "max_abs_delta <= pre-registered tolerance, "
                           "evaluated on the IDENTICAL shared time base "
                           "only (the overlapping domain)",
        "comparison": {
            "per_sample_abs_deltas": deltas,
            "max_abs_delta": max_delta,
            "max_delta_time_s": times_a[max_index],
            "max_delta_sample_index": max_index,
            "shared_sample_count": len(deltas),
            "tolerance": registered_tolerance,
            "within_tolerance": max_delta <= registered_tolerance,
        },
        "series_blocks": [
            {
                "model_class": class_a,
                "label": label_a,
                "times_s": list(times_a),
                "values": list(values_a),
            },
            {
                "model_class": class_b,
                "label": label_b,
                "times_s": list(times_b),
                "values": list(values_b),
            },
        ],
        "note": "agreement evidence on the overlapping domain only — "
                "never a validation of either model against reality "
                "(X-5: the two class blocks are never averaged or "
                "merged; the comparison carries deltas only)",
    }


# ---------------------------------------------------------------------------
# CC-3 — vector agreement (ANDES power flow vs pandapower power flow)
# ---------------------------------------------------------------------------


def vector_agreement(
    index_a: list,
    values_a: list,
    index_b: list,
    values_b: list,
    *,
    tolerance,
    label_a: str,
    label_b: str,
    class_a: str,
    class_b: str,
    quantity: str,
    sign_convention_a: str,
    sign_convention_b: str,
) -> dict:
    """Per-element agreement between two injected result vectors.

    Built for CC-3: ANDES power flow vs pandapower power flow on the SAME
    case with the SAME inputs — bus-voltage magnitudes (pu) and branch
    power flows (MW/MVAr). The two index lists (bus/branch ids) must be
    IDENTICAL INCLUDING ORDER: a reordered or partially different index
    list is refused (INDEX_BASE_MISMATCH), never silently aligned — a
    silent alignment would let a voltage be compared against the wrong
    bus, producing agreement (or disagreement) about nothing.

    Units: voltage pu (solver representation); flows MW/MVAr; `P > 0` =
    export at the point of connection; the sign convention is DECLARED
    (§10.2) and a comparison across different declared conventions is
    refused (SIGN_CONVENTION_MISMATCH).
    """
    _validate_class_and_quantity(class_a, class_b, quantity)
    _validate_nonempty_str("label_a", label_a)
    _validate_nonempty_str("label_b", label_b)
    _validate_nonempty_str("sign_convention_a", sign_convention_a)
    _validate_nonempty_str("sign_convention_b", sign_convention_b)
    if sign_convention_a != sign_convention_b:
        raise CrossCheckError(
            "SIGN_CONVENTION_MISMATCH: the two vectors declare different "
            "sign conventions (%r vs %r) — a delta between "
            "differently-signed values is meaningless (§10.2: the sign "
            "is declared per grid model and device profile, never "
            "assumed)" % (sign_convention_a, sign_convention_b)
        )
    sign_convention = sign_convention_a
    for tag, index_list in (("index_a", index_a), ("index_b", index_b)):
        if not isinstance(index_list, (list, tuple)):
            raise CrossCheckError("%s must be a sequence of element ids" % tag)
        if len(index_list) == 0:
            raise CrossCheckError("%s must be non-empty" % tag)
        for i, element in enumerate(index_list):
            if not isinstance(element, str) or not element:
                raise CrossCheckError("%s[%d] must be a non-empty string id" % (tag, i))
    if len(index_a) != len(values_a) or len(index_b) != len(values_b):
        raise CrossCheckError(
            "INDEX_BASE_MISMATCH: each index list must match its values "
            "list in length (%d vs %d, %d vs %d)"
            % (len(index_a), len(values_a), len(index_b), len(values_b))
        )
    if len(index_a) != len(index_b) or any(x != y for x, y in zip(index_a, index_b)):
        raise CrossCheckError(
            "INDEX_BASE_MISMATCH: the two index lists are not identical "
            "(including order); comparison is refused — never silently "
            "aligned or reordered (a silent alignment could compare a "
            "value against the wrong bus/branch)"
        )
    for tag, values in (("values_a", values_a), ("values_b", values_b)):
        if not isinstance(values, (list, tuple)):
            raise CrossCheckError("%s must be a sequence of numbers" % tag)
        for i, v in enumerate(values):
            if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
                raise CrossCheckError("%s[%d] is not a finite real number" % (tag, i))
    registered_tolerance = _validate_tolerance(tolerance)
    deltas = [abs(vb - va) for va, vb in zip(values_a, values_b)]
    max_delta = max(deltas)
    max_element = index_a[deltas.index(max_delta)]
    return {
        "check": "vector_agreement",
        "quantity": quantity,
        "sign_convention": sign_convention,
        "comparison_rule": "max_abs_delta <= pre-registered tolerance, "
                           "element-by-element on the IDENTICAL index "
                           "base (same ids, same order)",
        "comparison": {
            "per_element_abs_deltas": deltas,
            "max_abs_delta": max_delta,
            "max_delta_element": max_element,
            "element_count": len(deltas),
            "tolerance": registered_tolerance,
            "within_tolerance": max_delta <= registered_tolerance,
        },
        "series_blocks": [
            {
                "model_class": class_a,
                "label": label_a,
                "index": list(index_a),
                "values": list(values_a),
            },
            {
                "model_class": class_b,
                "label": label_b,
                "index": list(index_b),
                "values": list(values_b),
            },
        ],
        "note": "steady-state consistency check ONLY (X-5: a power flow "
                "evaluates a state; it is not transient-stability "
                "evidence); a difference beyond tolerance is recorded "
                "AND explained (model conventions, model scope), never "
                "absorbed by widening the tolerance (X-8)",
    }


# ---------------------------------------------------------------------------
# CC-1 / CC-2 — HELICS transport checks (time steps, granted times, dropout)
# ---------------------------------------------------------------------------


def _validate_federation(reports: list, function_name: str) -> list:
    if not isinstance(reports, (list, tuple)):
        raise CrossCheckError("%s: reports must be a sequence of records" % function_name)
    if len(reports) < 2:
        raise CrossCheckError(
            "FEDERATION_NEEDS_TWO_FEDERATES: a cross-check needs at least "
            "two federate records (got %d); a one-federate 'federation' "
            "has nothing to agree with" % len(reports)
        )
    validated: list = []
    seen_names: set = set()
    for record in reports:
        if not isinstance(record, dict):
            raise CrossCheckError("%s: every federate record must be a dict" % function_name)
        name = record.get("name")
        _validate_nonempty_str("federate name", name)
        if name in seen_names:
            raise CrossCheckError(
                "FEDERATE_NAME_DUPLICATE: federate %r appears more than "
                "once; a duplicated record would double-count a federate "
                "in the sync/dropout checks" % name
            )
        seen_names.add(name)
        validated.append(record)
    return validated


def federate_time_step_check(federate_configs: list) -> dict:
    """CC-1: detect a declared time-step mismatch across federates.

    Input: one record per federate {"name": str,
    "declared_time_step_seconds": positive finite number}. A mismatch is
    DETECTED and REPORTED with the mismatching federates named, and the
    verdict refuses any co-simulation interaction claim built on the
    mismatched federation — the interaction effect of a federation whose
    federates advance on different declared steps is exactly the silent
    corruption the ledger names as the risk, so the claim is refused, not
    qualified.

    HELICS is TRANSPORT, not physics: this check is a statement about the
    coordinator's configuration, never a power-system result.
    """
    validated = _validate_federation(federate_configs, "federate_time_step_check")
    steps: dict = {}
    for record in validated:
        step = record.get("declared_time_step_seconds")
        if isinstance(step, bool) or not isinstance(step, (int, float)) or not math.isfinite(step) or step <= 0.0:
            raise CrossCheckError(
                "TIME_STEP_INVALID: federate %r declares %r — "
                "declared_time_step_seconds must be a positive finite "
                "number" % (record.get("name"), step)
            )
        steps[record["name"]] = float(step)
    distinct = sorted(set(steps.values()))
    mismatch_detected = len(distinct) > 1
    report = {
        "check": "federate_time_step_check",
        "declared_time_steps_s": steps,
        "distinct_declared_steps_s": distinct,
        "time_step_consistent": not mismatch_detected,
        "mismatch_detected": mismatch_detected,
        "mismatching_federates": (
            [name for name, step in steps.items() if step != distinct[0]]
            if mismatch_detected
            else []
        ),
        "verdict": (
            "INTERACTION_CLAIM_REFUSED: federates declare mismatched time "
            "steps %s — any interaction effect measured on this "
            "federation is refused as uninterpretable until the declared "
            "steps agree (the mismatch is reported, never silently "
            "absorbed)" % distinct
            if mismatch_detected
            else "declared time steps agree (transport-level fact); the "
                 "interaction claim still requires the granted-time and "
                 "physics records, not this check alone"
        ),
        "note": "HELICS is TRANSPORT, not physics (report 19.1): this is "
                "a statement about the federation's declared "
                "synchronization configuration, never a power-system "
                "result",
    }
    return report


def granted_time_check(exit_reports: list, scenario_end_time_s=None) -> dict:
    """CC-2: granted-time synchronization and federate-dropout detection.

    Input: one exit report per federate {"name": str,
    "final_granted_time_s": non-negative finite number,
    "granted_step_count": positive integer}; optional
    `scenario_end_time_s` (the configured scenario end). Checks:

    - synchronization: all final granted times must be EQUAL — unequal
      final granted times are a reported synchronization mismatch;
    - dropout: a federate whose final granted time is below the scenario
      end (or, when the end is not configured, below the latest federate's
      final time) is recorded as a DROPOUT with the federate NAMED — a
      federate that stops responding mid-run is never silently treated as
      "no change" (the ledger's named failure mode).

    Every detected condition refuses the interaction claim. Transport
    facts only — never physics (§19.1).
    """
    validated = _validate_federation(exit_reports, "granted_time_check")
    if scenario_end_time_s is not None:
        _validate_scalar("scenario_end_time_s", scenario_end_time_s)
        if scenario_end_time_s < 0.0:
            raise CrossCheckError("scenario_end_time_s must be non-negative")
    finals: dict = {}
    step_counts: dict = {}
    for record in validated:
        final = record.get("final_granted_time_s")
        count = record.get("granted_step_count")
        if isinstance(final, bool) or not isinstance(final, (int, float)) or not math.isfinite(final) or final < 0.0:
            raise CrossCheckError(
                "GRANTED_TIME_INVALID: federate %r reports final_granted_time_s=%r — "
                "must be a non-negative finite number" % (record.get("name"), final)
            )
        if isinstance(count, bool) or not isinstance(count, int) or count < 1:
            raise CrossCheckError(
                "STEP_COUNT_INVALID: federate %r reports granted_step_count=%r — "
                "must be a positive integer (a zero-step federate is a "
                "dropout, recorded as such by the scenario record, never "
                "a valid report)" % (record.get("name"), count)
            )
        finals[record["name"]] = float(final)
        step_counts[record["name"]] = count
    latest_final = max(finals.values())
    synchronized = len(set(finals.values())) == 1
    dropouts: list = []
    if scenario_end_time_s is not None:
        dropouts = [
            {"federate": name, "final_granted_time_s": final,
             "scenario_end_time_s": float(scenario_end_time_s)}
            for name, final in sorted(finals.items())
            if final < float(scenario_end_time_s)
        ]
    else:
        dropouts = [
            {"federate": name, "final_granted_time_s": final,
             "latest_federate_final_time_s": latest_final}
            for name, final in sorted(finals.items())
            if final < latest_final
        ]
    detected = dropouts or not synchronized
    reasons: list = []
    if not synchronized:
        reasons.append(
            "synchronization mismatch: final granted times %s are not all "
            "equal" % sorted(set(finals.values()))
        )
    for dropout in dropouts:
        reasons.append(
            "FEDERATE_DROPOUT_DETECTED: federate %r stopped at %s s "
            "before the scenario end (%s s) — recorded as a dropout, "
            "NEVER treated as 'no change'"
            % (
                dropout["federate"],
                dropout["final_granted_time_s"],
                dropout.get("scenario_end_time_s",
                            dropout.get("latest_federate_final_time_s")),
            )
        )
    report = {
        "check": "granted_time_check",
        "final_granted_times_s": finals,
        "granted_step_counts": step_counts,
        "synchronization_consistent": synchronized,
        "all_federates_completed": not dropouts,
        "dropouts": dropouts,
        "detected_conditions": reasons,
        "verdict": (
            "INTERACTION_CLAIM_REFUSED: %s — any interaction effect "
            "measured on this federation run is refused as uninterpretable"
            % "; ".join(reasons)
            if detected
            else "all federates completed to the same final granted time "
                 "(transport-level fact); the interaction claim still "
                 "requires the physics records, not this check alone"
        ),
        "note": "HELICS is TRANSPORT, not physics (report 19.1); the "
                "granted-time/step-count cross-check is the recorded "
                "time-synchronization scheme (P5.16/P8.18); a deeper "
                "message-timestamp audit remains the recorded follow-up",
    }
    return report


# ---------------------------------------------------------------------------
# In-code enforcement: structural separation (X-5) and proposal-only (ADR-010)
# ---------------------------------------------------------------------------


def _walk_for_merge_keys(node, path: str, findings: list) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            here = "%s.%s" % (path, key) if path else key
            if key in FORBIDDEN_MERGE_KEYS:
                findings.append(
                    "MERGED_SERIES_REFUSED: key %r at %s — a "
                    "merged/averaged/blended series across model classes "
                    "is unrepresentable in a cross-check record (X-5: "
                    "steady-state and dynamic results are never averaged "
                    "or merged)" % (key, here)
                )
            if key == "comparison" and isinstance(value, dict):
                for comparison_key in value:
                    if comparison_key in FORBIDDEN_COMPARISON_KEYS:
                        findings.append(
                            "COMPARISON_CARRIES_VALUES_REFUSED: key %r "
                            "inside the comparison block at %s — the "
                            "comparison block carries DELTAS ONLY; a "
                            "value there would be a joint series across "
                            "model classes (X-5)" % (comparison_key, here)
                        )
            if key == "series_blocks" and isinstance(value, list):
                for position, block in enumerate(value):
                    if not isinstance(block, dict) or block.get("model_class") not in MODEL_CLASSES:
                        findings.append(
                            "CLASS_DECLARATION_REQUIRED: series_blocks[%d] "
                            "at %s does not declare a model_class from %s "
                            "— an undeclared class makes the X-5 "
                            "separation unenforceable"
                            % (position, here, list(MODEL_CLASSES))
                        )
            _walk_for_merge_keys(value, here, findings)
    elif isinstance(node, (list, tuple)):
        for position, value in enumerate(node):
            _walk_for_merge_keys(value, "%s[%d]" % (path, position), findings)


def assert_structural_separation(document) -> None:
    """Refuse any document that expresses a merge across model classes.

    Enforced IN CODE (X-5), not prose:
      1. no FORBIDDEN_MERGE_KEYS key anywhere ("merged_series",
         "averaged", ...);
      2. the `comparison` block carries DELTAS ONLY — no values/series key
         inside it;
      3. every `series_blocks` entry declares `model_class` from the
         closed vocabulary — values exist only inside their own
         declared-class block, so a cross-class merged series cannot even
         be expressed.
    """
    findings: list = []
    _walk_for_merge_keys(document, "", findings)
    if findings:
        raise CrossCheckError(findings[0])


def _walk_for_forbidden_keys(node, findings: list) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if key in FORBIDDEN_RESULT_KEYS:
                findings.append(key)
            _walk_for_forbidden_keys(value, findings)
    elif isinstance(node, (list, tuple)):
        for value in node:
            _walk_for_forbidden_keys(value, findings)


def assert_proposal_only(document) -> None:
    """The in-code proposal-only boundary (ADR-010; P8.04 posture).

    A cross-check record is diagnostics. If any authorization- or
    command-shaped key appears anywhere in the document, the emission is
    refused: the record type cannot express a dispatch decision.
    """
    findings: list = []
    _walk_for_forbidden_keys(document, findings)
    if findings:
        raise CrossCheckError(
            "PROPOSAL_ONLY_REFUSED: forbidden key(s) %s present — a "
            "cross-check record is a research diagnostic (ADR-010); its "
            "type cannot express an authorization token or a device "
            "command" % sorted(set(findings))
        )


def build_crosscheck_record(pair_id: str, checks: list, provenance=None) -> dict:
    """Assemble the cross-check record document and enforce the boundaries.

    The record carries: the protocol tag, the pair id, the checker
    reports, the boundary statements VERBATIM, and the WRITTEN-UNRUN
    honesty note. Both in-code boundaries are enforced on the ASSEMBLED
    document before it is returned (structural separation, X-5;
    proposal-only, ADR-010) — a record that violates either is refused,
    never repaired.
    """
    _validate_nonempty_str("pair_id", pair_id)
    if not isinstance(checks, (list, tuple)) or not checks:
        raise CrossCheckError("checks must be a non-empty sequence of checker reports")
    document = {
        "protocol": PROTOCOL_ID,
        "module_version": MODULE_VERSION,
        "pair_id": pair_id,
        "checks": list(checks),
        "result_class": "agreement_evidence_only",
        "boundary_statements": list(BOUNDARY_STATEMENTS),
        "written_unrun_note": WRITTEN_UNRUN_NOTE,
        "provenance": dict(provenance) if provenance else {
            "module": "cosim_crosschecks",
            "data_source": "INJECTED by the caller — this module performs "
                           "no I/O and runs no simulator",
        },
    }
    assert_structural_separation(document)
    assert_proposal_only(document)
    return document


# ══════════════════════════════════════════════════════════════════════════
# IN-MODULE SELF-TESTS — HAND-COMPUTED expected values.
#
# WRITTEN, NEVER EXECUTED (loop bans). Every expected figure below is
# derived BY HAND from the constructed input, with the arithmetic stated
# in the comment; nothing is read back from the functions under test.
# The human build/test pass runs them via:
#   python apps/worker/python/andes_worker/cosim_crosschecks.py
# or via the pytest mirror
#   apps/worker/python/andes_worker/tests/test_cosim_crosschecks.py
# These tests pin the checker DEFINITIONS; they are NOT cross-check
# measurements and fill no result cell in the decision record.
# ══════════════════════════════════════════════════════════════════════════


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. This function itself has
    NEVER been executed in this loop; the expected values are hand
    derivations, not observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: endpoint agreement within a pre-registered tolerance --
    # Constructed series over a shared 1-hour-step time base
    # t = [0, 3600, 7200] s (MW; P > 0 = export):
    #   steady-state schedule endpoints  A = [2.0, 3.0, -1.0]
    #   dynamic simulation endpoints     B = [2.0, 3.05, -1.0]
    # HAND ARITHMETIC: deltas = [|2.0-2.0|, |3.05-3.0|, |-1.0-(-1.0)|]
    #                  = [0.0, 0.05, 0.0]  → max_abs_delta = 0.05 MW at
    # t = 3600.0 s. Pre-registered tolerance 0.1 MW → within tolerance.
    shared_times = [0.0, 3600.0, 7200.0]
    schedule = [2.0, 3.0, -1.0]
    dynamic_endpoints = [2.0, 3.05, -1.0]
    within = endpoint_agreement(
        shared_times, schedule, shared_times, dynamic_endpoints,
        tolerance=0.1,
        label_a="quasi-static schedule endpoints",
        label_b="ANDES TDS end-of-window endpoints",
        class_a="steady_state", class_b="dynamic",
        quantity="p_mw", sign_convention_a="export_positive",
        sign_convention_b="export_positive",
    )
    check(
        "endpoint_agreement_within_tolerance",
        within["comparison"]["within_tolerance"] is True
        and abs(within["comparison"]["max_abs_delta"] - 0.05) < 1e-12
        and within["comparison"]["max_delta_time_s"] == 3600.0
        and within["comparison"]["shared_sample_count"] == 3
        and within["comparison"]["per_sample_abs_deltas"] == [0.0, 0.05, 0.0],
        "expected deltas [0.0, 0.05, 0.0] MW by hand, max 0.05 MW at "
        "t=3600.0 s, within the pre-registered 0.1 MW tolerance",
    )

    # ---- Test 2: endpoint agreement BEYOND tolerance (honest verdict) ---
    # Same schedule A; dynamic endpoints B' = [2.0, 3.2, -1.0].
    # HAND ARITHMETIC: deltas = [0.0, |3.2-3.0|, 0.0] = [0.0, 0.2, 0.0]
    # → max 0.2 MW > 0.1 MW → within_tolerance is False. The module
    # reports the disagreement; it never widens the tolerance.
    beyond = endpoint_agreement(
        shared_times, schedule, shared_times, [2.0, 3.2, -1.0],
        tolerance=0.1,
        label_a="quasi-static schedule endpoints",
        label_b="ANDES TDS end-of-window endpoints",
        class_a="steady_state", class_b="dynamic",
        quantity="p_mw", sign_convention_a="export_positive",
        sign_convention_b="export_positive",
    )
    check(
        "endpoint_agreement_beyond_tolerance",
        beyond["comparison"]["within_tolerance"] is False
        and abs(beyond["comparison"]["max_abs_delta"] - 0.2) < 1e-12,
        "expected max delta 0.2 MW (|3.2-3.0|) beyond the 0.1 MW "
        "tolerance — reported as disagreement, tolerance never fitted",
    )

    # ---- Test 3: structural separation inside the produced record -------
    # The record from Test 1 must pass the in-code X-5 enforcement: the
    # comparison block carries deltas only and each series sits in its
    # own declared-class block.
    try:
        assert_structural_separation(within)
        check(
            "structural_separation_record_passes",
            within["series_blocks"][0]["model_class"] == "steady_state"
            and within["series_blocks"][1]["model_class"] == "dynamic"
            and "values" not in within["comparison"]
            and "per_sample_abs_deltas" in within["comparison"],
            "record passes: two declared-class blocks + a deltas-only "
            "comparison block (X-5 enforced in code)",
        )
    except CrossCheckError as error:
        check("structural_separation_record_passes", False,
              "the well-formed record must pass, got %r" % error)

    # ---- Test 4: merged series refused (X-5, in code) -------------------
    doctored = {"pair_id": "CC-4", "merged_series": [1.0, 2.0]}
    try:
        assert_structural_separation(doctored)
        check("merged_series_refused", False, "the merged key must be refused")
    except CrossCheckError as error:
        check(
            "merged_series_refused",
            str(error).startswith("MERGED_SERIES_REFUSED"),
            "a merged/averaged series key is refused: %s" % str(error).split(":")[0],
        )

    # ---- Test 5: values inside the comparison block refused (X-5) -------
    doctored_comparison = {"pair_id": "CC-4",
                           "comparison": {"values": [1.0, 2.0]}}
    try:
        assert_structural_separation(doctored_comparison)
        check("comparison_carries_values_refused", False,
              "values inside the comparison block must be refused")
    except CrossCheckError as error:
        check(
            "comparison_carries_values_refused",
            str(error).startswith("COMPARISON_CARRIES_VALUES_REFUSED"),
            "the comparison block is deltas-only by construction",
        )

    # ---- Test 6: undeclared model class refused (X-5) -------------------
    undeclared = {"pair_id": "CC-4",
                  "series_blocks": [{"label": "x", "values": [1.0]}]}
    try:
        assert_structural_separation(undeclared)
        check("class_declaration_required", False,
              "a series block without model_class must be refused")
    except CrossCheckError as error:
        check(
            "class_declaration_required",
            str(error).startswith("CLASS_DECLARATION_REQUIRED"),
            "every series block must declare its model class",
        )

    # ---- Test 7: mismatched time base refused, never interpolated -------
    # The dynamic series' grid is [0, 3600, 10800] s vs the schedule's
    # [0, 3600, 7200] s: the third timestamps differ (7200 != 10800) →
    # TIME_BASE_MISMATCH, comparison refused.
    try:
        endpoint_agreement(
            shared_times, schedule, [0.0, 3600.0, 10800.0], dynamic_endpoints,
            tolerance=0.1,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        )
        check("time_base_mismatch_refused", False,
              "a mismatched time base must be refused")
    except CrossCheckError as error:
        check(
            "time_base_mismatch_refused",
            str(error).startswith("TIME_BASE_MISMATCH"),
            "comparison on non-identical grids is refused, never "
            "interpolated (interpolation would fabricate samples)",
        )

    # ---- Test 8: no comparison without a pre-registered tolerance (X-8) -
    try:
        endpoint_agreement(
            shared_times, schedule, shared_times, dynamic_endpoints,
            tolerance=None,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        )
        check("threshold_not_pre_registered", False,
              "a None tolerance must be refused")
    except CrossCheckError as error:
        check(
            "threshold_not_pre_registered",
            str(error).startswith("THRESHOLD_NOT_PRE_REGISTERED"),
            "no verdict exists without a pre-registered tolerance (X-8)",
        )

    # ---- Test 9: declared sign convention must agree (§10.2) ------------
    # Control: matching conventions ("export_positive" on both sides —
    # the same comparison as Test 1) must be ACCEPTED.
    try:
        endpoint_agreement(
            shared_times, schedule, shared_times, dynamic_endpoints,
            tolerance=0.1,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        )
        same_ok = True
    except CrossCheckError:
        same_ok = False
    # Mismatch case: one side declares "load_positive" — REFUSED, because
    # a delta between differently-signed values is meaningless (§10.2:
    # the sign is declared per grid model and device profile, never
    # assumed).
    try:
        endpoint_agreement(
            shared_times, schedule, shared_times, dynamic_endpoints,
            tolerance=0.1,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="load_positive",
        )
        mismatch_raised = False
    except CrossCheckError as error:
        mismatch_raised = str(error).startswith("SIGN_CONVENTION_MISMATCH")
    check(
        "sign_convention_mismatch_refused",
        same_ok and mismatch_raised,
        "§10.2: the sign convention is DECLARED and a comparison across "
        "different declared conventions is refused; matching "
        "conventions are accepted",
    )

    # ---- Test 10: vector agreement within tolerance (CC-3) --------------
    # Three buses; pandapower PF voltages vs ANDES PF voltages (pu):
    #   pp  = [1.000, 0.980, 1.020]
    #   an  = [1.000, 0.981, 1.020]
    # HAND ARITHMETIC: deltas = [0.000, 0.001, 0.000] pu → max 0.001 pu
    # at bus_2; pre-registered tolerance 0.005 pu → within tolerance.
    buses = ["bus_1", "bus_2", "bus_3"]
    v_pp = [1.000, 0.980, 1.020]
    v_an = [1.000, 0.981, 1.020]
    vec_within = vector_agreement(
        buses, v_pp, buses, v_an,
        tolerance=0.005,
        label_a="pandapower PF bus voltages",
        label_b="ANDES steady-state PF bus voltages",
        class_a="steady_state", class_b="steady_state",
        quantity="voltage_pu", sign_convention_a="nominal_base",
        sign_convention_b="nominal_base",
    )
    check(
        "vector_agreement_within_tolerance",
        vec_within["comparison"]["within_tolerance"] is True
        and abs(vec_within["comparison"]["max_abs_delta"] - 0.001) < 1e-12
        and vec_within["comparison"]["max_delta_element"] == "bus_2"
        and vec_within["comparison"]["per_element_abs_deltas"] == [0.0, 0.001, 0.0],
        "expected deltas [0.000, 0.001, 0.000] pu by hand, max 0.001 pu "
        "at bus_2, within the pre-registered 0.005 pu tolerance",
    )

    # ---- Test 11: vector agreement beyond tolerance ----------------------
    # ANDES PF voltages [1.000, 0.990, 1.020]: HAND deltas
    # [0.000, 0.010, 0.000] → max 0.01 pu > 0.005 pu → beyond; recorded,
    # to be EXPLAINED (model conventions), never absorbed (X-8).
    vec_beyond = vector_agreement(
        buses, v_pp, buses, [1.000, 0.990, 1.020],
        tolerance=0.005,
        label_a="pandapower PF bus voltages",
        label_b="ANDES steady-state PF bus voltages",
        class_a="steady_state", class_b="steady_state",
        quantity="voltage_pu", sign_convention_a="nominal_base",
        sign_convention_b="nominal_base",
    )
    check(
        "vector_agreement_beyond_tolerance",
        vec_beyond["comparison"]["within_tolerance"] is False
        and abs(vec_beyond["comparison"]["max_abs_delta"] - 0.01) < 1e-12,
        "expected max delta 0.01 pu beyond the 0.005 pu tolerance; "
        "recorded and to be explained, never absorbed by widening",
    )

    # ---- Test 12: index-base mismatch refused, never silently aligned ---
    try:
        vector_agreement(
            buses, v_pp, ["bus_1", "bus_3", "bus_2"], v_an,
            tolerance=0.005,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="steady_state",
            quantity="voltage_pu", sign_convention_a="nominal_base",
            sign_convention_b="nominal_base",
        )
        check("index_base_mismatch_refused", False,
              "a reordered index base must be refused")
    except CrossCheckError as error:
        check(
            "index_base_mismatch_refused",
            str(error).startswith("INDEX_BASE_MISMATCH"),
            "a reordered/partially different index list is refused — a "
            "silent alignment could compare a voltage against the wrong "
            "bus",
        )

    # ---- Test 13: declared time steps consistent (CC-1) -----------------
    consistent = federate_time_step_check([
        {"name": "flexcore_asset", "declared_time_step_seconds": 1.0},
        {"name": "flexcore_grid", "declared_time_step_seconds": 1.0},
        {"name": "flexcore_research", "declared_time_step_seconds": 1.0},
    ])
    check(
        "federate_time_steps_consistent",
        consistent["time_step_consistent"] is True
        and consistent["mismatch_detected"] is False,
        "all declared steps 1.0 s → consistent (transport-level fact)",
    )

    # ---- Test 14: declared time-step MISMATCH detected (CC-1) -----------
    # flexcore_grid declares 2.0 s while the others declare 1.0 s.
    mismatched = federate_time_step_check([
        {"name": "flexcore_asset", "declared_time_step_seconds": 1.0},
        {"name": "flexcore_grid", "declared_time_step_seconds": 2.0},
        {"name": "flexcore_research", "declared_time_step_seconds": 1.0},
    ])
    check(
        "federate_time_step_mismatch_detected",
        mismatched["mismatch_detected"] is True
        and mismatched["time_step_consistent"] is False
        and mismatched["mismatching_federates"] == ["flexcore_grid"]
        and mismatched["verdict"].startswith("INTERACTION_CLAIM_REFUSED"),
        "mismatch {1.0, 2.0} detected with flexcore_grid named and the "
        "interaction claim refused — never silently absorbed",
    )

    # ---- Test 15: granted times synchronized, no dropout (CC-2) ---------
    synced = granted_time_check(
        [
            {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
             "granted_step_count": 3600},
            {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
             "granted_step_count": 3600},
        ],
        scenario_end_time_s=3600.0,
    )
    check(
        "granted_times_synchronized_no_dropout",
        synced["synchronization_consistent"] is True
        and synced["all_federates_completed"] is True
        and synced["dropouts"] == [],
        "both federates completed to 3600.0 s = scenario end → "
        "synchronized, no dropout",
    )

    # ---- Test 16: federate DROPOUT detected and named (CC-2) -------------
    # flexcore_research stops at 1800.0 s of a 3600.0 s scenario (and its
    # final granted time differs from the others').
    dropout = granted_time_check(
        [
            {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
             "granted_step_count": 3600},
            {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
             "granted_step_count": 3600},
            {"name": "flexcore_research", "final_granted_time_s": 1800.0,
             "granted_step_count": 1800},
        ],
        scenario_end_time_s=3600.0,
    )
    check(
        "federate_dropout_detected_and_named",
        dropout["all_federates_completed"] is False
        and dropout["synchronization_consistent"] is False
        and len(dropout["dropouts"]) == 1
        and dropout["dropouts"][0]["federate"] == "flexcore_research"
        and dropout["verdict"].startswith("INTERACTION_CLAIM_REFUSED"),
        "the mid-run stop is DETECTED and the federate NAMED — never "
        "silently treated as 'no change'",
    )

    # ---- Test 17: proposal-only boundary on the assembled record --------
    # A record carrying an authorization-shaped key must be refused
    # (ADR-010, in code).
    try:
        build_crosscheck_record(
            "CC-4",
            [within],
            provenance={"authorization_token": "smuggled"},
        )
        check("proposal_only_refused", False,
              "an authorization-shaped key must be refused")
    except CrossCheckError as error:
        check(
            "proposal_only_refused",
            str(error).startswith("PROPOSAL_ONLY_REFUSED"),
            "the record type cannot express an authorization token "
            "(ADR-010)",
        )

    # ---- Test 18: well-formed record builds and carries the statements --
    record = build_crosscheck_record("CC-4", [within])
    check(
        "record_builds_with_boundary_statements",
        record["protocol"] == PROTOCOL_ID
        and record["result_class"] == "agreement_evidence_only"
        and len(record["boundary_statements"]) == len(BOUNDARY_STATEMENTS)
        and "AGREEMENT EVIDENCE ON THE OVERLAPPING DOMAIN" in record["boundary_statements"][0],
        "the assembled record carries the protocol tag, the "
        "agreement-evidence-only class, and the boundary statements "
        "verbatim",
    )

    # ---- Test 19: fail-closed input validation --------------------------
    def expects_error(name: str, fn, prefix: str, detail: str) -> None:
        try:
            fn()
        except CrossCheckError as error:
            check(name, str(error).startswith(prefix), detail)
            return
        except Exception as error:  # noqa: BLE001 — pin the error class
            check(name, False, "%s; raised %r instead of CrossCheckError" % (detail, error))
            return
        check(name, False, "%s; raised nothing" % detail)

    expects_error(
        "length_mismatch_refused",
        lambda: endpoint_agreement(
            [0.0, 3600.0], [2.0, 3.0], shared_times, dynamic_endpoints,
            tolerance=0.1, label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        ),
        "TIME_BASE_MISMATCH",
        "different-length series are refused (2 vs 3 samples) — never "
        "padded or resampled",
    )
    expects_error(
        "nonfinite_value_refused",
        lambda: endpoint_agreement(
            shared_times, [2.0, float("nan"), -1.0], shared_times, dynamic_endpoints,
            tolerance=0.1, label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        ),
        "series_a",
        "a NaN value is refused before any delta is computed",
    )
    expects_error(
        "unknown_quantity_refused",
        lambda: endpoint_agreement(
            shared_times, schedule, shared_times, dynamic_endpoints,
            tolerance=0.1, label_a="a", label_b="b",
            class_a="steady_state", class_b="dynamic",
            quantity="horsepower", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        ),
        "QUANTITY_UNKNOWN",
        "an undeclared quantity is refused (units never assumed)",
    )
    expects_error(
        "unknown_model_class_refused",
        lambda: endpoint_agreement(
            shared_times, schedule, shared_times, dynamic_endpoints,
            tolerance=0.1, label_a="a", label_b="b",
            class_a="transient", class_b="dynamic",
            quantity="p_mw", sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        ),
        "MODEL_CLASS_UNKNOWN",
        "a class outside the closed vocabulary is refused",
    )
    expects_error(
        "negative_tolerance_refused",
        lambda: vector_agreement(
            buses, v_pp, buses, v_an,
            tolerance=-0.005,
            label_a="a", label_b="b",
            class_a="steady_state", class_b="steady_state",
            quantity="voltage_pu", sign_convention_a="nominal_base",
            sign_convention_b="nominal_base",
        ),
        "TOLERANCE_INVALID",
        "a negative tolerance is refused",
    )
    expects_error(
        "single_federate_refused",
        lambda: federate_time_step_check(
            [{"name": "flexcore_asset", "declared_time_step_seconds": 1.0}]
        ),
        "FEDERATION_NEEDS_TWO_FEDERATES",
        "a one-federate 'federation' has nothing to agree with",
    )
    expects_error(
        "duplicate_federate_refused",
        lambda: granted_time_check(
            [
                {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
            ]
        ),
        "FEDERATE_NAME_DUPLICATE",
        "a duplicated federate record would double-count it",
    )
    expects_error(
        "zero_step_count_refused",
        lambda: granted_time_check(
            [
                {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_grid", "final_granted_time_s": 0.0,
                 "granted_step_count": 0},
            ],
            scenario_end_time_s=3600.0,
        ),
        "STEP_COUNT_INVALID",
        "a zero-step report is invalid — a real dropout is recorded in "
        "the scenario record, never smuggled in as a 'valid' report",
    )

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
