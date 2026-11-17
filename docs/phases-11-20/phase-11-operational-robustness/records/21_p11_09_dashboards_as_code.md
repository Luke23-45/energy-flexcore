# P11.09 — Dashboard-as-code (Grafana provisioning + recording rules)

| Field | Value |
|---|---|
| Subtask | P11.09 (dashboard-as-code: Grafana provisioning JSONs + recording rules checked into git) |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | WRITTEN-UNRUN: no Grafana container, no Prometheus, and no metrics store has been started against these files in this program. The live provisioning/rendering/evaluation check is a HUMAN GATE INPUT, recorded UNRUN. |
| Sources | [S-61] (Google SRE Workbook — golden-signal dashboards and SLI shapes; no claim about FlexCore); the P11.01 taxonomy record (`records/01_p11_01_metric_taxonomy.md`); the P11.02 SLO register (`records/02_p11_02_slo_register.md`) |
| Files touched | `deploy/compose/observability/grafana/provisioning/datasources/prometheus.yml` (new), `deploy/compose/observability/grafana/provisioning/dashboards/flexcore.yml` (new), `deploy/compose/observability/grafana/dashboards/control_plane_golden_signals.json` (new), `deploy/compose/observability/grafana/dashboards/command_lifecycle.json` (new), `deploy/compose/observability/grafana/dashboards/ingestion_pipeline.json` (new), `deploy/compose/observability/prometheus/rules/recording_rules.yml` (new) |

## 1. What was written

The new checked-in ops tree `deploy/compose/observability/`:

```text
deploy/compose/observability/
  grafana/
    provisioning/datasources/prometheus.yml   — datasource (uid flexcore-prom), store-agnostic shape
    provisioning/dashboards/flexcore.yml      — dashboard provider (folder FlexCore)
    dashboards/control_plane_golden_signals.json
    dashboards/command_lifecycle.json
    dashboards/ingestion_pipeline.json
  prometheus/
    rules/recording_rules.yml                 — SLI-shaped recording rules
    rules/slo_burn_rate_alerts.yml            — P11.03 (handed to B by A's record 01 §4 / close-out)
    rules/dlq_depth_alerts.yml                — P11.13 DLQ-depth alerts
```

## 2. The family-to-panel mapping (no panel invents a metric)

Every panel query and every rule expression references ONLY the families
named in the P11.01 taxonomy record (§2/§4) or the pre-existing P1.20 /
P2.21 registry families. The mapping:

| Panel / rule | Family |
|---|---|
| request rate, per-status rate, burn denominators | `flexcore_http_requests_total{method,route,status}` (P1.20) |
| error ratio, burn numerators | `flexcore_http_errors_total` (P1.20) |
| p95 latency, latency good ratio (le="1000") | `flexcore_api_request_duration_milliseconds` (P11.01 family 1) |
| in-flight | `flexcore_api_in_flight_requests` (P11.01 family 2) |
| transitions by from/to, terminal ratio | `flexcore_command_state_transitions_total{from,to}` (P11.01 family 4) |
| accept latency p95, good ratio (le="500") | `flexcore_command_accept_duration_milliseconds` (P11.01 family 3) |
| stuck commands | `flexcore_commands_stuck` (P11.01 family 5 — named, implemented in wave 2/P11.20; the panel and the rule are written but CANNOT render/fire until then, recorded on the panel description and in the rule annotations) |
| telemetry events / quarantine / queue depth / quality states / stale assets / delay summary | the P2.21 families, referenced by name only (the P11.01 record's rule: not re-declared) |

No dashboard, panel, or rule references a family outside that list. The
worker-side consumer counters of P11.12
(`flexcore_command_consumer_*`) are deliberately NOT rendered by any
panel: they are outside the P11.01 taxonomy (the taxonomy record says a
needed family amends the record first) and the worker has no `/metrics`
surface (A's record 01 §5 note 2) — handoff note in the P11.12 record.

## 3. Recorded decisions

1. **Reconciliation with A's taxonomy** — the dashboards were written
   AFTER A's taxonomy record landed (checked mid-run and again at close);
   all names match A's record 01 §4 verbatim. The terminal-state ratio
   uses the P11.02 register's SLO 4 definition (terminal transitions /
   transitions to APPROVED), not a naive all-transitions denominator.
2. **P11.03 delivered here.** Agent A's plan range includes P11.03, but
   A's record 01 §4 and close-out §1 explicitly hand the burn-rate rule
   implementation to B (the ops tree is B's exclusive ownership). The
   file `rules/slo_burn_rate_alerts.yml` implements the [S-61]
   multi-window multi-burn-rate pattern (14.4x / 1h+5m page, 6x /
   6h+30m page, 1x / 3d ticket) over A's §4 names, with the budgets
   (0.001 availability, 0.01 latency) matching the P11.02 register's
   RECOMMENDATION targets. The ledger's P11.03 tick cites this record
   plus A's records.
3. **Provisioning paths are documented, not wired.** The Compose
   services for Grafana / Prometheus (and the rule-file mounts) are NOT
   in `docker-compose.yml` — that file's wave-1 additions are Agent A's
   (the Collector gateway, record 06). Adding the metrics-store and
   Grafana services is the recorded handoff note (§4) and interacts with
   the OPEN P11-E-2 store selection.
4. **DLQ alert metric names are an ASSUMPTION.** The DLQ-depth rules
   reference `jetstream_stream_messages{stream=...}` — the NATS
   Prometheus exporter's stream-monitoring family shape. No exporter is
   deployed in this program and the exact exported names are UNVERIFIED
   against a live exporter; the stream-name label constants are
   authoritative (they come from `topology.rs`). Recorded in the
   P11.13 record and on the rule file header.

## 4. Handoff notes

1. **Wave 2 / operator**: add the Prometheus (or P11-E-2 DEFAULT
   VictoriaMetrics single-node, which speaks the Prometheus query API
   shape the datasource file targets) and Grafana services to
   `deploy/compose/docker-compose.yml` with mounts of this tree
   (provisioning → `/etc/grafana/provisioning`, dashboards →
   `/var/lib/grafana/dashboards`, rules → the store's rule path).
2. **P11.02 authority**: when the P11.02 register's targets are signed
   off (or amended), the burn budgets inside
   `rules/slo_burn_rate_alerts.yml` follow the register — they are
   written to match it today and must not drift silently.
3. **P11.20 (wave 2)**: implement `flexcore_commands_stuck` exactly as
   the P11.01 record names it; the panel and the stuck-state rule then
   render/fire with zero dashboard edits.

## 5. Honesty boundary

Nothing in this tree has been rendered, provisioned, or evaluated. Every
threshold is a REGISTERED RECOMMENDATION (X-8) pending operator
sign-off. [S-61] is cited for the golden-signal dashboard grouping and
the burn-rate pattern; it makes no claim about FlexCore.
