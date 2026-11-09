# flexcore-nats-adapter (P1.15 / P1.16)

NATS JetStream adapter for Neryva FlexCore: stream/consumer provisioning
(P1.14 topology), the transactional-outbox publisher (report §6.3 steps 2–3),
and the idempotent consumer helper with the inbox pattern (report §6.3 step 4,
§6.4). Ledger subtasks P1.15 and P1.16.

## Delivery rule — at-least-once, never exactly-once (P1.14 D-J6)

Everything in this crate implements **at-least-once delivery**. JetStream
redelivers unacknowledged messages; there is no exactly-once guarantee from
broker acknowledgements, from `Nats-Msg-Id` deduplication, or from any
combination of them. The durable guarantees are:

1. the §6.3 four-step outbox discipline — state change + outbox row in one
   storage transaction (the postgres adapter, P1.13) → publish → mark
   delivered **only after the broker publish ack** → idempotent consumers;
2. the consumer **inbox** — a per-consumer record keyed by the idempotency
   key with a unique constraint (P1.16).

The broker is transport (report §16.1 row 5), never the authoritative record.

## Known limit: the duplicate window

`Nats-Msg-Id`-based broker deduplication applies **only within the configured
duplicate window** (deployment configuration, `DeploymentLimits::duplicate_window`).
It is a courtesy that collapses re-publications after a crash between publish
and mark; it is **not** the guarantee. If a re-publication lands after the
window expired, the message is delivered twice and the consumer inbox — not
the broker — prevents a duplicate side effect. No code path here relies on the
window for correctness.

Also recorded (P1.15 step 5): publisher retry backoff is per drain cycle; the
`PendingOutboxEntry` port view carries no `updated_at`, so per-entry backoff
scheduling is not expressible through the port and was not improvised.

## What is here

| Module | Subtask | Content |
|---|---|---|
| `topology` | P1.15 | P1.14 §3/§4 stream definitions, subject→stream mapping, Phase 1 provisioning (`FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ`, D-J7), durable pull consumers with `AckExplicit` |
| `publisher` | P1.15 | `OutboxPublisher` — drains the outbox via the sync `EventOutbox` port, publishes with `Nats-Msg-Id` = idempotency key, marks delivered only after ack, backoff, `Failed` after the attempt budget (never deleted, never silently dropped) |
| `inbox` | P1.16 | The `InboxStore` consumer-side trait (single home, see below) + in-memory test double |
| `consumer` | P1.16 | `IdempotentConsumer` — check inbox → skip; process → handler commits inbox + state change in one transaction; ack strictly after commit; redelivery → acked without re-execution; after `max_deliver` → `FLEXCORE_DLQ` + ERROR log + counter, never a silent drop |

## Decisions recorded here

1. **`InboxStore` single home: this crate.** The inbox is consumer-side
   delivery-deduplication infrastructure, not a domain capability port, and
   flexcore-ports was outside the P1.16 edit scope. The durable
   implementation (PostgreSQL table with the unique constraint on
   `(consumer_name, idempotency_key)`, consumed-at timestamp, consumer
   identity — P1.16 step 1) lives in `crates/adapters/postgres`, which owns
   that schema, and implements this trait verbatim. If a later phase
   promotes the trait to flexcore-ports, the definition moves there and is
   deleted here — one home, one definition.

2. **Sync ports / async adapter.** `EventOutbox` and `InboxStore` are sync
   (house rule: no port names a runtime). The adapter *consumes* them — it
   implements no port — and calls them as short database round-trips between
   async broker operations. The consumer's handler is likewise a sync
   `FnMut`; an async handler bridges internally. If a composition root's
   store blocks long enough to matter, it hosts the loop on a blocking
   thread with a channel bridge; this crate does not require it.

3. **The one-transaction obligation is by convention.** "Record inbox +
   state change in ONE transaction" cannot be expressed in a sync trait
   method: the handler owns its storage transaction, calls
   `InboxStore::record` inside it, and rolls back on
   `RecordOutcome::AlreadyRecorded`; the helper acknowledges only after the
   handler returns (i.e. after the commit). The helper deliberately does
   **not** re-record on the handler's behalf — an inbox row written outside
   the handler's transaction would weaken the rule. The helper passes the
   inbox handle to the handler to make the joining possible.

4. **No `futures-util`/`tokio-stream` (dependency ban).** The streaming
   consumer APIs need a `StreamExt` import, so pulls use the documented
   JetStream pull protocol over the core request surface
   (`$JS.API.CONSUMER.MSG.NEXT.<stream>.<consumer>`, batch 1 with
   `expires`), acks are published to the message's reply subject
   (`+ACK` / `+TERM` / `-NAK {"delay":…}`), and delivery counts are parsed
   from the `$JS.ACK.…` reply-subject token layout. Every protocol constant
   carries a `VERIFY-BUILD:` comment; re-verify them mechanically at the
   first `cargo build`.

5. **Subject scheme verification result (P1.14 §9 binding).** The
   flexcore-events scheme *shape* matches D-J1
   (`flexcore.v1.<area>.<slug>`, lower snake_case slugs, no tenant token,
   wildcards rejected) but the **area tokens diverge on 8 of 26 events**:
   flexcore-events uses `asset` (for both `AssetRegistered`/
   `AssetCapabilityUpdated` and `AssetAvailabilityChanged`), `schedule`,
   `feasibility`, `plugin`, and `operator`, where P1.14 §3 defines
   `registry`, `state`, `scheduling`, `validation`, `plugins`, and `audit`.
   Consequence: subjects for events 1–2 would land on no Phase 1 stream
   (`asset.*` matches no filter), and `operator_action_recorded` likewise
   misses `FLEXCORE_AUDIT`. The adapter is scheme-agnostic — the publisher
   publishes the outbox `destination_subject` verbatim and the consumer
   derives DLQ subjects mechanically — so the reconciliation belongs in
   flexcore-events (out of this subtask's edit scope). **This must be
   reconciled before any real event flows.**

6. **Hosting.** `OutboxPublisher::run` and `IdempotentConsumer::run` are
   loops driven by a `tokio::sync::watch` shutdown receiver. apps/worker
   (P1.19) provisions streams/consumers at start and hosts both as
   background tasks; this crate ships the types, the worker ships the
   runtime wiring.

## Running the integration tests

`tests/integration.rs` needs a real NATS server (2.15.x) and skip cleanly
without it. No database is needed here (the durable inbox tests live with
the postgres adapter — decision 1).

```sh
# start a NATS server (e.g. docker run -p 4222:4222 nats:2.15 -js)
export FLEXCORE_TEST_NATS_URL=nats://127.0.0.1:4222
cargo test -p flexcore-nats-adapter
```

Without `FLEXCORE_TEST_NATS_URL` every integration test returns immediately
("skipping: FLEXCORE_TEST_NATS_URL is not set").

Test isolation: subjects, durable names and idempotency keys carry a per-run
suffix, so re-runs against a persistent dev server are independent.

Recorded limit: the "broker restart" evidence uses a fresh publisher against
the same broker plus injected crash points (kill between publish and mark;
kill mid-batch). Killing the server process itself is the P1.25 Compose
stack's server-kill test; the adapter-side invariants asserted here (entries
stay pending, no duplicate side effect, dedup on republish) are exactly the
ones a server restart exercises.

## Scope and control status

Simulation-only infrastructure (AGENTS.md §5). Nothing here touches a field
device and nothing here authorises field control; FlexCore is a supervisory
orchestration layer performing no primary frequency response, synthetic
inertia, relay protection, inverter inner-loop control, or sub-cycle control.
