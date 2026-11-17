# P11.05 — OTel in the Python workers + trace_id propagation through JetStream headers

| Field | Value |
|---|---|
| Subtask | P11.05 (OTel in Python workers; trace_id propagation through JetStream message headers) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: Python modules and Rust seam written, never executed, never compiled. opentelemetry was NEVER installed (the import-inside-function pattern keeps the Python modules parseable without it — the recorded server.py precedent). |
| Sources | [S-63] (OTel project maturity — the Python SDK is Stable; the Rust maturity note lives in record 04); W3C Trace Context grammar (the `traceparent` shape the pure parser implements); version check against PyPI (opentelemetry-sdk 1.45.2, released 2026-10-29), performed 2026-11-14 |
| Files touched | `python/workers/worker_template/otel_tracing.py` (new), `python/workers/worker_template/config.py` (additive OTel keys), `python/workers/worker_template/__init__.py` (module-map note), `python/workers/requirements.txt` (additive pins), `python/workers/tests/test_log_schema_and_trace_propagation.py` (new, UNRUN), `crates/adapters/nats/src/publisher.rs` (the publish-side header seam), `apps/worker/src/otel.rs` (new), `apps/worker/src/lib.rs` (SHARED with Agent B — additive module line), `apps/worker/src/main.rs` (SHARED — marked additive block), `apps/worker/src/runner.rs` (marked one-line wire), `apps/worker/Cargo.toml` (additive dependency lines only) |

## 1. The propagation contract (publisher sets, consumer reads)

- **Header name**: the W3C standard `traceparent` (plus any other pair
  the active propagator writes), set as a JetStream MESSAGE HEADER on
  every published message.
- **Publisher SETS** (`crates/adapters/nats/src/publisher.rs`): the
  `OutboxPublisher` gains an additive `TraceHeaderProvider` seam — a
  `Arc<dyn Fn() -> Vec<(String, String)>>` the composition root
  supplies; `publish_entry` appends the returned pairs to the header map
  (after the mandatory `Nats-Msg-Id`, which is untouched). The DEFAULT
  provider returns nothing, so the seam is INERT unless composed (the
  existing construction sites and tests are unchanged). The adapter
  crate itself stays OTel-free — the provider is a plain callback (the
  dependency rule holds; the OTel SDK lives in the composition root's
  `apps/worker/src/otel.rs`).
- **Worker composition** (`apps/worker/src/otel.rs`): the global W3C
  propagator is set in ALL modes (export disabled included);
  `otel::trace_header_pairs()` is the provider wired into
  `runner.rs::run_outbox_publisher` (one marked edit). An absent active
  span (the drain loop's steady state) injects NOTHING — no trace id is
  ever fabricated (recorded limitation: automatic end-to-end tracing
  requires producers to run inside spans; the seam carries whatever
  context exists).
- **Consumer READS**: the command consumer's header read is Agent B's
  file (P11.12). HANDOFF NOTE TO AGENT B: read the W3C `traceparent`
  from the JetStream message headers (case-insensitive lookup is the
  W3C convention; the pure reference parser is
  `worker_template/otel_tracing.carrier_trace_id` / the Rust consumer
  reads the header via async-nats's `HeaderMap`); an absent or
  unparseable header means "no inherited context" — never a synthesized
  trace id.

## 2. Python side (the template)

1. **`otel_tracing.py`** — the ONLY opentelemetry touchpoint (the
   `require_grpc` choke-point pattern): `init_tracing(config)` builds
   the TracerProvider (Resource service name, `ParentBased(
   TraceIdRatioBased)` — the P11.06 sampling record) with a
   `BatchSpanProcessor(OTLPSpanExporter(endpoint))`, sets the global
   provider and `TraceContextTextMapPropagator`; every SDK call is
   marked `UNVERIFIED — otel API`. A blank endpoint DISABLES export and
   returns None (the honest no-op). Pure stdlib helpers are runnable
   today: `trace_id_from_traceparent` (W3C grammar: exact shape,
   all-zero trace/span ids invalid), `carrier_trace_id`
   (case-insensitive header lookup), `extract_trace_context_into`.
2. **Config keys** (`config.py`, additive): `FLEXCORE_WORKER_OTEL_EXPORTER_OTLP_ENDPOINT`
   (DEFAULT blank = export disabled) and
   `FLEXCORE_WORKER_OTEL_TRACE_SAMPLE_RATIO` (DEFAULT 1.0, fail-closed
   parse in [0.0, 1.0]); `WorkerConfig` gains the two defaulted fields
   (no existing construction site changes).
3. **`requirements.txt`** (additive): `opentelemetry-sdk==1.45.2`
   (checked on PyPI 2026-11-14, released 2026-10-29) and
   `opentelemetry-exporter-otlp-proto-grpc==1.45.2` — the exporter pin
   is an ASSUMPTION (same coordinated release as the SDK), re-verified
   at activation per the file's UNPINNED-REVERIFY procedure. Never
   installed in this loop.
4. **Tests** (`test_log_schema_and_trace_propagation.py`, UNRUN): the
   module imports WITHOUT opentelemetry; the named deployment
   requirement fires without it; the traceparent parser (valid shape,
   uppercase normalization, all-zero ids invalid, junk → None);
   case-insensitive carrier lookup; the JetStream header shape; absent
   context → None; the fail-closed config keys.

## 3. Honesty boundary

- The Python SDK calls are UNVERIFIED (opentelemetry never installed);
  the Rust seam compiles-or-not only at the first human build (the
  pinned-version ASSUMPTION is stated in the module headers and the
  Cargo.toml comments).
- No end-to-end propagation has been observed or is claimed; what is
  written is the seam, the parser, and the honest no-op behaviors.
