# P11 wave 1 Agent A — close-out record (P11.01–P11.08)

| Field | Value |
|---|---|
| Wave / agent | Phase 11 wave 1, Agent A |
| Subtasks in this batch | P11.01, P11.02, P11.04, P11.05, P11.06, P11.07, P11.08 (P11.03 is Agent B's; this batch delivers its metric-name input per record 01 §4) |
| Status | WRITTEN-UNRUN throughout. No build, no test run, no install, no docker command was executed (conventions §1). The three permitted verification tools were run at the end; their verbatim outputs are quoted in §3. |

## 1. Deliverables in this batch (all WRITTEN, never run)

| Subtask | Record | Code / config delivered |
|---|---|---|
| P11.01 | `01_p11_01_metric_taxonomy.md` (written BEFORE the families were added — the recorded discipline) | `apps/control-plane/src/metrics.rs`: the four named families + recording APIs + rendering + unit tests; `flexcore_commands_stuck` named but NOT added (wave 2, P11.20) |
| P11.02 | `02_p11_02_slo_register.md` | The register + error-budget policy; ALL targets RECOMMENDATION; low-traffic synthetic-traffic note per [S-61] |
| P11.04 | `04_p11_04_otel_tracing_rust.md` | `apps/control-plane/src/otel_telemetry.rs` (new), lib.rs module line, main.rs marked block + explicit guard shutdown, `apps/control-plane/Cargo.toml` additive pins |
| P11.05 | `05_p11_05_otel_python_and_trace_headers.md` | `crates/adapters/nats/src/publisher.rs` (additive `TraceHeaderProvider` seam), `apps/worker/src/otel.rs` (new), `apps/worker/src/lib.rs` (shared, additive), `apps/worker/src/main.rs` (shared, marked block), `apps/worker/src/runner.rs` (marked one-line wire), `apps/worker/Cargo.toml` additive pins, `python/workers/worker_template/otel_tracing.py` (new), `config.py` additive keys, `__init__.py` module-map note, `requirements.txt` additive pins |
| P11.06 | `06_p11_06_collector_gateway.md` | `deploy/compose/otel-collector-config.yaml` (new), `deploy/compose/docker-compose.yml` gateway service + app-service env vars |
| P11.07 | `07_p11_07_log_schema_v1.md` | `python/workers/worker_template/log_schema.py` (new); the control-plane correlation line carries the schema fields (`schema_version`, `severity_number`, `tenant_id` rename, `command_id`); `severity_number_for_level` (Rust half) in `metrics.rs` |
| P11.08 | `08_p11_08_redaction.md` | `python/workers/worker_template/redaction.py` (new); emit-time integration in `log_schema.emit_log_line`; `python/workers/tests/test_redaction.py` + `python/workers/tests/test_log_schema_and_trace_propagation.py` (complete, UNRUN) |

Test files written complete and NEVER RUN: the two new files under
`python/workers/tests/` (their execution is the human gate), plus the
Rust unit tests inside `metrics.rs`, `otel_telemetry.rs`, `otel.rs`
(executed only by the first human `cargo test`, which this program
never runs).

## 2. Version checks performed (documentation reads, not installs)

Checked 2026-11-14, i.e. after the 2026-11-07 research cut-off used by
the gap-analysis register — recorded here as the "what changed": exact
crate/package version numbers were added from the live release indexes.

- opentelemetry-rust releases page: latest tag `opentelemetry-0.33.1`
  ("The OTLP exporters for Logs and Metrics are now stable").
- crates.io: `tracing-opentelemetry` 0.34.0 (2026-09-23;
  `opentelemetry ^0.33.0`, `tracing ^0.1.35`); `opentelemetry-appender-tracing`
  0.33.1 (2026-10-09).
- PyPI: `opentelemetry-sdk` 1.45.2 (2026-10-29).
- opentelemetry-collector-releases releases page: v0.165.0 (2026-11-07).

All compile-compatibility conclusions are labeled ASSUMPTION in the
code comments, Cargo.toml comments, and records 04/05 — verified only
at the first human build, which has never run.

## 3. Permitted-tool outputs (verbatim)

`python tools/verify_docs.py`:

```text
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
```

`python tools/validate_agent.py`:

```text
skills: 5  roles: 5
OK: skills, roles, and README index are valid
```

`python -I tools/check_dependency_rule.py`:

```text
OK — dependency rule holds for 20 workspace members.
```

## 4. Deviations from the plan (all recorded, none silent)

1. **Record numbering skips 03.** P11.03 is Agent B's subtask; Agent A's
   record 03 slot is left empty so B's P11.03 record can take its own
   number without collision (B uses 21–39 per the dispatch brief; the
   03 slot is a courtesy gap, not a delivered record).
2. **No `apps/worker` OTel logs bridge.** The worker Rust module wires
   TRACES only (the worker's recorded subscriber shape is plain `fmt`,
   not JSON; adding a logs bridge there would change a surface no
   subtask in wave 1 owns). Recorded in `otel.rs`'s header.
3. **`flexcore_commands_stuck` named, not implemented** — per the plan's
   wave split (P11.20 is wave 2 Agent B's, sequential).
4. **Command-accept / gate-transition recording APIs exist without
   call-site wiring** — the call sites are Agent B's route and consumer
   files (handoff notes in record 01 §5 and record 05 §1). No fake
   wiring was placed in files Agent A does not own.

## 5. Handoff notes to Agent B (wave 1) and wave 2

1. **P11.03 (B)**: bind the burn-rate rules to the record 01 §4 names
   verbatim; `flexcore_commands_stuck` will not render until wave 2
   implements it — write the rule, record that it cannot fire yet.
2. **P11.12 consumer (B)**: read the W3C `traceparent` JetStream header
   case-insensitively (the reference parser shape is
   `worker_template/otel_tracing.carrier_trace_id`); absent/unparseable
   = no inherited context, never a synthesized trace id (record 05 §1).
3. **Command routes (B)**: call
   `MetricsRegistry::record_command_accept_duration_millis` around the
   committed PROPOSED record; populate `command_id` on command-scoped
   log emissions (record 01 §5, record 07 §3).
4. **Wave 2 (P11.20)**: add the `flexcore_commands_stuck` gauge exactly
   as named in record 01 §2 family 5 (no labels).
5. **Wave 2 / operator**: replace the Collector's `debug` exporter only
   after P11-E-2 closes (record 06 §2); the synthetic-traffic decision
   for the low-traffic command-lifecycle SLOs is an operator call
   (record 02 §3).
6. **Wave 2 (Rust redaction)**: the closed rule set exists only in
   Python; a Rust `redact_string` twin for the `tracing` emit path is
   recorded as not-yet-existing (record 08 §3).
