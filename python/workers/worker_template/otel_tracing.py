# P11.05 — The OpenTelemetry tracing seam for Python workers: the ONLY
#         opentelemetry touchpoint in the template.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — READ BEFORE TRUSTING THIS FILE
#
# 1. opentelemetry was NEVER installed and this module was NEVER run.
#    Every opentelemetry call below is marked `UNVERIFIED — otel API` and
#    follows the documented Python SDK API as of the 2026-11-07 research
#    cut-off (the SDK is STABLE for traces/metrics/logs — the platform's
#    source register [S-63] for the Rust maturity note, the Python SDK
#    stable line is the same project's). All `import opentelemetry...`
#    statements happen INSIDE the functions that need them, so this file
#    PARSES, IMPORTS, and passes the pure-logic tests WITHOUT
#    opentelemetry installed — the named ImportError below is the
#    deployment requirement message a missing opentelemetry produces.
#    Deployment requirement: install the pins recorded in
#    `python/workers/requirements.txt` (P11.05 entries, checked
#    2026-11-14, never installed in this loop) before enabling export.
# 2. The pure-logic helpers at the bottom (`trace_id_from_traceparent`,
#    `carrier_trace_id`) ARE runnable today (stdlib-only) and carry the
#    W3C `traceparent` parsing that the propagation contract rests on —
#    the JetStream publish side (Rust, P11.05 seam in
#    `crates/adapters/nats/src/publisher.rs`) SETS the header; the
#    consumer side (Rust command consumer, Agent B's files; and any
#    Python worker that receives context) READS it. A Python worker
#    receiving an Execute call reads the `traceparent` gRPC metadata the
#    same way (UNVERIFIED — grpcio metadata shape, marked below).
# 3. Sampling: parent-based first (the P11.06 sampling policy record) —
#    `ParentBased(TraceIdRatioBased)`; tail sampling is DEFERRED with its
#    recorded trigger in the P11.06 record.
# 4. No capability claims: none of this has executed; the first human
#    run is the verification gate (program conventions §1).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import re
from typing import Optional

# The named deployment requirement a missing opentelemetry surfaces
# (raised by `require_opentelemetry`); the requirements.txt P11.05
# entries repeat the pins.
OTEL_DEPLOYMENT_REQUIREMENT = (
    "flexcore worker otel tracing requires the opentelemetry deployment "
    "dependency: install the pins recorded in python/workers/"
    "requirements.txt (P11.05 entries, UNPINNED-REVERIFY procedure "
    "applies at activation) before enabling trace export"
)

# The W3C traceparent grammar (W3C Trace Context, the format the Rust
# publisher writes into JetStream headers and the propagator expects):
# version 2 hex - trace-id 32 hex (not all zeros) - span-id 16 hex (not
# all zeros) - flags 2 hex.
_TRACEPARENT_PATTERN = re.compile(
    r"^([0-9a-fA-F]{2})-([0-9a-fA-F]{32})-([0-9a-fA-F]{16})-([0-9a-fA-F]{2})$"
)


def require_opentelemetry() -> object:
    """Import and return the `opentelemetry` root module or raise the
    named requirement.

    The single choke point for the opentelemetry import; every SDK use
    in this module goes through functions that call this first, so the
    deployment requirement is stated exactly once (the server.py
    `require_grpc` pattern).
    """
    try:
        import opentelemetry  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as exc:  # pragma: no cover - depends on deployment
        raise ImportError(OTEL_DEPLOYMENT_REQUIREMENT) from exc
    return opentelemetry


def init_tracing(
    config: object,
    *,
    service_name: Optional[str] = None,
) -> object:
    """Initialize the OTel trace provider and the W3C propagator (the
    Python SDK seam).

    UNVERIFIED — otel API, every call below (module header item 1):
    - `opentelemetry.sdk.trace.TracerProvider(resource=Resource.create(
      {"service.name": ...}))` — the documented provider shape.
    - `opentelemetry.sdk.trace.export.BatchSpanProcessor(
      OTLPSpanExporter(endpoint=...))` — batched OTLP export over gRPC
      (`opentelemetry.exporter.otlp.proto.grpc.trace_exporter`).
    - `opentelemetry.sdk.trace.sampling.ParentBased(
      TraceIdRatioBased(ratio))` — parent-based head sampling (the P11.06
      sampling policy record; the ratio comes from the worker config,
      DEFAULT 1.0).
    - `opentelemetry.trace.set_tracer_provider(provider)` and
      `opentelemetry.propagate.set_global(
      TraceContextTextMapPropagator())` — global installation.

    `config` is the template's `WorkerConfig` (the
    `otel_exporter_otlp_endpoint` / `otel_trace_sample_ratio` fields, or
    any object carrying the same attributes). A blank endpoint DISABLES
    export and the function returns None — the honest no-op (no
    fabricated provider). Returns the provider (the caller keeps it and
    passes it to `shutdown_tracing` at worker stop).
    """
    endpoint = getattr(config, "otel_exporter_otlp_endpoint", "")
    if not endpoint:
        return None
    require_opentelemetry()  # UNVERIFIED — otel API (named requirement above).

    # UNVERIFIED — otel API: deferred imports; the deployment pins live in
    # requirements.txt (P11.05 entries).
    from opentelemetry import propagate, trace  # noqa: PLC0415
    from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import (  # noqa: PLC0415
        OTLPSpanExporter,
    )
    from opentelemetry.sdk.resources import Resource  # noqa: PLC0415
    from opentelemetry.sdk.trace import TracerProvider  # noqa: PLC0415
    from opentelemetry.sdk.trace.export import BatchSpanProcessor  # noqa: PLC0415
    from opentelemetry.sdk.trace.sampling import (  # noqa: PLC0415
        ParentBased,
        TraceIdRatioBased,
    )

    name = service_name or getattr(config, "worker_id", "flexcore-worker")
    ratio = float(getattr(config, "otel_trace_sample_ratio", 1.0))
    provider = TracerProvider(
        resource=Resource.create({"service.name": name}),
        sampler=ParentBased(TraceIdRatioBased(max(0.0, min(1.0, ratio)))),
    )
    provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter(endpoint=endpoint)))
    trace.set_tracer_provider(provider)
    propagate.set_global(propagate.TraceContextTextMapPropagator())
    return provider


def shutdown_tracing(provider: object) -> None:
    """Shut the provider down (flush batched spans best-effort).

    UNVERIFIED — otel API: `TracerProvider.shutdown()` is the documented
    shutdown call; a None provider (export disabled) is a no-op.
    """
    if provider is None:
        return
    provider.shutdown()  # type: ignore[attr-defined]  # UNVERIFIED — otel API.


def extract_trace_context_into(carrier: dict) -> None:
    """Extract W3C trace context from a header/metadata dict into the
    CURRENT context (the consumer-side READ of the propagation contract).

    UNVERIFIED — otel API: `propagate.extract(carrier)` with a plain dict
    carrier is the documented shape (the default Getter handles
    case-insensitive lookups for the standard propagator). With no
    global propagator installed (export disabled, `init_tracing` never
    ran) this is a no-op — never a fabricated context.
    """
    if not isinstance(carrier, dict):
        return
    require_opentelemetry()  # UNVERIFIED — otel API (named requirement above).
    from opentelemetry import propagate  # noqa: PLC0415 — deferred, see header.

    propagate.extract(carrier)  # UNVERIFIED — otel API.


def trace_id_from_traceparent(traceparent: str) -> Optional[str]:
    """The 32-hex trace id of a W3C `traceparent` header value, or None
    when the value does not parse (pure stdlib logic — the runnable core
    of the propagation contract; the log-schema `trace_id` attribute
    comes from this parse when a worker consumes a JetStream message or
    an Execute call carrying the header).

    Validity rules implemented (W3C Trace Context): exact shape, and the
    all-zeros trace id / span id are INVALID and yield None.
    """
    if not isinstance(traceparent, str):
        return None
    match = _TRACEPARENT_PATTERN.match(traceparent.strip())
    if match is None:
        return None
    _, trace_id, span_id, _ = match.groups()
    if trace_id == "0" * 32 or span_id == "0" * 16:
        return None
    return trace_id.lower()


def carrier_trace_id(carrier: dict) -> Optional[str]:
    """The trace id carried by a header dict (a JetStream message header
    map or gRPC metadata), found case-insensitively under the W3C name
    `traceparent`. None when absent or unparseable — absent context is
    the honest outcome, never a synthesized id."""
    if not isinstance(carrier, dict):
        return None
    for key in carrier:
        if isinstance(key, str) and key.lower() == "traceparent":
            return trace_id_from_traceparent(str(carrier[key]))
    return None
