# P11.05 / P11.07 — The trace-propagation and log-schema test suite:
#         pure-logic parts that RUN WITHOUT opentelemetry. Written and
#         NEVER RUN (the execution is a human gate — program conventions
#         §1).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. Same run procedure as test_worker_template.py
#    (scratch venv, pytest installed, `python/workers/` on PYTHONPATH,
#    `python -m pytest python/workers/tests/ -v`).
# 2. Coverage (what the suite pins):
#    - P11.05 propagation contract, pure half: the W3C `traceparent`
#      parser (shape, case-insensitivity, the all-zeros invalid ids,
#      junk inputs), the case-insensitive JetStream-header lookup, and
#      the absent-context honest outcome (no id synthesized). The
#      opentelemetry SDK calls of `otel_tracing` are UNVERIFIED and are
#      NOT exercised here — the module is imported only to prove it
#      parses/imports WITHOUT opentelemetry installed (the server.py
#      import pattern).
#    - P11.05 config: the fail-closed OTel keys (blank endpoint = export
#      disabled; ratio bounds; non-number ratio rejected).
#    - P11.07 schema: the SeverityNumber mapping (stdlib/structlog
#      names, unknown → 0), the mandatory attribute keys on every line,
#      the schema_version constant, severity band bases per [S-62], and
#      the field shape of `emit_log_line`.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import io

import pytest

from worker_template import log_schema, otel_tracing
from worker_template.config import (
    DEFAULT_OTEL_EXPORTER_OTLP_ENDPOINT,
    DEFAULT_OTEL_TRACE_SAMPLE_RATIO,
    ENV_OTEL_EXPORTER_OTLP_ENDPOINT,
    ENV_OTEL_TRACE_SAMPLE_RATIO,
    ConfigError,
    from_mapping,
)


# ---------------------------------------------------------------------------
# otel_tracing — the module imports WITHOUT opentelemetry installed
# ---------------------------------------------------------------------------


class TestModuleImportPattern:
    def test_module_imports_without_opentelemetry(self) -> None:
        # The import above IS the assertion: `otel_tracing` parses and
        # imports with no opentelemetry in the environment (the deferred
        # import pattern of the template).
        assert callable(otel_tracing.trace_id_from_traceparent)
        assert callable(otel_tracing.carrier_trace_id)

    def test_require_opentelemetry_names_the_deployment_requirement(self) -> None:
        # Without opentelemetry installed (this loop's recorded state)
        # the choke point raises the NAMED requirement — never a bare
        # ImportError. With it installed (the human-gated activation
        # state) the import succeeds and the requirement constant still
        # carries the deployment instruction.
        try:
            import opentelemetry  # noqa: F401, PLC0415
        except ImportError:
            with pytest.raises(ImportError, match="deployment dependency"):
                otel_tracing.require_opentelemetry()
        else:
            assert "requirements.txt" in otel_tracing.OTEL_DEPLOYMENT_REQUIREMENT


# ---------------------------------------------------------------------------
# P11.05 — W3C traceparent parsing (the pure propagation core)
# ---------------------------------------------------------------------------


VALID_TRACEPARENT = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"


class TestTraceparentParsing:
    def test_valid_traceparent_yields_the_trace_id(self) -> None:
        assert (
            otel_tracing.trace_id_from_traceparent(VALID_TRACEPARENT)
            == "4bf92f3577b34da6a3ce929d0e0e4736"
        )

    def test_uppercase_input_is_normalized_to_lower_hex(self) -> None:
        assert (
            otel_tracing.trace_id_from_traceparent(VALID_TRACEPARENT.upper())
            == "4bf92f3577b34da6a3ce929d0e0e4736"
        )

    def test_all_zero_trace_id_is_invalid(self) -> None:
        bogus = "00-" + "0" * 32 + "-00f067aa0ba902b7-01"
        assert otel_tracing.trace_id_from_traceparent(bogus) is None

    def test_all_zero_span_id_is_invalid(self) -> None:
        bogus = "00-4bf92f3577b34da6a3ce929d0e0e4736-" + "0" * 16 + "-01"
        assert otel_tracing.trace_id_from_traceparent(bogus) is None

    def test_junk_inputs_yield_none(self) -> None:
        for junk in ("", "not-a-traceparent", "00-abc", "00-" + "a" * 31 + "-b" * 16 + "-01",
                     123, None):
            assert otel_tracing.trace_id_from_traceparent(junk) is None  # type: ignore[arg-type]

    def test_carrier_lookup_is_case_insensitive(self) -> None:
        for key in ("traceparent", "TraceParent", "TRACEPARENT"):
            carrier = {key: VALID_TRACEPARENT}
            assert (
                otel_tracing.carrier_trace_id(carrier)
                == "4bf92f3577b34da6a3ce929d0e0e4736"
            ), key

    def test_absent_context_is_the_honest_outcome(self) -> None:
        # No header, no parseable header: None — never a synthesized id.
        assert otel_tracing.carrier_trace_id({}) is None
        assert otel_tracing.carrier_trace_id({"traceparent": "junk"}) is None
        assert otel_tracing.carrier_trace_id("not-a-dict") is None  # type: ignore[arg-type]

    def test_jetstream_message_header_shape(self) -> None:
        # The publish side (Rust, P11.05 seam) writes the W3C header
        # alongside the broker's own headers; the read side finds it.
        message_headers = {
            "Nats-Msg-Id": "evt-0001",
            "traceparent": VALID_TRACEPARENT,
        }
        assert (
            otel_tracing.carrier_trace_id(message_headers)
            == "4bf92f3577b34da6a3ce929d0e0e4736"
        )


# ---------------------------------------------------------------------------
# P11.05 — the fail-closed OTel configuration keys
# ---------------------------------------------------------------------------


class TestOtelConfigKeys:
    def test_defaults_disable_export(self) -> None:
        config = from_mapping({"FLEXCORE_WORKER_ID": "w"})
        assert config.otel_exporter_otlp_endpoint == ""
        assert config.otel_exporter_otlp_endpoint == DEFAULT_OTEL_EXPORTER_OTLP_ENDPOINT
        assert config.otel_trace_sample_ratio == DEFAULT_OTEL_TRACE_SAMPLE_RATIO

    def test_endpoint_enables_export(self) -> None:
        config = from_mapping({
            "FLEXCORE_WORKER_ID": "w",
            ENV_OTEL_EXPORTER_OTLP_ENDPOINT: "http://127.0.0.1:4317",
        })
        assert config.otel_exporter_otlp_endpoint == "http://127.0.0.1:4317"

    def test_blank_endpoint_stays_disabled(self) -> None:
        config = from_mapping({
            "FLEXCORE_WORKER_ID": "w",
            ENV_OTEL_EXPORTER_OTLP_ENDPOINT: "   ",
        })
        assert config.otel_exporter_otlp_endpoint == ""

    def test_ratio_bounds_are_fail_closed(self) -> None:
        ok = from_mapping({
            "FLEXCORE_WORKER_ID": "w",
            ENV_OTEL_TRACE_SAMPLE_RATIO: "0.25",
        })
        assert ok.otel_trace_sample_ratio == 0.25
        for bad in ("1.5", "-0.1", "abc"):
            with pytest.raises(ConfigError):
                from_mapping({"FLEXCORE_WORKER_ID": "w", ENV_OTEL_TRACE_SAMPLE_RATIO: bad})


# ---------------------------------------------------------------------------
# P11.07 — the log schema v1 shape and the SeverityNumber mapping
# ---------------------------------------------------------------------------


class TestSeverityNumberMapping:
    def test_stdlib_and_structlog_names_map_to_band_bases(self) -> None:
        assert log_schema.severity_number_for_level("trace") == 1
        assert log_schema.severity_number_for_level("debug") == 5
        assert log_schema.severity_number_for_level("info") == 9
        assert log_schema.severity_number_for_level("warning") == 13
        assert log_schema.severity_number_for_level("warn") == 13
        assert log_schema.severity_number_for_level("error") == 17
        assert log_schema.severity_number_for_level("critical") == 21
        assert log_schema.severity_number_for_level("fatal") == 21
        # Case-insensitive (the emitter uppercases severity_text itself).
        assert log_schema.severity_number_for_level("ERROR") == 17
        assert log_schema.severity_number_for_level("Info") == 9

    def test_unknown_levels_map_to_unspecified(self) -> None:
        assert log_schema.severity_number_for_level("not-a-level") == 0
        assert log_schema.severity_number_for_level("") == 0
        assert log_schema.severity_number_for_level(7) == 0  # type: ignore[arg-type]


class TestLogSchemaLine:
    def test_line_carries_the_schema_and_severity(self) -> None:
        line = json.loads(log_schema.emit_log_line(
            "warning", "backpressure shed", stream=io.StringIO()
        ))
        assert line["schema_version"] == log_schema.LOG_SCHEMA_VERSION
        assert line["severity_number"] == 13
        assert line["severity_text"] == "WARNING"
        assert line["message"] == "backpressure shed"

    def test_mandatory_attributes_present_even_when_unknown(self) -> None:
        line = json.loads(log_schema.emit_log_line(
            "info", "served", stream=io.StringIO()
        ))
        for attribute in log_schema.MANDATORY_ATTRIBUTES:
            assert attribute in line, attribute
            assert line[attribute] == ""  # empty = not applicable on this event

    def test_provided_correlation_ids_travel_verbatim(self) -> None:
        line = json.loads(log_schema.emit_log_line(
            "info",
            "frame stored",
            tenant_id="7f3e-tenant",
            command_id="cmd-0007",
            trace_id="4bf92f3577b34da6a3ce929d0e0e4736",
            quantity="ActivePower",
            stream=io.StringIO(),
        ))
        assert line["tenant_id"] == "7f3e-tenant"
        assert line["command_id"] == "cmd-0007"
        assert line["trace_id"] == "4bf92f3577b34da6a3ce929d0e0e4736"
        assert line["quantity"] == "ActivePower"

    def test_lines_are_one_json_object_per_call(self) -> None:
        stream = io.StringIO()
        log_schema.emit_log_line("info", "one", stream=stream)
        log_schema.emit_log_line("info", "two", stream=stream)
        lines = stream.getvalue().splitlines()
        assert len(lines) == 2
        for text in lines:
            assert isinstance(json.loads(text), dict)


if __name__ == "__main__":  # pragma: no cover — manual local check only.
    pytest.main([__file__])
