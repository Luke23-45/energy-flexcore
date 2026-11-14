# P8.22 — Portfolio analytics research (pure, stdlib-only): DERIVED
#         REPORTING over raw scenario data — recording-only outputs,
#         never a dispatch input.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. The ledger P8.22 analytics core (ledger step 1):
#    pure stdlib functions that derive REPORTING records — available vs
#    delivered flexibility (MW/MWh), shortfall (MWh), delivery ratio,
#    uncertainty-band calibration coverage, and the baseline-accounting
#    comparison — over INJECTED raw scenario data. Every output is a
#    RECORD: it carries the fixed reporting-only statement, and the
#    module namespace contains NO dispatch/issue/authorize/approve/
#    command/RANK-shaped callable (pinned by a self-test) — the ledger's
#    named risk ("an analytic that silently becomes a dispatch input,
#    e.g. a 'flexibility score' used to rank dispatch candidates") is
#    structurally absent: there is no scoring or ranking function here,
#    and `assert_reporting_only` refuses any document that carries a
#    dispatch-shaped key at any depth.
# 2. NOTHING HERE HAS EVER RUN, AND NO MEASURED CELL EXISTS. The module
#    was WRITTEN, NEVER RUN (Phase 8 loop bans: nothing installed, built,
#    or executed; no pytest). The hand-computed expectations in
#    `run_self_tests` are derivations from constructed inputs with the
#    arithmetic STATED in each test (e.g. 2.0 MW over 0.5 h is exactly
#    1.0 MWh) — they pin the analytic DEFINITIONS and are NOT a
#    measurement. Every measured cell in the P8.22 record
#    (`ledger/phase-08-advanced-coordination/22_portfolio_analytics.md`)
#    is EMPTY until the human/lab gate fills it.
# 3. DERIVED REPORTING, NOT DISPATCH (the subtask's stated boundary).
#    These analytics are computed over raw scenario data for REPORTS;
#    they are never an input to scheduling, feasibility, authorization,
#    or any dispatch-lifecycle stage. The independent-recompute
#    verification method (the ledger's: "Recompute one analytic from raw
#    scenario data independently and compare") is exercisable because the
#    functions are pure and their inputs are raw records — not because
#    anything here has produced a number (no number exists).
# 4. UNITS AND SIGNS (§10.2, canonical). Power in MW, energy in MWh;
#    `P > 0` means export at the point of connection (a caller-side sign
#    declaration carried by the direction field, never assumed);
#    SOC is a fraction in [0, 1] at the API boundary; telemetry quality
#    is exactly one of GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE per value
#    and is CARRIED, not stripped. Negative available/delivered magnitudes
#    are refused: direction belongs in the declared `direction` field, so
#    a sign is never silently encoded in a magnitude.
# 5. CALIBRATION IS ASSESSED ON THE TESTED REGIMES ONLY (the ledger's
#    limit). The coverage figure is a fact about the injected records;
#    the output carries the regime-scope statement verbatim and no
#    generalization beyond the tested regimes is claimed anywhere.
# 6. THE BASELINE ACCOUNTING COMPARISON NEEDS THE FROZEN ARTIFACT. The
#    deterministic baseline's accounting artifact does not exist until
#    the first human/lab run (P8.06 §3 recorded this); the comparison
#    function takes the tolerance as INJECTED and PRE-REGISTERED
#    (TOLERANCE_NOT_PRE_REGISTERED otherwise) and reports discrepancies
#    AS DISCREPANCIES — never absorbed, never silently reconciled.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

# The analytics output schema tag (carried on every derived record).
PORTFOLIO_ANALYTICS_SCHEMA = "flexcore-portfolio-analytics/1"

# The telemetry quality states (§10.2 — exactly one per value; carried,
# never stripped).
TELEMETRY_QUALITY_STATES: frozenset = frozenset(
    {"GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"}
)

# The declared direction vocabulary (the caller-side sign declaration:
# `P > 0` = export at the point of connection; a magnitude never encodes
# a sign silently).
DIRECTIONS: frozenset = frozenset({"export", "import"})

REPORTING_ONLY_STATEMENT = (
    "derived reporting only — a RECORD computed over raw scenario data; "
    "it never enters the dispatch lifecycle and no function in this "
    "module produces a dispatch, scheduling, authorization, or ranking "
    "input (ADR-010; the P8.22 boundary)"
)

CALIBRATION_SCOPE_STATEMENT = (
    "calibration coverage is assessed on the TESTED REGIMES ONLY — it is "
    "a fact about the injected records and claims nothing about regimes "
    "outside them (the P8.22 recorded limit)"
)


class AnalyticsInputError(ValueError):
    """A malformed analytics input or an unrepresentable output.

    Named errors (message prefix): EVENT_INVALID, CALIBRATION_INVALID,
    BASELINE_COMPARE_INVALID, TOLERANCE_NOT_PRE_REGISTERED,
    REPORTING_ONLY_VIOLATION, DIVISOR_ZERO. A refusal is a RESULT, never
    a silent repair, never a fabricated figure.
    """


# ---------------------------------------------------------------------------
# The reporting-only boundary, enforced in code (ledger step 2; the
# ledger's named risk)
# ---------------------------------------------------------------------------

# The key names that would make a derived record a dispatch-shaped
# artifact if they appeared at ANY depth of an analytics output.
DISPATCH_SHAPED_KEYS: frozenset = frozenset(
    {
        "authorization",
        "authorization_token",
        "command",
        "command_id",
        "dispatch",
        "dispatch_command",
        "dispatch_rank",
        "dispatch_score",
        "issued_command",
        "approval",
        "grant",
        "schedule",
        "setpoint",
        "setpoint_mw",
        "flexibility_score",
        "priority_score",
        "dispatch_candidate",
    }
)


def assert_reporting_only(document: object) -> None:
    """Refuses any dispatch-shaped key at ANY depth of an analytics
    output (pure; the boundary test the ledger's verification method
    exercises by inspection + this check).

    This is the enforcement of the subtask's stated boundary: NO analytic
    silently becomes a dispatch input. A record that would carry a
    ranking, score, setpoint, schedule, or command key is refused — the
    outputs of this module can only be READ as reports.
    """
    if isinstance(document, dict):
        for key, value in document.items():
            if (
                isinstance(key, str)
                and key.strip().lower() in DISPATCH_SHAPED_KEYS
            ):
                raise AnalyticsInputError(
                    "REPORTING_ONLY_VIOLATION: key %r is dispatch-shaped "
                    "— analytics outputs are recording-only (ADR-010); "
                    "no analytic may become a dispatch input" % key
                )
            assert_reporting_only(value)
    elif isinstance(document, list):
        for item in document:
            assert_reporting_only(item)


# ---------------------------------------------------------------------------
# Ledger step 1 — the analytics, each with formula, inputs, and units
# ---------------------------------------------------------------------------


def validate_flexibility_event(event: object) -> dict:
    """Validates ONE raw flexibility event record; returns normalized.

    Inputs (raw scenario data):
      event_id            non-empty string
      interval_hours      finite > 0 (the decision-interval length, h)
      available_mw        finite >= 0 (the available flexibility power, MW)
      delivered_mw        finite >= 0 (the delivered flexibility power, MW)
      direction           "export" | "import" (the declared sign context;
                          `P > 0` = export at the point of connection)
      quality_state       exactly one of the §10.2 telemetry quality
                          states (CARRIED on the output, never stripped)
      soc_start_fraction  optional fraction in [0, 1]
      soc_end_fraction    optional fraction in [0, 1]
    """
    if not isinstance(event, dict):
        raise AnalyticsInputError("EVENT_INVALID: event must be an object")
    event_id = event.get("event_id")
    if not isinstance(event_id, str) or not event_id.strip():
        raise AnalyticsInputError(
            "EVENT_INVALID: event_id must be a non-empty string"
        )
    interval_hours = _finite_non_negative(event, "interval_hours")
    if interval_hours <= 0.0:
        raise AnalyticsInputError(
            "EVENT_INVALID: interval_hours must be > 0 (got %r)"
            % (interval_hours,)
        )
    available_mw = _finite_non_negative(event, "available_mw")
    delivered_mw = _finite_non_negative(event, "delivered_mw")
    direction = event.get("direction")
    if direction not in DIRECTIONS:
        raise AnalyticsInputError(
            "EVENT_INVALID: direction %r must be one of %s — the sign "
            "context is DECLARED per record, never assumed from a "
            "magnitude" % (direction, sorted(DIRECTIONS))
        )
    quality_state = event.get("quality_state")
    if quality_state not in TELEMETRY_QUALITY_STATES:
        raise AnalyticsInputError(
            "EVENT_INVALID: quality_state %r must be exactly one of %s "
            "— the quality state is carried, never stripped"
            % (quality_state, sorted(TELEMETRY_QUALITY_STATES))
        )
    socs = {}
    for field in ("soc_start_fraction", "soc_end_fraction"):
        value = event.get(field)
        if value is None:
            continue
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise AnalyticsInputError(
                "EVENT_INVALID: %s must be a number or null" % field
            )
        value = float(value)
        if not math.isfinite(value) or not (0.0 <= value <= 1.0):
            raise AnalyticsInputError(
                "EVENT_INVALID: %s %r is outside [0, 1] (SOC is a "
                "fraction at the API boundary)" % (field, value)
            )
        socs[field] = value
    return {
        "event_id": event_id,
        "interval_hours": interval_hours,
        "available_mw": available_mw,
        "delivered_mw": delivered_mw,
        "direction": direction,
        "quality_state": quality_state,
        **socs,
    }


def event_energy(event: object) -> dict:
    """ONE event's derived energy record (pure).

    Formula (stated): available_mwh = available_mw * interval_hours;
    delivered_mwh = delivered_mw * interval_hours;
    shortfall_mwh = max(0, available_mwh - delivered_mwh). The shortfall
    is the under-delivery ONLY — over-delivery is NOT folded in as a
    negative shortfall (it would silently net against a real shortfall
    elsewhere in the portfolio; over-delivery is carried as its own
    over_delivery_mwh figure, honest and separable).
    """
    normalized = validate_flexibility_event(event)
    available_mwh = normalized["available_mw"] * normalized["interval_hours"]
    delivered_mwh = normalized["delivered_mw"] * normalized["interval_hours"]
    gap_mwh = available_mwh - delivered_mwh
    return {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "event_id": normalized["event_id"],
        "direction": normalized["direction"],
        "quality_state": normalized["quality_state"],
        "interval_hours": normalized["interval_hours"],
        "available_mw": normalized["available_mw"],
        "delivered_mw": normalized["delivered_mw"],
        "available_mwh": available_mwh,
        "delivered_mwh": delivered_mwh,
        "shortfall_mwh": max(0.0, gap_mwh),
        "over_delivery_mwh": max(0.0, -gap_mwh),
    }


def aggregate_delivery(events: list) -> dict:
    """The portfolio delivery record over the injected raw events (pure).

    Formula (stated, per event then summed):
      available_total_mwh = Σ available_mw * interval_hours
      delivered_total_mwh = Σ delivered_mw * interval_hours
      shortfall_total_mwh = Σ max(0, available_mwh - delivered_mwh)
      delivery_ratio      = delivered_total_mwh / available_total_mwh
                            (DIVISOR_ZERO refused when the available
                            total is 0 — a ratio over an empty
                            denominator would be a fabricated figure;
                            the refusal names it, never silently 1.0)

    Per-event records travel inside the output so every aggregate can be
    RECOMPUTED INDEPENDENTLY from the raw data (the ledger's verification
    method), and the telemetry quality-state mix is carried (a ratio
    built over STALE/ESTIMATED values is visible as such).
    """
    if not isinstance(events, list) or not events:
        raise AnalyticsInputError(
            "EVENT_INVALID: aggregate_delivery requires a non-empty "
            "event list (an aggregate over nothing is a fabricated "
            "figure)"
        )
    per_event = [event_energy(event) for event in events]
    assert_reporting_only(per_event)
    available_total_mwh = sum(r["available_mwh"] for r in per_event)
    delivered_total_mwh = sum(r["delivered_mwh"] for r in per_event)
    shortfall_total_mwh = sum(r["shortfall_mwh"] for r in per_event)
    over_delivery_total_mwh = sum(r["over_delivery_mwh"] for r in per_event)
    quality_mix: dict = {}
    for r in per_event:
        quality_mix[r["quality_state"]] = (
            quality_mix.get(r["quality_state"], 0) + 1
        )
    if available_total_mwh == 0.0:
        raise AnalyticsInputError(
            "DIVISOR_ZERO: the available-energy total is 0 MWh — a "
            "delivery ratio over an empty denominator is refused, not "
            "silently reported as a perfect ratio"
        )
    return {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "event_count": len(per_event),
        "available_total_mwh": available_total_mwh,
        "delivered_total_mwh": delivered_total_mwh,
        "shortfall_total_mwh": shortfall_total_mwh,
        "over_delivery_total_mwh": over_delivery_total_mwh,
        "delivery_ratio": delivered_total_mwh / available_total_mwh,
        "quality_state_mix": quality_mix,
        "per_event": per_event,
        "recompute_note": (
            "every aggregate is recomputable by hand from per_event "
            "(the ledger's independent-recompute verification method); "
            "this record is reporting only"
        ),
    }


def uncertainty_calibration(records: list) -> dict:
    """The uncertainty-band calibration coverage (pure).

    Inputs: one record per forecast/observation pair —
      declared_band_low   finite number, same unit as the actual
      declared_band_high  finite number >= declared_band_low
      actual              finite number
      unit                non-empty string (declared: MW or MWh — never
                          assumed)
      regime              non-empty string (the regime the record came
                          from — the scope statement is pinned to it)

    Formula (stated): coverage = |{records with declared_band_low <=
    actual <= declared_band_high}| / |records|. This is the empirical
    coverage of the DECLARED band — a calibration ASSESSMENT, valid for
    the injected regimes only (CALIBRATION_SCOPE_STATEMENT travels on the
    output; the ledger's recorded limit).
    """
    if not isinstance(records, list) or not records:
        raise AnalyticsInputError(
            "CALIBRATION_INVALID: uncertainty_calibration requires a "
            "non-empty record list"
        )
    units: set = set()
    regimes: set = set()
    covered = 0
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise AnalyticsInputError(
                "CALIBRATION_INVALID: records[%d] must be an object" % index
            )
        low = record.get("declared_band_low")
        high = record.get("declared_band_high")
        actual = record.get("actual")
        unit = record.get("unit")
        regime = record.get("regime")
        for name, value in (
            ("declared_band_low", low),
            ("declared_band_high", high),
            ("actual", actual),
        ):
            if (
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(float(value))
            ):
                raise AnalyticsInputError(
                    "CALIBRATION_INVALID: records[%d].%s must be a "
                    "finite number" % (index, name)
                )
        if not isinstance(unit, str) or not unit.strip():
            raise AnalyticsInputError(
                "CALIBRATION_INVALID: records[%d].unit must be a "
                "non-empty string (the unit is declared, never assumed)"
                % index
            )
        if not isinstance(regime, str) or not regime.strip():
            raise AnalyticsInputError(
                "CALIBRATION_INVALID: records[%d].regime must be a "
                "non-empty string (coverage is assessed per tested "
                "regime)" % index
            )
        if float(high) < float(low):
            raise AnalyticsInputError(
                "CALIBRATION_INVALID: records[%d] declares an inverted "
                "band (high < low) — refused, never silently swapped"
                % index
            )
        units.add(unit)
        regimes.add(regime)
        if float(low) <= float(actual) <= float(high):
            covered += 1
    if len(units) != 1:
        raise AnalyticsInputError(
            "CALIBRATION_INVALID: the records mix units %s — one "
            "calibration assessment covers ONE declared unit"
            % sorted(units)
        )
    total = len(records)
    return {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "record_count": total,
        "covered_count": covered,
        "coverage": covered / total,
        "unit": units.pop(),
        "regimes_assessed": sorted(regimes),
        "calibration_scope": CALIBRATION_SCOPE_STATEMENT,
    }


def compare_to_baseline_accounting(
    *,
    analytics_totals: dict,
    baseline_totals: dict,
    pre_registered_tolerance_mwh: float,
) -> dict:
    """The ledger step-3 validation record: the derived analytics vs the
    deterministic baseline's accounting on the SAME scenario (pure).

    Both totals must be aggregate_delivery-shaped (schema-checked); the
    tolerance is INJECTED and must be a finite number >= 0 recorded
    BEFORE the comparison (TOLERANCE_NOT_PRE_REGISTERED otherwise — the
    same pre-registration discipline as the P8.15/P8.09 harnesses). Each
    compared quantity carries its absolute difference; a difference
    beyond the tolerance is reported AS A DISCREPANCY (never absorbed,
    never silently reconciled) — the discrepancy IS the deliverable of
    ledger step 3 ("report discrepancies").
    """
    for name, totals in (
        ("analytics_totals", analytics_totals),
        ("baseline_totals", baseline_totals),
    ):
        if (
            not isinstance(totals, dict)
            or totals.get("schema") != PORTFOLIO_ANALYTICS_SCHEMA
            or "delivered_total_mwh" not in totals
            or "shortfall_total_mwh" not in totals
            or "available_total_mwh" not in totals
        ):
            raise AnalyticsInputError(
                "BASELINE_COMPARE_INVALID: %s must be an "
                "aggregate_delivery record (schema %r)"
                % (name, PORTFOLIO_ANALYTICS_SCHEMA)
            )
    tolerance = pre_registered_tolerance_mwh
    if (
        isinstance(tolerance, bool)
        or not isinstance(tolerance, (int, float))
        or not math.isfinite(float(tolerance))
        or float(tolerance) < 0.0
    ):
        raise AnalyticsInputError(
            "TOLERANCE_NOT_PRE_REGISTERED: the comparison tolerance must "
            "be a finite number >= 0, recorded BEFORE the comparison — "
            "never fitted after seeing the difference"
        )
    tolerance = float(tolerance)
    fields = (
        "available_total_mwh",
        "delivered_total_mwh",
        "shortfall_total_mwh",
    )
    comparisons = {}
    discrepancies = []
    for field in fields:
        difference = abs(
            float(analytics_totals[field]) - float(baseline_totals[field])
        )
        within = difference <= tolerance
        comparisons[field] = {
            "analytics": float(analytics_totals[field]),
            "baseline": float(baseline_totals[field]),
            "absolute_difference_mwh": difference,
            "within_tolerance": within,
        }
        if not within:
            discrepancies.append(field)
    assert_reporting_only(
        {"comparisons": comparisons, "discrepancies": discrepancies}
    )
    return {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "usage": REPORTING_ONLY_STATEMENT,
        "tolerance_mwh": tolerance,
        "comparisons": comparisons,
        "discrepancy_fields": discrepancies,
        "all_within_tolerance": discrepancies == [],
        "discrepancy_note": (
            "a difference beyond the tolerance is REPORTED as a "
            "discrepancy (ledger step 3), never absorbed or silently "
            "reconciled"
        ),
    }


def _finite_non_negative(event: dict, field: str) -> float:
    value = event.get(field)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise AnalyticsInputError(
            "EVENT_INVALID: %s must be a finite number" % field
        )
    value = float(value)
    if not math.isfinite(value) or value < 0.0:
        raise AnalyticsInputError(
            "EVENT_INVALID: %s must be finite and >= 0 (a negative "
            "magnitude would silently encode a sign — declare it in the "
            "direction field instead)" % field
        )
    return value


# ---------------------------------------------------------------------------
# Hand-computed self-tests (NEVER RUN in this loop; every expectation is a
# hand derivation from constructed inputs, with the arithmetic STATED).
# Run instruction (human gate):
#   python apps/worker/python/portfolio_analytics/analytics.py
# ---------------------------------------------------------------------------


def _event(
    event_id: str,
    available_mw: float,
    delivered_mw: float,
    interval_hours: float,
    **overrides,
) -> dict:
    base = dict(
        event_id=event_id,
        interval_hours=interval_hours,
        available_mw=available_mw,
        delivered_mw=delivered_mw,
        direction="export",
        quality_state="GOOD",
    )
    base.update(overrides)
    return base


def run_self_tests() -> list:
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: one event's energy — hand-computed ---------------------
    # HAND CHECK: 2.0 MW over 0.5 h = 1.0 MWh available; 1.5 MW over
    # 0.5 h = 0.75 MWh delivered; gap 1.0 - 0.75 = 0.25 MWh -> shortfall
    # 0.25 MWh, over-delivery 0.0 MWh.
    record = event_energy(_event("ev-1", 2.0, 1.5, 0.5))
    check(
        "event_energy_hand_computed",
        record["available_mwh"] == 1.0
        and record["delivered_mwh"] == 0.75
        and record["shortfall_mwh"] == 0.25
        and record["over_delivery_mwh"] == 0.0
        and record["quality_state"] == "GOOD"
        and record["direction"] == "export"
        and record["usage"] == REPORTING_ONLY_STATEMENT,
        "2.0 MW x 0.5 h = 1.0 MWh; 1.5 MW x 0.5 h = 0.75 MWh; shortfall "
        "1.0 - 0.75 = 0.25 MWh; quality state and direction CARRIED",
    )

    # ---- Test 2: over-delivery is separable, never a negative shortfall -
    # HAND CHECK: delivered 2.5 MW over 1.0 h = 2.5 MWh vs available
    # 2.0 MW x 1.0 h = 2.0 MWh -> gap -0.5 -> shortfall 0.0, over-
    # delivery 0.5 MWh (never folded into the shortfall).
    record2 = event_energy(_event("ev-2", 2.0, 2.5, 1.0))
    check(
        "over_delivery_separable",
        record2["shortfall_mwh"] == 0.0
        and record2["over_delivery_mwh"] == 0.5,
        "gap 2.0 - 2.5 = -0.5 MWh -> shortfall max(0, -0.5) = 0.0 and "
        "over-delivery 0.5 MWh, reported separately",
    )

    # ---- Test 3: portfolio aggregate — hand-computed --------------------
    # HAND CHECK: ev-1 (1.0 / 0.75 MWh) + ev-3 (4.0 MW x 1.0 h = 4.0 MWh
    # available, 4.0 MWh delivered) -> totals available 5.0, delivered
    # 4.75, shortfall 0.25; ratio 4.75 / 5.0 = 0.95.
    aggregate = aggregate_delivery(
        [_event("ev-1", 2.0, 1.5, 0.5), _event("ev-3", 4.0, 4.0, 1.0)]
    )
    check(
        "aggregate_hand_computed",
        aggregate["event_count"] == 2
        and aggregate["available_total_mwh"] == 5.0
        and aggregate["delivered_total_mwh"] == 4.75
        and aggregate["shortfall_total_mwh"] == 0.25
        and aggregate["delivery_ratio"] == 0.95
        and aggregate["quality_state_mix"] == {"GOOD": 2},
        "available 1.0 + 4.0 = 5.0 MWh; delivered 0.75 + 4.0 = 4.75 MWh; "
        "shortfall 0.25 MWh; ratio 4.75/5.0 = 0.95",
    )

    # ---- Test 4: zero available total is DIVISOR_ZERO, never ratio 1.0 --
    def zero_divisor() -> None:
        aggregate_delivery([_event("ev-4", 0.0, 0.0, 1.0)])

    _expects_error(results, zero_divisor, "DIVISOR_ZERO")

    # ---- Test 5: quality state and sign discipline ----------------------
    def bad_quality() -> None:
        event_energy(_event("ev-5", 1.0, 1.0, 1.0, quality_state="fine"))

    _expects_error(results, bad_quality, "quality_state_closed_vocabulary")

    def negative_magnitude() -> None:
        event_energy(_event("ev-6", -2.0, 1.0, 1.0))

    _expects_error(results, negative_magnitude, "negative_magnitude_refused")

    def undeclared_direction() -> None:
        event_energy(_event("ev-7", 1.0, 1.0, 1.0, direction="up"))

    _expects_error(results, undeclared_direction, "direction_declared")

    def soc_out_of_range() -> None:
        event_energy(_event("ev-8", 1.0, 1.0, 1.0, soc_start_fraction=1.5))

    _expects_error(results, soc_out_of_range, "soc_fraction_in_[0,1]")

    # ---- Test 6: calibration coverage — hand-computed -------------------
    # HAND CHECK: band [1.0, 2.0] MW; actuals 1.5 (in), 0.9 (out),
    # 2.0 (in — the boundary IS inside, <= / >= inclusive) -> coverage
    # 2/3 = 0.6666...; compared with the exact fraction, not a decimal.
    calibration = uncertainty_calibration(
        [
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 1.5,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 0.9,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 2.0,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
        ]
    )
    check(
        "calibration_coverage_hand_computed",
        calibration["covered_count"] == 2
        and calibration["record_count"] == 3
        and calibration["coverage"] == 2 / 3
        and calibration["unit"] == "MW"
        and calibration["calibration_scope"] == CALIBRATION_SCOPE_STATEMENT
        and calibration["regimes_assessed"] == ["RECORD-AT-FIRST-RUN"],
        "actuals 1.5 (in), 0.9 (out), 2.0 (in, boundary inclusive) -> "
        "coverage 2/3; the scope statement is carried verbatim (tested "
        "regimes only)",
    )

    # ---- Test 7: calibration refusals -----------------------------------
    def inverted_band() -> None:
        uncertainty_calibration(
            [
                {
                    "declared_band_low": 2.0,
                    "declared_band_high": 1.0,
                    "actual": 1.5,
                    "unit": "MW",
                    "regime": "r",
                }
            ]
        )

    _expects_error(results, inverted_band, "inverted_band_refused")

    def mixed_units() -> None:
        uncertainty_calibration(
            [
                {
                    "declared_band_low": 1.0,
                    "declared_band_high": 2.0,
                    "actual": 1.5,
                    "unit": "MW",
                    "regime": "r",
                },
                {
                    "declared_band_low": 1.0,
                    "declared_band_high": 2.0,
                    "actual": 1.5,
                    "unit": "MWh",
                    "regime": "r",
                },
            ]
        )

    _expects_error(results, mixed_units, "calibration_unit_consistency")

    # ---- Test 8: baseline-accounting comparison — hand-computed ---------
    # HAND CHECK: the Test-3 aggregate (delivered 4.75, shortfall 0.25)
    # vs a baseline record of the SAME scenario (available 5.0,
    # delivered 4.75, shortfall 0.25) -> differences exactly 0.0, all
    # within the 1e-9 MWh tolerance, discrepancy_fields [].
    baseline_same = {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "available_total_mwh": 5.0,
        "delivered_total_mwh": 4.75,
        "shortfall_total_mwh": 0.25,
    }
    comparison = compare_to_baseline_accounting(
        analytics_totals=aggregate,
        baseline_totals=baseline_same,
        pre_registered_tolerance_mwh=1e-9,
    )
    check(
        "baseline_comparison_zero_discrepancy_hand_computed",
        comparison["all_within_tolerance"] is True
        and comparison["discrepancy_fields"] == []
        and comparison["comparisons"]["delivered_total_mwh"][
            "absolute_difference_mwh"
        ]
        == 0.0,
        "analytics 4.75 vs baseline 4.75 MWh -> difference exactly 0.0 "
        "within the pre-registered 1e-9 MWh tolerance",
    )

    # HAND CHECK: a 0.2 MWh delivered difference against a 0.1 MWh
    # tolerance is BEYOND -> reported as a discrepancy (never absorbed).
    baseline_off = {
        "schema": PORTFOLIO_ANALYTICS_SCHEMA,
        "available_total_mwh": 5.0,
        "delivered_total_mwh": 4.55,
        "shortfall_total_mwh": 0.45,
    }
    comparison2 = compare_to_baseline_accounting(
        analytics_totals=aggregate,
        baseline_totals=baseline_off,
        pre_registered_tolerance_mwh=0.1,
    )
    check(
        "discrepancy_reported_not_absorbed",
        comparison2["all_within_tolerance"] is False
        and sorted(comparison2["discrepancy_fields"])
        == ["delivered_total_mwh", "shortfall_total_mwh"]
        and abs(
            comparison2["comparisons"]["delivered_total_mwh"][
                "absolute_difference_mwh"
            ]
            - 0.2
        )
        < 1e-12,
        "|4.75 - 4.55| = 0.2 MWh > 0.1 MWh tolerance (compared with the "
        "stated 1e-12 epsilon because 4.55 is not exactly representable "
        "in binary floating point — the stated arithmetic is exact) -> "
        "the discrepancy is the recorded result, never reconciled",
    )

    def missing_tolerance() -> None:
        compare_to_baseline_accounting(
            analytics_totals=aggregate,
            baseline_totals=baseline_same,
            pre_registered_tolerance_mwh=None,
        )

    _expects_error(
        results, missing_tolerance, "TOLERANCE_NOT_PRE_REGISTERED"
    )

    # ---- Test 9: the reporting-only boundary ----------------------------
    def dispatch_shaped_key() -> None:
        assert_reporting_only(
            {"delivery_ratio": 0.95, "dispatch_rank": [1, 2, 3]}
        )

    _expects_error(
        results, dispatch_shaped_key, "dispatch_shaped_key_refused_at_any_depth"
    )

    def nested_dispatch_key() -> None:
        assert_reporting_only({"a": [{"b": {"flexibility_score": 0.9}}]})

    _expects_error(results, nested_dispatch_key, "nested_dispatch_key_refused")

    check(
        "clean_aggregate_passes_boundary_scan",
        _scans_clean(aggregate) and _scans_clean(comparison),
        "the real outputs pass assert_reporting_only at every depth — "
        "they are recording-only by construction",
    )

    # ---- Test 10: no dispatch/rank-shaped callable in this module -------
    forbidden_tokens = (
        "dispatch",
        "issue",
        "authorize",
        "approve",
        "command",
        "rank",
        "score",
        "schedule",
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
        "approve/command/rank/score/schedule-shaped callable — there is "
        "no 'flexibility score' that could silently become a dispatch "
        "input (the ledger's named risk, absent by construction)",
    )

    return results


def _scans_clean(document: object) -> bool:
    try:
        assert_reporting_only(document)
        return True
    except AnalyticsInputError:
        return False


def _expects_error(results: list, fn, name: str) -> None:
    try:
        fn()
    except AnalyticsInputError:
        results.append(
            {
                "name": name,
                "passed": True,
                "detail": "raised AnalyticsInputError as expected",
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of AnalyticsInputError" % error,
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
