# P3.21 — Decision note: optimization result contract — completion, persistence, event

- **Date:** 2026-11-07.
- **Subtask:** P3.21 (ledger_phase_3.md §P3.21). Report trace: §12.6, §15.3,
  §16.1, §16.2, §21.3.
- **Deliverables:** `crates/flexcore-application/src/optimization_result_record.rs`
  (NEW: assembly, snapshot linking, `ScheduleProposed` emission),
  `crates/flexcore-storage/src/schedule_store.rs` (NEW: the persistence
  contract), `crates/adapters/postgres/src/schedule_store.rs` (NEW:
  PostgreSQL implementation) + `migrations/0013_optimization_results_p3_21.{up,down}.sql`
  (NEW), tests (`crates/flexcore-application/tests/optimization_result_record.rs`,
  `crates/adapters/postgres/tests/schedule_store.rs`).
- **Reuse discipline:** the §12.6 contract types are NOT restated. The single
  home stays `flexcore-optimization-api` (`result::SolveStatus` six-status
  vocabulary, `OptimizationResult::validate_complete`, `SolverConfig` +
  digest, `DeterminismRecord`, `InfeasibilityEvidence`, `RelaxationDecision`);
  P3.21 consumes them and adds only the wiring (snapshot, traceability,
  persistence, event).

## 1. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P3.21-1 | **Record assembly over the port contract.** `assemble_schedule_result_record` runs the port-level completeness gate (`OptimizationResult::validate_complete`) FIRST — a result missing any required field never becomes a schedule record — then completes the wiring fields: `input_snapshot_id` = the snapshot's deterministic reference; `grid_model_version` = the traceability's version. A provider-set wiring field that DISAGREES with the supplied snapshot/traceability is an error, never silently overwritten. The P3.15 `ScalingWarning`s and — for every Tier 2 problem class — the P3.18 `SOCP_EXACTNESS_CAVEAT` are merged into `result.warnings` (deterministic order; the caveat is never duplicated). | `optimization_result_record.rs` (assembly + `ScheduleResultRecord::validate_record_complete`) |
| D-P3.21-2 | **Input snapshot shape (ledger step 2, exactly):** capability versions (P3.01 `CAPABILITY_SCHEMA_VERSION` per asset + P2.02 profile reference; a mismatched version is a named error), envelope version (P3.12/P3.13 `EnvelopeVersion` inside the P3.13 `ScheduleEnvelopeReference`), forecast version (P3.31 — accepted ONLY through the `LabeledSeriesValues` labeled accessor, so the `Synthetic` provenance label cannot be dropped; an exhaustive match forces a recorded decision before any future provenance variant enters a record), objective configuration (P3.14 `ObjectiveConfig::record_fields()` verbatim — the SYNTHETIC provenance label, the not-a-market-product flag, every term's weight/unit/sign, and the §7.5 exclusion statement survive into the persisted record), solver configuration (P3.19 `SolverConfig::canonical_json()` + `config_digest()` — verifiable, not a blob), and the telemetry snapshot reference (caller-supplied pointer, non-empty). The snapshot reference string `schedule_input_snapshot:<envelope_version>:<forecast_version>:<provenance>:<solver_digest>:telemetry=<ref>:assets=<n>` is deterministic; the result references the exact snapshot used. | `InputSnapshotRecord::assemble` / `InputSnapshotRecord::reference` |
| D-P3.21-3 | **Grid-model traceability (ledger step 2 note).** The §12.6 "grid model version" comes from the Phase 2 Site grid-model reference (P2.02 `Site::grid_model_reference`) — the version of the network model the site references. No Phase 3 or Phase 4 model exists; none may be invented. `GridModelTraceability` is constructed EXCLUSIVELY from a domain `Site`, records the site id + the verbatim reference next to the version, and the assembly rejects a traceability naming a site other than the record's own site (the version must be traceable to THE record's site). Recorded verbatim in `GRID_MODEL_PROVENANCE_STATEMENT` and persisted as `schedule_results.grid_model_site_id` + `grid_model_site_reference` (composite FK to `sites(site_id, tenant_id)` + a CHECK that the traceability site IS the record site). | `GridModelTraceability`; migration 0013 constraints |
| D-P3.21-4 | **`ScheduleProposed` payload gap (recorded honestly, the D-FX-9 / D-P3.12-7 precedent):** the frozen `ScheduleProposedPayload` (P0.14/P1.09) carries correlation/tenant/portfolio/site/asset + event_time only — NO candidate id, NO result-record reference. flexcore-events was frozen for this subtask, so (a) the nil asset id denotes "the schedule scope, not a single asset" (the envelope's recorded convention), and (b) the result-record reference travels in the deterministic idempotency key `schedule_proposed:<portfolio>:<site>:<candidate_id>` (republication of one candidate's proposal dedupes durably) and in the persisted record (`schedule_results.schedule_proposed_key`). **Needs extension:** additive `serde(default)` payload fields (`candidate_id`, `result_record_ref`) per the P2.16 precedent, for a future subtask that owns flexcore-events. The event is a fact that a CANDIDATE was proposed — never a feasibility, authorization, or dispatch claim. | `schedule_proposed_publication`; `SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE` |
| D-P3.21-5 | **Persistence before use (§16.1, §15.5), as types.** The storage-side contract (`flexcore-storage::schedule_store::ScheduleRecordStore` + `ScheduleRecordRow`/receipt/error) makes an incomplete record unrepresentable (one validating constructor; closed status CHECK; objective completeness for `optimal`/`feasible`). The row records the `ScheduleProposed` idempotency key it was persisted with — the pairing proof that the state row and the outbox row belong to ONE transaction. The application's `PersistedResultRecord` proof is verified against the record (reference + key equality) before `schedule_proposed_publication` assembles anything: an event for an unpersisted or mismatched record is unrepresentable in the assembled output. The PostgreSQL implementation runs the insert inside the CALLER's `PgUnitOfWork` transaction (same class as the P1.13 repositories and P2.06 telemetry store), so record + outbox row commit or roll back together; the DB-gated test proves the rollback case. | `schedule_store.rs` (both), `PERSIST_BEFORE_USE_STATEMENT`, `tests/schedule_store.rs` |
| D-P3.21-6 | **Storage-trait home deviation (honestly recorded).** The P1.10 single-home rule puts repository traits in flexcore-ports. The P3.21 subtask's file ownership granted the schedule-record store to flexcore-storage (flexcore-ports was frozen for the batch). The trait is shaped as an append-oriented RECORD store (insert + tenant-scoped find; no update/delete — immutable records), not a Stage 1 aggregate repository, and its surface is designed to move to flexcore-ports verbatim if a later subtask relocates it. The crate README's "no repository trait is defined here" statement carries the recorded exception. | `flexcore-storage/src/schedule_store.rs` module header; `flexcore-storage/src/lib.rs` |
| D-P3.21-7 | **Dependency edge, recorded with its follow-up.** `flexcore-application` now depends on `flexcore-optimization-api` (the §12.6 types are reused, never restated — the explicit ledger requirement). The edge is consistent with the hexagonal layering: `flexcore-optimization-api` is a port-contract core crate (no infrastructure; `check_dependency_rule.py` already enforces core-crate rules on it). The manifest carries a NOTE FOR THE COORDINATOR: `ALLOWED_INTERNAL_EDGES["flexcore-application"]` in `tools/check_dependency_rule.py` must gain `"flexcore-optimization-api"` (that file was outside the subtask's ownership). Until then the dependency-rule check reports this one edge as a violation — the only known check failure introduced by this subtask. | `crates/flexcore-application/Cargo.toml` |
| D-P3.21-8 | **Immutability and grants (migration 0013).** `candidate_id` is the PRIMARY KEY (§12.6 candidate identity); records are immutable — the runtime grants are INSERT + SELECT only (P2.06 append-only discipline). A candidate collision is `ScheduleStoreError::Conflict`; a re-solve is a NEW candidate (P3.22 owns revision/supersede identity — if that subtask needs an UPDATE grant for SUPERSEDED marking, it is that subtask's recorded decision). RLS follows the exact P1.12 §5.1 conventions: owner `flexcore_migrator`, runtime `flexcore_app`, `ENABLE` + `FORCE` + the shared `tenant_isolation` policy shape; `tenant_id` on the row and in every query scope (§16.2). | migration `0013_optimization_results_p3_21.up.sql`; `ScheduleStoreError::Conflict` |

## 2. Completeness test design (ledger step 4 / acceptance)

- `tests/optimization_result_record.rs` runs the §12.6 checklist **per
  provider path** (the P3.19 risk note "diagnostics dropped for one solver
  path"): Tier 1 optimal, Tier 1 infeasible (conflict evidence present, no
  schedule), Tier 1 timeout (residuals explicitly not available WITH a
  reason), Tier 2 optimal (SOCP caveat exactly once). Every path asserts:
  status in the closed vocabulary; problem class + solver/provider identity;
  objective value AND units for solving statuses; primal/dual/residuals
  populated or explicitly not available with a non-empty reason; runtime +
  resources recorded; input snapshot + grid-model version wired; synthetic
  assumptions declared; warnings present (SOCP caveat on Tier 2); candidate
  id; determinism record (thread count 1, digest).
- Missing-field rejections: candidate id, determinism record, empty solver
  identification, objective without units on a solving status, primal
  diagnostics missing on a solving status, empty not-available reason,
  mismatched provider-set snapshot reference / grid-model version.
- Snapshot resolution: the result's `input_snapshot_id` equals the
  snapshot's own reference, and every ledger-required constituent is visible
  in it (envelope version, forecast version + provenance, solver digest,
  telemetry reference, capability count; objective record fields verbatim).
- `tests/schedule_store.rs` (DB-gated on `FLEXCORE_TEST_DATABASE_URL`, the
  recorded harness decision): insert/find round-trip, record + outbox
  atomicity under rollback, duplicate-candidate conflict, foreign-tenant
  read = `None`, and the DATABASE-level objective-completeness CHECK as the
  last line of defence behind the row constructor and the port gate.

## 3. VERIFY-BUILD items (not executed — loop rules; all are build-time checks)

1. **Dependency-rule gate:** add the edge per D-P3.21-7, then run
   `python tools/check_dependency_rule.py` — expect the edge violation to
   clear.
2. **`PgUnitOfWork` field access precedent:** `adapters/postgres/src/schedule_store.rs`
   accesses the private `tx`/`runtime` fields from a sibling module, exactly
   as the existing `telemetry.rs`/`corrections.rs` `impl PgUnitOfWork` blocks
   do. If rustc rejects that access (E0616), the one-line fix is
   `pub(crate)` on the two field declarations in `unit_of_work.rs`, which
   repairs every module at once (that file was outside this subtask's
   ownership).
3. **Migration replay:** 0013 up/down is exercised by the adapter migration
   replay (the P1.13/P2 pattern) on PostgreSQL 18.
4. **HiGHS duals surface:** the Tier 1 test fixture records duals as
   not-available with the VERIFY-BUILD reason from decision note
   17_optimizer_port.md; the real binding build decides the final reason
   text (the record machinery already accepts either availability state).

## 4. Open items carried forward

- The P3.22 subtask owns candidate/revision identity, SUPERSEDED marking,
  and the schedule-id reference surface the dispatch commands will use; the
  `candidate_id` primary key and the immutable-record design here are its
  inputs.
- The `ScheduleProposed` payload extension (D-P3.21-4) is queued with the
  other recorded payload extensions (D-FX-9, D-P3.12-7) for the subtask that
  owns flexcore-events.
