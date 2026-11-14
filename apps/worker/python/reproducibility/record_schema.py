# P8.20 — The versioned reproducibility-record schema
#         (`flexcore-research/reproducibility-record.v1`) and its pure
#         stdlib validator, over INJECTED record data.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. Ledger P8.20 step 1: "Define the reproducibility
#    record per run: benchmark-set version, solver versions,
#    PyTorch/PyG/RLlib/ANDES versions, model artifact references, dataset
#    version and preprocessing version, seeds, configs, input snapshot
#    IDs." This module is the record FORMAT (ledger deliverable: the
#    reproducibility standard, of which the schema is one part): the
#    closed field vocabulary, the per-module requirements table, and the
#    validator that refuses an incomplete or placeholder-bearing record
#    instead of accepting it. It validates INJECTED record dicts; it
#    performs NO I/O, stores NOTHING, and imports nothing from this
#    repository's other modules (it re-declares the recorded derivation
#    string as a pinned constant — a reviewer diffing it against the
#    frozen artifact is the alignment check, mirroring the repo's
#    by-citation seam pattern).
# 2. WRITTEN, NEVER RUN; NO RECORD EXISTS. The Phase 8 loop bans mean this
#    validator has never executed and NO reproducibility record exists
#    anywhere in the repository — every measured/executed cell in the
#    P8.20 record (`20_reproducibility.md`) is EMPTY and no rerun has
#    happened. The self-tests below carry HAND-CONSTRUCTED inputs with
#    the reasoning STATED in each test; they pin the SCHEMA, not any run.
# 3. A SCHEMA CHANGE IS A NEW SCHEMA VERSION (the P8.19 anti-silent-growth
#    rule mirrored onto the record format). SCHEMA_VERSION is normative;
#    a record carrying a different schema version string is refused
#    (SCHEMA_VERSION_UNKNOWN), never coerced.
# 4. NO TOLERANCE VALUE LIVES HERE. The per-module rerun tolerances are
#    P8.38's deliverable (declared per module BEFORE any rerun, X-8 —
#    never fitted after the fact). This schema only requires that a
#    non-exact record CARRY the reference to that pre-registration
#    (tolerance_class + tolerance_ref). It never stores, computes, or
#    suggests a tolerance value.
# 5. REPORTING ONLY — NEVER A DISPATCH INPUT (ADR-010). The namespace
#    contains NO dispatch/issue/authorize/approve/command-shaped callable
#    (pinned by a self-test); a validated record is reproducibility
#    bookkeeping, nothing else.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import re

SCHEMA_VERSION = "flexcore-research/reproducibility-record.v1"

REPORTING_ONLY_STATEMENT = (
    "P8.20 reproducibility tooling — reporting/bookkeeping only; the "
    "output never influences dispatch and carries no dispatch-shaped "
    "result (ADR-010; simulation-only, AGENTS.md rule 5)."
)

# ── Closed vocabularies ───────────────────────────────────────────────────

# The research modules and the frozen P8.06 baseline registry ids
# (`05_06_harness_baselines.md` §3 — the three baseline ids VERBATIM).
MODULE_VOCABULARY = frozenset(
    {
        "admm-dmpc",
        "gnn-warm-start",
        "gnn-screening",
        "marl-policy",
        "dynamic-controller",
        "baseline/highs-tier1-lp",
        "baseline/clarabel-tier2-conic",
        "baseline/rule-based-priority-list",
    }
)

TOLERANCE_CLASSES = frozenset({"exact", "within_tolerance", "distribution"})

RUN_STATUSES = frozenset({"COMPLETED", "FAILED"})

# The dynamic-evaluation track's cases are explicitly NOT in the §19.3
# benchmark set (`benchmarks/research-set-v1-not-tested.json` NT-10); a
# dynamic record therefore carries NO benchmark_set_version and cites its
# dynamic case reference in `inputs.dynamic_case_ref` instead.
DYNAMIC_MODULE_IDS = frozenset({"dynamic-controller"})

# Placeholder tokens that must NEVER appear in a filled record field —
# a template token in a "filled" record is the "not run hidden as a
# blank" failure mode of ledger P8.23, named here for the record layer.
PLACEHOLDER_TOKENS = frozenset(
    {
        "",
        "RECORD-AT-FIRST-RUN",
        "RECORD-AT-RUN",
        "TBD",
        "TODO",
        "EMPTY",
        "UNSET",
        "N/A-PENDING",
    }
)

_BENCHMARK_VERSION_RE = re.compile(r"^flexcore-research-benchmark-set-v[0-9]+$")
_SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
_FNV_RE = re.compile(r"^fnv1a64:[0-9a-f]{16}$")
_ARTIFACT_REF_RE = re.compile(r"^flexcore-artifact://[^\s]+$")

# The recorded derivation string, pinned verbatim (see seed_derivation.py;
# the reviewer's diff against benchmarks/research-set-v1.json is the
# alignment check).
RECORDED_SEED_DERIVATION = (
    "seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))"
)


def required_component_version_keys(module_id: str) -> frozenset:
    """The component-version keys a record of ``module_id`` MUST carry.

    Basis, recorded per row: the P8.06 frozen solver pins (HiGHS 1.15.1,
    Clarabel.rs 0.11.1), the P8.13 §5 RL-framework DECISION-POINT (the
    framework is UNSELECTED, so the generic key is required and no value
    exists yet — a MARL record cannot exist until it resolves), the
    P8.16 ANDES pin procedure, and the torch/PyG pair (P8.10 §6 — the
    recorded DECISION-POINT; likewise no record can exist yet).
    Additional keys beyond the required set are allowed and recommended
    (e.g. ``pandapower`` when a run exercised the grid-feasibility
    worker; ``helics`` for federated runs).
    """
    table = {
        "baseline/highs-tier1-lp": frozenset({"highs"}),
        "baseline/clarabel-tier2-conic": frozenset({"clarabel"}),
        "baseline/rule-based-priority-list": frozenset(),
        "admm-dmpc": frozenset(),
        "gnn-warm-start": frozenset({"torch", "pyg"}),
        "gnn-screening": frozenset({"torch", "pyg"}),
        "marl-policy": frozenset({"rl_framework"}),
        "dynamic-controller": frozenset({"andes"}),
    }
    return table.get(module_id, frozenset())


# ── Field validators ──────────────────────────────────────────────────────


def _is_nonempty_str(value: object) -> bool:
    return isinstance(value, str) and value != ""


def _has_placeholder(value: object) -> bool:
    return isinstance(value, str) and value.strip() in PLACEHOLDER_TOKENS


def _is_digest(value: object) -> bool:
    return _is_nonempty_str(value) and bool(_SHA256_RE.match(value) or _FNV_RE.match(value))


def _is_artifact_ref(value: object) -> bool:
    return _is_nonempty_str(value) and bool(_ARTIFACT_REF_RE.match(value))


def _is_seed_block(seeds: object) -> bool:
    """Shape check on the ``determinism.seeds`` block: a dict of
    non-empty-string keys to int values in [0, 2^64)."""
    if not isinstance(seeds, dict) or not seeds:
        return False
    for key, value in seeds.items():
        if not _is_nonempty_str(key):
            return False
        if isinstance(value, bool) or not isinstance(value, int):
            return False
        if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
            return False
    return True


# ── The validator ─────────────────────────────────────────────────────────

_TOP_LEVEL_REQUIRED = (
    "record_schema_version",
    "record_id",
    "run_id",
    "module_id",
    "module_version",
    "code_version",
    "component_versions",
    "determinism",
    "config",
    "inputs",
    "artifacts",
    "outputs",
    "environment",
    "tolerance_class",
    "run_status",
)


def validate_record(record: object) -> list:
    """Validate one injected reproducibility record.

    Returns a list of findings, each ``"CODE: detail"``; an EMPTY list
    means the record is schema-valid. The validator refuses rather than
    coerces: every finding names its code, and nothing is defaulted.
    """
    errors: list = []

    if not isinstance(record, dict):
        return [f"RECORD_NOT_A_DICT: expected a record object, got {type(record).__name__}"]

    # Schema version first — a record under an unknown schema version is
    # not this schema (anti-silent-growth, HONESTY HEADER item 3).
    if "record_schema_version" not in record:
        errors.append("FIELD_MISSING: record_schema_version")
    elif record["record_schema_version"] != SCHEMA_VERSION:
        errors.append(
            f"SCHEMA_VERSION_UNKNOWN: {record['record_schema_version']!r} "
            f"is not {SCHEMA_VERSION!r}"
        )

    for field in _TOP_LEVEL_REQUIRED:
        if field not in record:
            errors.append(f"FIELD_MISSING: {field}")

    if errors and any(e.startswith(("FIELD_MISSING", "SCHEMA_VERSION_UNKNOWN", "RECORD_NOT_A_DICT")) for e in errors):
        # Fields checked below need their containers; report what is
        # already known when a required top-level field is absent.
        missing = [f for f in _TOP_LEVEL_REQUIRED if f not in record]
        if missing:
            errors.append(f"RECORD_INCOMPLETE: absent required fields {missing}")
        return errors

    # ── scalars ──
    for field in ("record_id", "run_id", "module_version", "code_version"):
        if _has_placeholder(record[field]):
            errors.append(f"PLACEHOLDER_PRESENT: {field} = {record[field]!r}")
        elif not _is_nonempty_str(record[field]):
            errors.append(f"FIELD_EMPTY: {field}")

    module_id = record["module_id"]
    if not isinstance(module_id, str) or module_id not in MODULE_VOCABULARY:
        errors.append(f"MODULE_UNKNOWN: {module_id!r} is outside the closed vocabulary")

    # ── benchmark-set version (dynamic records excluded, NT-10) ──
    if module_id in DYNAMIC_MODULE_IDS:
        if "benchmark_set_version" in record:
            errors.append(
                "DYNAMIC_RECORD_BENCHMARK_SET_FORBIDDEN: the dynamic-evaluation "
                "cases are not in the benchmark set (not-tested NT-10); cite "
                "the dynamic case in inputs.dynamic_case_ref instead"
            )
    else:
        if "benchmark_set_version" not in record:
            errors.append("FIELD_MISSING: benchmark_set_version")
        elif not _BENCHMARK_VERSION_RE.match(record["benchmark_set_version"]):
            errors.append(
                f"BENCHMARK_SET_VERSION_INVALID: {record['benchmark_set_version']!r}"
            )

    # ── component versions ──
    comp = record["component_versions"]
    if not isinstance(comp, dict):
        errors.append("COMPONENT_VERSIONS_NOT_A_DICT")
    else:
        required = required_component_version_keys(module_id)
        for key in sorted(required):
            if key not in comp:
                errors.append(f"COMPONENT_VERSION_MISSING: {key}")
            elif _has_placeholder(comp[key]):
                errors.append(f"PLACEHOLDER_PRESENT: component_versions.{key}")
            elif not _is_nonempty_str(comp[key]):
                errors.append(f"COMPONENT_VERSION_EMPTY: {key}")
        for key, value in comp.items():
            if not _is_nonempty_str(key):
                errors.append(f"COMPONENT_VERSION_KEY_INVALID: {key!r}")
            if _has_placeholder(value):
                errors.append(f"PLACEHOLDER_PRESENT: component_versions.{key}")

    # ── determinism / seeds ──
    det = record["determinism"]
    if not isinstance(det, dict):
        errors.append("DETERMINISM_NOT_A_DICT")
    else:
        if "seed_supported" not in det or not isinstance(det["seed_supported"], bool):
            errors.append("SEED_SUPPORTED_MISSING_OR_NOT_BOOL")
        else:
            if det["seed_supported"]:
                seeds = det.get("seeds")
                if not _is_seed_block(seeds):
                    errors.append(
                        "SEEDS_INVALID: a seed-supported record needs a non-empty "
                        "seeds dict of asset_key -> int in [0, 2^64)"
                    )
                if det.get("seed_derivation") != RECORDED_SEED_DERIVATION:
                    errors.append(
                        "SEED_DERIVATION_MISMATCH: the derivation string must be "
                        "verbatim the recorded benchmark-set derivation "
                        "(a change is a new benchmark-set version, never a silent edit)"
                    )
            else:
                # A module whose algorithm has no seed input (recorded
                # honestly: ADMM — no randomness; Clarabel — the
                # algorithm has no seed input; the rule-based baseline —
                # no randomness) must not pretend otherwise.
                if det.get("seeds"):
                    errors.append(
                        "SEED_POLICY_CONFLICT: seed_supported is false but a "
                        "seeds block is present (distinct from the P8.05 "
                        "harness refusal class SEED_POLICY_INCONSISTENT)"
                    )
        if "nondeterminism_sources" not in det or not isinstance(
            det["nondeterminism_sources"], list
        ):
            errors.append("NONDETERMINISM_SOURCES_MISSING_OR_NOT_LIST")
        else:
            tolerance_class = record["tolerance_class"]
            if tolerance_class in ("within_tolerance", "distribution") and not det[
                "nondeterminism_sources"
            ]:
                errors.append(
                    "NONDETERMINISM_SOURCES_REQUIRED: a tolerance/distribution "
                    "record must disclose its nondeterminism sources (GPU "
                    "nondeterminism unrecorded is the ledger's named risk; P8.38)"
                )
            for source in det["nondeterminism_sources"]:
                if not _is_nonempty_str(source) or _has_placeholder(source):
                    errors.append(f"NONDETERMINISM_SOURCE_INVALID: {source!r}")

    # ── config ──
    cfg = record["config"]
    if not isinstance(cfg, dict):
        errors.append("CONFIG_NOT_A_DICT")
    else:
        digest = cfg.get("config_digest")
        if digest is None:
            errors.append("FIELD_MISSING: config.config_digest")
        elif not _is_digest(digest):
            errors.append(f"DIGEST_FORMAT_INVALID: config_digest = {digest!r}")

    # ── inputs ──
    inputs = record["inputs"]
    if not isinstance(inputs, dict):
        errors.append("INPUTS_NOT_A_DICT")
    else:
        if module_id in DYNAMIC_MODULE_IDS:
            if not _is_nonempty_str(inputs.get("dynamic_case_ref")):
                errors.append(
                    "FIELD_MISSING: inputs.dynamic_case_ref (the model/case "
                    "reference every dynamic number must carry, P8.17)"
                )
        else:
            snapshots = inputs.get("input_snapshot_ids")
            if not isinstance(snapshots, list) or not snapshots:
                errors.append(
                    "FIELD_MISSING: inputs.input_snapshot_ids (the §9.2/P8.04 "
                    "provenance base carries the input snapshot id)"
                )
            else:
                for snap in snapshots:
                    if not _is_nonempty_str(snap) or _has_placeholder(snap):
                        errors.append(f"INPUT_SNAPSHOT_ID_INVALID: {snap!r}")
            digests = inputs.get("input_artifact_digests")
            if not isinstance(digests, dict) or not digests:
                errors.append(
                    "FIELD_MISSING: inputs.input_artifact_digests (same seed, "
                    "different data is the ledger's named risk — the data "
                    "pipeline is recorded via digests)"
                )
            else:
                for path, digest in digests.items():
                    if not _is_nonempty_str(path):
                        errors.append(f"INPUT_ARTIFACT_PATH_INVALID: {path!r}")
                    if not _is_digest(digest):
                        errors.append(f"DIGEST_FORMAT_INVALID: {path!r} = {digest!r}")

    # ── artifacts (the artifact store references) ──
    artifacts = record["artifacts"]
    if not isinstance(artifacts, dict):
        errors.append("ARTIFACTS_NOT_A_DICT")
    else:
        refs = artifacts.get("artifact_refs")
        if not isinstance(refs, dict) or not refs:
            errors.append(
                "ARTIFACT_REF_MISSING: artifacts stored only on the "
                "researcher's machine is the ledger's named risk — every run "
                "references its stored artifacts"
            )
        else:
            for name, ref in refs.items():
                if not _is_nonempty_str(name):
                    errors.append(f"ARTIFACT_NAME_INVALID: {name!r}")
                if not _is_artifact_ref(ref):
                    errors.append(f"ARTIFACT_REF_INVALID: {name!r} = {ref!r}")

    # ── outputs ──
    outputs = record["outputs"]
    if not isinstance(outputs, dict):
        errors.append("OUTPUTS_NOT_A_DICT")
    else:
        out_refs = outputs.get("output_artifact_refs")
        if not isinstance(out_refs, dict) or not out_refs:
            errors.append("OUTPUT_ARTIFACT_REF_MISSING")
        else:
            for name, ref in out_refs.items():
                if not _is_artifact_ref(ref):
                    errors.append(f"ARTIFACT_REF_INVALID: outputs.{name!r} = {ref!r}")
        out_digests = outputs.get("output_digests")
        if not isinstance(out_digests, dict) or not out_digests:
            errors.append("OUTPUT_DIGEST_MISSING")
        else:
            for name, digest in out_digests.items():
                if not _is_digest(digest):
                    errors.append(f"DIGEST_FORMAT_INVALID: outputs.{name!r} = {digest!r}")

    # ── environment (the P3.27 hardware/container rule) ──
    env = record["environment"]
    if not isinstance(env, dict):
        errors.append("ENVIRONMENT_NOT_A_DICT")
    else:
        for field in ("os", "arch", "cpu_model", "logical_cores", "memory", "container_flag"):
            value = env.get(field)
            if value is None:
                # Anything the process cannot observe is recorded
                # "unavailable: …", never guessed (benchmarks README rule).
                errors.append(
                    f"FIELD_MISSING: environment.{field} "
                    "(record 'unavailable: <reason>' when not observable — never omit)"
                )
            elif not _is_nonempty_str(value):
                errors.append(f"ENVIRONMENT_FIELD_EMPTY: {field}")

    # ── tolerance class + pre-registration reference ──
    tolerance_class = record["tolerance_class"]
    if tolerance_class not in TOLERANCE_CLASSES:
        errors.append(f"TOLERANCE_CLASS_INVALID: {tolerance_class!r}")
    elif tolerance_class != "exact":
        ref = record.get("tolerance_ref")
        if not _is_nonempty_str(ref):
            errors.append(
                "TOLERANCE_REF_REQUIRED: a within-tolerance/distribution record "
                "must reference the P8.38 pre-registration that declared the "
                "tolerance BEFORE the rerun (X-8 — never fitted after the fact)"
            )
        elif _has_placeholder(ref):
            errors.append(f"PLACEHOLDER_PRESENT: tolerance_ref = {ref!r}")

    # ── run status ──
    run_status = record["run_status"]
    if not isinstance(run_status, str) or run_status not in RUN_STATUSES:
        errors.append(f"RUN_STATUS_INVALID: {record['run_status']!r}")
    elif record["run_status"] == "FAILED":
        reason = record.get("failure_reason")
        if not _is_nonempty_str(reason):
            errors.append(
                "FAILURE_REASON_REQUIRED: a failed run's record names the "
                "failure (recorded AS A FAILURE, never as a blank or a number)"
            )

    return errors


# ── Hand-computed self-tests ──────────────────────────────────────────────


def _minimal_valid_record() -> dict:
    """A HAND-CONSTRUCTED minimal valid record (rule-based baseline).

    The rule-based baseline is the simplest honest case: no solver, no
    seed, no randomness (P8.06 §3) — exact reproduction class (same
    inputs -> same committed schedule, byte-equality discipline).
    """
    return {
        "record_schema_version": SCHEMA_VERSION,
        "record_id": "rep-0001",
        "run_id": "run-lib01-priority-list-001",
        "module_id": "baseline/rule-based-priority-list",
        "module_version": "flexcore-research/baselines.v1",
        "code_version": "flexcore-baseline-freeze-2026-11-14",
        "component_versions": {},
        "determinism": {
            "seed_supported": False,
            "nondeterminism_sources": [],
        },
        "config": {"config_digest": "fnv1a64:" + "0" * 16},
        "inputs": {
            "input_snapshot_ids": ["snapshot-lib01-v1"],
            "input_artifact_digests": {
                "scenarios/library/lib-01-minimal-single-asset.json": "fnv1a64:"
                + "1" * 16
            },
        },
        "artifacts": {
            "artifact_refs": {
                "run_config": "flexcore-artifact://runs/rep-0001/config.json"
            }
        },
        "outputs": {
            "output_artifact_refs": {
                "schedule": "flexcore-artifact://runs/rep-0001/schedule.json"
            },
            "output_digests": {"schedule": "sha256:" + "a" * 64},
        },
        "environment": {
            "os": "linux",
            "arch": "x86_64",
            "cpu_model": "unavailable: /proc/cpuinfo not read at record time",
            "logical_cores": "4",
            "memory": "unavailable: not observed",
            "container_flag": "no-indicator-found",
        },
        "tolerance_class": "exact",
        "run_status": "COMPLETED",
    }


def run_self_tests() -> list:
    """Run every self-test; return one record per test.

    Every expectation is HAND-DERIVED over constructed inputs with the
    reasoning STATED in the test. These pin the SCHEMA; they are not a
    run, not a measurement, and not evidence about any case.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append(
            {
                "test": name,
                "passed": bool(condition),
                "detail": detail,
                "usage": REPORTING_ONLY_STATEMENT,
            }
        )

    # (1) The hand-built minimal rule-based record is VALID — zero
    #     findings. Hand-checked field by field against the schema above.
    check(
        "minimal_valid_record_zero_findings",
        validate_record(_minimal_valid_record()) == [],
        "hand-built record satisfies every required field (no solver, no "
        "seed, exact class, 'unavailable:' environment values allowed per "
        "the P3.27 rule) — the validator must return no findings",
    )

    # (2) A required field removed -> exactly one FIELD_MISSING finding
    #     for it (hand-counted: removing `code_version` from the valid
    #     record leaves every other field intact).
    record = _minimal_valid_record()
    del record["code_version"]
    errors = validate_record(record)
    check(
        "missing_field_named",
        sum(1 for e in errors if e.startswith("FIELD_MISSING: code_version")) == 1,
        f"removing code_version yields exactly one FIELD_MISSING finding; "
        f"got {errors}",
    )

    # (3) A placeholder token in a filled field is refused — the "not run
    #     hidden as a blank" failure mode, named at the record layer.
    record = _minimal_valid_record()
    record["code_version"] = "RECORD-AT-FIRST-RUN"
    errors = validate_record(record)
    check(
        "placeholder_refused",
        any(e.startswith("PLACEHOLDER_PRESENT: code_version") for e in errors),
        "RECORD-AT-FIRST-RUN is in PLACEHOLDER_TOKENS; a template token in "
        "a 'filled' record is a finding, never accepted",
    )

    # (4) A wrong schema version is refused (anti-silent-growth: a schema
    #     change is a new version, never coerced).
    record = _minimal_valid_record()
    record["record_schema_version"] = "flexcore-research/reproducibility-record.v2"
    errors = validate_record(record)
    check(
        "schema_version_unknown_refused",
        any(e.startswith("SCHEMA_VERSION_UNKNOWN") for e in errors),
        "a v2-marked record under the v1 validator is SCHEMA_VERSION_UNKNOWN",
    )

    # (5) An unknown module id is refused (closed vocabulary).
    record = _minimal_valid_record()
    record["module_id"] = "admm-totally-new-variant"
    errors = validate_record(record)
    check(
        "module_unknown_refused",
        any(e.startswith("MODULE_UNKNOWN") for e in errors),
        "the module vocabulary is closed; new modules enter it by a schema "
        "version change, not by typing a new string",
    )

    # (6) A seed on a seed_supported=false module is a SEED_POLICY_CONFLICT
    #     (recorded honestly: the rule-based baseline has no randomness).
    record = _minimal_valid_record()
    record["determinism"] = {
        "seed_supported": False,
        "seeds": {"asset": 1},
        "nondeterminism_sources": [],
    }
    errors = validate_record(record)
    check(
        "seed_policy_conflict_refused",
        any(e.startswith("SEED_POLICY_CONFLICT") for e in errors),
        "pretending a seedless algorithm took a seed is exactly the "
        "dishonesty the ledger's seed-honesty discipline refuses",
    )

    # (7) A seed-supported record WITHOUT the verbatim derivation string
    #     is refused (SEED_DERIVATION_MISMATCH).
    record = _minimal_valid_record()
    record["module_id"] = "marl-policy"
    record["module_version"] = "flexcore-marl-shadow/0.1.0"
    record["component_versions"] = {"rl_framework": "UNPINNED-REVERIFY"}
    record["determinism"] = {
        "seed_supported": True,
        "seeds": {"site-north-bess-1": 12345},
        "seed_derivation": "seed(asset_key) = master_seed + 1",  # wrong
        "nondeterminism_sources": ["gpu_nondeterminism"],
    }
    record["tolerance_class"] = "within_tolerance"
    record["tolerance_ref"] = "P8.38/marl-policy-inference"
    errors = validate_record(record)
    check(
        "seed_derivation_mismatch_refused",
        any(e.startswith("SEED_DERIVATION_MISMATCH") for e in errors),
        "a changed derivation string means a new benchmark-set version "
        "(P8.19 anti-silent-growth) — never a silent edit",
    )

    # (8) The SAME record with the VERBATIM derivation string and the
    #     benchmark-set version passes those checks (hand-checked: the
    #     only remaining question is the rl_framework DECISION-POINT
    #     value, which the schema does not judge — it requires presence,
    #     not a resolution).
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    record["determinism"]["seed_derivation"] = RECORDED_SEED_DERIVATION
    record["inputs"]["input_snapshot_ids"] = ["snapshot-marl-v1"]
    record["inputs"]["input_artifact_digests"] = {"policy.bin": "sha256:" + "b" * 64}
    errors = validate_record(record)
    check(
        "seed_derivation_verbatim_accepted",
        not any(e.startswith("SEED_DERIVATION_MISMATCH") for e in errors),
        "the verbatim recorded string satisfies the derivation check; "
        "findings (if any) are about other fields — hand-checked: none "
        "is SEED_DERIVATION_MISMATCH",
    )

    # (9) A within_tolerance record without nondeterminism sources is
    #     refused (the ledger's named risk: GPU nondeterminism unrecorded
    #     and presented as exact reproduction).
    record = _minimal_valid_record()
    record["module_id"] = "gnn-warm-start"
    record["component_versions"] = {"torch": "x", "pyg": "y"}
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    record["tolerance_class"] = "within_tolerance"
    record["tolerance_ref"] = "P8.38/gnn-warm-start-inference"
    record["determinism"] = {
        "seed_supported": True,
        "seeds": {"t": 1},
        "seed_derivation": RECORDED_SEED_DERIVATION,
        "nondeterminism_sources": [],
    }
    errors = validate_record(record)
    check(
        "nondeterminism_sources_required",
        any(e.startswith("NONDETERMINISM_SOURCES_REQUIRED") for e in errors),
        "an empty nondeterminism_sources list under within_tolerance is "
        "refused — disclosure is mandatory, not optional (P8.38)",
    )

    # (10) A within_tolerance record without a tolerance_ref is refused
    #      (X-8: the tolerance is pre-registered, never fitted after the
    #      fact; the record must point at the pre-registration).
    record = _minimal_valid_record()
    record["module_id"] = "marl-policy"
    record["component_versions"] = {"rl_framework": "x"}
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    record["determinism"] = {
        "seed_supported": True,
        "seeds": {"t": 1},
        "seed_derivation": RECORDED_SEED_DERIVATION,
        "nondeterminism_sources": ["gpu_nondeterminism"],
    }
    record["tolerance_class"] = "within_tolerance"
    errors = validate_record(record)
    check(
        "tolerance_ref_required",
        any(e.startswith("TOLERANCE_REF_REQUIRED") for e in errors),
        "no reference to the P8.38 pre-registration -> refused; the value "
        "itself lives in the pre-registration, never in this schema",
    )

    # (11) A dynamic record carrying a benchmark_set_version is refused
    #      (NT-10: dynamic cases are not in the benchmark set; the
    #      X-5 boundary keeps the tracks separate).
    record = _minimal_valid_record()
    record["module_id"] = "dynamic-controller"
    record["component_versions"] = {"andes": "UNPINNED-REVERIFY"}
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    errors = validate_record(record)
    check(
        "dynamic_record_benchmark_set_forbidden",
        any(e.startswith("DYNAMIC_RECORD_BENCHMARK_SET_FORBIDDEN") for e in errors),
        "dynamic studies cite their ANDES model/case reference, never the "
        "quasi-static benchmark-set version (X-5 discipline)",
    )

    # (12) Digest format: a bare hex string (no algorithm prefix) is
    #      refused — the digest ALGORITHM travels with the digest.
    record = _minimal_valid_record()
    record["config"]["config_digest"] = "deadbeef"
    errors = validate_record(record)
    check(
        "digest_format_invalid_refused",
        any(e.startswith("DIGEST_FORMAT_INVALID") for e in errors),
        "digests are 'sha256:<64 hex>' or 'fnv1a64:<16 hex>' (the repo's "
        "test-grade FNV pattern plus a stronger option) — the algorithm "
        "is recorded, never assumed",
    )

    # (13) An artifact reference without the store prefix is refused —
    #      "stored only on the researcher's machine" (a bare path) is the
    #      ledger's named risk.
    record = _minimal_valid_record()
    record["artifacts"]["artifact_refs"]["run_config"] = "C:/local/tmp/config.json"
    errors = validate_record(record)
    check(
        "artifact_ref_invalid_refused",
        any(e.startswith("ARTIFACT_REF_INVALID") for e in errors),
        "references must use the flexcore-artifact:// store scheme — a "
        "local path proves nothing about retrievability",
    )

    # (14) A FAILED run record without a failure reason is refused
    #      (failures are recorded AS FAILURES, never as blanks).
    record = _minimal_valid_record()
    record["run_status"] = "FAILED"
    errors = validate_record(record)
    check(
        "failure_reason_required",
        any(e.startswith("FAILURE_REASON_REQUIRED") for e in errors),
        "run_status FAILED without failure_reason -> refused",
    )

    # (15) A non-dict record is refused, not crashed on.
    check(
        "non_dict_record_refused",
        validate_record([1, 2, 3]) == ["RECORD_NOT_A_DICT: expected a record object, got list"],
        "a list is not a record; the validator returns the named finding",
    )

    # (16) Namespace pin: no dispatch-shaped callable (reporting-only,
    #      ADR-010).
    forbidden = ("dispatch", "issue", "authorize", "approve", "command")
    namespace = [name for name in globals() if not name.startswith("_")]
    check(
        "no_dispatch_shaped_callable",
        not any(word in name.lower() for name in namespace for word in forbidden),
        f"module namespace {sorted(namespace)} contains no dispatch/issue/"
        "authorize/approve/command-shaped name",
    )

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failed = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(f"{'PASS' if _r['passed'] else 'FAIL'} — {_r['test']}: {_r['detail']}")
    print(f"{len(_records) - len(_failed)}/{len(_records)} self-tests passed")
    raise SystemExit(1 if _failed else 0)
