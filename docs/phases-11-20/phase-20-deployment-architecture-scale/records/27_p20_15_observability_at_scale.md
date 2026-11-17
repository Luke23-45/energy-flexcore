<!-- P20.15 — Observability at scale. Collector tiering when >1 host (agent →
     gateway [S-64]); long-term store topology consuming P11.10/P11-E-2
     (single-node DEFAULT + the scale conditions that would change it);
     cost-control notes consuming Agent A's P20.01 §4 cardinality budget
     (BY CITATION — the single definition, never restated). P20-E-3
     registered OPEN with DEFAULT (a) in this record. Nothing deployed,
     nothing measured, no retention value set; every measured/decision
     cell EMPTY. -->
# P20.15 — Observability at Scale (topology design — WRITTEN-UNRUN; P20-E-3 registered OPEN with DEFAULT (a); every measured cell EMPTY)

| Field | Value |
|---|---|
| Subtask | P20.15 — Observability at scale (`ledger_phase_20.md` §P20.15) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **TOPOLOGY DESIGN RECORD — WRITTEN-UNRUN.** No second host exists, no agent-tier Collector is deployed, no long-term metrics store exists (the P11.06 `debug` exporter placeholder is still the forward target), and no retention value is set. Every measured, consumed, and decision cell is EMPTY. |
| Consumed by citation (never restated) | P11.06 Collector gateway record (`docs/phases-11-20/phase-11-operational-robustness/records/06_p11_06_collector_gateway.md` — the single-gateway Compose shape and its `debug`-exporter honesty); P11.10/P11-E-2 selection record (`records/22_p11_10_metrics_store.md` — the single-node DEFAULT, OPEN); P11.01 metric taxonomy / P11.07 log schema (the label discipline the budget consumes — cited, not redefined); **Agent A's P20.01 §4 cardinality budget** (`records/01_p20_01_capacity_model_framework.md` — the SINGLE definition; consumed BY CITATION, never restated) |
| Sources | [S-64] (OTel Collector gateway deployment pattern — the agent → gateway tiering is the documented pattern class; mechanism-level only, no capability claim); [S-61] (SRE workbook context for retention-window thinking — no retention number is taken from it); [S-65] (JetStream, cited only where the event plane meets observability pipelines — no new stream claim) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No observability infrastructure beyond the written-unrun P11 Compose
shape exists: one gateway Collector service, a `debug` exporter, no
long-term store, no agent tier, no second host.** P11-E-2 remains OPEN
with the single-node DEFAULT, and nothing here closes it. This record
delivers exactly what plan §2 assigns: the collector tiering design for
the day a deployment has more than one host (§2 — a design against the
P20.04 T-1 trigger, INERT while it is UNFIRED), the long-term store
topology consuming P11.10/P11-E-2 with the scale conditions that would
move the DEFAULT (§3), the cost-control notes consuming the P20.01 §4
cardinality budget by citation (§4), and the P20-E-3 register entry OPEN
with DEFAULT (a) (§5). No capacity, volume, or cost figure exists.

## 2. Collector tiering when >1 host — the agent → gateway pattern [S-64]

| Element | Design | Status |
|---|---|---|
| Current shape (1 host) | ONE `otel-collector` gateway service; the two app services export OTLP to it over the Compose network; the `debug` exporter is the forward target until P11-E-2 closes (the P11.06 §2 honesty, consumed) | Exists as WRITTEN-UNRUN config; unchanged |
| Trigger for tiering | The P20.04 trigger register (`records/04_p20_04_kubernetes_trigger_register.md` §2, T-1 multi-host, **UNFIRED** — the single list; this record enumerates no second list) | UNFIRED; the design below is INERT |
| Agent tier | One per-host OTel Collector AGENT receiving local OTLP (loopback), applying `memory_limiter` + `batch` — the same processor class the P11.06 gateway config already carries, consumed not re-derived | INERT DESIGN |
| Gateway tier | The central gateway receives from every agent [S-64]; host-identity labeling, if used, must stay WITHIN the P20.01 §4 budget (a per-host label is exactly the label-combination class that budget governs — the budget's amendment rule is the only legal way to add it) | INERT DESIGN |
| Routing honesty | A Collector is a FORWARDER, not a liveness dependency — the P11.06 recorded rationale carries verbatim in substance; an agent's loss of the gateway buffers and retries (SDK retry semantics, the P11.04/P11.05 seam) and is never a health-gate signal | INVARIANT consumed |
| What never changes | The `/metrics` scrape topology (Prometheus-format exposition, the P1.20/P11.21 registry) stays DIRECT scrape per service — the Collector tiering applies to the OTLP TRACE/LOG pipeline, never to the metrics scrape path, which the long-term store owns | INVARIANT |

## 3. Long-term store topology — consuming P11.10/P11-E-2 (single-node DEFAULT)

The P11.10 selection record (`docs/phases-11-20/phase-11-operational-robustness/records/22_p11_10_metrics_store.md`) registers P11-E-2 OPEN with
DEFAULT (a): a VictoriaMetrics-class single-node store speaking the
Prometheus query surface, keeping the P11.09 datasource store-agnostic.
That DEFAULT is consumed unchanged; this record adds only the SCALE
conditions that would justify moving it — each an observable, evidenced
condition the OWNER records, never an agent inference:

| Condition (would move P11-E-2 / P20-E-3) | Observable evidence required | Status |
|---|---|---|
| SC-1 Cardinality pressure | The P20.01 §4 measured-count column shows sustained operation near a registered ceiling, or a legitimate need for a per-class ceiling raise that materially grows the total budget | `<EMPTY — UNMEASURED>` |
| SC-2 Retention-vs-storage pressure | A recorded retention duration (an owner decision — NO retention value exists today) implies a single-node footprint the operator judges uneconomical, evidenced against the store's own measured consumption on the real deployment | `<EMPTY — UNMEASURED>` |
| SC-3 Availability contract | A recorded availability requirement for the OBSERVABILITY plane itself (analogous to the P12.05 T1 trigger class) — noted honestly: observability data loss is not a dispatch-safety item (the P12.04 T7 posture, consumed), so this condition demands an explicit owner statement, not an inference | `<EMPTY — NOT STARTED>` |

Until a condition fires with evidence, the single-node DEFAULT stands —
one additional Compose-class service per deployment, no sharded/clustered
tier, no vendor selection. The UNVERIFIED-list discipline carries: no
vendor performance, capacity, or comparison claim is made (P11.10 §3).

## 4. Cost-control notes — consuming the P20.01 §4 cardinality budget (by citation)

**The budget's single definition is `records/01_p20_01_capacity_model_framework.md` §4 (≤ 640 active series total; per-tenant/per-asset labels
FORBIDDEN — they go to logs per the P11.01 taxonomy). This record does
not restate, re-derive, or locally amend it**; the two notes below consume
it:

| Note | Statement |
|---|---|
| CN-1 Cardinality budget consumption | Every observability consumer added at scale (dashboards, alert files, canary analysis per P20.11 §4) draws its series EXCLUSIVELY from the budget's registered classes; a consumer needing a new label combination amends P20.01 §4 by its recorded amendment rule — never a silent growth. The budget's measured-count column stays EMPTY until a real deployment scrapes the registry |
| CN-2 Retention vs storage | Retention DURATION is the operator's number (P11.10 §2 — none registered here or anywhere); the honest cost-control mechanism at this stage is structural, not numeric: (1) the ≤ 640-series budget bounds ingest volume by construction; (2) per-tenant/per-asset cardinality is forbidden as labels, which is the single largest structural cost lever recorded; (3) retention duration and downsample/aggregate policy are owner decisions gated on SC-2 evidence — a retention value written before evidence is exactly the no-basis-no-deploy rule (P9.10 §6 class), applied to observability |

## 5. P20-E-3 — registered OPEN with DEFAULT (a)

| Field | Value |
|---|---|
| Decision ID | **P20-E-3** (phase-20 plan §6 wording, carried unchanged) |
| Question | Long-term metrics topology at scale: does the P11.10 single-node DEFAULT change? |
| Options | (a) keep the P11.10 DEFAULT (single-node VictoriaMetrics-class) until cardinality/volume evidence says otherwise; (b) clustered/ha store topology (paired with the P20.04 multi-host trigger); (c) managed vendor service (a budget/vendor E-decision that belongs to the owner) |
| **DEFAULT (RECOMMENDATION)** | **(a)** — extends P11-E-2 with the scale conditions recorded in §3; this record says exactly what evidence would move it (SC-1 cardinality-budget consumption, SC-2 retention-vs-storage economics, SC-3 an explicit availability contract) |
| What would close it | The owner recording volume/cost evidence exceeding the single-node envelope, or naming a vendor |
| Closer | the human owner — agents never close E-decisions (conventions §4) |
| Status | **OPEN** |

## 6. What this record does NOT establish

1. No collector agent, gateway fleet, second host, or long-term store is
   deployed; the P11.06 `debug` exporter remains the forward target until
   P11-E-2/P20-E-3 close with an owner decision.
2. No retention duration, storage figure, cardinality count, or cost
   number exists — every such cell is EMPTY (X-8), and the §3 conditions
   are UNFIRED.
3. It does not redefine the metric registry, the log schema, or the
   sampling policy — P11.01/P11.07/P11.06 own them, consumed by path.
4. It does not close P11-E-2, P11-E-3, or P20-E-3, and it adds no
   dispatch authority (ADR-010): observability is a supervision-plane
   capability; no topology here touches the command lifecycle gates, the
   never-granted permission set, or tenant isolation.
5. §13.3 semantics: a future measured topology is a tested-day outcome,
   never a guarantee.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every measured/evidence cell is
   EMPTY with the triggers UNFIRED.
2. Confirm §2 enumerates NO second trigger list (the P20.04 §2 register
   is cited) and that the metrics-scrape-path invariant is stated.
3. Confirm §3 consumes the P11.10 DEFAULT unchanged and that the SC
   conditions require owner-recorded evidence; confirm no vendor claim.
4. Confirm §4 cites P20.01 §4 as the single definition and contains no
   restated ceiling or new series count.
5. Confirm §5 registers P20-E-3 OPEN with DEFAULT (a) and that no ledger
   checkbox was ticked by this agent.

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.15-1 | The agent → gateway Collector tiering is the documented OTel deployment pattern class [S-64] | Mechanism-level citation; no agent is deployed; specifics re-verified against primary sources at trigger time (R-P20-2 discipline) |
| A-P20.15-2 | The P20.01 §4 budget (≤ 640 series) is Agent A's registered DESIGN CONSTRAINT, not a measured count | Consumed by citation; its measured-count column is EMPTY and this record never fills it |
| F-P20.15-1 | No long-term store exists; the P11.06 `debug` exporter placeholder stands | Binding honesty statement, unchanged from the P11.06 record |
| F-P20.15-2 | No retention duration exists anywhere in the observability plane | Carried deliberately — a duration before evidence is the no-basis rule applied; CN-2 records the structural levers instead |

Status: **WRITTEN-UNRUN** — a topology design cannot collect or store by
being written; the first multi-host deployment, the store selection, and
every retention decision are human gates that have not happened and are
not scheduled.
