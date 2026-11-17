# P11.06 — OTel Collector gateway deployment config + sampling policy

| Field | Value |
|---|---|
| Subtask | P11.06 (OTel Collector gateway deployment config; sampling policy record) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: config and Compose shape written; never pulled, never built, never run (the loop bans docker pulls/builds). A live Collector integration check is a human gate input, recorded UNRUN (plan §5 risk 5). |
| Sources | [S-64] (OTel Collector gateway deployment pattern — a single gateway receives from every process, batches/samples/forwards); image version check against the opentelemetry-collector-releases releases page, performed 2026-11-14 |
| Files touched | `deploy/compose/docker-compose.yml` (gateway service + env vars on the two app services; Agent B owns the `deploy/compose/observability/` ops TREE, not this file — the compose file is Agent A's Collector-gateway ownership per plan §3), `deploy/compose/otel-collector-config.yaml` (new) |

## 1. The gateway shape

Single `otel-collector` service in the Compose stack [S-64]:

- Image pinned `otel/opentelemetry-collector-contrib:0.165.0` — the
  latest collector release at the 2026-11-14 check (v0.165.0, released
  2026-11-07). The pin is an ASSUMPTION that the tag exists on Docker
  Hub in the expected form; re-verified at the first human build (never
  pulled in this loop).
- Config mounted read-only from `deploy/compose/otel-collector-config.yaml`:
  `otlp` receivers on gRPC :4317 and HTTP :4318 (the two app services
  export to `http://otel-collector:4317`), `memory_limiter` +
  `batch` processors, a `health_check` extension, and the `debug`
  exporter as the recorded PLACEHOLDER forward target.
- Host ports 4317/4318 are bound to 127.0.0.1 only (loopback
  inspection); the app services reach the gateway over the Compose
  network.
- The app services `depends_on` the gateway with
  `condition: service_started` (NOT healthy): an OTLP exporter build
  does not dial the gateway, and export failures are retried by the SDK
  — the collector is a forwarder, not a liveness dependency (the
  recorded rationale sits as a comment in the compose file).

## 2. Export target honesty

No long-term metrics/trace store is deployed in Phase 11: P11.10 is a
selection record and P11-E-2 stays OPEN. The `debug` exporter (stdout,
container log) is the recorded placeholder; the config file header
states that replacing it with the selected backend's exporter happens
only after P11-E-2 closes. The gateway forwards nowhere that does not
exist.

## 3. Sampling policy (the recorded decision)

- **Head/parent-based first**: sampling is decided ONCE at the root
  span (`Sampler::ParentBased(TraceIdRatioBased)` in the Rust SDKs —
  `apps/control-plane/src/otel_telemetry.rs` and `apps/worker/src/otel.rs`;
  `ParentBased(TraceIdRatioBased)` in the Python seam) and propagated to
  all children. DEFAULT ratio 1.0 (sample everything) — a RECOMMENDATION
  registered before any run (X-8), never a tuned production value.
- **Tail sampling DEFERRED.** Recorded trigger for revisiting: when OTLP
  trace volume (head-sampled at the configured ratio) makes the gateway
  export cost an operational problem, the gateway gains a
  `tail_sampling` processor with the policy set REGISTERED in an
  amendment to this record BEFORE it is enabled (X-8 pre-registration).
  Until that trigger fires, no tail-sampling config is written.

## 4. Honesty boundary

- The compose service and YAML are validated by inspection and the
  permitted tools only; no collector has ever parsed them.
- [S-64] is cited for the gateway pattern; it makes no claim about
  FlexCore's deployment.
