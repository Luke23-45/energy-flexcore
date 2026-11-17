# P15.02 — The forecast baseline worker: an isolated, proposal-only,
#         stdlib-only, guarded-deps Class C worker skeleton.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS WORKER WAS WRITTEN, NEVER EXECUTED. The pure-logic core
#    (persistence, seasonal-naive, degree-day regression) is stdlib-only
#    and imports and self-tests with NOTHING installed; the numerical
#    framework (numpy) is a guarded import INSIDE its function raising
#    the NAMED DECISION-POINT error below — the gnn_worker pattern
#    (apps/worker/python/gnn_worker). No baseline output produced by any
#    run of this file is a skill claim: the worker labels every output
#    "no skill until evaluated" and the P15.05 evaluation framework owns
#    every result cell (all EMPTY).
# 2. WHAT THE WORKER IS FOR: the statistical load/PV baseline forecasts
#    the P15.02 design freezes (persistence.v1, seasonal-naive.v1,
#    degree-day.v1) as deterministic reference forecasts — the baselines
#    every future model must beat to claim skill. The Rust-side rule
#    sets are the single home (crates/flexcore-application/
#    forecast_baselines.rs); this worker mirrors them for the
#    out-of-process execution seam and pins the mirror by identical
#    hand-computed test vectors.
# 2b. P15.17 COMPLETION: the worker additionally serves mode
#    `publish_forecast_record` — the same frozen baseline run wrapped as
#    a provenance-stamped forecast-artifact RECORD (the P15.01
#    provenance layer mirrored: required input/model/eval-set links
#    validated before computation, a loudly-labelled test-grade FNV-1a 64
#    integrity digest, UNSIGNED — no signing identity exists). A
#    publication RECORD is an input to scheduling analytics, never a
#    command, and the digest is explicitly NOT a security primitive.
# 3. CONTROL BOUNDARY (absolute; ADR-010 extension, program conventions
#    6.3): this worker has NO field-write path and NO device channel —
#    its only output is ONE JSON document on stdout. It never issues,
#    approves, or carries a command; every output is a PROPOSAL/RECORD;
#    assert_proposal_only() scans the result tree for authorization/
#    command-shaped keys and refuses emission. Mode-locked to
#    simulation/shadow (a constant; no request field can change it).
#    The worker's declared permission vocabulary is exactly the forecast
#    producer's: telemetry.read + forecast.publish — nothing else, and
#    the request cannot add a permission.
# 4. PROVENANCE: every output carries the baseline version, the
#    model-registry reference (UNTRAINED — P15.06 makes UNTRAINED a
#    first-class registered state), the frozen eval-set reference
#    (forecast-eval-set-v1, P15.05), the input snapshot id, and the
#    request id. A request without provenance is refused BEFORE any
#    computation (fail-closed).
# 5. SCOPE DISCIPLINE: a request whose declared permissions exceed the
#    producer vocabulary, whose series values are non-finite, or whose
#    baseline is unknown is REFUSED with a named error and NOTHING on
#    stdout (exit 2). There is no default baseline and no degraded-mode
#    forecast.
# 6. TRANSPORT CONTRACT (mirrors ../gnn_worker/gnn_inference_worker.py):
#    ONE JSON document on stdin (the `flexcore-forecast-worker/1`
#    request), ONE JSON document on stdout (the result envelope).
#    Malformed request → NOTHING on stdout, reason on stderr, exit 2.
#    NO accuracy, latency, or skill figure exists anywhere in this file:
#    every measured cell belongs to the P15.05 human/lab gate and is
#    EMPTY here.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import math
import sys

# The worker-transport protocol tag (mirrors `flexcore-gnn-worker/1`;
# validated on every request, fail-closed).
PROTOCOL_ID = "flexcore-forecast-worker/1"

WORKER_VERSION = "flexcore-forecast-worker/0.2.0"

# The closed mode vocabulary (P15.17 completion): `baseline_forecast`
# (the P15.02 skeleton contract, unchanged) and `publish_forecast_record`
# (the P15.01 publication-record seam — the same baseline run wrapped as
# a provenance-stamped forecast-artifact RECORD).
MODE_BASELINE = "baseline_forecast"
MODE_PUBLISH = "publish_forecast_record"
MODES: frozenset = frozenset({MODE_BASELINE, MODE_PUBLISH})

# Control-authority mode this worker is LOCKED to (program conventions
# 6.7). A constant, not a setting: no request field can change it, no
# plugin can select a mode.
MODE_LOCKED = "simulation-shadow"

# The closed baseline vocabulary (the P15.02 frozen registry's ids,
# mirrored; a request outside it is refused, never defaulted).
BASELINE_PERSISTENCE = "forecast-baseline/persistence"
BASELINE_SEASONAL_NAIVE = "forecast-baseline/seasonal-naive"
BASELINE_DEGREE_DAY = "forecast-baseline/degree-day-regression"
BASELINES: frozenset = frozenset(
    {BASELINE_PERSISTENCE, BASELINE_SEASONAL_NAIVE, BASELINE_DEGREE_DAY}
)

# The baseline versions the rule sets carry (the frozen-version
# discipline: the result names its rule set).
BASELINE_VERSIONS: dict = {
    BASELINE_PERSISTENCE: "persistence.v1",
    BASELINE_SEASONAL_NAIVE: "seasonal-naive.v1",
    BASELINE_DEGREE_DAY: "degree-day.v1",
}

# The forecast producer's permission vocabulary (P15.01; the
# forecast-worker manifest's classes — NO dispatch permission). A
# request declaring permissions outside this set is refused.
PRODUCER_PERMISSIONS: frozenset = frozenset({"telemetry.read", "forecast.publish"})

# The NEVER-GRANTED permission set, mirrored VERBATIM from
# crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS
# (alignment seam; a change on either side is a deliberate, recorded
# seam update). A request declaring any of these is refused first.
NEVER_GRANTED_PERMISSIONS: frozenset = frozenset(
    {"dispatch.authorize", "dispatch.issue", "secret.read", "network.any"}
)

# The closed baseline-rule error vocabulary.
class WorkerRequestError(ValueError):
    """A malformed forecast request — fail-closed BEFORE any computation."""


# Keys that must NEVER appear anywhere in a result document (the in-code
# proposal-only boundary; ADR-010 extension).
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
        "approved",
        "approval_token",
    }
)

# The boundary statements carried VERBATIM in every result envelope.
BOUNDARY_STATEMENTS: tuple = (
    "A baseline forecast is an INPUT to scheduling analytics, never a "
    "command: no forecast artifact, model, or score this worker produces "
    "can construct or influence a dispatch command (ADR-010 extension, "
    "program conventions 6.3 — structural, not conventional).",
    "NO baseline claims skill: skill exists only as a result of an "
    "X-8-gated evaluation run against the persistence reference on a "
    "frozen eval set (P15.05); every evaluation cell is EMPTY.",
    "The model registry state of every baseline is UNTRAINED (P15.06 "
    "first-class registered state); the P3.10 SYNTHETIC_BASELINE_"
    "DISCLAIMER is superseded only by EVALUATED baselines, never "
    "silently.",
    "This worker's outputs are research PROPOSALS and RECORDS only "
    "(ADR-010 extension): no authorization token exists in this "
    "response type and nothing here can be issued to a device.",
    "This worker is mode-locked to simulation/shadow; it cannot select "
    "or raise a control-authority mode (program conventions 6.7).",
    "The worker's declared permission vocabulary is exactly "
    "telemetry.read + forecast.publish (the forecast-worker manifest's "
    "classes); no request can add a permission.",
)

# The frozen eval-set and registry references the outputs cite (P15.05 /
# P15.06; the version ids are the frozen artifacts, never recomputed here).
EVAL_SET_REF = "benchmarks/forecast-eval-set-v1.json (forecast-eval-set-v1)"
MODEL_REGISTRY_REF = "flexcore-application/model-registry.v1 (UNTRAINED)"

NO_SKILL_NOTE = (
    "NO-SKILL-UNTIL-EVALUATED: this output is a deterministic baseline "
    "forecast (the reference other models are compared against), not a "
    "validated forecast. No accuracy, error, or skill figure is attached "
    "or claimed; the P15.05 evaluation framework owns every result cell "
    "(all EMPTY)."
)

NUMPY_DEPLOYMENT_REQUIREMENT = (
    "flexcore forecast worker numpy path requires numpy: NOT INSTALLED "
    "and the numerical framework is a recorded DECISION-POINT for the "
    "forecast worker (P15.02 design; the stdlib path covers the three "
    "frozen baselines). Pin a verified version into an ISOLATED research "
    "environment (never the platform runtime) only if a baseline rule "
    "set ever requires it; the frozen rule sets are pure Python."
)


class ForecastComputationError(ValueError):
    """A baseline rule refused its inputs — an in-band refusal that is a
    RESULT envelope (termination "failed"), never a crash."""


# ---------------------------------------------------------------------------
# Guarded framework import — the single numpy choke point (the honesty
# gate: without numpy the stdlib path still runs; this function exists so
# a future framework-dependent rule set fails NAMED, never implicitly).
# ---------------------------------------------------------------------------


def import_numpy():
    """Imports `numpy` or raises the NAMED DECISION-POINT requirement.

    The import lives INSIDE this function so the rest of the module
    parses, imports, and self-tests with nothing installed (the
    gnn_worker guarded-import pattern). The frozen baseline rule sets
    below are pure Python and never call this function.
    """
    try:
        import numpy  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as error:
        raise ImportError(NUMPY_DEPLOYMENT_REQUIREMENT) from error
    return numpy


# ---------------------------------------------------------------------------
# The baseline rule sets (mirrors of crates/flexcore-application/
# forecast_baselines.rs — pinned by identical hand-computed test vectors;
# the Rust side is the single home, this is the out-of-process mirror).
# ---------------------------------------------------------------------------


def _validate_finite_series(values, where: str) -> list:
    validated = []
    for index, value in enumerate(values):
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise WorkerRequestError("%s[%d] must be a finite number" % (where, index))
        if not math.isfinite(value):
            raise WorkerRequestError("%s[%d] must be finite" % (where, index))
        validated.append(float(value))
    return validated


def persistence_forecast(history, horizon_steps) -> list:
    """persistence.v1: forecast(t) = last observed value, carried forward
    unchanged for every horizon step. Mirrors the Rust rule set."""
    if not isinstance(horizon_steps, int) or isinstance(horizon_steps, bool):
        raise WorkerRequestError("request.horizon.steps must be an integer")
    if horizon_steps < 1:
        raise ForecastComputationError(
            "the horizon must ask for at least 1 step (a forecast of nothing is a "
            "formulation error)"
        )
    series = _validate_finite_series(history, "request.history")
    if not series:
        raise ForecastComputationError(
            "persistence requires at least one observed value; an empty history is "
            "refused, never zero-filled"
        )
    return [series[-1]] * horizon_steps


def seasonal_naive_forecast(history, season_length, horizon_steps) -> list:
    """seasonal-naive.v1: forecast(t) = the value exactly one seasonal
    period earlier; longer horizons wrap within the LAST season in the
    history (the declared wrap rule). Mirrors the Rust rule set."""
    if not isinstance(horizon_steps, int) or isinstance(horizon_steps, bool):
        raise WorkerRequestError("request.horizon.steps must be an integer")
    if not isinstance(season_length, int) or isinstance(season_length, bool):
        raise WorkerRequestError("request.season_length must be an integer")
    if horizon_steps < 1:
        raise ForecastComputationError(
            "the horizon must ask for at least 1 step"
        )
    if season_length < 1:
        raise ForecastComputationError("the season length must be at least 1 step")
    series = _validate_finite_series(history, "request.history")
    if len(series) < season_length + 1:
        raise ForecastComputationError(
            "seasonal-naive requires a history of at least season_length + 1 = %d "
            "values; got %d" % (season_length + 1, len(series))
        )
    n = len(series)
    return [series[n - season_length + (step % season_length)] for step in range(horizon_steps)]


def solve_3x3_gaussian_first_max_pivot(matrix, rhs) -> list:
    """Deterministic Gaussian elimination with first-max partial
    pivoting; a pivot below the frozen epsilon is a named refusal (the
    Rust rule set is the single home of the epsilon)."""
    a = [list(row) for row in matrix]
    b = list(rhs)
    epsilon = 1e-12
    for col in range(3):
        pivot_row, pivot_mag = col, abs(a[col][col])
        for row in range(col + 1, 3):
            if abs(a[row][col]) > pivot_mag:
                pivot_row, pivot_mag = row, abs(a[row][col])
        if pivot_mag < epsilon:
            raise ForecastComputationError(
                "the degree-day design matrix is singular (column %d pivot magnitude "
                "%r below the frozen epsilon %r); the rule refuses, never pseudo-inverts"
                % (col, pivot_mag, epsilon)
            )
        a[col], a[pivot_row] = a[pivot_row], a[col]
        b[col], b[pivot_row] = b[pivot_row], b[col]
        for row in range(col + 1, 3):
            factor = a[row][col] / a[col][col]
            for j in range(col, 3):
                a[row][j] -= factor * a[col][j]
            b[row] -= factor * b[col]
    x = [0.0, 0.0, 0.0]
    for i in (2, 1, 0):
        acc = b[i]
        for j in range(i + 1, 3):
            acc -= a[i][j] * x[j]
        x[i] = acc / a[i][i]
    return x


def _degree_day_sample(sample, where: str) -> tuple:
    if not isinstance(sample, dict):
        raise WorkerRequestError("%s must be an object" % where)
    load = sample.get("load_mw")
    hdd = sample.get("heating_degree_days")
    cdd = sample.get("cooling_degree_days")
    values = _validate_finite_series([load, hdd, cdd], where)
    if values[1] < 0.0 or values[2] < 0.0:
        raise ForecastComputationError(
            "%s: degree days are counts above a base temperature and are never "
            "negative" % where
        )
    return (values[0], values[1], values[2])


def fit_degree_day_regression(samples) -> tuple:
    """degree-day.v1 fit: OLS of load on (intercept, HDD, CDD) via the
    normal equations. Mirrors the Rust rule set."""
    if not isinstance(samples, list) or len(samples) < 3:
        raise ForecastComputationError(
            "the degree-day fit requires at least 3 training samples; got %d"
            % (len(samples) if isinstance(samples, list) else 0)
        )
    parsed = [
        _degree_day_sample(s, "request.degree_day.training_samples[%d]" % i)
        for i, s in enumerate(samples)
    ]
    m = [[0.0] * 3 for _ in range(3)]
    rhs = [0.0, 0.0, 0.0]
    for load, hdd, cdd in parsed:
        row = (1.0, hdd, cdd)
        for i in range(3):
            for j in range(3):
                m[i][j] += row[i] * row[j]
            rhs[i] += row[i] * load
    coefficients = solve_3x3_gaussian_first_max_pivot(m, rhs)
    return tuple(coefficients)


def degree_day_predict(coefficients, forecast_hdd, forecast_cdd) -> list:
    """degree-day.v1 predict: linear prediction in the training data's
    units and sign convention. Mirrors the Rust rule set."""
    if len(forecast_hdd) != len(forecast_cdd):
        raise ForecastComputationError(
            "the forecast HDD vector (%d values) and CDD vector (%d values) must "
            "have the same length" % (len(forecast_hdd), len(forecast_cdd))
        )
    if not forecast_hdd:
        raise ForecastComputationError("the horizon must ask for at least 1 step")
    hdd = _validate_finite_series(forecast_hdd, "request.degree_day.forecast_hdd")
    cdd = _validate_finite_series(forecast_cdd, "request.degree_day.forecast_cdd")
    for values, where in ((hdd, "forecast_hdd"), (cdd, "forecast_cdd")):
        for index, value in enumerate(values):
            if value < 0.0:
                raise ForecastComputationError(
                    "%s[%d] is %r; degree days are never negative" % (where, index, value)
                )
    intercept, hdd_coef, cdd_coef = coefficients
    return [intercept + hdd_coef * h + cdd_coef * c for h, c in zip(hdd, cdd)]


# ---------------------------------------------------------------------------
# Pure request validation (no I/O beyond the given dict)
# ---------------------------------------------------------------------------


def _require_nonempty_str(container: dict, field: str, where: str) -> str:
    value = container.get(field)
    if not isinstance(value, str) or not value.strip():
        raise WorkerRequestError("%s.%s must be a non-empty string" % (where, field))
    return value


def validate_request(request) -> dict:
    """Validates the forecast request; raises WorkerRequestError on
    refusal. Refusals happen BEFORE any computation. Returns the
    validated fields, normalized."""
    if not isinstance(request, dict):
        raise WorkerRequestError("request must be a JSON object")
    if request.get("protocol") != PROTOCOL_ID:
        raise WorkerRequestError(
            "request.protocol %r is not %r" % (request.get("protocol"), PROTOCOL_ID)
        )
    mode = request.get("mode")
    if mode not in MODES:
        raise WorkerRequestError(
            "request.mode %r is not one of %s (baseline_forecast, or "
            "publish_forecast_record for the P15.01 publication-record seam)"
            % (mode, sorted(MODES))
        )

    baseline = request.get("baseline")
    if baseline not in BASELINES:
        raise WorkerRequestError(
            "request.baseline %r is not one of %s (the frozen registry is closed; "
            "there is no default baseline)" % (baseline, sorted(BASELINES))
        )

    # The permission gate (P15.01): the request may DECLARE only the
    # producer vocabulary; the never-granted set is checked FIRST. The
    # declaration is validated, never honored as a grant — the worker's
    # own permission vocabulary is fixed.
    declared = request.get("permissions_declared", [])
    if not isinstance(declared, list):
        raise WorkerRequestError("request.permissions_declared must be a list")
    for permission in declared:
        if permission in NEVER_GRANTED_PERMISSIONS:
            raise WorkerRequestError(
                "request.permissions_declared names %r, which is NEVER granted to "
                "any plugin — not grantable through any manifest, profile, or "
                "worker request" % (permission,)
            )
        if permission not in PRODUCER_PERMISSIONS:
            raise WorkerRequestError(
                "request.permissions_declared names %r, which is outside the "
                "forecast-producer vocabulary %s (ADR-010 extension: a forecast "
                "producer has no dispatch reach)"
                % (permission, sorted(PRODUCER_PERMISSIONS))
            )

    scope = request.get("scope")
    if not isinstance(scope, dict):
        raise WorkerRequestError(
            "request.scope is required (scope_kind, scope_id)"
        )
    scope_kind = scope.get("scope_kind")
    if scope_kind not in ("site", "portfolio"):
        raise WorkerRequestError(
            "request.scope.scope_kind %r is not one of ['portfolio', 'site']"
            % (scope_kind,)
        )
    scope_id = _require_nonempty_str(scope, "scope_id", "request.scope")

    horizon = request.get("horizon")
    if not isinstance(horizon, dict):
        raise WorkerRequestError("request.horizon is required (steps, interval_millis)")
    horizon_steps = horizon.get("steps")
    if not isinstance(horizon_steps, int) or isinstance(horizon_steps, bool) or horizon_steps < 1:
        raise WorkerRequestError("request.horizon.steps must be a positive integer")
    interval_millis = horizon.get("interval_millis")
    if (
        not isinstance(interval_millis, int)
        or isinstance(interval_millis, bool)
        or interval_millis < 1
    ):
        raise WorkerRequestError("request.horizon.interval_millis must be a positive integer")

    history = request.get("history")
    if not isinstance(history, list):
        raise WorkerRequestError(
            "request.history is required (a list of finite numbers in the "
            "caller's declared sign convention)"
        )

    season_length = request.get("season_length")
    degree_day = request.get("degree_day")
    if baseline == BASELINE_SEASONAL_NAIVE:
        if not isinstance(season_length, int) or isinstance(season_length, bool) or season_length < 1:
            raise WorkerRequestError(
                "request.season_length must be a positive integer for the "
                "seasonal-naive baseline (the season is declared, never inferred)"
            )
    if baseline == BASELINE_DEGREE_DAY:
        if not isinstance(degree_day, dict):
            raise WorkerRequestError(
                "request.degree_day is required for the degree-day baseline "
                "(training_samples, forecast_hdd, forecast_cdd)"
            )
        if not isinstance(degree_day.get("training_samples"), list):
            raise WorkerRequestError(
                "request.degree_day.training_samples must be a list"
            )
        if not isinstance(degree_day.get("forecast_hdd"), list) or not isinstance(
            degree_day.get("forecast_cdd"), list
        ):
            raise WorkerRequestError(
                "request.degree_day.forecast_hdd and forecast_cdd must be lists"
            )

    publication = request.get("publication")
    if mode == MODE_PUBLISH:
        # The P15.01 publication-record seam (mirrored from
        # forecast_architecture.rs): the provenance record's REQUIRED
        # links travel on the request — a record without them is refused
        # BEFORE any computation, so provenance cannot silently detach
        # from the record (the fail-closed mirror discipline).
        if not isinstance(publication, dict):
            raise WorkerRequestError(
                "request.publication is required for publish_forecast_record "
                "(forecast_version, artifact_kind, input_data_ref, "
                "producing_model_ref, eval_set_ref)"
            )
        forecast_version = _require_nonempty_str(
            publication, "forecast_version", "request.publication"
        )
        if any(c.isspace() for c in forecast_version):
            raise WorkerRequestError(
                "request.publication.forecast_version must be whitespace-free "
                "(the ForecastVersion validation mirrored)"
            )
        artifact_kind = publication.get("artifact_kind")
        if artifact_kind not in ("load", "pv_generation", "price"):
            raise WorkerRequestError(
                "request.publication.artifact_kind %r is not one of "
                "['load', 'price', 'pv_generation'] (the P15.01 closed "
                "artifact-kind vocabulary)" % (artifact_kind,)
            )
        for link in ("input_data_ref", "producing_model_ref", "eval_set_ref"):
            _require_nonempty_str(publication, link, "request.publication")

    provenance = request.get("provenance")
    if not isinstance(provenance, dict):
        raise WorkerRequestError(
            "request.provenance is required (input_snapshot_id, request_id)"
        )
    input_snapshot_id = _require_nonempty_str(
        provenance, "input_snapshot_id", "request.provenance"
    )
    request_id = _require_nonempty_str(provenance, "request_id", "request.provenance")

    return {
        "mode": mode,
        "baseline": baseline,
        "permissions_declared": list(declared),
        "scope": {"scope_kind": scope_kind, "scope_id": scope_id},
        "horizon": {
            "steps": int(horizon_steps),
            "interval_millis": int(interval_millis),
        },
        "history": history,
        "season_length": season_length,
        "degree_day": degree_day,
        "publication": publication,
        "input_snapshot_id": input_snapshot_id,
        "request_id": request_id,
    }


# ---------------------------------------------------------------------------
# Pure result envelope (proposal-only by construction)
# ---------------------------------------------------------------------------


def assert_proposal_only(document) -> None:
    """Refuses to emit any document carrying a forbidden key.

    The in-code proposal-only boundary: scans EVERY mapping key in the
    document tree (recursively) against FORBIDDEN_RESULT_KEYS. A result
    that somehow grew an authorization/command-shaped field is refused
    (RuntimeError) instead of emitted — the response TYPE cannot express
    an authorization token, and this is the enforced proof, not prose.
    """
    if isinstance(document, dict):
        for key, value in document.items():
            if isinstance(key, str) and key.strip().lower() in FORBIDDEN_RESULT_KEYS:
                raise RuntimeError(
                    "proposal-only boundary violation: result key %r is "
                    "forbidden (ADR-010 extension: a forecast result never "
                    "carries an authorization token or a device command)" % key
                )
            assert_proposal_only(value)
    elif isinstance(document, list):
        for item in document:
            assert_proposal_only(item)


def build_result(*, validated: dict, termination_status: str, outputs, diagnostics: list) -> dict:
    """Builds the `flexcore-forecast-worker/1` result envelope (pure).

    termination_status is "completed" when a frozen baseline rule set ran
    over the declared inputs (deterministic arithmetic — NOT an evaluation
    run and NOT a skill claim); "failed" marks an in-band refusal with
    diagnostics. NO accuracy, error, or skill figure exists in any
    envelope: measured cells belong to the P15.05 human/lab gate and are
    EMPTY here.
    """
    return {
        "protocol": PROTOCOL_ID,
        "kind": "baseline_forecast_result",
        "ok": termination_status == "completed",
        "mode": validated["mode"],
        "payload_type": "flexcore.forecast.baseline_forecast.v1",
        # Proposal-only posture (ADR-010 extension), enforced in code by
        # assert_proposal_only before emission.
        "proposal_only": True,
        "authority": "none",
        "control_authority_mode_locked": MODE_LOCKED,
        "boundary_statements": list(BOUNDARY_STATEMENTS),
        "forecast": {
            "baseline": validated["baseline"],
            "baseline_version": BASELINE_VERSIONS[validated["baseline"]],
            "scope": validated["scope"],
            "horizon": validated["horizon"],
            "outputs": outputs,
        },
        "honesty": {
            "no_skill_note": NO_SKILL_NOTE,
            "model_registry_state": "UNTRAINED",
            "model_registry_ref": MODEL_REGISTRY_REF,
            "eval_set_ref": EVAL_SET_REF,
            "eval_set_status": "every result cell EMPTY (no evaluation run exists)",
            "synthetic_baseline_disclaimer_supersession": (
                "the P3.10 SYNTHETIC_BASELINE_DISCLAIMER is superseded only by "
                "EVALUATED baselines (P15.05), never silently"
            ),
        },
        "permissions_declared": validated["permissions_declared"],
        "worker_permission_vocabulary": sorted(PRODUCER_PERMISSIONS),
        "provenance": {
            "worker_version": WORKER_VERSION,
            "input_snapshot_id": validated["input_snapshot_id"],
            "request_id": validated["request_id"],
            "termination_status": termination_status,
            "diagnostics": list(diagnostics),
        },
        "unverified_note": (
            "UNRUN worker: this file was written, never executed (program "
            "conventions 1). The pure core imports and self-tests with nothing "
            "installed; the first execution belongs to the human build/test pass."
        ),
    }


# ---------------------------------------------------------------------------
# The computation path (pure rule sets; no framework import on this path)
# ---------------------------------------------------------------------------


def run_forecast(request: dict) -> dict:
    """Runs one baseline forecast request (pure; no torch/numpy on this
    path — the frozen rule sets are stdlib arithmetic).

    Order of gates:
      1. pure request validation (refusals before anything else);
      2. the frozen baseline rule set runs; a rule refusal (empty
         history, short season, singular design matrix) is converted into
         the IN-BAND failed envelope — a refusal is a result, not a crash.

    Raises WorkerRequestError (malformed request) only; everything else
    is in-band.
    """
    validated = validate_request(request)
    diagnostics: list = []
    baseline = validated["baseline"]
    horizon_steps = validated["horizon"]["steps"]

    try:
        if baseline == BASELINE_PERSISTENCE:
            values = persistence_forecast(validated["history"], horizon_steps)
        elif baseline == BASELINE_SEASONAL_NAIVE:
            values = seasonal_naive_forecast(
                validated["history"],
                validated["season_length"],
                horizon_steps,
            )
        else:  # BASELINE_DEGREE_DAY
            degree_day = validated["degree_day"]
            coefficients = fit_degree_day_regression(degree_day["training_samples"])
            values = degree_day_predict(
                coefficients,
                degree_day["forecast_hdd"],
                degree_day["forecast_cdd"],
            )
            diagnostics.append(
                "degree-day fit: intercept=%r hdd_coef=%r cdd_coef=%r (deterministic "
                "arithmetic over the declared inputs — NOT a validated model)"
                % coefficients
            )
        termination = "completed"
        outputs = {
            "values": values,
            "steps": len(values),
            "sign_convention": (
                "the caller's declared convention, preserved verbatim (P > 0 = export "
                "at the point of connection; the baseline never flips or normalizes "
                "signs)"
            ),
        }
    except ForecastComputationError as error:
        diagnostics.append("baseline rule refused in-band: %s" % error)
        termination = "failed"
        outputs = None

    return build_result(
        validated=validated,
        termination_status=termination,
        outputs=outputs,
        diagnostics=diagnostics,
    )


# ---------------------------------------------------------------------------
# P15.17 completion — the publication-record seam (mode
# `publish_forecast_record`): the same frozen baseline run, wrapped as a
# provenance-stamped forecast-artifact RECORD. Mirrors the P15.01
# provenance + signing layer (crates/flexcore-application/
# forecast_architecture.rs) BY CITATION: the schema id, the required
# provenance links (input_data_ref, producing_model_ref, eval_set_ref —
# validated in validate_request so they cannot detach), the loudly
# labelled test-grade FNV-1a 64 digest, and the refusal when a signer
# identity is demanded. A publication RECORD is integrity-stamped, NOT
# signed: no signing identity exists in this program, and the digest is
# the test-grade checksum, explicitly NOT a security primitive.
# ---------------------------------------------------------------------------

# The provenance-record schema id, mirrored VERBATIM from the P15.01
# constant FORECAST_PROVENANCE_SCHEMA_ID (alignment seam; recorded).
PROVENANCE_SCHEMA_ID_MIRROR = "flexcore-forecast-artifact-provenance-v1"

# The test-grade digest algorithm tag (mirrors FORECAST_DIGEST_ALGORITHM_
# FNV1A64_TEST — the same spelling the Rust mirror pins).
DIGEST_ALGORITHM_FNV1A64_TEST = "fnv1a64-test"

# The test-grade digest boundary statement (pinned by self-tests).
TEST_GRADE_DIGEST_STATEMENT = (
    "The publication record's digest is the FNV-1a 64 TEST-GRADE checksum "
    "over the record's canonical rendering. It is an integrity stamp for "
    "replay comparison, explicitly NOT a security primitive and NOT a "
    "signature: no signing identity exists in this program, and a "
    "production digest (SHA-256 with a managed key) is a recorded "
    "deployment requirement before any release-grade provenance exists."
)

# The publication boundary statement appended to every publication
# envelope (pinned by self-tests).
PUBLICATION_BOUNDARY_STATEMENT = (
    "A publication RECORD is a provenance-stamped forecast artifact "
    "record: an INPUT to scheduling analytics, never a command. The "
    "record carries the P15.01 required provenance links (input_data_ref, "
    "producing_model_ref, eval_set_ref) as caller-declared references "
    "that were validated before computation; it carries no authorization "
    "token and no dispatch reach (ADR-010 extension)."
)


def fnv1a64(data: bytes) -> int:
    """FNV-1a 64 (deterministic, dependency-free, test-grade). Mirrors the
    Rust helper; pinned by the published test vectors (empty input, "a",
    "foobar") — the same pins the Rust module carries."""
    hash_value = 0xCBF29CE484222325
    for byte in data:
        hash_value ^= byte
        hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return hash_value


def _canonical_publication_string(
    *, artifact_kind: str, forecast_version: str, scope: dict, baseline: str,
    baseline_version: str, values: list, input_data_ref: str,
    producing_model_ref: str, eval_set_ref: str,
) -> str:
    """The canonical digest input: stable field order, `|` separators —
    the deterministic string the test-grade digest is computed over (the
    ForecastBaselineRecord.canonical_string pattern, mirrored)."""
    return "|".join(
        [
            "kind:%s" % artifact_kind,
            "version:%s" % forecast_version,
            "scope:%s:%s" % (scope["scope_kind"], scope["scope_id"]),
            "baseline:%s" % baseline,
            "baseline_version:%s" % baseline_version,
            "values:%s" % ",".join(repr(v) for v in values),
            "input_data_ref:%s" % input_data_ref,
            "producing_model_ref:%s" % producing_model_ref,
            "eval_set_ref:%s" % eval_set_ref,
        ]
    )


def run_publication(request: dict) -> dict:
    """Runs one `publish_forecast_record` request (pure).

    The frozen baseline rule set runs first (identical gates to
    `baseline_forecast`); on completion the result is wrapped as the
    `flexcore-forecast-worker/1` publication-record envelope with the
    P15.01 provenance record (required links, test-grade digest). An
    in-band rule refusal is the failed envelope — a refusal is a result,
    never a crash.
    """
    validated = validate_request(request)
    diagnostics: list = []
    baseline = validated["baseline"]
    horizon_steps = validated["horizon"]["steps"]
    publication = validated["publication"]

    try:
        if baseline == BASELINE_PERSISTENCE:
            values = persistence_forecast(validated["history"], horizon_steps)
        elif baseline == BASELINE_SEASONAL_NAIVE:
            values = seasonal_naive_forecast(
                validated["history"],
                validated["season_length"],
                horizon_steps,
            )
        else:  # BASELINE_DEGREE_DAY
            degree_day = validated["degree_day"]
            coefficients = fit_degree_day_regression(degree_day["training_samples"])
            values = degree_day_predict(
                coefficients,
                degree_day["forecast_hdd"],
                degree_day["forecast_cdd"],
            )
        termination = "completed"
        canonical = _canonical_publication_string(
            artifact_kind=publication["artifact_kind"],
            forecast_version=publication["forecast_version"],
            scope=validated["scope"],
            baseline=baseline,
            baseline_version=BASELINE_VERSIONS[baseline],
            values=values,
            input_data_ref=publication["input_data_ref"],
            producing_model_ref=publication["producing_model_ref"],
            eval_set_ref=publication["eval_set_ref"],
        )
        digest = "%016x" % fnv1a64(canonical.encode("utf-8"))
        outputs = {
            "values": values,
            "steps": len(values),
            "sign_convention": (
                "the caller's declared convention, preserved verbatim (P > 0 = export "
                "at the point of connection; the baseline never flips or normalizes "
                "signs)"
            ),
            "publication": {
                "forecast_version": publication["forecast_version"],
                "artifact_kind": publication["artifact_kind"],
                "provenance_record": {
                    "schema_id": PROVENANCE_SCHEMA_ID_MIRROR,
                    "artifact_ref": "flexcore-forecast-artifact:%s:%s:%s"
                    % (
                        validated["scope"]["scope_kind"],
                        validated["scope"]["scope_id"],
                        publication["forecast_version"],
                    ),
                    "digest": digest,
                    "digest_algorithm": DIGEST_ALGORITHM_FNV1A64_TEST,
                    "digest_note": TEST_GRADE_DIGEST_STATEMENT,
                    "input_data_ref": publication["input_data_ref"],
                    "producing_model_ref": publication["producing_model_ref"],
                    "eval_set_ref": publication["eval_set_ref"],
                    "signer_key_reference": None,
                    "signing_status": (
                        "UNSIGNED (no signing identity exists; the record is "
                        "integrity-stamped with the test-grade digest only)"
                    ),
                },
                "canonical_digest_input": canonical,
            },
        }
    except ForecastComputationError as error:
        diagnostics.append("baseline rule refused in-band: %s" % error)
        termination = "failed"
        outputs = None

    result = build_result(
        validated=validated,
        termination_status=termination,
        outputs=outputs,
        diagnostics=diagnostics,
    )
    if termination == "completed":
        # The publication envelope carries the boundary statement; the
        # kind field marks it as a publication record.
        result["kind"] = "forecast_publication_record"
        result["payload_type"] = "flexcore.forecast.publication_record.v1"
        result["boundary_statements"] = list(BOUNDARY_STATEMENTS) + [
            PUBLICATION_BOUNDARY_STATEMENT
        ]
    return result


# ---------------------------------------------------------------------------
# Composition edge: one JSON document in, one JSON document out
# ---------------------------------------------------------------------------


def main() -> int:
    """Reads ONE request document on stdin, emits ONE result on stdout.

    Fail-closed: any malformed input is answered with NOTHING on stdout,
    a reason on stderr, and exit 2. A successful forecast emits exactly
    one `flexcore-forecast-worker/1` envelope, after
    assert_proposal_only.
    """
    raw = sys.stdin.read()
    try:
        request = json.loads(raw)
    except json.JSONDecodeError as error:
        sys.stderr.write(
            "flexcore-forecast-worker: request is not a valid JSON document: %s\n"
            % error
        )
        return 2
    try:
        validation_probe = validate_request(request)
        del validation_probe  # validation is the gate; the runner revalidates
        if request.get("mode") == MODE_PUBLISH:
            result = run_publication(request)
        else:
            result = run_forecast(request)
        assert_proposal_only(result)
    except WorkerRequestError as error:
        sys.stderr.write("flexcore-forecast-worker: request refused: %s\n" % error)
        return 2
    except RuntimeError as error:
        sys.stderr.write("flexcore-forecast-worker: refused: %s\n" % error)
        return 2
    sys.stdout.write(json.dumps(result))
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0


# ---------------------------------------------------------------------------
# In-module self-tests for the PURE-LOGIC core (hand-computed; NEVER RUN
# in this loop). Run instruction (human gate):
#   python apps/worker/python/forecast_worker/forecast_baseline_worker.py
# (the pytest suite mirrors these cases in
# tests/test_forecast_baseline_worker.py).
# ---------------------------------------------------------------------------


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. This function itself has
    NEVER been executed in this loop; the expectations are hand
    derivations from the constructed inputs, never observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- persistence: hand-computed known answers -----------------------
    values = persistence_forecast([-0.4, -0.5, -0.6], 3)
    check(
        "persistence_carries_the_last_value_forward",
        values == [-0.6, -0.6, -0.6],
        "history [-0.4, -0.5, -0.6] MW, horizon 3 -> [-0.6, -0.6, -0.6] "
        "(sign preserved verbatim)",
    )

    def empty_history() -> None:
        persistence_forecast([], 1)

    _expects_error(results, empty_history, "persistence_empty_history_refused")

    def nan_history() -> None:
        persistence_forecast([0.5, float("nan")], 1)

    _expects_error(results, nan_history, "persistence_nan_history_refused")

    # ---- seasonal-naive: hand-computed known answers --------------------
    values = seasonal_naive_forecast([-0.0, -0.1, -0.2, -0.3, -0.4], 3, 3)
    check(
        "seasonal_naive_returns_the_value_one_season_earlier",
        values == [-0.2, -0.3, -0.4],
        "history of 5, season 3, horizon 3 -> indices 2, 3, 4",
    )
    values = seasonal_naive_forecast([10.0, 20.0, 30.0, 40.0], 2, 4)
    check(
        "seasonal_naive_wraps_within_the_last_season",
        values == [30.0, 40.0, 30.0, 40.0],
        "history of 4, season 2, horizon 4 -> wrap rule over the last season",
    )

    def short_season() -> None:
        seasonal_naive_forecast([1.0, 2.0], 3, 1)

    _expects_error(results, short_season, "seasonal_naive_short_history_refused")

    # ---- degree-day fit: hand-computed exact coefficients ---------------
    # Orthogonal regressors (the same construction as the Rust test):
    # three all-zero points at load 1.0, one pure-HDD point at load 3.0,
    # one pure-CDD point at load 4.0 -> intercept 1.0, hdd 2.0, cdd 3.0
    # (HAND CHECK of the normal equations recorded in the Rust test).
    orthogonal = [
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 3.0, "heating_degree_days": 1.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 4.0, "heating_degree_days": 0.0, "cooling_degree_days": 1.0},
    ]
    coefficients = fit_degree_day_regression(orthogonal)
    check(
        "degree_day_fit_recovers_exact_coefficients",
        all(abs(a - b) < 1e-9 for a, b in zip(coefficients, (1.0, 2.0, 3.0))),
        "intercept 1.0, hdd 2.0, cdd 3.0 (exact, hand-verified)",
    )
    values = degree_day_predict(coefficients, [2.0, 0.0], [1.0, 0.0])
    check(
        "degree_day_predict_is_linear",
        values == [8.0, 1.0],
        "(hdd, cdd) = (2, 1) -> 1 + 4 + 3 = 8.0; (0, 0) -> 1.0",
    )

    def singular() -> None:
        fit_degree_day_regression(
            [
                {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
                {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
                {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
            ]
        )

    _expects_error(results, singular, "degree_day_singular_design_refused")

    def negative_degree_days() -> None:
        fit_degree_day_regression(
            [
                {"load_mw": 1.0, "heating_degree_days": -1.0, "cooling_degree_days": 0.0},
                {"load_mw": 2.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
                {"load_mw": 3.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
            ]
        )

    _expects_error(results, negative_degree_days, "degree_day_negative_refused")

    # ---- the permission gate (P15.01) -----------------------------------
    def dispatch_permission() -> None:
        validate_request(_valid_request(permissions=["dispatch.issue"]))

    _expects_error(results, dispatch_permission, "dispatch_permission_never_granted")

    def out_of_vocabulary_permission() -> None:
        validate_request(_valid_request(permissions=["schedule.propose"]))

    _expects_error(
        results, out_of_vocabulary_permission, "producer_vocabulary_closed"
    )

    # ---- the proposal-only boundary --------------------------------------
    def forbidden_key() -> None:
        assert_proposal_only({"outputs": {"values": [1.0], "approval": True}})

    _expects_runtime_error(results, forbidden_key, "proposal_only_forbidden_key")

    try:
        assert_proposal_only({"outputs": {"values": [1.0]}})
        clean_ok = True
    except RuntimeError:
        clean_ok = False
    check(
        "proposal_only_clean_tree_passes",
        clean_ok,
        "a clean result tree passes the proposal-only scan",
    )

    # ---- the full request path -------------------------------------------
    result = run_forecast(_valid_request())
    check(
        "full_request_completes_with_the_fixed_posture",
        result["ok"] is True
        and result["proposal_only"] is True
        and result["authority"] == "none"
        and result["control_authority_mode_locked"] == MODE_LOCKED
        and result["forecast"]["baseline_version"] == "persistence.v1"
        and result["honesty"]["model_registry_state"] == "UNTRAINED"
        and "forecast-eval-set-v1" in result["honesty"]["eval_set_ref"],
        "the envelope carries proposal_only/authority none/mode lock, the "
        "baseline version, the UNTRAINED registry state, and the frozen "
        "eval-set ref",
    )
    try:
        assert_proposal_only(result)
        envelope_clean = True
    except RuntimeError:
        envelope_clean = False
    check(
        "envelope_passes_proposal_only",
        envelope_clean,
        "the built envelope passes the proposal-only scan",
    )

    # ---- in-band refusal is a result, not a crash -------------------------
    refused = run_forecast(_valid_request(history=[]))
    check(
        "in_band_refusal_is_a_failed_envelope",
        refused["ok"] is False
        and refused["provenance"]["termination_status"] == "failed"
        and refused["forecast"]["outputs"] is None,
        "an empty history yields termination failed with diagnostics — a "
        "refusal is a result, never a crash",
    )

    # ---- the unknown baseline is refused before computation ---------------
    def unknown_baseline() -> None:
        validate_request(_valid_request(baseline="forecast-baseline/ml-dream"))

    _expects_error(results, unknown_baseline, "unknown_baseline_refused")

    # ---- the guarded numpy import raises the NAMED requirement ------------
    _expects_named_import_error(
        results,
        import_numpy,
        "numpy_import_raises_named_requirement",
        "the guarded numpy import raises the named NUMPY_DEPLOYMENT_REQUIREMENT "
        "(the honesty gate; the frozen rule sets never call it)",
    )

    # ---- P15.17 completion: the publication-record seam --------------------
    # The FNV-1a 64 mirror against the published vectors (the same pins
    # the Rust module carries).
    check(
        "fnv1a64_matches_published_vectors",
        fnv1a64(b"") == 0xCBF29CE484222325
        and fnv1a64(b"a") == 0xAF63DC4C8601EC8C
        and fnv1a64(b"foobar") == 0x85944171F73967E8,
        "empty input = offset basis; 'a'; 'foobar' (the published vectors)",
    )
    publication_result = run_publication(_valid_publication_request())
    provenance = publication_result["forecast"]["outputs"]["publication"][
        "provenance_record"
    ]
    check(
        "publication_record_carries_required_provenance_links",
        publication_result["ok"] is True
        and publication_result["kind"] == "forecast_publication_record"
        and provenance["schema_id"] == "flexcore-forecast-artifact-provenance-v1"
        and provenance["digest_algorithm"] == "fnv1a64-test"
        and provenance["input_data_ref"] == "fixture-forecast-input-record"
        and provenance["producing_model_ref"] == "forecast-baseline/persistence@v1"
        and provenance["eval_set_ref"] == "forecast-eval-set-v1"
        and provenance["signer_key_reference"] is None
        and "UNSIGNED" in provenance["signing_status"],
        "the publication envelope carries the schema id, the test-grade "
        "digest tag, the three REQUIRED provenance links (validated before "
        "computation), and the unsigned/no-signing-identity statement",
    )
    check(
        "publication_digest_is_deterministic_and_content_sensitive",
        run_publication(_valid_publication_request())["forecast"]["outputs"][
            "publication"
        ]["provenance_record"]["digest"]
        == provenance["digest"]
        and run_publication(
            _valid_publication_request(forecast_version="v2")
        )["forecast"]["outputs"]["publication"]["provenance_record"]["digest"]
        != provenance["digest"],
        "the same request reproduces the same test-grade digest; a content "
        "change (the forecast version) changes it",
    )
    check(
        "publication_envelope_passes_proposal_only",
        _runs_clean(lambda: assert_proposal_only(publication_result)),
        "the publication envelope passes the proposal-only scan (no "
        "authorization/command-shaped key anywhere in the tree)",
    )
    check(
        "publication_statements_are_the_recorded_strings",
        "NOT a security primitive" in TEST_GRADE_DIGEST_STATEMENT
        and "never a command" in PUBLICATION_BOUNDARY_STATEMENT,
        "the test-grade digest statement and the publication boundary "
        "statement are the recorded strings",
    )

    # Missing required provenance links are refused BEFORE computation.
    def missing_link() -> None:
        request = _valid_publication_request()
        del request["publication"]["producing_model_ref"]
        validate_request(request)

    _expects_error(results, missing_link, "publication_missing_provenance_link_refused")

    # An out-of-vocabulary artifact kind is refused.
    def bad_kind() -> None:
        validate_request(
            _valid_publication_request(artifact_kind="temperature")
        )

    _expects_error(results, bad_kind, "publication_unknown_artifact_kind_refused")

    return results


def _valid_publication_request(**overrides) -> dict:
    """The canonical valid `publish_forecast_record` request used by the
    self-tests (all values are CONSTRUCTED test inputs, never
    measurements). Overrides apply at the top level; `publication`
    overrides merge into the publication object."""
    publication_overrides = {
        key: overrides.pop(key)
        for key in ("forecast_version", "artifact_kind")
        if key in overrides
    }
    request = _valid_request(mode=MODE_PUBLISH)
    request["publication"] = {
        "forecast_version": "v1",
        "artifact_kind": "load",
        "input_data_ref": "fixture-forecast-input-record",
        "producing_model_ref": "forecast-baseline/persistence@v1",
        "eval_set_ref": "forecast-eval-set-v1",
    }
    request["publication"].update(publication_overrides)
    request.update(overrides)
    return request


def _runs_clean(fn) -> bool:
    """Self-test helper: fn must run without raising (True) — used for
    the proposal-only clean-scan checks."""
    try:
        fn()
        return True
    except Exception:  # noqa: BLE001 — the scan must simply not raise
        return False


def _valid_request(**overrides) -> dict:
    """The canonical valid request used by the self-tests (all values are
    CONSTRUCTED test inputs, never measurements)."""
    request = {
        "protocol": PROTOCOL_ID,
        "mode": "baseline_forecast",
        "baseline": BASELINE_PERSISTENCE,
        "permissions_declared": ["telemetry.read", "forecast.publish"],
        "scope": {"scope_kind": "site", "scope_id": "site-north"},
        "horizon": {"steps": 3, "interval_millis": 900000},
        "history": [-0.4, -0.5, -0.6],
        "season_length": None,
        "degree_day": None,
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def _expects_error(results: list, fn, name: str) -> None:
    """Self-test helper: fn must raise WorkerRequestError or
    ForecastComputationError and nothing else."""
    try:
        fn()
    except (WorkerRequestError, ForecastComputationError):
        results.append(
            {"name": name, "passed": True, "detail": "raised a named refusal"}
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of a named refusal" % error,
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


def _expects_runtime_error(results: list, fn, name: str) -> None:
    try:
        fn()
    except RuntimeError:
        results.append({"name": name, "passed": True, "detail": "refused emission"})
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of RuntimeError" % error,
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


def _expects_named_import_error(results: list, fn, name: str, detail: str) -> None:
    try:
        fn()
    except ImportError as error:
        results.append(
            {
                "name": name,
                "passed": NUMPY_DEPLOYMENT_REQUIREMENT in str(error)
                and "DECISION-POINT" in str(error),
                "detail": "%s (message names the requirement: %s)"
                % (detail, NUMPY_DEPLOYMENT_REQUIREMENT in str(error)),
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "%s; raised %r instead of ImportError" % (detail, error),
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "%s; raised nothing" % detail})


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    if "--self-test" in sys.argv[1:]:
        _records = run_self_tests()
        _failures = [r for r in _records if not r["passed"]]
        for _r in _records:
            print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
        print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
        raise SystemExit(1 if _failures else 0)
    raise SystemExit(main())
