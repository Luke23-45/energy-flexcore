# P1.14 — NATS JetStream topology design (subjects, streams, consumers, retention)

| Field | Value |
|---|---|
| Subtask | P1.14 — Design the NATS JetStream topology (streams, subjects, consumers, retention) |
| Report trace | §15.4 (NATS and delivery semantics), §15.5 (transactional outbox), §6.3 (outbox four-step sequence), §6.4 (Outbox/Inbox guardrail), §16.1 (JetStream is messaging, not the authoritative database), §17.2 (security zones), §15.3 (event vocabulary via P0.14) |
| Inputs | P1.09 (event vocabulary types, outbox record); P0.14 (event vocabulary freeze, delivery rule); `RESEARCH.md` §5 (NATS server 2.15.x, async-nats 0.50.0, Jepsen durability caution, CVE-2026-33222/33223 note); P1.12 (outbox schema, `destination_subject`) |
| Status | Design for Phase 1 implementation (P1.15/P1.16). No stream is provisioned and no publisher/consumer code is written by this document |

---

## 1. Decisions made once

| ID | Decision | Rationale | Alternative considered and rejected |
|---|---|---|---|
| D-J1 | Subject scheme: **`flexcore.v1.<area>.<event_slug>`** (lowercase tokens; `event_slug` = lower_snake_case of the exact §15.3 event name; `v1` tracks the event schema major version) | One mechanical rule maps every event to exactly one subject with no per-event judgement; the version token gives breaking event-schema changes (P1.09 versioning rule) a subject-visible home (`v2`, never a silent repurpose) | Per-entity subjects (e.g. embedding `asset_id`): rejected — unbounded subject cardinality, and it would embed entity identifiers in subject strings (same leak class as tenant ids, D-J2). Also rejected: §15.3 event names verbatim in CamelCase tokens — subject tokens are conventionally lowercase |
| D-J2 | **No tenant identifier in any subject.** Tenant scoping happens at consumption, from the payload, under the same `TenantContext` rules as the API (P0.16 rule 5) | Subject names are enumerable by any connected client and surface in advisories, logs, and monitoring; embedding tenant ids would let any subscriber enumerate tenant activity (existence + volume), and per-tenant subject ACLs would couple authorization to subject naming and grow unbounded with the tenant count. Security note recorded per the ledger criterion: the scheme avoids tenant enumeration by construction; NATS account permissions are account-level in Phase 1, never per-tenant subjects | Tenant-token subjects (`flexcore.v1.<tenant_id>....`) with per-tenant ACLs: rejected for the reasons above; if a future phase revisits this, it requires a dedicated security review, not a config change |
| D-J3 | **One stream per bounded-context area** (11 event areas → 11 streams + 1 quarantine stream) | §15.4 requires retention, replication, acknowledgement, replay, and DLQ policy **per stream**; areas differ by orders of magnitude in volume and required retention (26 dispatch events/year vs millions of telemetry points/day in Phase 2), so per-area streams let Phase 2 telemetry adopt its own limits without touching Phase 1 streams or re-draining consumers | Single `FLEXCORE` stream with subject filters: simpler ops, but one retention/limits policy for everything — adopting telemetry in Phase 2 would force a redesign of the only stream; rejected |
| D-J4 | Retention: **`Limits`** on every stream; the actual `max_age`/`max_msgs`/`max_bytes` limits are **deployment configuration, not design values** (mirrors D-S3/P0.16: retention is configuration) | `Limits` retention keeps history replayable within the configured window for every consumer type (§15.4 replay); `Interest` retention would couple stream lifetime to consumer liveness and break replay-after-recovery; `WorkQueue` semantics are provided per-consumer by the inbox pattern instead, not by the stream | `Interest` / `WorkQueue` retention: rejected (reason above) |
| D-J5 | Replication factor **1 for Phase 1 (single-node Compose dev)**; clustered/HA JetStream is **a deployment decision, not a Phase 1 default** | Phase 1 is simulation-only (§21.4); the durability guarantee is the outbox + idempotency keys, not broker topology (§6.3, D-J6). Clustering changes failure semantics and must be sized with the deployment, with the fsync policy decided alongside it | Shipping a clustered default: rejected — it would imply an availability/durability posture Phase 1 does not test or claim |
| D-J6 | Delivery rule: **at-least-once, never exactly-once**; durability claims stay modest; fsync policy is a deployment decision | §15.4 [R6]: consumers must be idempotent; do not promise exactly-once from broker acknowledgements or deduplication options. Jepsen analysis of JetStream 2.12.1 (`RESEARCH.md` §5) found lost writes when data files were truncated or corrupted on a minority of nodes, and documented the default fsync policy (flush every two minutes rather than before acknowledgement) as a risk — hence no zero-loss claim from the broker alone | — |

## 2. Core NATS vs JetStream

- **All 26 §15.3 events travel through the transactional outbox into JetStream streams** (§15.4: JetStream for durable work/events needing persistence and replay; §16.1 row 5: JetStream is messaging, never the authoritative business database).
- **NATS Core** is reserved for ephemeral notifications where loss is acceptable (§15.4); Phase 1 uses none — everything the Stage 1 workflow emits must be durable and verifiable (P1.26 checks JetStream/inbox evidence).
- Publish path (P1.15): outbox publisher reads `pending` rows, publishes with `Nats-Msg-Id` = `idempotency_key`, and marks `delivered_at` **only after the broker publish acknowledgement** (§6.3 step 3). Broker-side deduplication covers only the duplicate window (deployment configuration, NATS default applies unless changed); the application idempotency keys and inbox records are the durable guarantee (§15.5, P1.16).

## 3. Subject hierarchy — complete §15.3 mapping (26 events)

Scheme (D-J1): `flexcore.v1.<area>.<event_slug>` — one event, exactly one subject. Area tokens are fixed below; the event slug is mechanical (lower_snake_case of the exact §15.3 name, P0.14 §1). Each event's stream follows from its area (D-J3); the "Provisioned" column says when the stream is created (D-J7, §4.2).

| # | Event (§15.3, exact name) | Subject | Stream | Provisioned in |
|---|---|---|---|---|
| 1 | `AssetRegistered` | `flexcore.v1.registry.asset_registered` | `FLEXCORE_REGISTRY` | Phase 1 |
| 2 | `AssetCapabilityUpdated` | `flexcore.v1.registry.asset_capability_updated` | `FLEXCORE_REGISTRY` | Phase 1 |
| 3 | `TelemetryReceived` | `flexcore.v1.telemetry.telemetry_received` | `FLEXCORE_TELEMETRY` | Phase 2 |
| 4 | `TelemetryQualityChanged` | `flexcore.v1.telemetry.telemetry_quality_changed` | `FLEXCORE_TELEMETRY` | Phase 2 |
| 5 | `AssetAvailabilityChanged` | `flexcore.v1.state.asset_availability_changed` | `FLEXCORE_STATE` | Phase 1 |
| 6 | `FlexibilityEnvelopeUpdated` | `flexcore.v1.flexibility.flexibility_envelope_updated` | `FLEXCORE_FLEXIBILITY` | Later phase (first publisher) |
| 7 | `ForecastPublished` | `flexcore.v1.forecast.forecast_published` | `FLEXCORE_FORECAST` | Later phase (first publisher) |
| 8 | `MarketSignalReceived` | `flexcore.v1.market.market_signal_received` | `FLEXCORE_MARKET` | Later phase (first publisher) |
| 9 | `ScheduleProposed` | `flexcore.v1.scheduling.schedule_proposed` | `FLEXCORE_SCHEDULING` | Later phase (first publisher) |
| 10 | `ScheduleValidated` | `flexcore.v1.scheduling.schedule_validated` | `FLEXCORE_SCHEDULING` | Later phase (first publisher) |
| 11 | `FeasibilityCheckCompleted` | `flexcore.v1.validation.feasibility_check_completed` | `FLEXCORE_VALIDATION` | Later phase (first publisher) |
| 12 | `DispatchProposed` | `flexcore.v1.dispatch.dispatch_proposed` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 13 | `DispatchApproved` | `flexcore.v1.dispatch.dispatch_approved` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 14 | `DispatchRejected` | `flexcore.v1.dispatch.dispatch_rejected` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 15 | `DispatchIssued` | `flexcore.v1.dispatch.dispatch_issued` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 16 | `DispatchAcknowledged` | `flexcore.v1.dispatch.dispatch_acknowledged` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 17 | `DispatchTrackingUpdated` | `flexcore.v1.dispatch.dispatch_tracking_updated` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 18 | `DispatchCompleted` | `flexcore.v1.dispatch.dispatch_completed` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 19 | `DispatchFailed` | `flexcore.v1.dispatch.dispatch_failed` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 20 | `DispatchExpired` | `flexcore.v1.dispatch.dispatch_expired` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 21 | `DispatchCancelled` | `flexcore.v1.dispatch.dispatch_cancelled` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 22 | `DispatchSuperseded` | `flexcore.v1.dispatch.dispatch_superseded` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 23 | `DispatchFallbackActivated` | `flexcore.v1.dispatch.dispatch_fallback_activated` | `FLEXCORE_DISPATCH` | Later phase (first publisher) |
| 24 | `PluginActivated` | `flexcore.v1.plugins.plugin_activated` | `FLEXCORE_PLUGINS` | Later phase (first publisher) |
| 25 | `PluginFailed` | `flexcore.v1.plugins.plugin_failed` | `FLEXCORE_PLUGINS` | Later phase (first publisher) |
| 26 | `OperatorActionRecorded` | `flexcore.v1.audit.operator_action_recorded` | `FLEXCORE_AUDIT` | Phase 1 |

Count: 26 events, 26 subjects, one subject per event, no event on two subjects. Every Phase 1 event (1, 2, 5, 26 per the Stage 1 workflow; the publisher wires exactly what P1.09/P1.26 emit) lands on a Phase 1 stream.

D-J7 (recorded): Phase 1 provisions `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, and `FLEXCORE_DLQ`. The remaining streams are created by the phase that first publishes into them — their definitions are fixed here so no later phase re-decides the scheme.

## 4. Streams

### 4.1 Per-stream policy (all streams share this shape)

| Property | Value | Notes |
|---|---|---|
| Retention | `Limits` (D-J4) | Replayable within the configured window |
| Storage | `File` | Durable across restarts; file store under a named volume in Compose (P1.25) |
| Replication | 1 (D-J5) | Clustering is a deployment decision, never a Phase 1 default |
| Discard policy | `DiscardOld` | Oldest first when a limit is hit; limits themselves are deployment configuration |
| Duplicate window | Deployment configuration | `Nats-Msg-Id` dedup applies within it only; the durable guarantee is the outbox + inbox (§2, D-J6) |
| Publish acknowledgement | Required | The outbox publisher marks `delivered_at` only after the JetStream publish ack (§6.3 step 3) |
| Consumer acknowledgement | `AckExplicit` on every consumer | See §5 |
| Replay | `ReplayInstant` default per consumer | `ReplayOriginal` selectable per consumer for review/replay scenarios; replay is bounded by the retained window — no unlimited-replay claim |
| Dead-letter / quarantine | Per-stream DLQ subject → `FLEXCORE_DLQ` | See §5.2; a poison message is never silently dropped |

Max-age / max-msgs / max-bytes: **no values are fixed in this design** (D-J4 mirrors the retention-is-configuration rule). The Compose dev stack (P1.25) sets explicit dev-only values, marked dev-only; production values are a deployment decision per data class (§16.3).

### 4.2 Stream inventory

| Stream | Subject filter | Covers | Provisioned in |
|---|---|---|---|
| `FLEXCORE_REGISTRY` | `flexcore.v1.registry.>` | Events 1–2 | Phase 1 |
| `FLEXCORE_STATE` | `flexcore.v1.state.>` | Event 5 | Phase 1 |
| `FLEXCORE_AUDIT` | `flexcore.v1.audit.>` | Event 26 | Phase 1 |
| `FLEXCORE_TELEMETRY` | `flexcore.v1.telemetry.>` | Events 3–4 | Phase 2 |
| `FLEXCORE_FLEXIBILITY` | `flexcore.v1.flexibility.>` | Event 6 | Later phase (first publisher) |
| `FLEXCORE_FORECAST` | `flexcore.v1.forecast.>` | Event 7 | Later phase (first publisher) |
| `FLEXCORE_MARKET` | `flexcore.v1.market.>` | Event 8 | Later phase (first publisher) |
| `FLEXCORE_SCHEDULING` | `flexcore.v1.scheduling.>` | Events 9–10 | Later phase (first publisher) |
| `FLEXCORE_VALIDATION` | `flexcore.v1.validation.>` | Event 11 | Later phase (first publisher) |
| `FLEXCORE_DISPATCH` | `flexcore.v1.dispatch.>` | Events 12–23 | Later phase (first publisher) |
| `FLEXCORE_PLUGINS` | `flexcore.v1.plugins.>` | Events 24–25 | Later phase (first publisher) |
| `FLEXCORE_DLQ` | `flexcore.dlq.v1.>` | Quarantine copies of poison messages from any stream | Phase 1 |

DLQ subjects are deliberately **outside** the `flexcore.v1` namespace (`flexcore.dlq.v1.<area>.<event_slug>` for the quarantined original) so a wildcard on `flexcore.v1.>` can never re-consume a poison message.

## 5. Consumer policy

### 5.1 Durable consumers, explicit acknowledgement, idempotency

- **Durable consumers only** for anything with side effects: durable name convention `<area>-<purpose>-v1` (e.g. `audit-persist-v1`). Ephemeral consumers may be used for debugging only and never for stateful work.
- **Ack mode:** `AckExplicit`; **acknowledge only after the processing transaction commits** (P1.16: check inbox → process → record inbox + state change in one PostgreSQL transaction → ack). Crash between commit and ack is safe: the redelivery is detected by the inbox and acknowledged without re-executing side effects (P1.16 tests).
- **Redelivery:** an unacked message is redelivered after `AckWait` expires; `AckWait`, `MaxAckPending`, and `MaxDeliver` are deployment configuration (dev-only values in Compose, P1.25) — no universal values are fixed here, for the same reason as D-J4.
- **Idempotent consumption:** every consumer maintains PostgreSQL inbox records keyed by the message idempotency key with a unique constraint (P1.16 owns that migration, alongside the P1.12 schema). The broker duplicate window (§2) is a courtesy, not the guarantee.
- Pull consumers are the Phase 1 default (the worker polls at its own pace, P1.19); push consumers are not excluded but are not required by this design.

### 5.2 Dead-letter / quarantine handling (max_deliver path)

1. A consumer redelivers up to its configured `max_deliver` times.
2. On the final failed attempt the consumer publishes the original message plus failure metadata (consumer name, attempt count, last error, timestamps) to `flexcore.dlq.v1.<area>.<event_slug>` — captured by `FLEXCORE_DLQ` — and then terminates the original delivery (`ack_term`).
3. An alert is emitted (log + metric, P1.20) on every DLQ write; **a poison message is never silently dropped** (ledger P1.16 step 4).
4. JetStream `MAX_DELIVERIES` advisories (`$JS.EVENT.ADVISORY.CONSUMER.MAX_DELIVERIES.>`) are monitored as the secondary signal that a message exhausted its budget.

## 6. Ordering

- **No global ordered queue is promised.** §15.4 explicitly orders the design to preserve *useful* ordering for an asset/command stream while avoiding one global ordered queue; cross-subject and cross-stream ordering is not guaranteed and no consumer may assume it.
- JetStream preserves publish order **per subject within a stream**; the scheme in §3 gives one subject per event type, so per-subject ordering is a weak property and the design relies on it only for same-subject sequences.
- **Per-asset / per-command ordering is a consumption rule, not a subject rule** (consequence of D-J1/D-J2): consumers group events by `asset_id` / command id carried in the payload (P1.09) and order within each group by source sequence and `event_time` (P0.09 §1), tolerating duplicates and out-of-order arrival per §6.3. This is what preserves "useful ordering for an asset/command stream" without embedding identifiers in subjects.
- Telemetry (Phase 2) inherits the same tolerance rule (§6.3 closing paragraph: tolerate duplicates and out-of-order messages using source sequence, event time, ingestion time, and idempotency keys).

## 7. Delivery rule and durability posture (D-J6)

- **At-least-once delivery.** JetStream redelivers after missed acknowledgements (§15.4, [R6]); consumers must be idempotent (§6.3 step 4; §6.4 guardrail: duplicate side effects are prevented by application logic — the inbox, not the broker).
- **Exactly-once is not promised** — not from acknowledgements, not from `Nats-Msg-Id` deduplication, not from any combination. No document, log line, or README produced from this design may claim exactly-once or zero-loss.
- **Jepsen finding, cited as the reason (RESEARCH.md §5):** the Jepsen analysis of NATS JetStream 2.12.1 found lost writes when data files were truncated or corrupted on a minority of nodes, and flagged the default fsync policy (flush every two minutes rather than before acknowledgement) as a risk; NATS has since documented that default. The Phase 1 pin is NATS 2.15.x (2.15.2 at the cut-off, which includes the CVE-2026-33222/33223 fixes — fixed in 2.11.15/2.12.6 per `RESEARCH.md` §5); the Jepsen finding is cited to keep the *claims* modest, not as a statement about a currently pinned version.
- **fsync policy is a deployment decision**, documented per deployment, never assumed and never claimed as a guarantee here.
- **The durable guarantee is the §6.3 four-step outbox discipline**: state + outbox row in one transaction → publish → mark delivered after ack → idempotent consumers. The broker is transport (§16.1 row 5).

## 8. Security boundary (§17.2)

- NATS is a **central application zone** component (§17.2 zone 2: control plane, database, event broker, plugin host, compute workers). Only documented flows between zones are permitted.
- **No direct NATS access from a field-device network** (§17.2 closing sentence): zones 3–5 (DMZ, OT/site control, field/device) have no NATS reachability. Edge integration is via the edge agent (Phase 6) through the DMZ with outbound-only connections; Phase 1 has no edge component at all.
- Phase 1 consumers and publishers run inside the central application zone (control plane, worker); there is no external subscriber surface.
- Credentials for NATS come from the secrets path (§17.3: secrets manager integration, no secrets in source control; P1.21); mTLS for service identities is a §17.3 control recorded as a deployment requirement — this design makes no deployment-hardening or certification claim.

## 9. Note for the flexcore-events implementer (P1.09)

The default subject scheme `flexcore.v1.<area>.<event-name>` currently implemented in flexcore-events as an **ASSUMPTION is CONFIRMED by this design**, with two bindings that must hold in code:

1. Area tokens are exactly the §3 set: `registry`, `telemetry`, `state`, `flexibility`, `forecast`, `market`, `scheduling`, `validation`, `dispatch`, `plugins`, `audit`.
2. The event slug is the lower_snake_case of the exact §15.3 event name (e.g. `dispatch_fallback_activated`, `operator_action_recorded`).

Any code divergence from these two bindings must be reconciled before P1.15 wires the publisher, because the outbox `destination_subject` column (P1.12 §4.6) and the stream subject filters (§4.2) both consume this scheme verbatim.

## 10. Acceptance-criteria coverage (ledger P1.14)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Every §15.3 event maps to exactly one subject; the mapping table is complete (26 events) | §3 (26-row table, mechanical scheme D-J1, count statement) |
| Each stream has retention, replication, ack, replay, and DLQ policy specified | §4.1 (per-stream policy shape), §4.2 (inventory), §5.2 (DLQ path) |
| The at-least-once rule and the no-global-ordering rule are stated | §7 (at-least-once, exactly-once disclaimed, Jepsen citation), §6 (no global ordered queue; per-asset ordering rule) |
| No subject scheme leaks tenant enumeration without a security review note | §3 + D-J2 (no tenant identifier in subjects; decision and rationale recorded) |

Verification method (ledger): design review against §15.4 and P0.14; mapping-table count check — §3 rows are countable (26) and P1.15/P1.16 tests exercise the consumer/DLQ policies specified in §5.
