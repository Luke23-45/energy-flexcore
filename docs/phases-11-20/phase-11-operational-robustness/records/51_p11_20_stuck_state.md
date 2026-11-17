# P11.20 — Stuck-state detection (`flexcore_commands_stuck`)

| Field | Value |
|---|---|
| Subtask | P11.20 (stuck-state detection — gauge + alarm feeding the P11.03 rules) |
| Wave / agent | Phase 11 wave 2, Agent B |
| Status | WRITTEN-UNRUN: the gauge, the scanner, and the tests are written code that has never been compiled or executed. The metric renders in the `/metrics` text format by construction of the render function — that is written code, never observed output. |
| Sources | The frozen 14-state / 27-edge command lifecycle (`crates/flexcore-domain/src/command_state.rs` — the terminal/non-terminal vocabulary, reused, not redefined); the P11.01 taxonomy record (`records/01_p11_01_metric_taxonomy.md` §2 family 5 — the naming authority); [S-61] (Google SRE Workbook — the alerting context the P11.03 rule files implement); the D-S1 identifier policy (`crates/adapters/postgres/src/id_generator.rs` — PostgreSQL 18 `uuidv7()`); the P4.10 command-record port (`crates/flexcore-storage/src/command_store.rs`) |
| Files touched | `apps/control-plane/src/metrics.rs` (gauge family + scanner + tests — this file is wave-2 Agent B's exclusively; all wave-1 families intact), `apps/control-plane/src/main.rs` (minimal marked composition block: the scanner task spawn) |

## 1. What was implemented (written, never run)

1. **The gauge family, exactly as named.** `flexcore_commands_stuck` — a
   gauge, NO labels — added to the SAME `MetricsRegistry` (no second
   metrics surface), always rendered (zero included), after the
   `flexcore_command_state_transitions_total` family. The wave-1 handoff
   (record 27 §5.1: "implement `flexcore_commands_stuck` exactly as the
   P11.01 record names it; the dashboard panel and the stuck-state rule
   then render/fire with zero edits") is honored verbatim: the P11.03
   rule (`deploy/compose/observability/prometheus/rules/
   slo_burn_rate_alerts.yml`, expression `flexcore_commands_stuck > 0`)
   and the P11.09 dashboard panel
   (`deploy/compose/observability/grafana/dashboards/command_lifecycle.json`)
   were NOT edited — zero edits, no renames.
2. **The registered threshold (X-8 pre-registration).**
   `COMMANDS_STUCK_THRESHOLD_MILLIS = 86_400_000` (24 h), a constant in
   `metrics.rs`, **labeled RECOMMENDATION pending operator tuning**. A
   command parked in a non-terminal frozen lifecycle state for STRICTLY
   LONGER than the threshold counts as stuck; a command exactly at the
   threshold does not. The value is registered before any run that could
   measure against it; no observed command-duration distribution exists
   (the 0–10 base is WRITTEN-UNRUN), and the legitimate dwell time at the
   three human/policy gates is an operator answer. Changing the value is a
   recorded amendment to this record. The scan interval
   (`STUCK_STATE_SCAN_INTERVAL`, 60 s) is registered under the same
   discipline with the same label.
3. **The detection logic** (all in `metrics.rs`):
   - `CommandStateReader` — the read port (app-layer, the worker's
     P11.12 `CommandStateGateway` pattern): `tenant_ids()` (platform-level
     `TenantRepository::list_all`) and `parked_commands(ctx)` (per-tenant).
   - `PgCommandStateReader` — the Postgres implementation: one read-only
     unit of work per tenant (dropped without commit — the P1.10 rule 4
     rollback contract), listing the tenant's currently-parked commands
     PER NON-TERMINAL STATE (keyset pagination, bounded 200-row pages).
     The nine non-terminal states are DERIVED from the frozen vocabulary
     (`CommandState::ALL` filtered by the domain's `is_terminal`) — no
     second state list exists anywhere. Per-state filtered listing rather
     than one unfiltered listing, because terminal command rows accumulate
     forever for audit: each per-state filter is bounded by what is
     actually parked, an unfiltered listing is not.
   - State-entry time: the transition journal's last row
     (`at_unix_millis`, ordered by seq — the journal is INSERT-only and
     every committed transition writes exactly one row, P4.10) is when the
     command entered its current state. An EMPTY journal means the command
     never transitioned — it is still in its initial PROPOSED state (no
     frozen edge re-enters PROPOSED) — so its state-entry time is its
     creation time, read from the D-S1 UUIDv7 identifier's embedded
     unix-millisecond field (`uuidv7_unix_millis`, which fails closed to
     `None` on any non-UUIDv7 version nibble or malformed text). A
     store/journal disagreement or a non-derivable time makes the command
     UNJUDGEABLE: logged, excluded from the gauge, never guessed.
   - `count_stuck_commands` — the pure classifier (unit-tested): non-terminal
     per the domain's `is_terminal`, park duration strictly beyond the
     threshold, future-dated entry times clamp to age zero, unknown state
     text and missing entry times count as unjudgeable.
   - `run_stuck_state_scanner` — the hosting loop (spawned by the
     composition root): scan → set the gauge → log the per-state
     breakdown → sleep → repeat; the scan runs under
     `tokio::task::block_in_place` (the recorded sync-port hosting rule —
     the reader blocks on the adapter's private runtime); on a scan ERROR
     the gauge keeps its PREVIOUS value (a scanner outage never reads as
     "no stuck commands") and the error is logged.
4. **Composition** (minimal marked block in `apps/control-plane/src/
   main.rs`): the scanner task spawns after the P5.25 live-forwarder
   spawn, sharing the shutdown watch, `store`, and `state.metrics`.

## 2. Cardinality and vocabulary decisions (the P2.21 discipline)

| Decision | Value |
|---|---|
| Labels | NONE (record 01 §2 family 5: "No labels: the gauge counts commands parked in a non-terminal state beyond the registered stuck threshold") |
| Per-state breakdown | LOGS only (`stuck_by_state` in the scan's log line), never labels |
| Terminal/non-terminal authority | `flexcore_domain::command_state::is_terminal` — COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK are terminal; FAILED and EXPIRED are non-terminal (they exit only via FALLBACK, frozen transitions 26–27). No second definition exists in this module. |
| Illegal/unknown state text | Unjudgeable (logged, excluded) — never a series, never guessed (the deny-by-default P2.21 rule) |

## 3. Alarm binding (zero-edit verification)

The P11.03 rule file binds `flexcore_commands_stuck > 0` (record 21
§3.2) and the command-lifecycle dashboard panel plots the same name. Both
were read at close and require NO edit: the family now renders (as
written code), so the rule's wave-1 caveat comment ("the family does not
render until wave 2, so this rule cannot fire today") is historically
accurate text whose condition this subtask closes — the comment was left
untouched per the zero-edit handoff, and the close record (53) notes this.
Whether the rule FIRES in a live Prometheus is a human-gate integration
check, recorded UNRUN (the Compose-only ceiling, plan risk 5).

## 4. ASSUMPTION / UNRUN labels

- **ASSUMPTION**: command IDs are UUIDv7 (D-S1: PostgreSQL 18 `uuidv7()`
  for every primary key and application identifier). The extraction fails
  closed if a deployment composes a different generator; the affected
  PROPOSED commands then read as unjudgeable (logged), never stuck.
- **UNRUN**: the scanner has never executed; the sync adapter ports it
  drives have never executed (the 0–10 base is WRITTEN-UNRUN).
- **UNRUN**: the unit tests in `metrics.rs` (gauge rendering, classifier
  semantics, UUIDv7 extraction, per-tenant merge) execute only at the
  first human `cargo test`.
- **RECOMMENDATION**: the 24 h threshold and the 60 s scan interval,
  registered before any run (X-8); neither claims operator approval.

## 5. Handoff notes

1. **Operator**: tune `COMMANDS_STUCK_THRESHOLD_MILLIS` (and the scan
   interval) on real data; the change is a one-constant edit plus a
   recorded amendment to this record.
2. **Orchestrator**: the ledger P11.20 tick cites this record; the tick
   is the orchestrator's action (program conventions §2.2).
3. **First human build**: the new code paths join the VERIFY-BUILD
   watchlist — the trait-object port calls
   (`CommandRecordStore::list_commands` / `transition_journal` over
   `PgUnitOfWork`, `TenantRepository::list_all`) are written against the
   adapter's recorded surface and compile-verify only at the build.
