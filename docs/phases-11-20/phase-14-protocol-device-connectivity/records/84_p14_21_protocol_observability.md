# P14.21 — Protocol observability: per-adapter metrics/traces/logs taxonomy, connection-state alarms, message-rate budgets

| Field | Value |
|---|---|
| Subtask | P14.21 (wave 3, Agent A) — per-adapter metrics/traces/logs taxonomy, connection-state alarms (`ConnectionState::name()`), message-rate budgets bound to the P11 alert-rule shape |
| Deliverable class | The registered taxonomy (record 83 §3's shared module) + ONE additive alert-rule file |
| Honesty boundary | WRITTEN BUT CANNOT FIRE: no live Prometheus has evaluated these rules, and NO adapter has ever rendered any of the families (no protocol connection exists; the 0–10 base is WRITTEN-UNRUN). The live-evaluation check and the alert-routing dry run are human gate inputs, recorded UNRUN. |

## 1. The taxonomy (one sentence of structure, then the pointers)

The metrics/traces/logs taxonomy is defined ONCE in
`crates/flexcore-protocol-ports/src/observability.rs` (record 83 §3):
five metric families with closed label sets; the structured-log shape
CONSUMES the P11.07 schema verbatim (`flexcore.log.v1`, `trace_id`,
`tenant_id`, `command_id`) and adds exactly one tier field
(`adapter_id`) — trace correlation is consumed, never redefined; the
connection-state label values are EXACTLY `ConnectionState::name()`
(the five-state vocabulary of `flexcore-protocol-ports::connection`).
This record owns the ALARM surface: the additive rule file that binds
those names and gives them operator thresholds.

## 2. The additive rule file

`deploy/compose/observability/prometheus/rules/
protocol_adapter_alerts.yml` — CREATED, ADDITIVE (the three P11 wave-1
rule files — `recording_rules.yml`, `dlq_depth_alerts.yml`,
`slo_burn_rate_alerts.yml` — are byte-untouched; this file follows
their comment pattern: WRITTEN-UNRUN header, X-8 RECOMMENDATION
thresholds, page/ticket severity labels with the unbranded routing
rule of P11-E-3 carried over).

Alerts (names bound to the shared vocabulary):

| Alert | Fires on | Severity |
|---|---|---|
| `FlexCoreProtocolAdapterUnreachable` | any adapter in `state="UNREACHABLE"` (`ConnectionState::name()` label match) | page |
| `FlexCoreProtocolAdapterDegraded` | any adapter in `state="DEGRADED"` | ticket |
| `FlexCoreProtocolAdapterBudgetRefusals` | sustained refused-message rate > 0 (15m) | ticket |
| `FlexCoreProtocolAdapterQueueDrops` | audited queue drops > 0 (1h) | ticket |
| `FlexCoreProtocolAdapterQueueDepthGrowing` | queue depth > the registered review depth (1000) | ticket |
| `FlexCoreProtocolAdapterMessageRateOverBudget` | the 5m message-rate recording aggregate > the registered placeholder | ticket |

Recording aggregates (the P11.09 shape):
`flexcore:sli:protocol_adapter_message_rate_5m`,
`flexcore:sli:protocol_adapter_inbound_rate_5m`,
`flexcore:sli:protocol_adapter_outbound_rate_5m`.

## 3. Recorded honesty caveats (the P11.13 pattern, applied twice)

1. **The families do not render yet.** Every rule above is WRITTEN BUT
   CANNOT FIRE until a real adapter composition lands and its transport
   wires exist — stated in the file header, exactly as the P11.03 file
   stated it for the wave-2 `flexcore_commands_stuck` family.
2. **The message-rate budget line is a REGISTERED PLACEHOLDER.** Each
   deployment's real budget is its declared `MessageRateBudget`
   (composition data — Rust constants Prometheus cannot read). The
   alert's 10 msg/s figure is an X-8 pre-registration placeholder the
   composition's budget declaration must confirm or mechanically update
   when the first real adapter lands — the same caveat class as the
   P11.13 file's UNVERIFIED exporter family names, and labeled as such
   in the file.

## 4. Files created / modified

| File | Status |
|---|---|
| `deploy/compose/observability/prometheus/rules/protocol_adapter_alerts.yml` | CREATED (the only file this subtask owns exclusively) |
| `crates/flexcore-protocol-ports/src/observability.rs` | the taxonomy module (record 83 is its owning record; P14.21 consumes it) |

Byte-untouched: the three existing rule files,
`apps/control-plane/src/metrics.rs`, `apps/control-plane/src/
dto_alarms.rs` and the alarm-route surface (the P11 alarm vocabulary is
not extended — protocol-tier alarms enter through the SAME
severity-labeled Prometheus rule shape, not a second alarm model),
`ledger_phase_14.md`.

## 5. Human gates named

(a) the first workspace build; (b) the first live Prometheus evaluation
of the rule group (UNRUN); (c) the alert-routing dry run (P11-E-3's
operator-configured webhook, UNRUN); (d) operator sign-off of every
registered threshold; (e) the placeholder-budget confirmation or
mechanical update at the first real adapter composition.
