# P11.04 — OTel tracing in the control plane (Rust)

| Field | Value |
|---|---|
| Subtask | P11.04 (OTel tracing in control plane) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: code written, never compiled, never run. Pinned versions are an ASSUMPTION that they compile together — verified ONLY at the first human build. |
| Sources | [S-63] (OTel Rust — traces Beta, metrics/logs Stable); [S-62] (OTel logs data model — the logs bridge); version checks against crates.io / the opentelemetry-rust releases page / the tracing-opentelemetry crate page, performed 2026-11-14 |
| Files touched | `apps/control-plane/src/otel_telemetry.rs` (new), `apps/control-plane/src/lib.rs` (module registration), `apps/control-plane/src/main.rs` (SHARED with Agent B — marked additive block, re-read before edit), `apps/control-plane/Cargo.toml` (additive dependency lines only) |

## 1. Pinned versions and their verification status

Checked 2026-11-14 against the public release indexes (no install, no
build — the loop bans both; a version lookup is a documentation read):

| Crate | Pin | Check result |
|---|---|---|
| `opentelemetry` | 0.33.1 | Latest release tag on the opentelemetry-rust releases page, "The OTLP exporters for Logs and Metrics are now stable" |
| `opentelemetry_sdk` | 0.33 | Coordinated wave ("all crates in this repo follow the same version"); its existence at ^0.33 is corroborated by the tracing-opentelemetry 0.34.0 dev-dependencies |
| `opentelemetry-otlp` | 0.33 (features `grpc-tonic`, `logs`) | Same coordinated wave; `grpc-tonic` is a feature used by tracing-opentelemetry's own dev-dependencies on otlp 0.33 |
| `tracing-opentelemetry` | 0.34.0 | crates.io: published 2026-09-23, depends on `opentelemetry ^0.33.0`, `tracing ^0.1.35`, `tracing-subscriber ^0.3.22` — compatible with the workspace's tracing 0.1.44 / tracing-subscriber 0.3.23 |
| `opentelemetry-appender-tracing` | 0.33.1 | crates.io: published 2026-10-09 |

ASSUMPTION (never verified by any build): the pinned set compiles
together and the builder/type names used in the module match the 0.33
SDK shape. Every SDK call in `otel_telemetry.rs` carries a VERIFY-BUILD
note; the module header states the same. The version check dates fall
at the research cut-off boundary (latest releases were checked
2026-11-14, i.e. after the 2026-11-07 cut-off used elsewhere); what
changed vs the cut-off-state register: exact version numbers were
added from the live release indexes. No other claim changed.

## 2. What was implemented (written code)

1. **`otel_telemetry::TelemetryConfig`** — env-driven configuration
   (`FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT` — blank/unset DISABLES OTel
   export; `FLEXCORE_OTEL_SERVICE_NAME`, DEFAULT
   `flexcore-control-plane`; `FLEXCORE_OTEL_TRACE_SAMPLE_RATIO`, DEFAULT
   1.0), fail-closed parsing (an unusable ratio is a named `Err`, and the
   composition root exits 2 — the P1.21 convention). Unit tests pin the
   defaults, the blank-endpoint disable, and the fail-closed ratio.
2. **`otel_telemetry::init_from_env`** — sets the global W3C
   `TraceContextPropagator` UNCONDITIONALLY (the P11.05 publish seam
   needs it even with export disabled; a noop context injects nothing),
   builds the OTLP span exporter (gRPC/tonic) and `SdkTracerProvider`
   with `Sampler::ParentBased(TraceIdRatioBased)` (head/parent-based
   first — the P11.06 sampling policy record), builds the OTLP log
   exporter and `SdkLoggerProvider` ([S-62] bridge), and composes the
   subscriber: registry + JSON fmt layer (the P1.20 stdout behavior,
   unchanged shape) + `tracing_opentelemetry` span layer + the
   `opentelemetry_appender_tracing` logs bridge. With export disabled
   the subscriber is the plain P1.20 JSON shape (behavior preserved).
   Exporter-construction failure is fail-closed (exit 2, named message).
3. **`otel_telemetry::TelemetryGuard`** — owns the providers; an
   explicit `shutdown()` the composition root calls BEFORE its recorded
   `std::process::exit(0)` (which skips `Drop` — the guard would never
   run otherwise; recorded limitation: an abrupt exit loses the
   unflushed batch).
4. **Span taxonomy** — `otel_telemetry::command_gate_span(from, to)`:
   ONE span per gate transition over the frozen 14-state lifecycle
   (`crates/flexcore-domain/src/command_state.rs`); the
   `command.state.from` / `command.state.to` attributes come from
   `CommandState::name()` ONLY — no span invents a state. The helper is
   exported for the code that commits transitions (the command routes /
   command consumer — Agent B's files; handoff note in record 01 §5).
   Opening a span has no authority effect (ADR-010 untouched;
   observability reads the lifecycle, never drives it).
5. **Wiring** — `main.rs` marked block
   (`// P11.04 — OTel tracing wiring (Phase 11 wave 1, Agent A)`):
   config parse + `init_from_env` replace the previous inline
   subscriber init (behavior identical when the endpoint is unset);
   explicit `otel_guard.shutdown()` before the recorded `exit(0)`.

## 3. Honesty boundary

- No span, export, or subscriber composition has ever executed. The
  Compose gateway (P11.06) is written, not pulled.
- No capability claim: nothing here says the control plane "has
  distributed tracing" — it has a WRITTEN tracing wiring whose first
  human build may surface recorded API mismatches (the ASSUMPTION above).
