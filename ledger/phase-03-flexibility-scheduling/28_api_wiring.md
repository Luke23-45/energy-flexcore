# P3.28 — Decision note: API, events, and persistence wiring (§15, §16)

- **Date:** 2026-11-07.
- **Subtask:** P3.28 (ledger_phase_3.md §P3.28). Report trace: §15.1–§15.3,
  §15.5, §16.1–§16.2, §10.5.
- **Deliverables:** `apps/control-plane/src/routes_phase3.rs` (NEW: the three
  §15.2 resources), `apps/control-plane/src/dto_phase3.rs` (NEW: response
  DTOs + request bodies + the recorded scope statements),
  `apps/control-plane/src/phase3_fakes.rs` (NEW: Test-backend record-store
  fakes + `FailingEventOutbox`), `apps/control-plane/src/state.rs` (the
  three record stores join the unit-of-work bundle),
  `apps/control-plane/src/auth.rs` (the Phase 3 grant surface),
  `apps/control-plane/src/openapi.rs` (additive paths + schemas),
  `apps/control-plane/src/{lib.rs,main.rs}` (additive mounting + the
  cumulative policy set), `crates/flexcore-authz/src/action.rs` (six Phase 3
  actions), `crates/flexcore-storage/src/envelope_store.rs` (NEW: the
  envelope persistence contract),
  `crates/adapters/postgres/src/envelope_store.rs` + migration
  `0015_envelopes_p3_28.{up,down}.sql` (NEW), contract tests
  (`apps/control-plane/tests/api_phase3.rs`).
- **Reuse discipline:** the persistence and event surfaces are NOT restated.
  The routes consume the P3.21 record store (`ScheduleRecordStore`), the
  P3.22 revision chain store (`ScheduleRevisionChainStore`), the P3.12 event
  assembly (`envelope_updated_publications`), and the P3.21 deterministic
  key (`schedule_proposed_idempotency_key`) through their existing homes;
  the API layer adds wiring, validation, and the recorded contract
  statements only.

## 1. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P3.28-1 | **NO execute-command endpoint exists (§15.2).** POST creates a CANDIDATE record only: an envelope record + its events, a schedule revision (CANDIDATE), or a §12.6 optimization-run record (CANDIDATE). There is no approval, dispatch, or execution operation anywhere on the Phase 3 surface; validation, feasibility, and authorization are distinct Phase 4 gates (§12.5). A structural contract test asserts the generated OpenAPI path set stays free of command/execute/dispatch/approve operations and the three resources expose exactly get+post. | `routes_phase3.rs` header; `tests/api_phase3.rs::the_surface_has_no_execute_command_operation` |
| D-P3.28-2 | **ScheduleValidated scope (the ledger requirement, stated everywhere).** "Validated" in Phase 3 means asset/time-coupled constraint validation ONLY (the P3.01–P3.11 capability models + the P3.03 time model — the P3.25/P3.26 assertion surface), explicitly NOT network feasibility (Phase 4, §12.5), NOT an authorization, NOT a dispatch verdict. **Payload gap (recorded, the D-P3.21-4 precedent):** the frozen `ScheduleValidatedPayload` carries no validation-scope field — it CANNOT carry the caveat. The caveat therefore travels in (a) the persisted/served record fields (`validation_scope: "asset_time_coupled"` + `validation_scope_statement` on every schedule response), (b) the OpenAPI description, and (c) this decision note. The event is emitted ONLY for a revision whose recorded asset/time-coupled validation outcome is PASS; a FAIL outcome is a 422 with NOTHING stored and NO event (fail-before-write). **Needs extension:** an additive `serde(default)` `validation_scope` payload field, for a future subtask that owns flexcore-events. | `dto_phase3.rs` (`SCHEDULE_VALIDATED_SCOPE_STATEMENT`); `routes_phase3.rs` `propose_schedule` |
| D-P3.28-3 | **Events via the transactional outbox (§15.5).** Each POST writes the state row(s) and the paired outbox row(s) inside ONE unit-of-work transaction: Postgres coerces the three record stores and the outbox from the SAME `PgUnitOfWork` allocation (state.rs), so inserts and events commit or roll back together. The assemblies reuse the existing proof-verified surfaces where the input shapes fit: the envelope route calls the P3.12 `envelope_updated_publications` (one `FlexibilityEnvelopeUpdated` per covered site, deterministic key `flexibility_envelope_updated:<portfolio>:<site>:<envelope_version>`); the schedule/run routes emit `ScheduleProposed` with the P3.21 deterministic key (`schedule_proposed:<portfolio>:<site>:<candidate_id>`) and the frozen payload. The P3.21 `schedule_proposed_publication` assembly requires the typed `ScheduleResultRecord` (the full §12.6 object), which a REST boundary does not receive — the composition root emits the IDENTICAL frozen payload + key and records this deviation honestly. The `ScheduleValidated` key mirrors the rule: `schedule_validated:<portfolio>:<site>:<candidate_id>`. | `state.rs::run_in_uow`; `routes_phase3.rs::emit_phase3_event` |
| D-P3.28-4 | **State-change receipts for the Phase 3 tables.** The P1.12 §3 canonical entity names cover the registry tables; the Phase 3 record tables are named by their table names (`flexibility_envelopes`, `schedule_results`, `schedule_revisions`). A revision insert pairs with TWO events (`ScheduleProposed` + `ScheduleValidated`); each `OutboxWrite` carries a receipt descriptor of the SAME state change — the one-receipt-one-event rule is read as one-receipt-per-event here (recorded). | `routes_phase3.rs::insert_receipt` |
| D-P3.28-5 | **Candidate identity discipline.** A schedule revision references the §12.6 optimization-run record of the SAME candidate (the migration 0014 composite FK); the route verifies the record exists inside the same transaction (a missing candidate is 422 naming the remediation: POST `/api/v1/optimization-runs` first). The grid-model traceability of a run is NOT caller-supplied: the route records the named site's own Phase 2 grid-model reference verbatim (the P3.21 `GRID_MODEL_PROVENANCE` rule that a grid-model version is never detached from its site); the caller supplies only the version. | `routes_phase3.rs::propose_schedule` / `create_optimization_run` |
| D-P3.28-6 | **Listing limits (the P2.12/P2.19 convention, recorded).** Default page 200, maximum page 1000 (over the maximum or below 1 is 422); keyset cursors only — opaque, malformed cursor is 422, never a silent "first page". NO offset pagination, NO unbounded query. No time-range parameter: the record primary keys are UUIDv7s whose embedded timestamps make keyset order a stable time order; the portfolio/site/status filters scope each query. The optimization-run status filter accepts ONLY the closed §12.6 vocabulary (a seventh value is 422). A schedule listing with `schedule_id` returns the FULL revision chain (superseded revisions retained for audit, §14.2) without pagination; a foreign-tenant chain is 404. | `routes.rs::apply_list_limit`; `dto_phase3.rs::PHASE3_DEFAULT_PAGE_SIZE` / `PHASE3_MAX_PAGE_SIZE`; `routes_phase3.rs` list handlers |
| D-P3.28-7 | **Cross-tenant reads (no existence leak, P0.16 rule 5).** Listings are tenant-scoped by construction (the stores filter by the caller's tenant; RLS is the outer gate on the Postgres backend); another tenant's records are invisible, and any by-id/chain query naming a foreign or unknown reference is 404 — never 403. The unit-of-work RLS context (`flexcore.tenant_id`, P1.12 §5.1) is set per request through `run_in_uow`. | `routes_phase3.rs` handlers; `state.rs` |
| D-P3.28-8 | **Phase 3 action grants (the P2.12 recorded-grants pattern).** Six new deny-by-default actions in flexcore-authz (`flexibility.envelope.read/create`, `schedule.read/propose`, `optimization_run.read/create`), all evaluated at TENANT scope (the routes enforce the ownership chain — portfolio/site existence and the revision chain — before any write, so scope granularity stays tenant-level in Phase 3). Grants: PlatformAdmin and TenantAdmin get the full surface; Operator additionally gets `schedule.propose` (proposing a CANDIDATE is the operator scheduling action — simulation-only in 0.x; the event is a proposal, never an approval); Viewer and Operator get all reads. Deliberately ABSENT: no envelope/schedule/run delete, update, or approve action; no dispatch/execute action anywhere. The composition root mounts the cumulative `phase3_default_policies()` (Phase 1 ⊕ Phase 2 ⊕ Phase 3 grants). | `crates/flexcore-authz/src/action.rs`; `auth.rs::phase3_default_policies` |
| D-P3.28-9 | **Envelope persistence (migration 0015), the D-P3.21-4 pattern.** `EnvelopeRecordStore` lives in flexcore-storage (same recorded home deviation as the P3.21 record store, shaped to move to flexcore-ports verbatim). The envelope record is an immutable, append-oriented computation artifact (insert + tenant-scoped find/list; no update, no delete — a recomputation publishes a NEW version); `(tenant, portfolio, envelope_version)` is UNIQUE so a silent overwrite is unrepresentable; runtime grants INSERT + SELECT only; RLS per the P1.12 §5.1 conventions; the full P3.12 computation result travels as opaque JSON (`envelope_payload_json`, adapter-validated). The insert executes inside the CALLER's unit of work — the same native transaction carries the paired `FlexibilityEnvelopeUpdated` outbox rows. | `crates/flexcore-storage/src/envelope_store.rs`; `crates/adapters/postgres/src/envelope_store.rs`; migration `0015_envelopes_p3_28.{up,down}.sql` |
| D-P3.28-10 | **Test-backend atomicity scope (recorded honestly).** The flexcore-ports fakes have no transaction, so the Test backend cannot prove commit-or-rollback; its evidence is (a) the journalled state-change → outbox-append ordering/pairing per write, (b) the `FailingEventOutbox` injection proving a forced publish failure fails CLOSED (opaque infrastructure error, no success response, no outbox row), and (c) fail-before-write assertions (a rejected request stores nothing, emits nothing). The database-gated atomicity evidence (state + outbox in ONE native transaction) is the adapter's, per the P1.13/P3.21 pattern. | `phase3_fakes.rs` header; `tests/api_phase3.rs::a_forced_publish_failure_fails_closed` |
| D-P3.28-11 | **Response contract statements carried IN the responses.** Every schedule-bearing response carries `candidate_status: "candidate"`, the frozen P3.21 `CANDIDATE_STATUS_STATEMENT`, `validation_scope`, and `validation_scope_statement`; the envelope response carries the recorded P3.12 `ENVELOPE_PAYLOAD_GAP_NOTE`; the optimization-run response carries the `schedule_proposed_key` (the state+outbox pairing proof) and the verbatim site grid-model reference. Contract tests validate response bodies against the GENERATED OpenAPI schemas (the spec is generated from the route annotations, never hand-written) and assert every outbox row carries `schema_version: 1`. | `dto_phase3.rs`; `tests/api_phase3.rs` |

## 2. Contract-test coverage (ledger step 4 / acceptance)

- `tests/api_phase3.rs` (Test backend, no network/database; the P1.17
  composition pattern with the Phase 3 record-store fakes):
  - **OpenAPI-shape validation**: 201/200 bodies validated field-by-field
    against the generated `components.schemas` (every returned field is a
    schema property; every required property is returned; nested `$ref`s
    recurse).
  - **Outbox schema version**: every outbox row's serialized envelope
    parses and carries `schema_version == 1` and the frozen event name.
  - **Atomic pairing**: state-change receipt followed by the paired outbox
    append per write; forced publish failure fails closed; validation
    rejections store and emit nothing.
  - **Cross-tenant**: foreign references read as 404 on all three
    resources (writes and the by-id chain read); foreign records never
    appear in any listing.
  - **ScheduleValidated**: tested as asset/time-coupled ONLY — the record
    fields carry the scope statement verbatim; a FAIL outcome is 422 with
    nothing stored and no event; the frozen payload is pinned to its exact
    six fields (the recorded gap: no validation-scope field can exist in
    it).
  - **Authorization**: viewer blocked from propose (403) but allowed
    reads; operator proposes schedules but cannot create envelope/run
    records; unauthenticated is 401 everywhere.
  - **Keyset pagination**: recorded limits (default 200, max 1000; both
    boundary violations are 422), cursor walk, malformed cursor 422,
    closed status filter, page bodies schema-valid.
  - **No execute-command operation**: structural (generated path set +
    operation shapes) and live probes of the forbidden routes; the emitted
    event names are pinned to exactly the three §15.3 Phase 3 events.

## 3. Handoffs / open items (for the coordinator)

- **flexcore-authz actions** (`crates/flexcore-authz/src/action.rs`): the
  six Phase 3 actions were added in the P2.12 pattern (id round-trip,
  `all()` inclusion). If the batch ownership did not include this crate,
  the additions are the handoff — the routes/auth policy set reference
  them and cannot compile without them.
- **VERIFY-BUILD (utoipa schema naming):** the contract tests resolve
  generic schemas as `DataEnvelope_<Item>` (the utoipa 5 naming
  convention) with a prefix-match fallback that panics naming the
  available schemas; no other API detail is assumed beyond the annotated
  shapes.
- **flexcore-events payload gaps** (unchanged from D-P3.12-7/D-P3.21-4 and
  D-P3.28-2): the frozen payloads carry no envelope version, no candidate
  id, and no validation-scope field. The references travel in the
  deterministic idempotency keys and the record fields; the additive
  `serde(default)` extensions are for a future subtask that owns
  flexcore-events.
