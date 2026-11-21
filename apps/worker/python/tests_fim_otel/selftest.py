# FIM-F09.04 — The pinned self-test suite for the FIM-F09 Python OTEL
#         instrumentation wave: propagation round-trips, malformed
#         refusals, JetStream header extraction (the exact Rust-side
#         header names), emit-time redaction of a seeded secret-bearing
#         record, no-op fallback equality with and without the SDK import
#         guarded out, and the structural Collector-config validator run
#         against the CHECKED-IN config.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. IMPLEMENTED UNDER FIM-F09. WRITTEN, NEVER RUN as part of any
#    verification pipeline in this program — execution IS the human gate
#    (the FIM standing boundary: no build, no test execution, no
#    installation). The suite is runnable TODAY with
#
#        python -I apps/worker/python/tests_fim_otel/selftest.py
#
#    because everything it imports is stdlib-only (the shared module and
#    the validator carry no third-party imports and the OpenTelemetry SDK
#    is OPTIONAL and NOT installed — the no-op fallback is the DEFAULT
#    posture).
# 2. WHAT A PASS PINS (and what it does NOT): the pure-logic contracts of
#    apps/worker/python/otel_instrumentation.py and
#    apps/worker/python/tools_fim/validate_collector_config.py, plus the
#    checked-in config's shape. It does NOT pin any live behaviour: no
#    OTLP export, no Collector receipt, no SLO alert — those are HUMAN
#    GATES (F-09 acceptance criteria 1-3 need live infrastructure). The
#    SDK-present path is UNPROVEN until a human runs it with the SDK
#    installed; the suite's "fake SDK" probe below proves only that the
#    GUARD degrades deterministically to the no-op posture with the same
#    log-line structure — never that real OTLP export works.
# 3. The redaction vectors use SYNTHETIC test strings (the P11.08 record's
#    discipline): no discovered credential of any kind appears here.
# 4. Worker entrypoints are NOT executed here (the FIM-F09.02 hooks live
#    in their main()/__main__ paths); the suite exercises the shared
#    module the hooks call, and the hook wiring itself was verified by the
#    wave's own python -I import checks (recorded in the wave report).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile

_HERE = os.path.dirname(os.path.abspath(__file__))
_PY_ROOT = os.path.dirname(_HERE)  # apps/worker/python
_TOOLS_FIM = os.path.join(_PY_ROOT, "tools_fim")
_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(_PY_ROOT)))
_CHECKED_IN_CONFIG = os.path.join(_REPO_ROOT, "deploy", "compose", "otel-collector-config.yaml")

for _path in (_PY_ROOT, _TOOLS_FIM):
    if _path not in sys.path:
        sys.path.insert(0, _path)

import otel_instrumentation  # noqa: E402 — the sys.path insert above is the point
import validate_collector_config  # noqa: E402

# The W3C Trace Context Recommendation's example value (the pinned valid
# vector; also the parse round-trip anchor).
W3C_EXAMPLE = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
W3C_TRACE_ID = "4bf92f3577b34da6a3ce929d0e0e4736"
W3C_SPAN_ID = "00f067aa0ba902b7"

# The exact JetStream header set the Rust publisher writes (the P11.05
# seam: the mandatory Nats-Msg-Id, the P11.13 flexcore-schema-version,
# and the W3C traceparent) — the names discovered in
# crates/adapters/nats/src/publisher.rs + apps/worker/src/otel.rs.
RUST_PUBLISHER_HEADERS = {
    "Nats-Msg-Id": "command-1234-attempt-1",
    "flexcore-schema-version": "1",
    "traceparent": W3C_EXAMPLE,
}

RESULTS: list = []


def check(name: str, condition: bool, detail: str) -> None:
    RESULTS.append({"name": name, "passed": bool(condition), "detail": detail})


# ---------------------------------------------------------------------------
# 1. traceparent round-trips + malformed refusals (pinned vectors).
# ---------------------------------------------------------------------------

def test_traceparent_round_trip() -> None:
    context = otel_instrumentation.parse_traceparent(W3C_EXAMPLE)
    check(
        "traceparent_round_trip",
        context is not None
        and context.trace_id == W3C_TRACE_ID
        and context.span_id == W3C_SPAN_ID
        and context.trace_flags == "01"
        and otel_instrumentation.serialize_traceparent(context) == W3C_EXAMPLE,
        "parse -> serialize -> parse round-trips the W3C example byte-exactly",
    )


def test_traceparent_malformed_refusals() -> None:
    malformed = [
        "",
        "00",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",  # no flags
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0101",  # long flags
        "00-4bf92f3577b34da6a3ce929d0e0e4736--00f067aa0ba902b7-01",  # empty field
        "00-4bf92f3577b34da6a3ce929d0e0e473G-00f067aa0ba902b7-01",  # non-hex
        "00-4bf92f3577b34da6a3ce929d0e0e4736 00f067aa0ba902b7-01",  # space separator
    ]
    check(
        "traceparent_malformed_refused",
        all(otel_instrumentation.parse_traceparent(v) is None for v in malformed),
        "7 malformed shapes yield None (no inherited context)",
    )


def test_traceparent_all_zero_refusal() -> None:
    check(
        "all_zero_trace_id_refused",
        otel_instrumentation.parse_traceparent(
            "00-" + "0" * 32 + "-00f067aa0ba902b7-01"
        ) is None,
        "the all-zero trace-id is invalid per the W3C grammar",
    )
    check(
        "all_zero_span_id_refused",
        otel_instrumentation.parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-" + "0" * 16 + "-01"
        ) is None,
        "the all-zero span-id is invalid per the W3C grammar",
    )


def test_traceparent_reserved_flags_and_versions() -> None:
    flags_ctx = otel_instrumentation.parse_traceparent(
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-03"
    )
    ff_ctx = otel_instrumentation.parse_traceparent(
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-ff"
    )
    check(
        "reserved_flags_propagate_verbatim",
        flags_ctx is not None and flags_ctx.trace_flags == "03" and ff_ctx is not None and ff_ctx.trace_flags == "ff",
        "reserved flag bits are propagated verbatim (ff is legal per the grammar); "
        "the module never SETS them (serialize emits parsed or default 01)",
    )
    check(
        "forbidden_version_ff_refused",
        otel_instrumentation.parse_traceparent(
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        ) is None,
        "version ff is forbidden per the W3C grammar",
    )
    check(
        "uppercase_refused_strict",
        otel_instrumentation.parse_traceparent(W3C_EXAMPLE.upper()) is None,
        "non-lowercase hex is refused (HEXDIGLC; recorded strict posture)",
    )


# ---------------------------------------------------------------------------
# 2. Header extraction with the exact Rust-side header names.
# ---------------------------------------------------------------------------

def test_header_extraction_rust_names() -> None:
    check(
        "header_name_constant_is_w3c_name",
        otel_instrumentation.TRACEPARENT_HEADER_NAME == "traceparent",
        "the module's header-name constant is the W3C standard name the Rust "
        "publisher/consumer use (crates/adapters/nats/src/consumer.rs)",
    )
    context = otel_instrumentation.parse_traceparent(W3C_EXAMPLE)
    from_rust_headers = otel_instrumentation.trace_context_from_headers(RUST_PUBLISHER_HEADERS)
    check(
        "jetstream_header_set_extraction",
        from_rust_headers == context,
        "the full Rust-publisher header set (Nats-Msg-Id + flexcore-schema-version "
        "+ traceparent) yields the pinned context",
    )
    check(
        "header_name_case_insensitive",
        otel_instrumentation.trace_context_from_headers({"TraceParent": W3C_EXAMPLE}) == context
        and otel_instrumentation.trace_context_from_headers({"TRACEPARENT": W3C_EXAMPLE}) == context,
        "header-NAME lookup is case-insensitive (NATS/W3C convention)",
    )
    check(
        "absent_or_unparseable_is_none",
        otel_instrumentation.trace_context_from_headers({"Nats-Msg-Id": "x"}) is None
        and otel_instrumentation.trace_context_from_headers({"traceparent": "junk"}) is None,
        "an absent or unparseable header means no inherited context (never synthesized)",
    )
    check(
        "envelope_and_env_carriers",
        otel_instrumentation.trace_context_from_envelope(
            {"protocol": "flexcore-gnn-worker/1", "traceparent": W3C_EXAMPLE}
        ) == context
        and otel_instrumentation.trace_context_from_environment({"TRACEPARENT": W3C_EXAMPLE}) == context,
        "the JSON-envelope and TRACEPARENT env carriers parse (the recorded channels)",
    )


# ---------------------------------------------------------------------------
# 3. Redaction of a seeded secret-bearing record (P11.08 posture; the
#    FORBIDDEN_RESULT_KEYS vocabulary + the credential vocabulary).
# ---------------------------------------------------------------------------

def test_redaction_seeded_record() -> None:
    seeded = {
        "message": "processing request with password=hunter2-super-secret",
        "authorization": "Bearer super-secret-token-value",
        "api_key": "AKIAIOSFODNN7EXAMPLE",
        "command": "issue-to-field-device",
        "dispatch": {"target": "battery-9", "setpoint": 1.25},
        "auth_token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl",
        "email": "operator@example.org",
        "tenant_ref": "tenant-7",
        "request_id": "req-0001",
        "trace_id": W3C_TRACE_ID,
    }
    redacted = otel_instrumentation.redact_pairs(seeded)
    redacted_message = otel_instrumentation.redact_string(seeded["message"])
    leak_free = (
        "hunter2-super-secret" not in redacted_message
        and "super-secret-token-value" not in json.dumps(redacted, default=str)
        and "AKIAIOSFODNN7EXAMPLE" not in json.dumps(redacted, default=str)
        and "issue-to-field-device" not in json.dumps(redacted, default=str)
        and "battery-9" not in json.dumps(redacted, default=str)
        and "eyJhbGciOiJIUzI1NiJ9" not in json.dumps(redacted, default=str)
        and "operator@example.org" not in json.dumps(redacted, default=str)
    )
    check(
        "redaction_of_seeded_record",
        leak_free
        and redacted["authorization"] == otel_instrumentation.REDACTION_MARKER
        and redacted["api_key"] == otel_instrumentation.REDACTION_MARKER
        and redacted["command"] == otel_instrumentation.REDACTION_MARKER
        and redacted["dispatch"] == otel_instrumentation.REDACTION_MARKER
        and redacted["auth_token"] == otel_instrumentation.REDACTION_MARKER
        and redacted["email"] == otel_instrumentation.REDACTION_MARKER,
        "every seeded secret shape is gone; credential AND proposal-only keys "
        "become the wholesale marker (P11.08 + FORBIDDEN_RESULT_KEYS)",
    )
    check(
        "redaction_preserves_correlation_content",
        redacted["tenant_ref"] == "tenant-7"
        and redacted["request_id"] == "req-0001"
        and redacted["trace_id"] == W3C_TRACE_ID,
        "ids, tenant refs, and the 32-hex trace id survive (no over-redaction; "
        "the long-hex pattern stays above the 32-hex handle length)",
    )


def test_log_line_redaction_at_emit() -> None:
    line = json.loads(
        otel_instrumentation.build_log_line(
            "info",
            "worker start with api_key=AKIAIOSFODNN7EXAMPLE",
            context=otel_instrumentation.parse_traceparent(W3C_EXAMPLE),
            secret="never-emit-this",
            command="field-dispatch",
        )
    )
    serialized = json.dumps(line)
    check(
        "log_line_redacted_at_emit",
        "AKIAIOSFODNN7EXAMPLE" not in serialized
        and "never-emit-this" not in serialized
        and "field-dispatch" not in serialized
        and otel_instrumentation.REDACTION_MARKER in serialized,
        "the schema-v1 line is redacted BEFORE serialization (emit boundary)",
    )
    check(
        "log_line_schema_v1_shape",
        line["schema_version"] == "flexcore.log.v1"
        and line["severity_number"] == 9
        and line["trace_id"] == W3C_TRACE_ID
        and line["span_id"] == W3C_SPAN_ID,
        "the line carries the P11.07 field set with the inherited trace context",
    )


# ---------------------------------------------------------------------------
# 4. No-op fallback equality — the same structure with and without the SDK
#    import guarded out (a fake `opentelemetry` module is injected in a
#    subprocess; the guarded init must degrade to the recorded no-op with
#    an IDENTICAL log-line structure).
# ---------------------------------------------------------------------------

_FIXED_CALL = dict(
    level="info",
    message="worker start",
    tenant_id="t-1",
    command_id="",
    context=otel_instrumentation.parse_traceparent(W3C_EXAMPLE),
    event="worker_start",
    worker="flexcore-selftest",
)

_CHILD_TEMPLATE = r'''
import json, sys, types, os
sys.path.insert(0, sys.argv[1])
mode = sys.argv[2]
if mode == "fake-sdk":
    fake = types.ModuleType("opentelemetry")
    sys.modules["opentelemetry"] = fake
import otel_instrumentation as oi
provider, status, reason = oi.try_init_otlp_export(
    service_name="flexcore-selftest",
    endpoint="http://127.0.0.1:4317",
    environ={},
)
line = oi.build_log_line(**json.loads(sys.argv[3]))
print(json.dumps({"status": status, "line": json.loads(line), "provider_is_none": provider is None}))
'''


def test_noop_fallback_equality() -> None:
    fixed_json = json.dumps(
        {
            "level": _FIXED_CALL["level"],
            "message": _FIXED_CALL["message"],
            "tenant_id": _FIXED_CALL["tenant_id"],
            "command_id": _FIXED_CALL["command_id"],
            "trace_id": _FIXED_CALL["context"].trace_id if _FIXED_CALL["context"] else "",
            "span_id": _FIXED_CALL["context"].span_id if _FIXED_CALL["context"] else "",
            "trace_flags": _FIXED_CALL["context"].trace_flags if _FIXED_CALL["context"] else "01",
            "event": "worker_start",
            "worker": "flexcore-selftest",
        }
    )
    child_path = os.path.join(tempfile.mkdtemp(prefix="fim09_noop_"), "child.py")
    with open(child_path, "w", encoding="utf-8") as handle:
        handle.write(_CHILD_TEMPLATE)
    outputs = {}
    for mode in ("no-sdk", "fake-sdk"):
        completed = subprocess.run(
            [sys.executable, "-I", child_path, _PY_ROOT, mode, fixed_json],
            capture_output=True,
            text=True,
            timeout=60,
        )
        try:
            outputs[mode] = json.loads(completed.stdout.strip().splitlines()[-1])
        except (IndexError, ValueError):
            check(
                "noop_fallback_equality",
                False,
                "the %s subprocess produced no parseable output: %r / %r"
                % (mode, completed.stdout[-200:], completed.stderr[-200:]),
            )
            return
    no_sdk, fake_sdk = outputs["no-sdk"], outputs["fake-sdk"]
    check(
        "noop_without_sdk",
        no_sdk["provider_is_none"] and no_sdk["status"] == otel_instrumentation.SDK_STATUS_UNAVAILABLE,
        "with the SDK import guarded out (absent), the init degrades to the "
        "recorded sdk-absent no-op",
    )
    check(
        "noop_with_fake_sdk",
        fake_sdk["provider_is_none"] and fake_sdk["status"] == otel_instrumentation.SDK_STATUS_INIT_FAILED,
        "with a fake opentelemetry module injected, the guard degrades to the "
        "recorded init-failed no-op (fail-open, never a crash)",
    )
    line_a, line_b = no_sdk["line"], fake_sdk["line"]
    check(
        "noop_log_line_structure_equal",
        set(line_a.keys()) == set(line_b.keys())
        and all(line_a[k] == line_b[k] for k in line_a if k != "timestamp"),
        "the emitted line structure is IDENTICAL across the no-op postures "
        "(same keys, same values; only the emit timestamp may differ)",
    )
    check(
        "noop_line_carries_context",
        line_a["trace_id"] == W3C_TRACE_ID and line_a["schema_version"] == "flexcore.log.v1",
        "both no-op lines carry the inherited context and the schema-v1 tag",
    )
    try:
        os.remove(child_path)
    except OSError:
        pass


# ---------------------------------------------------------------------------
# 5. The Collector-config validator against the CHECKED-IN config.
# ---------------------------------------------------------------------------

def test_checked_in_collector_config() -> None:
    try:
        with open(_CHECKED_IN_CONFIG, "r", encoding="utf-8") as handle:
            text = handle.read()
    except OSError as error:
        check("checked_in_collector_config", False, "the checked-in config is unreadable: %r" % error)
        return
    findings, document = validate_collector_config.validate_config_text(text)
    errors = [f for f in findings if f["severity"] == "error"]
    pipelines = (document or {}).get("service", {}).get("pipelines", {}) if document else {}
    expected_pipeline = {
        "receivers": ["otlp"],
        "processors": ["memory_limiter", "batch"],
        "exporters": ["debug"],
    }
    all_wired = all(pipelines.get(signal) == expected_pipeline for signal in ("traces", "metrics", "logs"))
    check(
        "checked_in_collector_config",
        document is not None and errors == [] and all_wired,
        "the checked-in gateway config parses under the subset, wires "
        "traces/metrics/logs with receivers [otlp] -> processors "
        "[memory_limiter, batch] -> exporters [debug], and yields zero "
        "errors (findings: %s)" % ([f["check"] for f in findings] or "none"),
    )


def run_all() -> list:
    test_traceparent_round_trip()
    test_traceparent_malformed_refusals()
    test_traceparent_all_zero_refusal()
    test_traceparent_reserved_flags_and_versions()
    test_header_extraction_rust_names()
    test_redaction_seeded_record()
    test_log_line_redaction_at_emit()
    test_noop_fallback_equality()
    test_checked_in_collector_config()
    return RESULTS


if __name__ == "__main__":
    _records = run_all()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
