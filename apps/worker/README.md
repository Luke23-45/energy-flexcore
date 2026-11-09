# flexcore-worker (P1.19)

Background worker for Neryva FlexCore: hosts the transactional-outbox
publisher (P1.15) and the idempotent JetStream consumers (P1.16) as
long-lived background tasks. Simulation-only infrastructure — the entire
0.x line grants no control authority and touches no field device.

## Why a separate process (report §8.3)

§8.3 extracts a service only when its workload requires independent
scaling or lifecycle. The outbox publisher and the event consumers are
long-lived background work (poll loops, broker sessions, durable
consumers) that is distinct from request handling: the control plane
answers requests with bounded latency, the worker drains queues with
unbounded backoff and retry budgets. Hosting them in the API process would
couple API restarts to outbox-drain progress and vice versa, so a separate
composition root is warranted now. The worker hosts **only** background
work — no business routes, no second API server.

## What it runs

| Component | Source | Behaviour |
|---|---|---|
| Outbox publisher loop | `flexcore-nats-adapter` (`OutboxPublisher`) | Polls pending outbox entries through the PostgreSQL adapter's `EventOutbox` port, publishes to JetStream with `Nats-Msg-Id` = idempotency key, marks delivered **only after the broker ack**; on failure the entry stays pending with an attempt count and exponential backoff, exhausting into `Failed` (never dropped). |
| Idempotent consumer loop × `FLEXCORE_CONCURRENCY` | `flexcore-nats-adapter` (`IdempotentConsumer`) | Durable pull consumer `registry-worker-v1` on `FLEXCORE_REGISTRY`; check inbox → skip; process → record inbox + side effect in one transaction; ack only after commit; poison path → `FLEXCORE_DLQ` + `+TERM` + ERROR alert, never a silent drop. |
| Probe server | this crate (`probes`) | `/health` (liveness) and `/ready` (readiness) on `FLEXCORE_PROBE_ADDR`. |

Phase 1 scope note: no business consumer exists yet, so the consumer's
side effect IS the durable inbox record (`runner::inbox_recording_handler`);
the full P1.16 flow — including dedup, ack-after-commit and the poison
path — runs unchanged for real consumers later.

## Single-home decision (durable inbox store)

The `InboxStore` trait lives once in `crates/adapters/nats/src/inbox.rs`
(P1.16 decision 2). The durable PostgreSQL implementation lives in this
crate (`src/inbox.rs`, `PgInboxStore`) because the P1.19 edit scope is
`apps/worker/**` and the postgres adapter's migration set is outside it.
The inbox table `flexcore_worker_inbox` is created idempotently at startup
(`ensure_table`) with the P1.16 schema obligations: per-consumer key,
UNIQUE constraint `(consumer_name, idempotency_key)`, consumed-at
timestamp, consumer identity. Promotion into `flexcore-postgres-adapter`
(with the DDL moving into its migration set) must delete `src/inbox.rs` —
one home, one definition.

## Configuration (`FLEXCORE_*`, fail-fast, no default credentials)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `FLEXCORE_DATABASE_URL` | yes | — | PostgreSQL URL (runtime role `flexcore_app` per P1.12 §3.1). Secret: never logged. |
| `FLEXCORE_NATS_URL` | yes | — | NATS server URL. Never logged. |
| `FLEXCORE_LOG_LEVEL` | no | `info` | `trace`/`debug`/`info`/`warn`/`error` (parsed, not free-form filter syntax). |
| `FLEXCORE_OUTBOX_POLL_INTERVAL_MS` | no | `200` | Publisher idle poll interval. |
| `FLEXCORE_CONCURRENCY` | no | `1` | Number of consumer loop tasks sharing the durable consumer (1..=64); the inbox makes parallel delivery safe. |
| `FLEXCORE_PROBE_ADDR` | no | `127.0.0.1:8081` | Probe server bind address. |
| `FLEXCORE_SHUTDOWN_TIMEOUT_MS` | no | `10000` | Bounded drain timeout for graceful shutdown. |

Invalid or missing required values abort startup with a named error
before any connection is opened. Error messages carry the variable name
and reason, never the value.

## Probes: liveness vs readiness

- `GET /health` — **liveness**: always 200 while the process runs, even
  when PostgreSQL/NATS are unreachable (restarting the worker cannot fix a
  stopped broker).
- `GET /ready` — **readiness**: 503 when PostgreSQL is unreachable
  (one-round-trip probe query), when the NATS client is not
  `Connected`, or while the worker is draining for shutdown.

## Graceful shutdown sequence (SIGTERM/SIGINT)

1. **Stop accepting new work** — the shutdown watch fires; readiness goes
   503; the probe server drains open connections.
2. **Finish or cancel in-flight work within `FLEXCORE_SHUTDOWN_TIMEOUT_MS`**
   — the publisher finishes its drain cycle (delivered is marked only
   after the broker ack), each consumer finishes its single in-flight
   message (ack only after the commit), then loops exit at the next
   shutdown check. On timeout the remaining tasks are aborted — safe by
   design: a pending outbox entry is re-published later (deduplicated by
   key), an unacknowledged message is redelivered (deduplicated by the
   inbox). At-least-once, never exactly-once.
3. **Close connections** — NATS connection closed, PostgreSQL pool closed
   after all tasks have exited.
4. **Exit** — 0 on clean drain, 1 if the bounded timeout cancelled work.

## Running

```sh
export FLEXCORE_DATABASE_URL=postgres://flexcore_app:...@localhost:5432/flexcore
export FLEXCORE_NATS_URL=nats://localhost:4222
cargo run -p flexcore-worker
```

The database migrations are applied by the operator/bootstrap path (the
worker connects as `flexcore_app`, not the owner role — P1.13 role model);
the worker only creates its own inbox table.

## Integration test

`tests/integration.rs` runs end-to-end (outbox entry via the store →
publisher drain with ack-before-mark → JetStream delivery → consumer
process → duplicate redelivery skipped by the inbox, one side effect) and
is gated on both `FLEXCORE_TEST_DATABASE_URL` and `FLEXCORE_TEST_NATS_URL`;
it skips cleanly when either is unset. It requires a multi-thread tokio
runtime (the loops drive the sync ports under `block_in_place` — see
`src/runner.rs`).
