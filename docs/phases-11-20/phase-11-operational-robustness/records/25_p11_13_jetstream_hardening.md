# P11.13 — JetStream hardening package

| Field | Value |
|---|---|
| Subtask | P11.13 (MaxDeliver alignment, dedicated DLQ streams, DLQ-depth alerts, bounded retention, schema_version headers) [S-65] |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the topology, consumer, publisher, and worker changes are written; nothing has been compiled, provisioned against a broker, or evaluated by Prometheus (the first human build and the live-broker check are the gates). |
| Sources | [S-65] (NATS JetStream docs — the delivery/advisory/DLQ concepts; no performance or version claim beyond the crate's existing 0.50.0 VERIFY-BUILD notes); the gap analysis §5 Tier 0 row 5 |
| Files touched | `crates/adapters/nats/src/topology.rs`, `crates/adapters/nats/src/consumer.rs`, `crates/adapters/nats/src/publisher.rs` (shared with A — additive marked block, re-read before edit), `apps/worker/src/main.rs` (shared, marked blocks) |

## 1. What was written (per hardening item)

### 1.1 MaxDeliver limits aligned between helper and provisioned policy

`ConsumerConfig::validate_against_policy(&ConsumerPolicy)` (consumer.rs)
+ the named `MaxDeliverMismatch` error. `ConsumerConfig::from_policy` is
aligned by construction; the composition root now builds BOTH consumer
configs via `from_policy` and fails fast on any drift (worker `main.rs`
marked block — abort with the named error, never a silent correction).
The advisory stream (`$JS.EVENT.ADVISORY.CONSUMER.MAX_DELIVERIES.>`)
remains the secondary signal exactly as the P1.16 header documents.

### 1.2 Dedicated DLQ streams per subject family

`DLQ_STREAMS` (topology.rs): eleven dedicated streams
`FLEXCORE_DLQ_<AREA>`, one per P1.14 §3 area, with filters
`flexcore.dlq.v1.<area.>`. The retained catch-all `FLEXCORE_DLQ` is
narrowed to the unrouted quarantine subject only
(`flexcore.dlq.v1.unrouted`) so family filters and the catch-all never
overlap. `stream_for_subject` routes conforming quarantine copies to
their family stream and falls back to the catch-all for unknown area
tokens (never `None` — a quarantine copy is never unrouted silently).
`provision_dlq_streams` provisions the family streams idempotently
(worker `main.rs` marked block, fail-fast like the rest of
provisioning). The consumer's quarantine doc-comments were updated to
the new routing (the DLQ copy subjects themselves are UNCHANGED —
`flexcore.dlq.v1.<area>.<event_slug>` — so no message compatibility
changes).

### 1.3 DLQ-depth alerts

`deploy/compose/observability/prometheus/rules/dlq_depth_alerts.yml`:
page on any nonzero poison depth per family stream, ticket above the
registered review depth (100), page on any unrouted quarantine copy
(a routing/producer defect). HONEST CAVEAT recorded on the file and in
record 21: the exporter metric family (`jetstream_stream_messages`) is
an ASSUMPTION — no NATS Prometheus exporter is deployed in this program
and the exact exported names are UNVERIFIED against a live exporter;
the stream-name label constants are authoritative (from topology.rs).

### 1.4 Bounded retention

`DeploymentLimits::bounded_simulation_defaults()` (duplicate window
120 s, max_age 7 days, max_messages 1,000,000, max_bytes 1 GiB) and
`DeploymentLimits::bounded_dlq_defaults()` (max_age 72 h, max_messages
10,000 per family stream, max_bytes 64 MiB per family stream) —
REGISTERED RECOMMENDATION values (X-8, written down before any run that
could measure against them; pending operator sign-off). The worker's
`deployment_limits()` switches from server-default `None` ceilings to
the bounded defaults (marked block). Production values remain a
per-deployment decision per data class (P1.14 §4.1/D-J4).

### 1.5 schema_version headers on published messages

`topology::SCHEMA_VERSION_HEADER` (`flexcore-schema-version`),
`topology::PUBLISHED_SCHEMA_VERSION` (1), and the single helper
`topology::published_message_headers(u32)`. The outbox publisher
(publisher.rs — additive marked block, after A's landed P11.05
trace-header seam, no overlap) inserts the header on EVERY publish.
The consumer reads it back (`DeliveredMessage::schema_version`,
surfaced verbatim; absence is surfaced, never guessed) and the
quarantine metadata carries it through. Delivery metadata only — the
envelope's own `schema_version` field (P1.09) stays the authority.

Recorded cross-check: the adapter crate's single core-crate edge is
flexcore-ports (its Cargo.toml note), so
`PUBLISHED_SCHEMA_VERSION == flexcore_events::envelope::
SUPPORTED_SCHEMA_VERSION` is asserted in the worker integration test
(`published_schema_version_matches_the_envelope_supported_version` —
UNRUN) instead of compiled.

## 2. P11.05 handoff honored

A's record 05 §1 hands the W3C `traceparent` header read to the P11.12
consumer. Implemented: `DeliveredMessage::traceparent` (opaque,
case-insensitive per NATS header semantics, absent = no inherited
context, never synthesized) and the command consumer's neutralization
log line carries it. No OTel span is created in the worker (A's close-out
deviation 2 records that the worker wires traces only at the publisher;
a consumer-side span is wave-2 work if wanted).

## 3. VERIFY-BUILD notes added (honest, not deferred tasks)

- `published_message_headers`: the async-nats 0.50.0 `HeaderMap::insert`
  name/value conversion shapes (same shapes A's trace-header loop
  relies on).
- The publisher's schema-header loop: same shapes.
All existing VERIFY-BUILD notes in topology.rs/consumer.rs are
unchanged and remain the first-build checklist.

## 4. Handoff notes

1. **Live-broker check**: the provisioning converges (create-or-update)
   and is idempotent by construction, but NO broker has ever run this
   topology. A human `docker compose up` against the worker is the gate
   that verifies the stream/consumer shapes (recorded UNRUN).
2. **Exporter choice** for the DLQ alerts: confirm or mechanically
   rename `jetstream_stream_messages` against the deployed exporter
   (the rule file header records this).
3. **Retention bounds** are recommendations for the simulation
   deployment; production retention is a per-deployment decision (the
   P1.14 rule stands).
