# Final Review R2 — Control Plane, Worker, Authz, Audit, Command Lifecycle (Iteration 1)

**Reviewer:** Agent B (wave 1)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 0 · P1: 4 · P2: 7

## 1. Scope reviewed

Static census (LOC via `wc -l`; deep-read = every line read during this review; structural skim
= signatures, wiring points, and call sites only):

| Area | Files | LOC (src) | Read depth |
|---|---|---|---|
| `apps/control-plane/src/` | 41 modules | 23,305 | Deep-read: `main.rs`, `lib.rs`, `routes.rs`, `routes_phase2.rs`, `routes_phase3.rs`, `routes_phase4.rs`, `routes_edge.rs`, `routes_live.rs`, `routes_alarms.rs`, `routes_plugins.rs`, `routes_research.rs`, `state.rs`, `auth.rs`, `error.rs`, `rate_limit.rs`, `idempotency.rs`, `metrics.rs`, `probes.rs`, `config.rs`, `openapi.rs`, `metrics_route.rs`, `encoding.rs`, `stepup.rs`, `session.rs`, `oidc.rs` (through the resolver core), `webhook_service.rs` (dispatcher + store port), `webhook_signing.rs` (scheme + key/secret handling), `audit_export.rs`, `notification_vocab.rs`, `notifications.rs` (map/registry/render core), `api_versioning.rs` (constants + diff checker). Structural skim: `dto*.rs` (6 files), `phase3_fakes.rs`, `phase4_fakes.rs`, `otel_telemetry.rs`, remainder of `oidc.rs`/`notifications.rs`/`webhook_service.rs`/`api_versioning.rs` (tests). Tests (11,789 LOC) scanned only for wiring assertions they depend on. |
| `apps/worker/src/` | 10 modules | 7,598 | Deep-read: `main.rs`, `lib.rs`, `command_consumer.rs`, `runner.rs`, `inbox.rs`, `config.rs`, `probes.rs`, `supervision.rs`, `ev_ingest.rs`. Structural skim: `ingestion.rs` (metrics surface + module headers; pipeline internals are R6/R7 territory), `maintenance.rs`, `otel.rs`. |
| `crates/flexcore-authz/src/` | 8 modules | 2,260 | Deep-read: `lib.rs`, `action.rs`, `policy.rs`, `phase4_policies.rs`, `phase13_policies.rs`, `phase16_market_policies.rs` (grants + header review verdict). Structural skim: `scope.rs`, `decision.rs` (public API + deny/allow shape). |
| `crates/flexcore-audit/src/` | 5 modules | 3,125 | Deep-read: `lib.rs`, `chain.rs` (full SHA-256/HMAC/chain/anchor implementation + canonical encoding), `sink.rs` (the one-method port + source-text test), `update_evidence.rs` (header, primitives reuse, domains). Structural skim: `record.rs`. |

Cross-boundary verification targets outside the primary scope were grep-verified, not deep-read:
`crates/flexcore-domain/src/command_state.rs` (the frozen lifecycle — read fully, it is the
lifecycle authority this area consumes), `crates/flexcore-storage/src/` port definitions
(`command_store.rs`, `audit_read.rs`, `edge_enrollment`, `schedule_*`, `envelope_store`,
`feasibility_check`), `crates/flexcore-events/src/events.rs` (vocabulary), and two grep-verified
implementation points in `crates/adapters/postgres/src/` (`command_store.rs` trait impl,
`audit_read.rs` cursor predicate) — R5 territory, touched only to confirm wiring claims.

## 2. Method

Concrete static checks run (each with its result):

1. **Route-table audit.** Grepped every `.route(` in `apps/control-plane/src` (results at
   `routes.rs:71-85`, `routes_phase2.rs:149-164`, `routes_phase3.rs:132-143`, `routes_phase4.rs:77-100`,
   `routes_live.rs:313`, `routes_alarms.rs:131`, `routes_edge.rs:387-389`, `routes_plugins.rs:418`,
   `routes_research.rs:408`). Cross-checked every registered handler name against its `fn`
   definition and its `#[utoipa::path]` annotation; cross-checked the OpenAPI `paths(...)`
   list in `openapi.rs:109-204` against the router. **Result: every route has an existing
   handler with a matching signature; no dead route, no unregistered handler, no route outside
   the recorded additive-path rule.**
2. **Composition-root wiring audit.** Read `main.rs` end to end and every `AppState` field in
   `state.rs` against its producer/consumer. **Result: 3 wiring gaps found (findings
   FRV1-R2-001/002/003); all other seams wired.**
3. **UnitOfWork port coercion check.** Verified every `UnitOfWork` field is consumed by at
   least one handler (`tenants/portfolios/sites/assets/outbox/ids/clock/schedule_records/
   schedule_revisions/envelope_records/command_records/feasibility_checks/audit_events`) and
   that the Postgres branch coerces all of them from the ONE `PgUnitOfWork` allocation
   (`state.rs:554-576`); verified `PgUnitOfWork` actually implements `CommandRecordStore`
   (`crates/adapters/postgres/src/command_store.rs:370`). **Result: consistent; the previously
   fixed `unit.command_records.find_command` defect class has no sibling — every trait call on
   the bundle resolves (`find_command`, `transition_journal`, `list_commands`,
   `insert_envelope_record`, `list_envelope_records`, `insert_schedule_record`,
   `find_schedule_record`, `list_schedule_records`, `mark_revision_superseded`,
   `insert_schedule_revision`, `revision_chain`, `list_revisions`,
   `list_feasibility_checks`, `find_feasibility_check`, `list_audit_events`,
   `find_audit_event` all exist on their storage traits with matching arity/types).**
4. **Definition-vs-call-site sweep across the area.** For each cross-module consumed item:
   `Action` (38 variants, `id`/`parse` exhaustive — `action.rs:217-308`), `Role` (5 variants),
   `Scope`/`ScopeKind`, `PolicySet::evaluate/evaluate_action_id`, `CommandState`/`try_transition`
   /`is_terminal`, `CommandStoreError`/`FeasibilityCheckStoreError`/`AuditEventReadError`
   match arms in the route error mappers, `EVENT_NOTIFICATION_MAP` vs `EVENT_DISPOSITIONS`,
   `FROZEN_EVENT_NAMES` vs `EVENT_COUNT = 26`. **Result: no name/arity/type/return-shape
   mismatch found; no import referencing a nonexistent item.**
5. **Worker command-consumer flow audit.** Traced consume → decode → tenant-context derivation
   → state read (`PgCommandStateGateway` → `find_command`) → oracle (`decide_transition` over
   `try_transition`) → ack/inbox record, plus the composition wiring in `main.rs:232-329`
   (stream + durable consumer provisioning, `validate_against_policy` alignment check,
   metric Arc sharing with the probe server, drain handling). **Result: flow complete and
   wired; the frozen-state-machine dedupe oracle matches the domain table exactly.**
6. **Frozen-invariant checks.**
   - Lifecycle: read `crates/flexcore-domain/src/command_state.rs` fully — 14 states, exactly
     the 27 frozen edges transcribed in `FROZEN_EDGES`, three gates distinct
     (PROPOSED→VALIDATED→FEASIBLE→APPROVED), FAILED/EXPIRED exit only via FALLBACK (edges
     26–27), five terminal states, acknowledgement ≠ delivery documented at the state level.
     **Intact.**
   - Never-granted set: `dispatch.authorize` / `dispatch.issue` / `secret.read` / `network.any`
     absent from `Action::parse` (`action.rs:375-378`), absent from every grant builder, pinned
     by tests (`phase13_policies.rs:213-219`, `action.rs:372-414`), quoted byte-identical in
     `phase16_market_policies.rs:43-48`. **Intact and unparseable-as-grantable.**
   - 26-event vocabulary: `EVENT_COUNT = 26` with the exact name list (`events.rs:479+`);
     `market_events_v2.rs` additive module only. Consumers (`command_state_for_event`,
     `dispatch_fact`, `notification_vocab`) cover exactly the dispatch/notification subsets and
     none invent names. **Intact.**
   - Audit chain std-only: `crates/flexcore-audit/Cargo.toml` declares `serde`,
     `flexcore-contracts`, `flexcore-domain` only — **no crypto dependency**; SHA-256/HMAC are
     hand-written in `chain.rs` anchored to FIPS 180-4 / RFC 4231 vectors; `update_evidence.rs`
     consumes the same primitives domain-separated. **Intact.**
7. **Authz gate coverage on every route.** Every mutating route (tenant/portfolio/site/asset
   POST/DELETE/PATCH/deactivate, the three candidate-submission POSTs, edge enroll, edge
   decommission) resolves identity via `CallerExtractor` and evaluates an explicit `Action` at
   an explicit scope before any state change; the recorded-drift interim gates
   (`telemetry.read` for live/edge-status/plugins/research, `asset.deactivate` for
   decommission) are all deny-by-default with named swap points. **Result: no mutating route
   without an authz check; the only gate not yet on the closed enum is a recorded drift
   (`authorize_live_stream`, `auth.rs:192-210`), not a missing check.**
8. **`unwrap()`/`expect()` sweep in non-test code.** All hits are either (a) mutex-poisoned
   guard messages, (b) startup-only (`main.rs` signal handlers, literal bind address),
   (c) guarded invariants with the guard stated at the call site (e.g. `audit_export.rs:301`
   `page.last().expect("non-empty page")` — reachable only when `page_len == EXPORT_PAGE_SIZE`),
   or (d) test fakes. **Result: no unguarded panic on a request path.** One doc-vs-code drift
   found in `metrics.rs` (FRV1-R2-005).
9. **Dead-wiring grep sweep.** For every pub recording/rendering API added by a later phase,
   grepped for call sites outside its own file and its tests:
   `record_command_state_transition`, `record_command_accept_duration_millis`,
   `record_telemetry_event`, `record_quarantine`, `record_quality_state`, `set_queue_depth`,
   `record_ingestion_delay_millis`, `record_staleness`, `set_webhook_dead_letters`,
   `IngestionMetrics::render_prometheus`, `RateLimitState::rejection_counters`,
   `route_ev_inbound`, `phase5_default_policies`, `phase13_policies`,
   `phase16_market_policies`, `phase4_command_policies`. **Result: findings FRV1-R2-001
   through FRV1-R2-004 and FRV1-R2-006/009; everything else on this list has live callers.**
10. **Cursor/pagination contract check.** Verified the keyset cursor semantics end to end for
    the audit-events listing: port contract "ordered by `(event_time, audit_id)`, strictly
    AFTER this audit_id in the listing order" (`flexcore-storage/src/audit_read.rs:292-337`)
    vs the Postgres predicate `(event_time, audit_id) > ((SELECT event_time ...), $cursor)`
    (`adapters/postgres/src/audit_read.rs:307-315`) vs the two consumers (`routes_phase4.rs:399-414`,
    `audit_export.rs:299-302`). **Result: consistent composite keyset; no skip/duplicate
    window.**
11. **Rate-limit / idempotency middleware ordering.** Verified the ServiceBuilder layer order
    in `main.rs:156-185` matches the recorded contract: metrics (outermost) → rate limit →
    idempotency → timeout → body limit; probes/`/metrics` bypass the limiter
    (`rate_limit.rs:305`); 429/5xx never stored by the idempotency cache
    (`idempotency.rs:336-363`). **Result: consistent.**

## 3. Checks performed, all consistent

The following cross-boundary checks PASSED (diligence evidence; each was executed as a
definition-vs-every-call-site comparison):

1. Every registered route → existing handler → correct request/return types (11 router files,
   47 route registrations, cross-checked against the generated-OpenAPI path list).
2. `UnitOfWork` bundle: 13 fields, all coerced from the one `PgUnitOfWork` allocation on the
   Postgres backend and all consumed by at least one handler; every storage-trait method
   called anywhere in `apps/control-plane` exists on its trait with matching arity.
3. `AppState` fields → producers/consumers (telemetry, edge_enrollment, enrollment_tokens,
   edge_status, plugins_inventory, research_status, idempotency, rate_limit, step_up, live) —
   gaps are reported as findings, not silently passed.
4. `Action::id`/`Action::parse` exhaustiveness over 38 variants; the four never-granted ids
   unparseable; every action granted by at least one grant builder.
5. Command lifecycle: 14 states / 27 edges byte-consistent across the three consumers
   (`command_state.rs` table, `command_state_for_event` (11 dispatch events),
   `COMMAND_STATES` (14) in flexcore-storage, `metrics.rs` transition counter's
   `try_transition` oracle, `notification_vocab`'s 11 transition rows).
6. Worker `PgCommandStateGateway.current_state` → `PgUnitOfWork::find_command` (trait impl
   verified at `adapters/postgres/src/command_store.rs:370`); no sibling of the previously
   fixed `unit.command_records.find_command` defect.
7. Tenant isolation on every query path: every listing/read route filters by the caller's
   `TenantContext` (store-level + RLS); cross-tenant by-id reads are 404 with no existence
   leak (`routes.rs`, `routes_phase4.rs`, `routes_edge.rs` `resolve_edge_session_scope`);
   no handler passes a request-carried tenant.
8. The three candidate-submission POSTs are exactly `COMMAND_SUBMISSION_PATHS`; both the
   P11.11 idempotency and P11.14 in-flight-cap contracts key off that closed list.
9. Audit-chain canonical encoding covers every persisted §10.1 field (`chain.rs:330-369`),
   with the correction-reason exclusion stated as a recorded limitation, and per-tenant
   namespace separation verified by the in-module tests.
10. `audit_export` pack chain: manifest signature and body-chain verification both use the
    same HMAC construction and constant-time compare; tenant scoping inherited from the port.
11. Notification interplay: `EVENT_DISPOSITIONS` (26 rows) ↔ `FROZEN_EVENT_NAMES` exact
    one-to-one, Notify rows ↔ `EVENT_NOTIFICATION_MAP` agreement enforced by
    `validate_registry()` and pinned by tests.
12. `flexcore-events` vocabulary untouched (26 events; `market_events_v2` additive only).
13. Worker topology provisioning matches the consumer policies (`STREAM_DISPATCH`,
    `dispatch-command-track-v1`, `validate_against_policy` fail-fast), and the command
    consumer drains in the same bounded shutdown window as the other loops.
14. All frozen-state metric label values flow from `CommandState::name()` only; illegal edges
    are never counted (`metrics.rs:407-427`).
15. Secrets discipline: no URL/secret value in any `Debug` impl (`config.rs`,
    `worker/config.rs`), no Authorization-header logging (`metrics.rs`, `rate_limit.rs`,
    `idempotency.rs`), no secret material in any webhook/audit type (opaque provider
    references + zeroizing key types).

## 4. Findings

### FRV1-R2-001 [P1] [phase-10] The `/api/v1/alarms` route is dead in the production composition root: the composed policy set predates the `alarm.read` grant

- Status: OPEN
- Location: `apps/control-plane/src/main.rs:144` (composition), `apps/control-plane/src/routes_alarms.rs:163` (gate), `apps/control-plane/src/auth.rs:458-479` (the composed set), `apps/control-plane/src/auth.rs:500-520` (the never-composed set)
- Issue: The composition root builds `AppState` with `phase4_default_policies()`:

  ```rust
  // main.rs:138-145
  let state = AppState::new(
      Backend::Postgres { ... },
      phase4_default_policies(),
      ...
  ```

  `phase4_default_policies` grants exactly `CommandRead`, `FeasibilityRead`,
  `AuditEventRead` (plus the cumulative P1–P3 surface). The alarms route gates on
  `Action::AlarmRead` (`routes_alarms.rs:163`:
  `authorize_tenant_scoped(&state.policies, &caller, Action::AlarmRead)?`), which is granted
  ONLY by `phase5_default_policies()` (`auth.rs:517` — `grant(role, A::AlarmRead, K::Tenant, rule, &mut p)`).
  Grep confirms `phase5_default_policies` has **no production caller** — it is referenced
  only by tests (`tests/api_edge.rs:98`, `tests/api_plugins.rs:151`, `tests/api_security_p6_23.rs:102`,
  `tests/p10_04_deny_all_and_default_policies.rs`, etc.). Consequence: in every real
  deployment (any resolver composed), **every role, including PlatformAdmin, receives 403 on
  `GET /api/v1/alarms`** — the derived-alarm surface is unreachable in production while its
  tests pass because the Test-backend fixtures compose the newer cumulative set. The recorded
  convention is that the phase policy set is cumulative ("the phase policy set is cumulative
  by construction", `auth.rs:431-433`); `main.rs` was never bumped past P4.30 when P5.31
  added `AlarmRead`.
- Recommendation: Minimal fix — compose the cumulative set in `main.rs`: change
  `phase4_default_policies()` to `phase5_default_policies()` (the superset builder, defined
  in the same file). Add one production-path test asserting the composed set allows
  `AlarmRead` for a granted role (the existing tests all build their own state, which is why
  this drifted).

### FRV1-R2-002 [P1] [phase-10] Edge enrollment seams are never composed in the production root: `/api/v1/edge/status` and `/api/v1/edge/decommission` are permanent infrastructure errors

- Status: OPEN
- Location: `apps/control-plane/src/main.rs:131-154` (AppState construction — no `with_edge_enrollment` call anywhere in the file), `apps/control-plane/src/state.rs:167-174` (`edge_enrollment: None`), `apps/control-plane/src/routes_edge.rs:885-889` and `:1007-1011` (the fail-closed `ok_or_else(ApiError::infrastructure)` arms)
- Issue: `AppState::new` initializes `edge_enrollment: None` and `enrollment_tokens:
  DenyAllEnrollmentTokens`; the composition root (`main.rs`) never calls
  `with_edge_enrollment` / `with_enrollment_tokens` (grep: the builders are called only from
  tests, e.g. `tests/api_edge.rs:117`). Meanwhile the real Postgres
  `EdgeEnrollmentStore` IS implemented on `FlexCoreStore`
  (`crates/adapters/postgres/src/edge_enrollment.rs:6` — "Implements
  `flexcore_storage::edge_enrollment::EdgeEnrollmentStore`", migration 0018). For the enroll
  route the deny-all token validator is the recorded fail-closed default (no token service in
  0.x — recorded). But `GET /api/v1/edge/status` and `POST /api/v1/edge/decommission` need
  only the STORE (the status read additionally needs no reporter — the shipped
  `NoEdgeStatusReported` default renders honest gaps by design), and both routes fail closed
  with `ApiError::infrastructure()` **unconditionally** in the shipped composition root:
  these two operator-facing surfaces can never return a successful response in any
  deployment built from this repo's `main.rs`, even though their persistence layer exists
  and their routes are complete. A port implemented but never registered in the composition
  root is exactly the "unwired seam" class; the fail-closed discipline covers *absent
  composition*, but the composition here is a one-line builder call away for the store half.
- Recommendation: In `main.rs`, compose the store:
  `let state = AppState::new(...).with_edge_enrollment(store.edge_enrollment_store())`
  (the accessor name per the adapter's implementation; keep `enrollment_tokens` deny-all —
  that half is correctly recorded). If intentionally deferred, record the deferral in
  `main.rs` next to the other recorded composition gaps (as done for `telemetry: None`) so
  the omission is visible at the composition root and not only in `state.rs` field docs.

### FRV1-R2-003 [P1] [phase-11] Nine metric families have no producer or no renderer: the control-plane registry is never driven, and the worker's ingestion metrics are never rendered

- Status: OPEN
- Location: `apps/control-plane/src/metrics.rs:304-447` (the recording APIs), `apps/worker/src/ingestion.rs:399` (`render_prometheus`), `apps/worker/src/probes.rs:127-134` (worker `/metrics` renders only `CommandConsumerMetrics`), `apps/control-plane/src/metrics.rs:445-447` (`set_webhook_dead_letters`)
- Issue: Grep over both apps shows **zero call sites outside `metrics.rs` itself** for
  `record_telemetry_event`, `record_quarantine`, `record_quality_state`, `set_queue_depth`,
  `record_ingestion_delay_millis`, `record_staleness`, `record_command_state_transition`,
  `record_command_accept_duration_millis`, and `set_webhook_dead_letters`. Consequences:
  (a) the control-plane `/metrics` renders `flexcore_telemetry_*` (6 families),
  `flexcore_command_state_transitions_total`, `flexcore_command_accept_duration_milliseconds`,
  and `flexcore_webhook_dead_letters` **permanently at zero**; (b) the actual P2.21 telemetry
  data IS recorded — in the worker's `IngestionMetrics` (`apps/worker/src/ingestion.rs:1353`,
  `:1515`, `:1666-1681`) — but `IngestionMetrics::render_prometheus` (`ingestion.rs:399`) is
  **never called by anything**: the worker's P11.21 `/metrics` route renders only
  `CommandConsumerMetrics::render()` (`probes.rs:133`), although `render_prometheus`'s own
  doc says "the worker's future /metrics surface renders these" (`ingestion.rs:396-398`) —
  that surface now exists and does not render them. Net effect: an operator scraping both
  services sees the complete P2.21/P11.01 family inventory, all zeros, while the real
  ingestion counters are collected and thrown away. The P13.09 dead-letter gauge additionally
  has no scanner wired anywhere (`count_dead_letters` exists on the store port,
  `webhook_service.rs:550`, but nothing in any composition root calls
  `set_webhook_dead_letters`). The `WRITTEN-UNRUN` labels cover "never executed"; they do
  not cover "recording API exists on one side of a process boundary and the producing side
  never calls it" — that is a wiring gap, not an unrun status.
- Recommendation: Minimal, two-sided: (1) render the ingestion families on the worker
  `/metrics` route — concatenate `command_metrics.render()` and
  `ingestion_metrics.render_prometheus()` in `probes.rs::metrics` (the Arc is already shared
  through the composition root); (2) either wire the two command-SLI recorders where a
  producer exists (none does in 0.x — the gates are driven from `apps/sim-runner`, outside
  R2 scope) or mark those two families in their HELP text as "no producer composed in 0.x"
  so a permanently-zero family is not mistaken for a healthy one. The webhook dead-letter
  scanner stays a recorded follow-up (P13.09), but the same HELP-text honesty applies.

### FRV1-R2-004 [P1] [phase-17] The EV ingest bridge has no production caller: `route_ev_inbound` is reachable only from its own tests

- Status: OPEN
- Location: `apps/worker/src/ev_ingest.rs:208` (`pub fn route_ev_inbound`), `apps/worker/src/lib.rs:95` (module declared), `apps/worker/src/main.rs` (no reference)
- Issue: Grep across `apps/` and `crates/` finds `route_ev_inbound` called only inside
  `ev_ingest.rs`'s own test module; `EvInboundFrame`, `TelemetryOffer`, and
  `SignedMeterValueSink` have no constructor or call site in any adapter
  (`crates/adapters/protocol-ocpp` contains zero references to the bridge; no adapter depends
  on `flexcore-worker`), and `apps/worker/src/main.rs` never spawns anything that consumes
  OCPP frames. The module header records that A's P17.01 adapter types are "consumed BY
  CITATION" and that "A's adapter constructs that shape at its boundary"
  (`ev_ingest.rs:32-40`) — statically, no such construction exists anywhere in the
  workspace, so the P17.06/P17.07 worker half is dead code pending an integration that no
  file owns. Not a compile defect and arguably a registered seam, but as of this tree the
  bridge is unreached in both directions, and no record pointer in `main.rs` or the OCPP
  adapter names the missing integration step.
- Recommendation: Either (a) record the missing integration explicitly at the seam — one
  comment in `apps/worker/src/main.rs` (and/or the protocol-ocpp inbound path) naming the
  follow-up that constructs `EvInboundFrame` and calls `route_ev_inbound` — or (b) defer the
  module behind the phase plan that lands the OCPP inbound path. No code change to the
  bridge logic is needed; it is internally consistent and never-auto-command by construction.

### FRV1-R2-005 [P2] [phase-11] `MetricsRegistry::leave_request` doc claims "saturating" but `fetch_sub` wraps: a lopsided decrement would render u64::MAX

- Status: OPEN
- Location: `apps/control-plane/src/metrics.rs:382-386`
- Issue: The doc comment says the in-flight gauge is "saturating — a lopsided call can never
  drive the gauge negative", but the implementation is a plain
  `self.api_in_flight_requests.fetch_sub(1, Ordering::Relaxed)`. `AtomicU64::fetch_sub`
  wraps on underflow; one unbalanced `leave_request` renders the gauge as
  `18446744073709551615` instead of clamping at zero. The in-module test never exercises the
  lopsided case (it leaves exactly the number it entered). Today `track_metrics` pairs
  enter/leave on one line so the case is unreachable, but the doc asserts a property the
  code does not have.
- Recommendation: Either use `fetch_update(.., |v| v.saturating_sub(1))` (or
  `compare_exchange`-based decrement) to make the doc true, or correct the doc comment to
  state the invariant ("callers must pair enter/leave; unbalanced calls wrap").

### FRV1-R2-006 [P2] [phase-11] `RateLimitState::rejection_counters` is rendered nowhere (dead counters), and the bucket-map eviction check runs after insertion

- Status: OPEN
- Location: `apps/control-plane/src/rate_limit.rs:231-241` (counters + "rendered nowhere yet" doc), `rate_limit.rs:198-214` (eviction)
- Issue: (a) `rejected_total` / `in_flight_rejected_total` are incremented on every 429
  (`rate_limit.rs:325-328`, `:337-341`) but `rejection_counters()` has no caller — the two
  counters are dead weight (the doc itself defers to the `flexcore_http_requests_total{status="429"}`
  series, which does cover the signal; the dedicated counters are redundant as shipped).
  (b) `try_consume` inserts the new bucket via `entry().or_insert_with(...)` BEFORE the
  `buckets.len() > MAX_TRACKED_BUCKETS` eviction check, so the map transiently holds
  MAX+1 entries and the eviction scan removes the oldest entry including possibly the
  one just inserted — harmless but off the stated bound.
- Recommendation: Either render the two counters under their own families on `/metrics`
  (closing the P11.14 handoff note) or delete them; move the size check before the
  `entry()` call (or evict unconditionally when `len() > MAX` after insert).

### FRV1-R2-007 [P2] [phase-13] `AppState.step_up` is a write-only composition hook: no route or middleware ever reads it

- Status: OPEN
- Location: `apps/control-plane/src/state.rs:214` (field), `state.rs:281-284` (builder), grep: zero readers
- Issue: `step_up: Option<Arc<StepUpService>>` is set only by `with_step_up` (never called
  in this repo — recorded) and **read by nothing**: no handler, middleware, or gate consults
  `state.step_up`, so even a deployment that composes a verifier would get no enforcement —
  the challenge service would be constructed and unused until a P13.17 consumer lands. The
  module header honestly states "no console flow consumes this yet (P13.17, wave 2)"
  (`stepup.rs:11-12`), but the state field carries no equivalent pointer, so a composor can
  believe step-up is active when it is inert.
- Recommendation: Add one doc line on the field ("consumed by nothing yet — wiring is the
  P13.17 surface; composing this alone enforces nothing") mirroring the `stepup.rs` header,
  or defer the field until the consuming surface exists.

### FRV1-R2-008 [P2] [phase-13] Webhook dispatcher never produces the `FATAL_FAILURE` attempt outcome it validates

- Status: OPEN
- Location: `apps/control-plane/src/webhook_service.rs:53-58` (vocabulary), `:670-674` (outcome selection)
- Issue: `ATTEMPT_OUTCOMES` includes `FATAL_FAILURE`, and `WebhookAttemptRecord::validate`
  accepts it, but `run_attempt` classifies every non-success as `"RETRYABLE_FAILURE"`
  (`webhook_service.rs:670-674`) — fatal classification is unreachable code in the only
  dispatcher. Failures therefore always consume the full 10-attempt schedule before
  dead-lettering. Consistent with the recorded bounded-retry design, but the vocabulary
  value is currently dead and a future fatal-class producer would be a semantic change
  worth registering.
- Recommendation: Either remove `FATAL_FAILURE` from the shipped vocabulary until a
  producer exists, or leave it with a one-line comment naming it as reserved for the
  transport seam's non-retryable classes.

### FRV1-R2-009 [P2] [phase-13] Three recorded grant sets are composed nowhere: `phase13_policies`, `phase16_market_policies`, `phase4_command_policies`

- Status: OPEN
- Location: `crates/flexcore-authz/src/phase13_policies.rs:69`, `crates/flexcore-authz/src/phase16_market_policies.rs:89`, `crates/flexcore-authz/src/phase4_policies.rs:62`; grep: no production caller for any
- Issue: The `SiteApprover` `command.approve` grants, the `apikey.*` administration grants,
  and the `market.*` grant set exist only as builders + tests. No REST surface evaluates
  `ApiKeyCreate/Rotate/Revoke/Read` or the `market.*` actions at all (no routes exist), and
  the command gates that would evaluate `command.create/validate/approve` are driven only
  from `apps/sim-runner` (outside R2 scope). This is the recorded "composition is a
  deployment act" posture and each module header says so; it is flagged here only so the
  release notes do not imply P13.08 API-key administration or the P16 market permission
  model is *enforceable* anywhere in a shipped binary — today those grants are latent
  vocabulary.
- Recommendation: No code change required for the recorded posture; ensure the release
  documentation states which grant sets have no evaluating surface in 0.x (apikey.*,
  market.*), and that the command-gate grants are exercised via sim-runner only.

### FRV1-R2-010 [P2] [phase-10] Stale doc-comment fragment on `map_stored_record` (copy-paste drift)

- Status: OPEN
- Location: `apps/control-plane/src/routes_phase2.rs:351-357`
- Issue: `map_stored_record`'s doc comment opens with the first sentence of
  `rfc3339_ingest`'s doc ("RFC 3339 rendering of an ingest timestamp (the ingest type is
  distinct from the event type, P0.09...)") before its own correct description — a
  copy-paste remnant; the same sentence correctly sits on `rfc3339_ingest` at
  `routes_phase2.rs:394-396`.
- Recommendation: Delete the stray first two lines of the doc comment.

### FRV1-R2-011 [P2] [phase-11] Idempotency middleware returns an empty body when terminal-response buffering fails

- Status: OPEN
- Location: `apps/control-plane/src/idempotency.rs:350-356`
- Issue: When `to_bytes` fails on a terminal (2xx/4xx) response, the entry is correctly
  removed but the client receives `Response::from_parts(response_parts, Body::empty())` —
  the original status code with an EMPTY body, dropping the stable error envelope (e.g. a
  bare 422). Correct-but-lossy; the cache-side bound (`MAX_IDEMPOTENT_BODY_BYTES`) equals
  the body-limit layer's cap, so the path is near-unreachable.
- Recommendation: Return `ApiError::infrastructure()` in that arm instead of an empty body,
  or document the lossy degradation in the comment.

## 5. Release-readiness verdict for this area

The control-plane/worker/authz/audit area is structurally strong: the frozen invariants this
review exists to protect are all intact — the 14-state/27-edge lifecycle with three distinct
gates is consumed verbatim everywhere (state machine, tracking-consumer oracle, metrics
oracle, notification registry), the never-granted permission set is structurally
ungrantable, the 26-event vocabulary is untouched with `market_events_v2` additive only, and
the HMAC-SHA-256 audit chain is genuinely std-only with published-vector anchors. Every one
of the 47 routes resolves to an existing, tenant-scoped, deny-by-default-authorized handler
with consistent error shaping; no compile-class (P0) cross-module mismatch exists anywhere in
the area, including the previously defective `PgUnitOfWork` port-call class, which now has
zero siblings. What must be fixed before release are the four P1 wiring gaps: the alarms
route is a guaranteed 403 in production because the composition root composes a pre-P5 policy
set (FRV1-R2-001 — one-line fix); the edge status/decommission surfaces can never succeed
because the implemented Postgres enrollment store is never composed (FRV1-R2-002); the
observability surface is misleading — nine metric families render permanent zeros while the
real ingestion counters are recorded on a renderer that nothing calls (FRV1-R2-003); and the
EV ingest bridge is unreached from both sides with no recorded pointer to the missing
integration (FRV1-R2-004). None of the P2 items blocks release: they are dead counters, an
unreachable vocabulary value, latent grant sets that are recorded composition-act posture,
doc drift, and two edge-case response degradations. Recommended gate for release: land
FRV1-R2-001 and FRV1-R2-002 (small, surgical), and either wire or honestly label the metric
families in FRV1-R2-003.
