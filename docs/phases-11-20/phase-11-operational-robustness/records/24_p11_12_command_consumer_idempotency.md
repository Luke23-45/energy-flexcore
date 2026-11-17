# P11.12 — Consumer idempotency keyed on command_id

| Field | Value |
|---|---|
| Subtask | P11.12 (consumer idempotency keyed on command_id; duplicate-delivery test suite complete and UNRUN) |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the consumer, oracle, gateway, wiring, and the duplicate-delivery test suite are written; nothing has been compiled or executed (the first human build/test is the gate). |
| Sources | [S-61] (operational practice), [S-65] (JetStream at-least-once delivery — the redelivery model the oracle neutralizes); the frozen 14-state / 27-edge lifecycle (`crates/flexcore-domain/src/command_state.rs`, P0.11); the P1.16 idempotent-consumer flow (`crates/adapters/nats/src/consumer.rs`) |
| Files touched | `apps/worker/src/command_consumer.rs` (new), `apps/worker/src/lib.rs` (shared, additive module line), `apps/worker/src/main.rs` (shared, marked blocks), `apps/worker/Cargo.toml` (additive `flexcore-storage`), `apps/worker/tests/command_duplicate_delivery.rs` (new — the test suite) |

## 1. The state machine is the dedupe oracle (implemented verbatim)

The plan's registered semantics — "an invalid-state transition on a
redelivered command is a no-op plus a duplicate-delivery metric" — are
implemented in `command_consumer.rs` exactly:

1. Parse the delivered payload as a P1.09 `EventEnvelope` (unknown
   schema version fails to parse there — the authority; the
   `flexcore-schema-version` header is delivery metadata only).
2. Map the event to its target state via the CLOSED eleven-variant
   dispatch mapping (`command_state_for_event` — no event is invented
   for VALIDATED or FEASIBLE, per the frozen §15.3 rule).
3. Derive the `TenantContext` from the payload's tenant (D-J2: tenant
   scoping at consumption, never the subject) and read the stored state
   through the `CommandStateGateway` port (the Postgres implementation
   reads through `CommandRecordStore::find_command` inside one unit of
   work; a foreign-tenant or unknown command reads as `None`).
4. **`flexcore_domain::command_state::try_transition(current, target)`
   is the ONLY decision authority** (`decide_transition`):
   - not a frozen edge (self-transition, backwards move, terminal exit,
     FAILED/EXPIRED exiting anywhere but FALLBACK) → **no-op + the
     duplicate-delivery metric** (`flexcore_command_consumer_duplicate_deliveries_total`)
     + an INFO log line carrying the W3C `traceparent` header value
     (A's record 05 handoff, carried opaquely);
   - a frozen edge the stored command has NOT taken → a named handler
     error (counted on
     `flexcore_command_consumer_unsupported_lifecycle_facts_total`) that
     rides the existing P1.16 poison path (retry → DLQ + `+TERM` + ERROR
     log). The consumer NEVER fabricates a journal row: the §14.1
     gate/reason/evidence journal rules require fields the §15.3
     dispatch payloads do not carry, and the producing gate owns the
     journal.

Why this is complete for redeliveries: every dispatch fact is published
via the transactional outbox only AFTER the producing side committed its
journal row, so a redelivery of an already-applied fact always lands on
the no-op arm. The P1.16 inbox check remains the first dedupe layer for
crash-after-commit redeliveries; the oracle is the durable second layer
that does not depend on any inbox row surviving.

## 2. The recorded two-transaction deviation

The P1.16 binding obligation is "handler records the inbox entry inside
its ONE storage transaction". The shipped shape records the inbox entry
AFTER the oracle decision, in its own single-statement transaction (the
`PgInboxStore` pool shape — the recorded single-home limitation). A
crash between the oracle no-op and the inbox record yields a redelivery
that re-runs the oracle and neutralizes again: the invariant "no
duplicate side effect" holds on the state machine alone. Closing the
deviation needs an inbox record ON the command transaction — the
already-registered inbox-promotion work.

## 3. Test suite — COMPLETE AND UNRUN

`apps/worker/tests/command_duplicate_delivery.rs` (integration) plus the
`#[cfg(test)]` module in `command_consumer.rs` (unit). Coverage:

| Test | Asserts |
|---|---|
| `redelivered_command_is_a_no_op_plus_the_duplicate_metric` | the P11.12 acceptance rule: equal stored/fact state → Ok + duplicate metric, no error, no write |
| `backwards_fact_neutralizes_as_a_duplicate` (unit) | a backwards fact is also a no-op + metric |
| `the_oracle_decides_exactly_on_the_frozen_table` (both files) | every decision traces to the frozen table, including FAILED→FALLBACK vs FAILED→EXPIRED |
| `unsupported_lifecycle_fact_is_surfaced_never_written` (both) | a frozen un-taken edge → named error, no inbox record, no journal fabrication |
| `unknown_command_surfaces_instead_of_being_dropped` (both) | unknown command → named error on the poison path, counted |
| `unparsable_payload_counts_as_a_decode_failure` / `keyless_delivery_is_refused_defensively` | the poison and defensive arms |
| `handled_deliveries_record_the_inbox_entry` / `the_first_dedupe_layer_skips_a_row_hit_before_the_oracle` | the P1.16 first-layer composition |
| `event_to_state_mapping_covers_exactly_the_dispatch_vocabulary` / `no_event_is_invented_for_the_gate_states` | the closed mapping; VALIDATED/FEASIBLE never mapped |
| `metrics_render_all_five_families_at_zero` / `duplicate_delivery_metrics_render_all_families` | always-render discipline |
| `dispatch_consumer_policy_and_helper_config_stay_aligned` | the P11.13 MaxDeliver alignment for the dispatch consumer |
| `published_schema_version_matches_the_envelope_supported_version` | the P11.13 recorded cross-check (adapter crate cannot compile-time link flexcore-events) |
| `traceparent_is_carried_opaquely_when_present_and_absent_when_not` | the P11.05 handoff semantics |

NO test in this suite has been compiled or executed. Execution is the
human gate (program conventions §1.3).

## 4. Domain purity and wiring

- `flexcore-domain` is only CALLED (`try_transition`, `CommandState`,
  `TenantContext`) — never modified; the metric counters live in the
  consumer layer, never the domain (the P11.12 constraint).
- Dependency-rule note: `apps/worker/Cargo.toml` gains
  `flexcore-storage = { workspace = true }` (the command-record port the
  gateway reads through) — an app/composition-root edge, catalogued in
  the workspace; no new version pin. The `check_dependency_rule.py`
  output is quoted in the close-out record.
- `main.rs` (shared, marked blocks) provisions the `FLEXCORE_DISPATCH`
  stream + `dispatch-command-track-v1` durable pull consumer, validates
  the P11.13 MaxDeliver alignment (fail-fast on mismatch), and spawns
  the `run_command_consumer` loop on the shutdown watch. The stream is
  provisioned at worker start — a RECORDED DEVIATION from the D-J7
  "created by the phase that first publishes" rule (a pull consumer
  requires its stream; no dispatch publisher exists in 0.x). The
  consumer idles at zero counters in a shipped deployment; no behavior
  beyond the written code is claimed.

## 5. Handoff notes

1. **Worker metrics surface (wave 2 / P11.21)**: the worker has no
   `/metrics` endpoint (A's record 01 §5 note 2). The
   `CommandConsumerMetrics::render()` function produces the
   Prometheus-text body; exposing it needs the probe-server extension
   (wave 2 Agent B's file). Until then the counters are log-visible
   only — recorded honestly, not silently promised.
2. **Family names outside the P11.01 taxonomy**: the five
   `flexcore_command_consumer_*` families are worker-side and NOT in
   A's taxonomy record (the taxonomy rule: a needed family amends the
   record first — an A-owned file). Handoff to A / wave 2 to amend
   record 01 when the worker metrics surface lands.
3. **Journal-writing dispatch facts**: if a later phase wants the
   consumer to APPLY unapplied lifecycle facts (not just dedupe), the
   §14.1 journal fields (gate/reason/evidence) must travel in the
   payload or be resolvable from the store — a recorded design
   prerequisite, not built here.
