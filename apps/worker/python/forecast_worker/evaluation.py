# FIM-F10 — the forecast evaluation harness (FIM-F10.03), the artifact
#           digest chain + replayability mirror (FIM-F10.04), and the
#           persistence-baseline fallback selection mirror (FIM-F10.02,
#           worker side). Pure stdlib; no framework import anywhere.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below (FIM-F10.06)
#
# 1. IMPLEMENTED-UNDER-FIM-F10, WRITTEN-UNRUN. This module was written,
#    never executed (the FIM hard boundary: no installs, no test runs).
#    The pinned expectations in `run_self_tests()` and in
#    `tests/test_forecast_evaluation.py` are hand-computed arithmetic over
#    CONSTRUCTED inputs (and published SHA-256 test vectors); the float
#    spellings were confirmed with `python -I` and are recorded in the FIM
#    close-out. Nothing is fabricated, nothing is measured.
# 2. NO SKILL IS CLAIMED ANYWHERE. Computing a metric over supplied series
#    is mathematics, not a claim. Every evaluation RESULT that would fill
#    the P15.05 result table is a HUMAN/LAB GATE: the table stays EMPTY in
#    every record until a real backtesting run completes against real data
#    (P15.05 — every result cell EMPTY; the frozen eval set
#    `forecast-eval-set-v1` is NOT edited by any run — a run lands in an
#    evaluation RECORD citing the set version and a gate authorization,
#    never by editing the artifact). The metric functions here mirror the
#    single Rust home (`crates/flexcore-application/src/
#    forecast_evaluation.rs`) BY CITATION: same error semantics, same
#    accumulation order, same refusal for a non-positive persistence
#    reference.
# 3. VENDOR FEEDS ARE HUMAN GATES. This module consumes SUPPLIED series;
#    it never fetches, never names a vendor (P15-E-1/P15-E-2 stay OPEN),
#    and never reads a token. A real vendor feed ingesting with
#    provenance is F-10 acceptance criterion (1) and exists only at the
#    human gate.
# 4. ADR-010 (FROZEN): every output of this module is an INPUT to
#    analytics. Nothing here constructs, influences, or carries a dispatch
#    command; there is no authorization vocabulary anywhere in this file.
# 5. THE SYNTHETIC FIXTURE STAYS THE LABELED FALLBACK SOURCE (P15.16): the
#    fallback selection's last resort returns the fixture id
#    `synthetic-forecast-fixture` and the `synthetic` provenance label —
#    the label must travel with every fixture value a consumer reads.
#    This module never generates fixture values.
# 6. ALIGNMENT REFUSES GAPS (never interpolates silently): the evaluation
#    joins forecast and observed series on exact timestamps; a missing or
#    duplicated timestamp, an unsorted series, a non-finite value, or a
#    length mismatch is a NAMED refusal — silently dropping a point would
#    fabricate an evaluation.
# 7. THE ARTIFACT DIGEST CHAIN (FIM-F10.04, P15.06 shape) mirrors the Rust
#    chain (`apps/worker/src/vendor_ingestion.rs` §8) byte-for-byte: the
#    same canonical rendering, the same SHA-256 chain input
#    (domain ‖ 0x1F ‖ previous_head ‖ 0x1F ‖ canonical), the same genesis
#    head (the published SHA-256 of the empty input), the same append-only
#    head-proof discipline, the same named tamper refusal. UNKEYED SHA-256
#    is an integrity/replay stamp — explicitly NOT a signature and NOT a
#    security primitive (no signing identity exists).
# 8. THE RUST ↔ PYTHON COMPOSITION SEAM (FIM-F10.02, recorded): no runtime
#    seam exists — the Rust worker never invokes a python interpreter;
#    the forecast worker is an OUT-OF-PROCESS stdin→stdout contract
#    driven by the operator/harness. The fallback selection therefore
#    composes on the Rust side (against
#    `flexcore_application::forecast_baselines::persistence_forecast`,
#    the single home of the rule) and the SAME pure function is mirrored
#    HERE for the worker side, pinned by identical hand-computed boundary
#    vectors (fresh at max-age, fallback at max-age + 1 — the P2.08
#    equality convention).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import hashlib
import math

# ---------------------------------------------------------------------------
# The closed vocabularies (mirrored from the Rust side / the frozen records)
# ---------------------------------------------------------------------------

# The chain's domain-separation prefix (byte-identical with the Rust
# `ARTIFACT_CHAIN_DOMAIN`: b"flexcore-forecast-artifact-chain-v1").
ARTIFACT_CHAIN_DOMAIN = b"flexcore-forecast-artifact-chain-v1"

# The chain digest algorithm label (mirrors the Rust constant).
ARTIFACT_CHAIN_ALGORITHM = "sha256-chain-v1"

# The empty-chain head: the published SHA-256 of the empty input (FIPS
# 180-4 test vector; the definitional genesis).
ARTIFACT_GENESIS_HEAD = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

# The closed artifact-kind vocabulary (the P15.01 publication seam's set,
# mirrored verbatim).
ARTIFACT_KINDS: frozenset = frozenset({"load", "pv_generation", "price"})

# The fixture fallback labels (P15.16 / P3.31 — the single homes live in
# the Rust `forecast_fixture`; these are the same spellings, mirrored).
FIXTURE_ID = "synthetic-forecast-fixture"
PROVENANCE_SYNTHETIC = "synthetic"

# The recorded default maximum published-forecast age (one hour — the
# same recorded default the P3.13 staleness and the P15.16 source
# selection carry; a CONFIGURATION default, not a report requirement).
DEFAULT_MAX_AGE_MILLIS = 3_600_000


class EvaluationError(ValueError):
    """A malformed evaluation input — a named refusal, fail-closed."""


class AlignmentError(EvaluationError):
    """The observed and forecast series cannot be aligned exactly — a gap,
    a duplicate timestamp, an unsorted series, or a length mismatch. The
    evaluation refuses; it NEVER interpolates or silently drops a point."""


class MetricRefusalError(EvaluationError):
    """A metric refused its inputs (non-finite values, an empty series, or
    a non-positive persistence reference — the division guard)."""


class ArtifactChainError(ValueError):
    """An artifact record or chain verification refused (head mismatch,
    tampered content, invalid fields). A tamper is named, never absorbed."""


# ---------------------------------------------------------------------------
# FIM-F10.03 — the metric vocabulary (mirrors forecast_evaluation.rs)
# ---------------------------------------------------------------------------


def _validate_finite_series(values, where: str) -> list:
    validated = []
    for index, value in enumerate(values):
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise MetricRefusalError("%s[%d] must be a finite number" % (where, index))
        if not math.isfinite(value):
            raise MetricRefusalError("%s[%d] must be finite" % (where, index))
        validated.append(float(value))
    return validated


def pointwise_errors(forecast, observations) -> list:
    """forecast − observation per point (the Rust `pointwise_errors`
    semantics: length mismatch and non-finite values are named refusals;
    the accumulation order is the interval order — deterministic)."""
    forecast = _validate_finite_series(forecast, "forecast")
    observations = _validate_finite_series(observations, "observations")
    if len(forecast) != len(observations):
        raise MetricRefusalError(
            "the forecast series (%d values) and the observed series (%d values) must "
            "have the same length; a silent trim would fabricate the evaluation"
            % (len(forecast), len(observations))
        )
    if not forecast:
        raise MetricRefusalError(
            "an empty series has no evaluation; an empty evaluation is a refusal, "
            "never a zero"
        )
    return [f - o for f, o in zip(forecast, observations)]


def mean_absolute_error(errors) -> float:
    """MAE over the pointwise errors (MW). Fixed accumulation order."""
    errors = _validate_finite_series(errors, "errors")
    if not errors:
        raise MetricRefusalError("no errors were supplied; MAE of nothing is a refusal")
    return sum(abs(error) for error in errors) / len(errors)


def root_mean_squared_error(errors) -> float:
    """RMSE over the pointwise errors (MW). Fixed accumulation order."""
    errors = _validate_finite_series(errors, "errors")
    if not errors:
        raise MetricRefusalError("no errors were supplied; RMSE of nothing is a refusal")
    return math.sqrt(sum(error * error for error in errors) / len(errors))


def skill_score_vs_persistence(model_mae_mw: float, persistence_mae_mw: float) -> float:
    """`1 − MAE_model / MAE_persistence` (unitless). A non-positive
    persistence reference is a NAMED refusal — never a division by zero
    (the Rust single home's exact semantics)."""
    if not math.isfinite(model_mae_mw) or not math.isfinite(persistence_mae_mw):
        raise MetricRefusalError("the skill inputs must be finite")
    if persistence_mae_mw <= 0.0:
        raise MetricRefusalError(
            "the persistence reference MAE is %r; the skill score is defined only "
            "against a strictly positive reference — a zero reference is a named "
            "refusal, never a division by zero" % persistence_mae_mw
        )
    return 1.0 - model_mae_mw / persistence_mae_mw


# ---------------------------------------------------------------------------
# FIM-F10.03 — alignment: the exact timestamp join, gap refusal
# ---------------------------------------------------------------------------


def _check_sorted_unique(timestamps, where: str) -> None:
    for index in range(1, len(timestamps)):
        if timestamps[index] == timestamps[index - 1]:
            raise AlignmentError(
                "%s carries the duplicate timestamp %d at positions %d and %d; a "
                "duplicate instant is a data error, never a silently-joined point"
                % (where, timestamps[index], index - 1, index)
            )
        if timestamps[index] < timestamps[index - 1]:
            raise AlignmentError(
                "%s is not in ascending timestamp order (first violation at index "
                "%d); the evaluation refuses unsorted input, never silently sorts"
                % (where, index)
            )


def align_series(observed, forecast) -> list:
    """The exact timestamp join.

    `observed` and `forecast` are lists of (timestamp_millis, value)
    pairs. Both series must be ascending and duplicate-free; the join
    returns the pairs present in BOTH series, in timestamp order. A
    timestamp present in one series and missing from the other is a GAP —
    a named refusal (never interpolated, never silently dropped: a
    silently-dropped point would fabricate the evaluation).
    """
    if not isinstance(observed, list) or not isinstance(forecast, list):
        raise AlignmentError("observed and forecast must be lists of (timestamp, value) pairs")
    if not observed or not forecast:
        raise AlignmentError("an empty series has no alignment; the evaluation refuses")
    observed_map: dict = {}
    for entry in observed:
        if not isinstance(entry, (list, tuple)) or len(entry) != 2:
            raise AlignmentError("each observed entry must be a (timestamp, value) pair")
        ts, value = entry
        if isinstance(ts, bool) or not isinstance(ts, int):
            raise AlignmentError("observed timestamps must be integers (epoch UTC millis)")
        if ts in observed_map:
            raise AlignmentError(
                "the observed series carries duplicate timestamp %d" % ts
            )
        observed_map[ts] = value
    forecast_map: dict = {}
    for entry in forecast:
        if not isinstance(entry, (list, tuple)) or len(entry) != 2:
            raise AlignmentError("each forecast entry must be a (timestamp, value) pair")
        ts, value = entry
        if isinstance(ts, bool) or not isinstance(ts, int):
            raise AlignmentError("forecast timestamps must be integers (epoch UTC millis)")
        if ts in forecast_map:
            raise AlignmentError(
                "the forecast series carries duplicate timestamp %d" % ts
            )
        forecast_map[ts] = value
    _check_sorted_unique([ts for ts, _ in observed], "the observed series")
    _check_sorted_unique([ts for ts, _ in forecast], "the forecast series")
    shared = sorted(set(observed_map) & set(forecast_map))
    if len(shared) != len(observed_map) or len(shared) != len(forecast_map):
        missing_in_observed = sorted(set(forecast_map) - set(observed_map))
        missing_in_forecast = sorted(set(observed_map) - set(forecast_map))
        raise AlignmentError(
            "the series do not align exactly: timestamps %s are missing from the "
            "observed series and %s are missing from the forecast series; gaps are "
            "REFUSED — the evaluation never interpolates or silently drops a point "
            "(a dropped point would fabricate the evaluation)"
            % (missing_in_forecast, missing_in_observed)
        )
    if not shared:
        raise AlignmentError("the series share no timestamp; there is nothing to evaluate")
    return [(ts, forecast_map[ts], observed_map[ts]) for ts in shared]


def persistence_reference(history, horizon_steps: int) -> list:
    """The persistence.v1 mirror (the P15.02 frozen rule set): the last
    observed value carried forward for every horizon step. Mirrors the
    Rust single home; pinned by the same hand-computed vectors."""
    if isinstance(horizon_steps, bool) or not isinstance(horizon_steps, int):
        raise EvaluationError("horizon_steps must be an integer")
    if horizon_steps < 1:
        raise MetricRefusalError("the horizon must ask for at least 1 step")
    series = _validate_finite_series(history, "history")
    if not series:
        raise MetricRefusalError(
            "persistence requires at least one observed value; an empty history is "
            "refused, never zero-filled"
        )
    return [series[-1]] * horizon_steps


def evaluate_against_persistence(observed, forecast, reference) -> dict:
    """The full evaluation over aligned series: MAE / RMSE / skill of the
    forecast against a REFERENCE forecast (typically the persistence
    baseline), computed on the exactly-aligned timestamps.

    Returns a dict with the aligned point count, the metric values, and
    the HONESTY fields (no run gate, no skill claim — the P15.05 result
    table stays EMPTY until the human/lab run gate records a run).

    `reference` is a list of (timestamp_millis, value) pairs like the
    other series; it is aligned through the SAME gap-refusing join, so a
    reference that skips a timestamp refuses the evaluation too.
    """
    aligned_of = align_series(observed, forecast)
    aligned_ref = align_series(observed, reference)
    if [point[0] for point in aligned_of] != [point[0] for point in aligned_ref]:
        raise AlignmentError(
            "the reference series does not cover exactly the observed/forecast "
            "timestamps; the evaluation refuses — a reference that skips a point "
            "would fabricate the comparison"
        )
    forecast_values = [point[1] for point in aligned_of]
    observed_values = [point[2] for point in aligned_of]
    reference_values = [point[1] for point in aligned_ref]
    errors = pointwise_errors(forecast_values, observed_values)
    reference_errors = pointwise_errors(reference_values, observed_values)
    model_mae = mean_absolute_error(errors)
    model_rmse = root_mean_squared_error(errors)
    reference_mae = mean_absolute_error(reference_errors)
    skill = skill_score_vs_persistence(model_mae, reference_mae)
    return {
        "aligned_points": len(errors),
        "mae_mw": model_mae,
        "rmse_mw": model_rmse,
        "reference": "persistence",
        "reference_mae_mw": reference_mae,
        "skill_score_vs_persistence": skill,
        # HONESTY (binding): a computed metric is mathematics, not a claim.
        # The P15.05 result table stays EMPTY until the human/lab run gate
        # records a real backtesting run over real data; this dict is NOT a
        # result cell and must never be presented as one.
        "no_skill_claim": (
            "NO-SKILL-UNTIL-EVALUATED: these values are deterministic arithmetic "
            "over the SUPPLIED series (mathematics, not a claim). The P15.05 "
            "result table stays EMPTY in every record until a real backtesting "
            "run completes (HUMAN/LAB GATE); no skill is claimed anywhere."
        ),
        "p15_05_result_cells": "EMPTY (no evaluation run exists)",
    }


# ---------------------------------------------------------------------------
# FIM-F10.04 — the artifact digest chain (mirrors the Rust chain §8)
# ---------------------------------------------------------------------------


def render_values_canonical(values) -> str:
    """The canonical payload rendering for a forecast VALUES vector:
    comma-joined fixed 6-decimal rendering (`%.6f`). Byte-identical with
    the Rust `{:.6}` rendering for the same values (the replay identity
    between the mirrors); a rendering change is a NEW algorithm version,
    never a silent one."""
    rendered = []
    for value in values:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise EvaluationError("values must be numbers")
        value = float(value)
        if not math.isfinite(value):
            # A non-finite value cannot be an identity digest input in the
            # pinned format — refused rather than rendered language-
            # dependently.
            raise EvaluationError(
                "a non-finite value cannot be rendered into the artifact identity"
            )
        rendered.append("%.6f" % value)
    return ",".join(rendered)


def sha256_hex(data: bytes) -> str:
    """SHA-256 over the bytes, lowercase hex (hashlib; FIPS 180-4)."""
    return hashlib.sha256(data).hexdigest()


def artifact_payload_digest(payload_canonical: str) -> str:
    """The payload identity digest: SHA-256 over the payload's canonical
    rendering alone."""
    return sha256_hex(payload_canonical.encode("utf-8"))


def artifact_chain_digest(previous_head: str, canonical: str) -> str:
    """The chain digest: SHA-256 over
    `DOMAIN ‖ 0x1F ‖ previous_head ‖ 0x1F ‖ canonical` (UTF-8). Byte-
    identical with the Rust `artifact_chain_digest`."""
    digest_input = ARTIFACT_CHAIN_DOMAIN + b"\x1f" + previous_head.encode("utf-8") + b"\x1f" + canonical.encode("utf-8")
    return sha256_hex(digest_input)


def build_artifact_record(
    *,
    artifact_ref: str,
    artifact_kind: str,
    version: str,
    produced_at_millis: int,
    source_ref: str,
    payload_canonical: str,
    previous_head: str,
) -> dict:
    """Builds and validates one artifact record (the P15.06 shape): the
    field rules first (closed kind, whitespace-free version, non-empty
    provenance and payload), then both digests from the content."""
    if not isinstance(produced_at_millis, int) or isinstance(produced_at_millis, bool):
        raise ArtifactChainError("produced_at_millis must be an integer (UTC millis)")
    if artifact_kind not in ARTIFACT_KINDS:
        raise ArtifactChainError(
            "artifact_kind %r is outside the closed vocabulary %s"
            % (artifact_kind, sorted(ARTIFACT_KINDS))
        )
    if not isinstance(version, str) or not version or any(c.isspace() for c in version):
        raise ArtifactChainError(
            "the version must be a non-empty whitespace-free string"
        )
    if not isinstance(source_ref, str) or not source_ref.strip():
        raise ArtifactChainError(
            "the source_ref must be non-empty (provenance cannot detach)"
        )
    if not isinstance(payload_canonical, str) or not payload_canonical:
        raise ArtifactChainError("the payload rendering must be non-empty")
    if not isinstance(previous_head, str) or not previous_head:
        raise ArtifactChainError("the previous_head proof must be non-empty")
    record = {
        "artifact_ref": artifact_ref,
        "artifact_kind": artifact_kind,
        "version": version,
        "produced_at_millis": produced_at_millis,
        "source_ref": source_ref,
        "payload_canonical": payload_canonical,
        "payload_digest": artifact_payload_digest(payload_canonical),
        "previous_head": previous_head,
        "digest": artifact_chain_digest(
            previous_head,
            "kind:%s|version:%s|produced_at:%d|source_ref:%s|payload:%s"
            % (artifact_kind, version, produced_at_millis, source_ref, payload_canonical),
        ),
        "digest_algorithm": ARTIFACT_CHAIN_ALGORITHM,
        # HONESTY: unkeyed SHA-256 is an integrity/replay stamp — NOT a
        # signature, NOT a security primitive (no signing identity exists;
        # the keyed audit chain is the deployment-gated posture).
        "digest_note": (
            "UNKEYED SHA-256 integrity/replay stamp; explicitly NOT a signature "
            "and NOT a security primitive."
        ),
    }
    return record


def _canonical_string_of(record: dict) -> str:
    return "kind:%s|version:%s|produced_at:%d|source_ref:%s|payload:%s" % (
        record["artifact_kind"],
        record["version"],
        record["produced_at_millis"],
        record["source_ref"],
        record["payload_canonical"],
    )


def _validate_record_fields(record: dict) -> None:
    if record.get("artifact_kind") not in ARTIFACT_KINDS:
        raise ArtifactChainError(
            "artifact_kind %r is outside the closed vocabulary" % (record.get("artifact_kind"),)
        )
    version = record.get("version")
    if not isinstance(version, str) or not version or any(c.isspace() for c in version):
        raise ArtifactChainError("the version must be non-empty and whitespace-free")
    if not str(record.get("source_ref", "")).strip():
        raise ArtifactChainError("the source_ref must be non-empty")
    if not record.get("payload_canonical"):
        raise ArtifactChainError("the payload rendering must be non-empty")


def verify_artifact_record(record: dict, expected_previous_head: str) -> None:
    """Standalone verification (the replay path): field rules, the exact
    expected head proof, and both digests recomputed from the content. A
    mismatch is a named tamper — never absorbed."""
    _validate_record_fields(record)
    if record.get("previous_head") != expected_previous_head:
        raise ArtifactChainError(
            "ARTIFACT_CHAIN_HEAD_MISMATCH: the record names head %r but %r was "
            "expected; every append must prove it saw the chain it extends"
            % (record.get("previous_head"), expected_previous_head)
        )
    if record.get("payload_digest") != artifact_payload_digest(record["payload_canonical"]):
        raise ArtifactChainError(
            "ARTIFACT_CHAIN_DIGEST_INVALID: the artifact %r fails payload digest "
            "verification — the content does not match its committed digest; a "
            "tamper is named, never absorbed" % (record.get("artifact_ref"),)
        )
    if record.get("digest") != artifact_chain_digest(
        record["previous_head"], _canonical_string_of(record)
    ):
        raise ArtifactChainError(
            "ARTIFACT_CHAIN_DIGEST_INVALID: the artifact %r fails chain digest "
            "verification — a tamper is named, never absorbed"
            % (record.get("artifact_ref"),)
        )


def verify_artifact_chain(records) -> str:
    """Walks the whole chain: field rules, head-link continuity from the
    genesis head, and every digest recomputation. Returns the final head;
    the first failure raises the named error with the offending position."""
    expected_head = ARTIFACT_GENESIS_HEAD
    for position, record in enumerate(records):
        try:
            verify_artifact_record(record, expected_head)
        except ArtifactChainError as error:
            raise ArtifactChainError(
                "at chain position %d: %s" % (position, error)
            ) from error
        expected_head = record["digest"]
    return expected_head


def replay_artifact(record: dict, inputs: dict) -> dict:
    """The replay path: re-derive the artifact from its DECLARED INPUTS and
    require identity with the record (replay requires artifact + inputs
    identity — FIM-F10.04).

    `inputs` carries: artifact_ref, artifact_kind, version,
    produced_at_millis, source_ref, values (the forecast values vector).
    The payload rendering is recomputed from the values; BOTH digests are
    recomputed; any divergence from the record raises the named tamper
    error. The same inputs reproduce the same artifact; one changed input
    breaks identity."""
    replayed_payload = render_values_canonical(inputs.get("values"))
    replayed = build_artifact_record(
        artifact_ref=inputs.get("artifact_ref", ""),
        artifact_kind=inputs.get("artifact_kind", ""),
        version=inputs.get("version", ""),
        produced_at_millis=inputs.get("produced_at_millis", -1),
        source_ref=inputs.get("source_ref", ""),
        payload_canonical=replayed_payload,
        previous_head=record.get("previous_head", ""),
    )
    if (
        replayed["payload_digest"] != record.get("payload_digest")
        or replayed["digest"] != record.get("digest")
        or replayed["artifact_ref"] != record.get("artifact_ref")
        or replayed["version"] != record.get("version")
        or replayed["produced_at_millis"] != record.get("produced_at_millis")
    ):
        raise ArtifactChainError(
            "REPLAY_IDENTITY_BROKEN: the declared inputs do not reproduce the "
            "artifact %r; replay requires (artifact + inputs) identity — a "
            "divergence is a named refusal, never a silent accept"
            % (record.get("artifact_ref"),)
        )
    verify_artifact_record(record, record.get("previous_head", ""))
    return replayed


# ---------------------------------------------------------------------------
# FIM-F10.02 (worker-side mirror) — the persistence-baseline fallback
# selection. The single home of the persistence RULE is the Rust
# `forecast_baselines.rs` (mirrored above as `persistence_reference`); the
# envelope-level staleness evaluation is the Rust P3.13/P15.16 surfaces —
# neither is restated. This mirror exists because NO Rust→python runtime
# seam exists (recorded in the module header): the worker side composes
# the same pure selection, pinned by the same boundary vectors.
# ---------------------------------------------------------------------------


def select_ingested_forecast(published, history, horizon_steps, max_age_millis, now_millis):
    """The ingestion-side source selection (the exact mirror of the Rust
    `select_ingested_forecast`).

    `published` is None or {"values": [...], "fetched_at_millis": int}.

    The P2.08 equality convention: age == max_age is FRESH (the live
    forecast is consumed); age == max_age + 1 falls back. A clock read
    before the fetch instant is `clock_before_fetch` — never a silent
    fresh verdict. A fallback over an empty/non-finite history escalates
    to the LABELED synthetic fixture reference (P15.16) — never a
    zero-fill."""
    reason = None
    if published is None:
        reason = "no_ingested_forecast"
    else:
        fetched_at = published.get("fetched_at_millis")
        if not isinstance(fetched_at, int) or isinstance(fetched_at, bool):
            raise EvaluationError("published.fetched_at_millis must be an integer")
        values = _validate_finite_series(published.get("values", []), "published.values")
        if now_millis < fetched_at:
            reason = "clock_before_fetch"
        elif now_millis - fetched_at > max_age_millis:
            reason = "max_age_exceeded"
    if reason is None:
        return {
            "decision": "live_ingested_forecast",
            "values": list(published["values"]),
            "fetched_at_millis": published["fetched_at_millis"],
        }
    try:
        values = persistence_reference(history, horizon_steps)
    except MetricRefusalError:
        return {
            "decision": "synthetic_fixture_fallback",
            "reason": reason,
            "fixture_id": FIXTURE_ID,
            "provenance_label": PROVENANCE_SYNTHETIC,
            "no_skill_claim": (
                "the synthetic fixture stays the LABELED fallback source (P15.16); "
                "the label travels with every value; no skill is claimed"
            ),
        }
    return {
        "decision": "persistence_baseline_fallback",
        "reason": reason,
        "values": values,
        "no_skill_claim": (
            "a persistence baseline never claims skill (P15.02 NO_SKILL_STATEMENT "
            "posture); the fallback is a recorded degradation, never a silent "
            "substitution"
        ),
    }


# ---------------------------------------------------------------------------
# In-module self-tests (hand-computed; NEVER RUN in this loop — the human
# gate owns the run. Run instruction:
#   python -I apps/worker/python/forecast_worker/evaluation.py --self-test
# The pytest suite mirrors these cases in tests/test_forecast_evaluation.py.)
# ---------------------------------------------------------------------------


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test."""
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- SHA-256: the published FIPS 180-4 vectors -----------------------
    check(
        "sha256_matches_published_vectors",
        sha256_hex(b"") == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        and sha256_hex(b"abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "empty input and 'abc' (the published FIPS vectors)",
    )

    # ---- metrics: hand-computed over constructed inputs ------------------
    observed = [(1000, 4.0), (2000, 4.0), (3000, 6.0), (4000, 6.0)]
    forecast = [(1000, 3.0), (2000, 4.0), (3000, 6.0), (4000, 6.0)]
    reference = [(1000, 3.0), (2000, 3.0), (3000, 5.0), (4000, 5.0)]
    evaluation = evaluate_against_persistence(observed, forecast, reference)
    # errors = [-1, 0, 0, 0] → MAE = 0.25; RMSE = sqrt(1/4) = 0.5
    # reference errors = [1, 1, 1, 1] → MAE_ref = 1.0; skill = 1 − 0.25/1.0 = 0.75
    check(
        "metrics_match_the_hand_computed_values",
        evaluation["mae_mw"] == 0.25
        and evaluation["rmse_mw"] == 0.5
        and evaluation["reference_mae_mw"] == 1.0
        and evaluation["skill_score_vs_persistence"] == 0.75
        and evaluation["aligned_points"] == 4,
        "observed [4,4,6,6], forecast [3,4,6,6], reference [3,3,5,5] → "
        "MAE 0.25, RMSE 0.5, MAE_ref 1.0, skill 0.75 (exact)",
    )
    check(
        "evaluation_carries_the_no_skill_honesty_fields",
        "NO-SKILL-UNTIL-EVALUATED" in evaluation["no_skill_claim"]
        and evaluation["p15_05_result_cells"] == "EMPTY (no evaluation run exists)",
        "the result dict states that the P15.05 cells stay EMPTY (HUMAN/LAB GATE)",
    )

    # A worse-than-reference model earns a NEGATIVE skill (still exact):
    check(
        "a_worse_model_scores_negative_skill",
        skill_score_vs_persistence(2.0, 1.0) == -1.0,
        "MAE_model 2.0, MAE_ref 1.0 → skill −1.0 (exact)",
    )

    def zero_reference() -> None:
        skill_score_vs_persistence(0.5, 0.0)

    _expects_error(results, zero_reference, "zero_persistence_reference_refused")

    def non_finite_metric() -> None:
        pointwise_errors([float("nan")], [1.0])

    _expects_error(results, non_finite_metric, "non_finite_metric_input_refused")

    def length_mismatch() -> None:
        pointwise_errors([1.0, 2.0], [1.0])

    _expects_error(results, length_mismatch, "length_mismatch_refused")

    # ---- alignment: gap refusal (never interpolate silently) -------------
    def gap_in_observed() -> None:
        align_series(
            [(1000, 4.0), (3000, 6.0)],
            [(1000, 3.0), (2000, 4.0), (3000, 6.0)],
        )

    _expects_error(results, gap_in_observed, "gap_in_observed_refused_never_interpolated")

    def duplicate_timestamp() -> None:
        align_series(
            [(1000, 4.0), (1000, 5.0)],
            [(1000, 3.0)],
        )

    _expects_error(results, duplicate_timestamp, "duplicate_timestamp_refused")

    def unsorted_series() -> None:
        align_series(
            [(2000, 4.0), (1000, 4.0)],
            [(1000, 3.0), (2000, 4.0)],
        )

    _expects_error(results, unsorted_series, "unsorted_series_refused_never_sorted")

    aligned = align_series(
        [(1000, 4.0), (2000, 4.0), (3000, 6.0)],
        [(1000, 3.0), (2000, 4.0), (3000, 6.0)],
    )
    check(
        "exact_join_returns_shared_timestamps_in_order",
        aligned == [(1000, 3.0, 4.0), (2000, 4.0, 4.0), (3000, 6.0, 6.0)],
        "the join returns (ts, forecast, observed) triples in timestamp order",
    )

    # ---- persistence reference mirror -------------------------------------
    check(
        "persistence_reference_carries_the_last_value",
        persistence_reference([-0.4, -0.5, -0.6], 3) == [-0.6, -0.6, -0.6],
        "history [-0.4, -0.5, -0.6], horizon 3 → [-0.6, -0.6, -0.6] (the P15.02 vector)",
    )

    def empty_history() -> None:
        persistence_reference([], 1)

    _expects_error(results, empty_history, "persistence_empty_history_refused")

    # ---- the artifact digest chain (FIM-F10.04) ----------------------------
    check(
        "render_values_canonical_is_the_pinned_cross_language_format",
        render_values_canonical([-0.6, -0.6]) == "-0.600000,-0.600000"
        and render_values_canonical([8.0, 1.0]) == "8.000000,1.000000"
        and render_values_canonical([42.5]) == "42.500000",
        "%.6f renders byte-identically with the Rust `{:.6}` for the fixture values",
    )

    payload_1 = render_values_canonical([-0.6, -0.6])
    payload_2 = render_values_canonical([8.0, 1.0])
    payload_3 = render_values_canonical([42.5])
    record_1 = build_artifact_record(
        artifact_ref="artifact:load:v1",
        artifact_kind="load",
        version="v1",
        produced_at_millis=1000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_1,
        previous_head=ARTIFACT_GENESIS_HEAD,
    )
    record_2 = build_artifact_record(
        artifact_ref="artifact:pv_generation:v2",
        artifact_kind="pv_generation",
        version="v2",
        produced_at_millis=2000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_2,
        previous_head=record_1["digest"],
    )
    record_3 = build_artifact_record(
        artifact_ref="artifact:price:v3",
        artifact_kind="price",
        version="v3",
        produced_at_millis=3000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_3,
        previous_head=record_2["digest"],
    )
    # The pinned digests (computed with `python -I` — the same vectors the
    # Rust tests pin: cross-language byte identity).
    check(
        "chain_digests_match_the_pinned_vectors",
        record_1["payload_digest"] == "668507de8a729fdcc4e208a8f1642fb64aa89dd62136abaa38e910986a4d17ff"
        and record_1["digest"] == "5fd7adb79b9dbe196a448878c97a504bcf55c15652cc623d6b6c983f34d4cf13"
        and record_2["digest"] == "dc5ffe60523f532c8ae44908cc6cf4b9675df54fa3e5bd513bc20977ccef9e86"
        and record_3["digest"] == "9dae0cd1f9f41fffbda4200587aa32472c3d4bc1e500dbee353e4eecca63d127",
        "the three chained digests equal the python -I computations (shared "
        "with the Rust tests)",
    )
    check(
        "chain_verification_walks_the_whole_chain",
        verify_artifact_chain([record_1, record_2, record_3]) == record_3["digest"],
        "the walk from the genesis head returns the final head",
    )

    def tampered_payload() -> None:
        forged = dict(record_1)
        forged["payload_canonical"] = render_values_canonical([-0.7, -0.6])
        verify_artifact_record(forged, ARTIFACT_GENESIS_HEAD)

    _expects_error(results, tampered_payload, "tampered_payload_named_never_absorbed")

    def forged_head() -> None:
        forged = dict(record_2)
        forged["previous_head"] = ARTIFACT_GENESIS_HEAD
        verify_artifact_record(forged, record_1["digest"])

    _expects_error(results, forged_head, "stale_head_proof_refused")

    def bad_kind() -> None:
        build_artifact_record(
            artifact_ref="artifact:temperature:v1",
            artifact_kind="temperature",
            version="v1",
            produced_at_millis=1000,
            source_ref="vendor://fixture/feed",
            payload_canonical=payload_1,
            previous_head=ARTIFACT_GENESIS_HEAD,
        )

    _expects_error(results, bad_kind, "closed_artifact_kind_vocabulary")

    def whitespace_version() -> None:
        build_artifact_record(
            artifact_ref="artifact:load:v1b",
            artifact_kind="load",
            version="v 1",
            produced_at_millis=1000,
            source_ref="vendor://fixture/feed",
            payload_canonical=payload_1,
            previous_head=ARTIFACT_GENESIS_HEAD,
        )

    _expects_error(results, whitespace_version, "whitespace_version_refused")

    # Replay identity: the same inputs reproduce the artifact; one changed
    # input breaks identity (named refusal).
    replayed = replay_artifact(
        record_1,
        {
            "artifact_ref": "artifact:load:v1",
            "artifact_kind": "load",
            "version": "v1",
            "produced_at_millis": 1000,
            "source_ref": "vendor://fixture/feed",
            "values": [-0.6, -0.6],
        },
    )
    check(
        "replay_from_the_same_inputs_reproduces_the_artifact",
        replayed["digest"] == record_1["digest"]
        and replayed["payload_digest"] == record_1["payload_digest"],
        "replay requires (artifact + inputs) identity — same inputs, same digests",
    )

    def drifted_replay() -> None:
        replay_artifact(
            record_1,
            {
                "artifact_ref": "artifact:load:v1",
                "artifact_kind": "load",
                "version": "v1",
                "produced_at_millis": 1001,
                "source_ref": "vendor://fixture/feed",
                "values": [-0.6, -0.6],
            },
        )

    _expects_error(results, drifted_replay, "replay_identity_break_on_changed_input")

    # ---- the fallback selection boundaries (FIM-F10.02 mirror) ------------
    fresh = select_ingested_forecast(
        {"values": [612.5, 610.0], "fetched_at_millis": 0},
        [-0.5, -0.6],
        2,
        DEFAULT_MAX_AGE_MILLIS,
        DEFAULT_MAX_AGE_MILLIS,
    )
    check(
        "age_at_max_age_is_fresh_p2_08",
        fresh["decision"] == "live_ingested_forecast"
        and fresh["values"] == [612.5, 610.0],
        "age == max_age is FRESH (the P2.08 equality convention)",
    )
    stale = select_ingested_forecast(
        {"values": [612.5], "fetched_at_millis": 0},
        [-0.5, -0.6],
        2,
        DEFAULT_MAX_AGE_MILLIS,
        DEFAULT_MAX_AGE_MILLIS + 1,
    )
    check(
        "max_age_plus_one_falls_back_to_persistence",
        stale["decision"] == "persistence_baseline_fallback"
        and stale["reason"] == "max_age_exceeded"
        and stale["values"] == [-0.6, -0.6],
        "age == max_age + 1 → the persistence fallback with the reason carried "
        "(last value [-0.6] carried forward)",
    )
    absent = select_ingested_forecast(None, [-0.5, -0.6], 2, 3_600_000, 1_000)
    check(
        "no_ingested_forecast_falls_back_and_reports_the_reason",
        absent["decision"] == "persistence_baseline_fallback"
        and absent["reason"] == "no_ingested_forecast",
        "no published forecast → the named fallback reason",
    )
    clock = select_ingested_forecast(
        {"values": [612.5], "fetched_at_millis": 10_000},
        [-0.5, -0.6],
        2,
        3_600_000,
        5_000,
    )
    check(
        "clock_before_fetch_is_stale_until_resolved",
        clock["decision"] == "persistence_baseline_fallback"
        and clock["reason"] == "clock_before_fetch",
        "the evaluation clock read before the fetch instant is never silently fresh",
    )
    fixture = select_ingested_forecast(None, [], 2, 3_600_000, 1_000)
    check(
        "empty_history_escalates_to_the_labeled_synthetic_fixture",
        fixture["decision"] == "synthetic_fixture_fallback"
        and fixture["fixture_id"] == "synthetic-forecast-fixture"
        and fixture["provenance_label"] == "synthetic",
        "P15.16: the fixture stays the LABELED synthetic fallback source; the "
        "label travels with the decision; no values are invented here",
    )
    check(
        "fallback_outputs_carry_the_no_skill_posture",
        "never claims skill" in stale["no_skill_claim"],
        "every fallback decision carries the standing no-skill posture",
    )

    return results


def _expects_error(results: list, fn, name: str) -> None:
    """Self-test helper: fn must raise an EvaluationError subclass
    (MetricRefusalError / AlignmentError / ArtifactChainError /
    EvaluationError) and nothing else."""
    try:
        fn()
    except (EvaluationError, ArtifactChainError):
        results.append({"name": name, "passed": True, "detail": "raised a named refusal"})
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {"name": name, "passed": False, "detail": "raised %r instead of a named refusal" % error}
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    import sys

    # Console-encoding robustness: the self-test detail strings carry
    # typographic characters (→, −) that some consoles (e.g. a Windows
    # cp1252 terminal) cannot encode. Reconfigure stdout/stderr to UTF-8
    # with replacement — the VERDICTS are ASCII; a console that still
    # cannot render a detail loses only decoration, never the pass/fail
    # record.
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8", errors="replace")

    if "--self-test" in sys.argv[1:]:
        _records = run_self_tests()
        _failures = [r for r in _records if not r["passed"]]
        for _r in _records:
            print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
        print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
        raise SystemExit(1 if _failures else 0)
    sys.stderr.write(
        "forecast evaluation harness: this module has no stdin transport; "
        "run `python evaluation.py --self-test` or import it (see README)\n"
    )
    raise SystemExit(2)
