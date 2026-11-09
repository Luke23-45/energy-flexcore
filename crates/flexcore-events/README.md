# flexcore-events (P1.09)

The §15.3 event vocabulary as typed Rust payloads, the versioned event
envelope, and the transactional-outbox record type (P0.14 inputs; report
§15.3, §15.4, §15.5, §6.3).

## Events are facts, never imperative commands

Events **describe facts that occurred**; commands are imperatives and are a
separate type family (report §15.3; P0.14 §2). `DispatchIssued` records that
a command reached the ISSUED state of the frozen lifecycle (P0.11) — it is
not itself the command, and no event name may be reused as a command name.
Treating an event as a command risks double-execution.

## Delivery semantics

- Delivery is **at-least-once** (NATS JetStream; report §15.4): consumers
  **must be idempotent**. Exactly-once side effects are never promised by
  broker acknowledgements or deduplication options alone; duplicate side
  effects are prevented by application logic (inbox records /
  idempotency keys).
- The **idempotency key is mandatory** on every envelope — construction and
  parsing both fail without a valid key. It is unique in the outbox store
  (P1.12 unique constraint) and will travel as `Nats-Msg-Id` in the NATS
  adapter (P1.15); broker-side deduplication covers only the duplicate
  window, so the application-level key remains the durable guarantee.
- The **transactional outbox** (report §6.3, four steps): (1) write the
  state change and an outbox record in one PostgreSQL transaction; (2)
  publish the outbox record to NATS; (3) mark it delivered only after broker
  acknowledgement; (4) make consumers idempotent. An outbox record marked
  `Failed` is never silently dropped.

## Versioned serialization rule

Every envelope carries a **schema version**. A breaking change requires a
new schema version — **never a silent field repurpose**. An unknown version
fails to parse; there is no silent default and no fallback reading. The
payload field selections (which tenant/portfolio/site/asset/command/plugin
IDs each event carries) are recorded as ASSUMPTIONS in the module headers of
`src/events.rs`; adjusting them is a schema-version change with a migration
note.

## P2.16 recorded decisions (telemetry event publication)

- **`TelemetryReceived` granularity — per record, DECIDED ONCE (P2.16 step
  1).** The frozen P1.09 `TelemetryReceivedPayload` carries a single
  (tenant, site, asset, event_time) — a batch-level event cannot name
  multiple assets without changing the frozen payload shape. Phase 2
  telemetry arrives at the scenario cadence (bounded per-asset emission,
  P2.13/P2.14), so the per-record volume risk the ledger names
  ("event volume unbounded for high-frequency telemetry") does not arise in
  Phase 2; batch aggregation would change the frozen payload shape and buy
  nothing here. Each emission carries a deterministic idempotency key
  (event + asset + event_time + source_sequence + quantity), so a
  redelivered batch cannot duplicate events (the outbox's key uniqueness
  deduplicates). If a future phase introduces batch emission, that is a
  schema-version change per the rule below — never a silent reshaping.
- **`TelemetryQualityChanged` payload revision (P2.16 step 2).** The
  payload now carries BOTH the previous and the new quality state: the
  frozen `quality` field keeps its name and meaning (the NEW state — no
  field repurpose), and the additive `previous_quality: Option<...>` field
  was added with a `serde(default)` so v1 envelopes produced before the
  revision still parse. This is a backward-compatible addition, NOT a
  breaking change: the envelope schema version stays 1. Emitters always set
  `previous_quality` to `Some` (the event only fires on an actual change).
- **`AssetAvailabilityChanged` payload shape (recorded assumption).** The
  frozen P1.09 payload carries the tenant/site/asset scope and
  `event_time` but no availability value; it is emitted when the
  availability rule (implemented as the pure
  `flexcore_application::availability` rule) computes a CHANGED
  availability, and consumers re-derive the concrete availability from the
  asset's `availability` telemetry. Enriching the payload would be another
  schema-version-reviewed change; recorded here for review.

## Subject naming (frozen per P1.14)

The subject scheme is `flexcore.v1.<area>.<event_slug>` (for example
`flexcore.v1.dispatch.dispatch_issued`), fixed by the frozen P1.14
JetStream topology design
(`ledger/phase-01-rust-foundation/14_jetstream_topology.md`, decisions
D-J1..D-J3; reconciled 2026-11-07, coordinator consistency pass). P0.14 defines no subject naming itself, so
the P1.14 §3 table is the authority: the `<area>` tokens are the eleven
stream/area names `registry | state | telemetry | flexibility | forecast |
market | scheduling | validation | dispatch | plugins | audit` (one
bounded-context stream per area, D-J3), and `<event_slug>` is the lower
snake_case of the frozen §15.3 event name (D-J1). The 26 events map
one-to-one onto 26 distinct subjects — note the areas are the P1.14
taxonomy, not the P0.14 domain-area labels (e.g. `AssetRegistered` →
`registry`, `AssetAvailabilityChanged` → `state`, `ScheduleProposed` →
`scheduling`, `FeasibilityCheckCompleted` → `validation`, `PluginActivated`
→ `plugins`, `OperatorActionRecorded` → `audit`). The scheme carries no
tenant token (no tenant-enumeration leak; frozen decision D-J2).

## Dependency boundary

This crate depends on `flexcore-contracts` and `flexcore-domain` (for the
typed identifiers) plus `serde`/`time` — no NATS, no PostgreSQL, no tokio
(`tools/check_dependency_rule.py` enforces this). `serde_json` is a
dev-dependency: the outbox record stores the payload already serialized by
the caller; the broker adapter (P1.15) and the outbox store (P1.13) own the
transport and persistence.

## Scope and control status

Event types for the design baseline only. The entire 0.x line is
simulation-only; publishing events authorises no field control and no device
writes. FlexCore is a supervisory orchestration layer and performs no
primary frequency response, synthetic inertia, relay protection, inverter
inner-loop control, or sub-cycle control.
