# P0.14 — Event vocabulary freeze (facts, not commands)

| Field | Value |
|---|---|
| Subtask | P0.14 — Freeze the event vocabulary (facts, not commands) |
| Report trace | §15.3 (event vocabulary), §15.4 (NATS and delivery semantics), §6.3 (transactional outbox), §6.4 (Outbox / Inbox pattern row), §15.5 (transactional outbox summary) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.08 (telemetry quality), P0.11 (command state machine) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. Initial domain events (§15.3 — all 26, spelled exactly as in the report)

The §15.3 list is reproduced verbatim, in report order, and verified name by name against §15.3 (not taken from the ledger list):

| # | Event | Domain area |
|---|---|---|
| 1 | `AssetRegistered` | Asset registry |
| 2 | `AssetCapabilityUpdated` | Asset registry |
| 3 | `TelemetryReceived` | Telemetry |
| 4 | `TelemetryQualityChanged` | Telemetry |
| 5 | `AssetAvailabilityChanged` | Asset state |
| 6 | `FlexibilityEnvelopeUpdated` | Flexibility |
| 7 | `ForecastPublished` | Forecasting |
| 8 | `MarketSignalReceived` | Market |
| 9 | `ScheduleProposed` | Scheduling |
| 10 | `ScheduleValidated` | Scheduling |
| 11 | `FeasibilityCheckCompleted` | Grid validation |
| 12 | `DispatchProposed` | Dispatch lifecycle |
| 13 | `DispatchApproved` | Dispatch lifecycle |
| 14 | `DispatchRejected` | Dispatch lifecycle |
| 15 | `DispatchIssued` | Dispatch lifecycle |
| 16 | `DispatchAcknowledged` | Dispatch lifecycle |
| 17 | `DispatchTrackingUpdated` | Dispatch lifecycle |
| 18 | `DispatchCompleted` | Dispatch lifecycle |
| 19 | `DispatchFailed` | Dispatch lifecycle |
| 20 | `DispatchExpired` | Dispatch lifecycle |
| 21 | `DispatchCancelled` | Dispatch lifecycle |
| 22 | `DispatchSuperseded` | Dispatch lifecycle |
| 23 | `DispatchFallbackActivated` | Dispatch lifecycle |
| 24 | `PluginActivated` | Plugins |
| 25 | `PluginFailed` | Plugins |
| 26 | `OperatorActionRecorded` | Audit |

Count: 26 events. Every dispatch-lifecycle event name corresponds to a frozen command state (`11_command_state_machine.md` §3) with the `Dispatch` prefix; `TelemetryQualityChanged` carries the six-state quality vocabulary from P0.08 (§10.4). Note that §15.3 defines events for the dispatch states listed and no others — there is no `DispatchValidated` or `DispatchFeasible` event in §15.3, and none may be invented in Phase 0 without a report change.

## 2. Events are facts, commands are imperatives

Events should describe facts that occurred, not imperative commands. Commands and events are separate types (§15.3, closing sentence).

Consequences frozen here:

- An event named `DispatchIssued` records that a command reached the ISSUED state (§14.2); it is not itself the command and must never carry imperative semantics.
- No event name from §1 may also be used as a command name anywhere in Phase 0 (ledger P0.14 acceptance criterion). Commands follow the §14 lifecycle and invariants; events are the audit/notification trail of what happened.
- Treating an event as a command risks double-execution; the inbox/idempotency rules in §4 exist precisely because delivery can repeat facts, never to re-issue imperatives (ledger P0.14 "Risks / failure modes").

## 3. Delivery rule (§15.4, §6.3 step 4)

| Transport | Use | Semantics |
|---|---|---|
| NATS Core | Ephemeral notifications where loss is acceptable (§15.4) | No persistence/replay guarantee |
| NATS JetStream | Durable work/events that need persistence and replay (§15.4) | At-least-once delivery, with redelivery after missed acknowledgements (§15.4) |

Because JetStream is at-least-once, **consumers must be idempotent; do not promise exactly-once side effects solely because the broker offers acknowledgements or deduplication options** (§15.4, [R6]; §6.3 step 4: durable messaging is normally at-least-once, "not a magical exactly-once execution guarantee").

Ordering and stream policy (§15.4, second paragraph): the partition/subject strategy should preserve useful ordering for an asset/command stream while avoiding one global ordered queue; retention, replication, acknowledgement, replay, dead-letter/quarantine, and recovery policy are defined per stream.

## 4. Transactional outbox (§6.3 four-step sequence; §15.5)

For an operation that must both update state and publish an event, use a transactional outbox (§6.3). The four-step sequence, reproduced as written in §6.3:

1. write the state change and an outbox record in one PostgreSQL transaction;
2. publish the outbox record to NATS;
3. mark it delivered after broker acknowledgement; and
4. make consumers idempotent because durable messaging is normally at-least-once, not a magical exactly-once execution guarantee. [R6]

§15.5 restates the same rule at the component level: a PostgreSQL transaction writes both authoritative state and an outbox record; a publisher reads unsent outbox entries, publishes them to JetStream, and marks them delivered; consumers maintain idempotency keys/inbox records for operations that must not be duplicated. The Outbox / Inbox pattern row (§6.4) adds the guardrail: at-least-once delivery is assumed; duplicate side effects are prevented by application logic.

Telemetry corollary (§6.3, closing paragraph): tolerate duplicates and out-of-order messages using source sequence, event time, ingestion time, and idempotency keys.

## 5. What this document does not do

- It defines no subject naming, no stream configuration, and no schema: §15.4 leaves the partition/subject strategy and per-stream policy to later design, and §10.5 governs Protobuf/schema governance.
- It publishes nothing: the entire 0.x line is simulation-only (§0, §2.1), and Phase 0 produces no code.
- It makes no delivery guarantee claim beyond the cited semantics: exactly-once is explicitly not promised by broker acknowledgements or deduplication alone (§15.4).

## 6. Acceptance-criteria coverage (ledger P0.14)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All 26 §15.3 events listed, spelled exactly as in the report | §1 (verified name-by-name against §15.3, report lines 1511–1542) |
| Events-are-facts rule stated | §2 |
| At-least-once/idempotency rule stated | §3 |
| Transactional-outbox four-step sequence from §6.3 reproduced | §4 |
| No event name also used as a command name in Phase 0 | §2 (rule stated; no Phase 0 artefact names any event as a command) |

Verification method (ledger): comparison against §15.3 and count check (26 events) — performed in §1.
