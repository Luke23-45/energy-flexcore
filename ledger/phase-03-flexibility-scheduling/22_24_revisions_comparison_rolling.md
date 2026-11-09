# P3.22/P3.23/P3.24 — Decision note: schedule revisions, comparison workflow, rolling horizon

- **Date:** 2026-11-07.
- **Subtasks:** P3.22, P3.23, P3.24 (`ledger_phase_3.md`). Report trace:
  §12.6, §14.2, §15.2, §15.3, §19.4, §23 Stage 3, §24 steps 3–4, §12.1,
  §10.2–§10.3.
- **Deliverables:**
  `crates/flexcore-storage/src/schedule_revision.rs` (NEW),
  `crates/flexcore-storage/src/schedule_store.rs` (additive: comparison
  storage), `crates/flexcore-storage/src/lib.rs` (additive decls),
  `crates/flexcore-storage/tests/schedule_revision.rs` (NEW),
  `crates/adapters/postgres/migrations/0014_schedule_revisions_p3_22.{up,down}.sql`
  (NEW), `crates/adapters/postgres/src/schedule_store.rs` (extended impl),
  `crates/adapters/postgres/tests/schedule_revisions.rs` (NEW, DB-gated),
  `crates/flexcore-application/src/schedule_comparison.rs` (NEW),
  `crates/flexcore-application/src/rolling_horizon.rs` (NEW),
  `crates/flexcore-application/src/lib.rs` (additive decls),
  `crates/flexcore-application/tests/{schedule_comparison,rolling_horizon}.rs`
  (NEW).
- **Reused, never restated:** the §12.6 result contract and the P3.18
  `Tier1Tier2ComparisonRecord` (flexcore-optimization-api); the P3.21
  record assembly, persistence proof, and `ScheduleProposed` machinery
  (flexcore-application `optimization_result_record`); the P3.13
  staleness triggers (the `envelope_invalidation` trigger cites them); the
  P3.11 EV opt-out semantics (`DispatchedAfterOptOut` discipline); the
  P3.03 rollover rule and interval semantics.

## 1. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P3.22-1 | **Revision identity scheme.** A schedule CHAIN is identified by `schedule_id` (canonical UUID text); `revision` is monotonically increasing per chain with NO gaps: revision 1 = initial solve; revision n > 1 supersedes exactly n − 1. The scope of the identity names the portfolio and the EXACT horizon (start-inclusive/end-exclusive UTC millis) per revision. A re-solve creates a NEW revision, never mutates the old one; every new revision is a CANDIDATE. The §12.6 candidate id of the revision is a composite FK to the P3.21 `schedule_results` record — the revision and its result record are two views of ONE solve. | `schedule_revision.rs` (`REVISION_IDENTITY_STATEMENT`); migration 0014 (`schedule_revisions_pk`, `schedule_revisions_initial_chain`, composite FKs) |
| D-P3.22-2 | **Supersede discipline.** The prior revision is marked SUPERSEDED (stored text `superseded`; §14.2, retained for audit, distinct from CANCELLED) — the ONLY state change a revision row ever undergoes. The link is complete in BOTH directions: the new row carries `supersedes_revision` + `supersedes_candidate_id`, the superseded row carries `superseded_by_revision` + `superseded_by_candidate_id`. Immutability at the storage layer: INSERT + SELECT grants plus a COLUMN-LEVEL UPDATE grant on exactly the three transition columns (the recorded exception anticipated by D-P3.21-8), and a BEFORE UPDATE trigger rejecting any content change and any state movement other than `active → superseded`. The one-active invariant is a partial unique index; the supersede marking and the new-revision insert run in ONE unit of work. | `schedule_revision.rs` (`SUPERSEDE_TRANSITION_STATEMENT`); migration 0014 (trigger, grants, `schedule_revisions_one_active`, `schedule_revisions_state_links`) |
| D-P3.22-3 | **Phase 4 surface, no command path.** `ScheduleRevisionChainStore::is_revision_superseded` is the check the P4.12 dispatch gate will call: `Some(true)` superseded, `Some(false)` active, `None` unknown-in-tenant-scope (the gate rejects both the superseded and the unknown case; `None` is never "usable"). No command, lifecycle, or dispatch code exists in this batch. | `schedule_revision.rs` (`PHASE4_GATE_NOTE`, trait method) |
| D-P3.23-1 | **Comparison record.** One shared modeling-input snapshot + the compared candidate IDs + the four §19.4 scheduling metrics per candidate (objective value with units, schedule deviation in MW, feasible interval count, runtime) + the tier/solver identity per candidate. A comparison is over PRODUCED schedules only (status `optimal`/`feasible`); both sides remain CANDIDATES — the comparison is diagnostic, never a feasibility verdict. At least two candidates. | `schedule_comparison.rs` (`COMPARISON_METRICS_STATEMENT`, `COMPARISON_CANDIDATE_STATEMENT`, `compare_candidate_schedules`) |
| D-P3.23-2 | **Cross-input policy, decided once: REJECTION with the named error** `ComparisonError::IncomparableInputs` (per differing constituent). The comparability fields are the MODELING constituents — envelope version, forecast version + provenance, objective name + objective config record fields (verbatim), telemetry reference — NOT the P3.21 snapshot reference string, which embeds the solver-config digest and legitimately differs across the tier-varying comparison; each candidate's own §12.6 snapshot reference is recorded verbatim on its entry. The display-side labelling path (`incomparability_label`) exists for rendering contexts and is NOT a second acceptance path. | `schedule_comparison.rs` (`CROSS_INPUT_POLICY`, gate in `compare_candidate_schedules`) |
| D-P3.23-3 | **Comparison persistence + API wiring.** The record persists in `schedule_comparisons` (migration 0014; immutable, INSERT+SELECT, RLS, tenant-scoped) through `flexcore-storage::schedule_store::ScheduleComparisonStore` and the PostgreSQL adapter — the same unit-of-work pattern as the P3.21 record store. The REST routes `/api/v1/schedules` + `/api/v1/optimization-runs` (§15.2) are P3.28's to mount; NO UI code exists in Phase 3 (§20.1 item 6). | `schedule_store.rs` (storage), adapter `schedule_store.rs`, `COMPARISON_API_WIRING_NOTE` |
| D-P3.23-4 | **First comparison case.** The P3.18 Tier 1 vs Tier 2 cross-check builds the record (`tier1_tier2_comparison_case`): the recorded 1 MW linearisation gap on the shared inverter fixture, with the P3.18 notes carried into the comparison record and the gap evidence recorded WITH its interpretation (a positive difference on a maximisation means the linearisation overestimates deliverable export). The single-interval fixture's deviation basis (0.0 MW) and feasible-interval count (1) are caller-supplied and recorded — the P3.18 record does not carry metrics. | `schedule_comparison.rs` (`Tier1Tier2ComparisonCaseInput`, `tier1_tier2_comparison_case`) |
| D-P3.24-1 | **Roll-trigger set (REQUIRED per revision).** `horizon_advance`, `envelope_invalidation` (P3.13 trigger evaluation), `asset_opt_out_dropout` (P3.11 semantics), `forecast_update`, `operator_request` — plus `initial_solve` so the required `roll_trigger` column stays total for the first revision of a chain. The stored string set is pinned in BOTH flexcore-storage and flexcore-application (tests in both crates assert the exact array). | `rolling_horizon.rs` (`ROLL_TRIGGER_STATEMENT`, `RollTrigger`); migration 0014 (`roll_trigger` CHECK); `schedule_revision.rs` (`SCHEDULE_ROLL_TRIGGERS`) |
| D-P3.24-2 | **Trigger/boundary consistency and the new horizon.** `horizon_advance` requires the prior horizon consumed (boundary at or beyond the end) and shifts a same-length new horizon starting at the boundary; every other trigger requires a mid-horizon boundary (< the horizon end). Mid-horizon triggers keep the prior horizon as the new solve's horizon. A mid-horizon roll's frozen set is the overlap; a rollover has NO overlap — the whole new horizon is re-optimized and the fully executed prior horizon is realised history retained in the superseded revision rows (the P3.03 rule: supersede, never a silent overwrite). | `rolling_horizon.rs` (`plan_rolling_horizon_roll`, `TriggerHorizonMismatch`) |
| D-P3.24-3 | **Realised-state carry-forward and the frozen past.** Actual SOC (fraction in [0,1] at the API boundary, §10.2) and actual load response enter the new solve as INITIAL CONDITIONS. An interval is FROZEN iff it started before the execution boundary (start-inclusive on purpose: the in-progress interval is frozen — re-solving it would contradict measured reality); only future intervals (start ≥ boundary) are re-optimized. Every frozen (interval, asset) pair requires a realised value unless the asset dropped out at or before the interval start; the projection enforces continuity, grid alignment, and the `DispatchedAfterOptOut` discipline by construction. | `rolling_horizon.rs` (`FROZEN_INTERVAL_STATEMENT`, `CARRY_FORWARD_STATEMENT`, `plan_rolling_horizon_roll`, `project_rolling_schedule`) |
| D-P3.24-4 | **Supersede chain per roll.** Every roll produces new revision = prior + 1 (no gaps, same `schedule_id`); the plan carries both sides of the link; the composition root marks the prior SUPERSEDED and inserts the new revision in ONE unit of work. `rolling_horizon_schedule_proposed_publication` verifies the plan's link is the gapless one-step shape before delegating to the P3.21 publication. | `rolling_horizon.rs` (`RollPlan`, `RevisionIdentity`, publication gate) |
| D-P3.24-5 | **`ScheduleProposed` per new revision — payload gap recorded honestly** (the D-FX-9 / D-P3.12-7 / D-P3.21-4 precedent): the frozen payload cannot carry the trigger or the supersede link. Both travel in the persisted revision row (migration 0014: `roll_trigger`, `supersedes_revision`, `supersedes_candidate_id`) and pair with the event through the deterministic `ScheduleProposed` idempotency key. **Needs extension:** additive `serde(default)` payload fields (`trigger`, `superseded_revision`) per the P2.16 precedent, for a future subtask that owns flexcore-events. | `rolling_horizon.rs` (`SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE`) |

## 2. Acceptance evidence (test map)

- **P3.22 full chain (solve → revise → SUPERSEDED → old retrievable; no
  in-place mutation):** `crates/adapters/postgres/tests/schedule_revisions.rs`
  (`the_full_revision_chain_supersedes_and_retains_the_old_revision`,
  `the_supersede_transition_is_one_way_and_revision_content_is_immutable` —
  includes the raw-UPDATE rejection of the payload and of a state
  regression, row unchanged after);
  `crates/flexcore-storage/tests/schedule_revision.rs` (gapless chain rule,
  bidirectional link, candidate status, gate semantics, horizon in the
  identity).
- **P3.23 metrics + cross-input rejection + first case:**
  `crates/flexcore-application/tests/schedule_comparison.rs` (all four
  metrics per candidate; named rejection per differing constituent;
  produced-schedule/metric/two-candidate gates; the P3.18 1 MW gap case
  with notes carried; the P3.28 wiring statement).
- **P3.24 mid-horizon dropout + realised carry-forward + frozen past:**
  `crates/flexcore-application/tests/rolling_horizon.rs` (the dropout roll
  respects realised SOC, freezes the realised past — not the prior plan's
  values, excludes the dropped asset, keeps the supersede chain; SOC-range
  and realised-coverage rejections; trigger/boundary consistency;
  `DispatchedAfterOptOut` in the projection; `ScheduleProposed` assembly
  through the P3.21 machinery).

## 3. VERIFY-BUILD items (not executed — loop rules)

1. **Dependency-rule gate:** the P3.21 note stands —
   `ALLOWED_INTERNAL_EDGES["flexcore-application"]` in
   `tools/check_dependency_rule.py` must contain
   `"flexcore-optimization-api"` (this batch adds no new crate edges;
   flexcore-storage gained no new dependencies either).
2. **Migration replay:** 0014 up/down is exercised by the adapter migration
   replay on PostgreSQL 18; the DB-gated suite above proves the trigger,
   the column-level grants, and the partial unique index behaviour.
3. **Trigger-string drift guard:** the closed trigger strings are pinned by
   tests in both flexcore-storage and flexcore-application; if either list
   changes, both tests fail by design.
4. **`PgUnitOfWork` field-access precedent:** unchanged from the P3.21 note
   (the same `pub(crate)` one-line fix if rustc rejects the sibling-module
   access, E0616).

## 4. Open items carried forward

- P3.28 mounts the REST routes (schedules, optimization-runs, comparisons)
  and wires the outbox publication through `event_assembly`.
- The Phase 4 P4.12 gate consumes `is_revision_superseded` (and should also
  reject `None`); the exact rejection wording is Phase 4's.
- The `ScheduleProposed` payload extension (D-P3.24-5) joins the queued
  payload extensions (D-FX-9, D-P3.12-7, D-P3.21-4) for the subtask that
  owns flexcore-events.
- The comparison record's candidate entries are stored as JSON; if Phase 4
  needs candidate-level SQL filtering, a follow-up migration can promote the
  metric columns — not done speculatively here.
