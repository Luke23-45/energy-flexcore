# P11.10 — Long-term metrics store selection record (P11-E-2)

| Field | Value |
|---|---|
| Subtask | P11.10 (long-term metrics store selection record) |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | DECISION REGISTERED OPEN — agents never close E-decisions (program conventions §4). The DEFAULT below is a RECOMMENDATION, implemented only as deployment shape when an operator closes the decision. |
| Sources | The gap analysis §11 register: [S-61] (SRE workbook context for retention windows); the UNVERIFIED list (gap analysis §11: "VictoriaMetrics/Mimir vendor benchmarks" — do not cite as fact). No vendor documentation is quoted in this record. |
| Files touched | none (a selection record; no code, no deployment change) |

## 1. The decision point (P11-E-2)

**Question:** where do metrics live beyond the Prometheus scrape window
(the `apps/control-plane/src/metrics.rs` `/metrics` body is scraped by
whatever store the deployment runs; nothing retains metrics long-term
today)?

**Options (the plan §4 P11-E-2 wording, unchanged):**

| Option | Shape | Recorded trade-off (no performance claims) |
|---|---|---|
| (a) VictoriaMetrics single-node | one additional Compose service; speaks the Prometheus query/exposition surface, so the P11.09 datasource and dashboards are store-agnostic under it | smallest moving part for the current single-node Compose shape; the Compose-only ceiling of this phase is respected |
| (b) A second Prometheus with `remote_write` | two Prometheus processes, one as the long-term sink | adds a second scrape topology and a remote-write pipeline to operate |
| (c) A Mimir / Thanos-class sharded store | horizontally scaled storage tier | explicitly premature at the current deployment scale (plan §4 wording) |

## 2. DEFAULT (RECOMMENDATION)

**(a) VictoriaMetrics single-node** — the plan's recorded DEFAULT,
unchanged here. The P11.09 Grafana datasource
(`deploy/compose/observability/grafana/provisioning/datasources/prometheus.yml`)
is written store-agnostic: under the DEFAULT, the `url` points at the
VictoriaMetrics single-node service's Prometheus-compatible query
endpoint instead of a Prometheus process — no dashboard or rule file
changes.

**What closes it** (the operator's acceptance, not an agent's):

1. the operator accepts the storage footprint and the retention design
   (retention duration and the per-deployment retention bounds are the
   operator's numbers to set — none are registered here);
2. a live deployment check of the chosen store against the P11.09
   provisioning tree (the human gate input, currently UNRUN).

## 3. Honesty boundary — the UNVERIFIED list

Vendor benchmarks for VictoriaMetrics and Mimir are on the gap analysis
§11 UNVERIFIED list. This record carries NO performance, capacity,
resource, or comparison claims of any kind about any option. The
trade-off column records deployment-shape complexity only.

## 4. Related open decision

**P11-E-3 (alert routing channel) is registered OPEN here** because
P11.09/P11.13 produce the alert rule files that a routing channel would
consume:

- **Question:** how do page-class (`severity: page`) and ticket-class
  (`severity: ticket`) alerts reach a human?
- **Options:** (a) generic operator-configured webhook receiver with a
  documented receiver contract; (b) a NATS-internal alert topic consumed
  by ops tooling; (c) none in-phase — alerts surface as metrics and
  dashboard panels only.
- **DEFAULT (RECOMMENDATION):** (a) — generic webhook, explicitly
  UNBRANDED: no PagerDuty, Slack, or any other vendor's conformance,
  integration, or compatibility claim is made anywhere in this phase's
  files; the rule files bind only neutral `severity` labels.
- **What closes it:** the human operator runbook (who is paged, where
  the receiver lives). "Who is on call" is an operator answer, not a
  platform one.

P11-E-2 and P11-E-3 both remain OPEN. The phase-close record (P11.22)
carries the formal register entries.
