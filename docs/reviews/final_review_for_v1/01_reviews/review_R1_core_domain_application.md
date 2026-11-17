# Final Review R1 — Core Domain, Application Services, Events, Storage (Iteration 1)

**Reviewer:** Agent A (wave 1)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 0 · P1: 4 · P2: 6

## 1. Scope reviewed

All paths relative to repo root. LOC counted with a `python -I` census
(encoding-tolerant line count).

| Crate | Files | LOC | Modules | Review depth |
|---|---|---|---|---|
| `crates/flexcore-contracts` | 8 | 3,956 | alarm, api_paths, capability, ids, quality, time_model, units | quality + units deep-read (frozen vocabulary); ids, time_model, alarm, capability, api_paths read at header/pub-surface level |
| `crates/flexcore-domain` | 37 | 31,024 | 30 modules + `test_rng` (cfg(test)) | Deep-read: command_state, command_lifecycle, tenant_context, ids, dispatch_command (field groups + accessors), ev_fleet_model (validation path), entities (constructors), registry_rules, quality_rules (pub surface). Structural skim of the remaining modules (alarm_rules, bess_*, capability_*, doe_envelope, ev_* (P17 set), flexible_load_model, ieee1547_settings, inverter_pq_model, interconnection_records, market_*, scheduling_time, settings_management, volt_var_scoping, warranty_evidence) — headers, pub surfaces, marker sweeps |
| `crates/flexcore-ports` | 15 | 6,978 | 14 modules + fakes | Trait surfaces of all 14 modules read; fakes.rs impl coverage cross-checked against every port trait; vendor-grep test table checked (15 entries = 15 source files, consistent) |
| `crates/flexcore-application` | 60 | 49,104 | 58 pub mods + private `event_assembly` | Deep-read (core): command_gates (full 3,324 lines incl. P13.06 approval engine), command_record, command_expiry, rolling_horizon, baseline_engine, scheduling_objective, event_assembly, error (taxonomy header), flexibility_envelope (aggregation core), dispatch_simulator (issue/ack/tracking path). Header-read: fallback_policy, feasibility_return. **Market/EV/forecast-specific modules (forecast_*, settlement_*, payment_lifecycle, revenue_share_billing, capacity_market_records, uk_lfm_records, jurisdiction_register*, ev_proposal_ingest, ev_meter_values_ingest, model_registry, fleet_health, telemetry_drift, rollout_orchestrator, etc.) are census-only here — they belong to review R7** |
| `crates/flexcore-events` | 6 | 2,591 | events, envelope, outbox, subject, market_events_v2 | events.rs deep-read (26-event frozen vocabulary + tests), envelope.rs and outbox.rs deep-read; subject.rs and market_events_v2.rs at surface level |
| `crates/flexcore-storage` | 13 | 6,692 | 12 modules | command_store.rs deep-read (full); lib.rs, schedule_store, schedule_revision, outbox_write, retention, unit_of_work read at surface level; all trait homes verified |

Total in-scope: 139 files, ~100,345 LOC. `crates/edge-agent` touchpoints were
not reviewed (R6); `crates/adapters/*` were read only as far as needed for the
unwired-seam sweep (§2, check 12).

## 2. Method

Static only. The concrete checks:

1. **Census.** Per-crate file/LOC/module inventory (python -I, read-only).
2. **Module-tree vs file-tree.** Every `pub mod`/`mod` declaration in each
   `lib.rs` compared against the actual `.rs` files. Result: consistent in all
   six crates (flexcore-application: 58 pub mods + private `event_assembly` +
   lib = 60 files, exact).
3. **Cross-crate import sweep.** A script extracted every `use
   flexcore_*::…` statement (including joined multi-line `use`s) and checked
   the referenced module path and final segment against the target crate's
   declared modules and pub items. All raw flags resolved to (a) macro-generated
   newtype IDs (`flexcore_domain::ids::{TenantId,…}` via the `typed_id!`
   macro — verified by reading `ids.rs`) and (b) macro/newtype-generated
   contract types (`flexcore_contracts::time_model::{EventTime, IngestTime,
   ValidUntil, ValidFrom}` via the UTC-millis newtype macro — verified by
   grep). **No unresolved import found.**
4. **Trait surface vs impl coverage.** Every port trait method list
   (repository×4, event_outbox, clock, id_generator, telemetry_source,
   telemetry_ingest×2, telemetry_read, tenant_context_source, weather_data,
   price_signal) compared against the implementations in
   `flexcore-ports/src/fakes.rs`. All trait methods have matching impls
   (arity and shape); `TelemetrySource::source_id` is a defaulted method.
5. **Trait home audit.** Grepped the whole repo for `impl … for` on every
   storage trait (`CommandRecordStore`, `ScheduleRecordStore`,
   `ScheduleRevisionChainStore`, `ScheduleComparisonStore`,
   `EnvelopeRecordStore`, `FeasibilityCheckStore`, `AuditEventReader`,
   `InterconnectionRecordStore`, `EdgeEnrollmentStore`, `MarketRecordsStore`)
   and every port trait. Results in §4 (unwired seams).
6. **Frozen lifecycle oracle diff.** `command_state::try_transition` edges
   re-derived by hand against the 27-row frozen table in the test module
   (`FROZEN_EDGES`): 14 states, 27 edges, terminal set {COMPLETED, REJECTED,
   CANCELLED, SUPERSEDED, FALLBACK}, FAILED/EXPIRED → FALLBACK only,
   three distinct gates as separate transition functions in
   `command_lifecycle.rs`. **Consistent** with the P0.11/P0.14 freeze and the
   AGENTS.md §4 invariant.
7. **Storage vocabulary mirror.** `COMMAND_STATES` (14),
   `COMMAND_GATES` (3), `COMMAND_STATES_REQUIRING_EVIDENCE/REASON/GATE`
   in `flexcore-storage/src/command_store.rs` diffed against
   `CommandState::ALL`/`Gate::as_str` and against the domain engine's
   evidence/reason/gate behaviour (`record_failure` records reason-only,
   `record_issue` evidence-only, etc.). **Consistent**; a crate test pins the
   state list.
8. **Roll-trigger string mirror.** `SCHEDULE_ROLL_TRIGGERS` (storage) vs
   `SCHEDULE_ROLL_TRIGGER_STRINGS` (application) — six strings, identical
   spellings and order.
9. **Event vocabulary.** `DomainEvent` (26 variants), `FROZEN_EVENT_NAMES`,
   `EVENT_COUNT`, no `DispatchValidated`/`DispatchFeasible` variant (the
   recorded §15.3 gap is stated, not silently filled); dispatch payloads
   carry `command_id`; the quality field carries the frozen six-state
   vocabulary; market v2 events live in a separately tagged enum and do not
   touch `events.rs`. **Consistent** with the freeze.
10. **Quality vocabulary.** `TelemetryQuality` has exactly six variants with
    canonical SCREAMING_SNAKE text; `SourceQualityState` mapping
    (Substituted→ESTIMATED, OutOfRange/Unsynchronized/Unreliable→SUSPECT,
    CommunicationLoss→STALE, no Good conversion) matches the P0.08
    non-silencing rule; domain/application usage census shows only the six
    frozen variants in use.
11. **Units.** `StateOfCharge` fraction [0,1] with separate explicit
    `from_percent`; canonical MW/MVAr/MVA/MWh newtypes; `P > 0 = export`
    encoded as a single-variant `ActivePowerSignConvention::ExportPositive`
    on every command and rendered into the §14.3 flat record
    (`render_target`); rolling-horizon carried SOC validated to [0,1].
    **Consistent.**
12. **Unwired-seam sweep (whole repo).** `impl … for` grep across
    `crates/`: Postgres adapter implements all ten storage traits on
    `PgUnitOfWork`; port traits implemented by fakes + Postgres (repos,
    outbox, clock, id generator) + simulator (`TelemetrySource`) — see
    findings FRV1-R1-003/004 for what is missing.
13. **Non-test `unwrap`/`expect` sweep.** Script located every
    `.unwrap()`/`.expect()` outside `#[cfg(test)]` regions in all six
    crates (~60 hits). Each inspected: nearly all are guarded invariants
    with justifying messages (e.g. `dispatch_simulator.rs:1333` "checked
    above", `ev_fleet_model.rs:1025` after the `NoVehiclesDeclared` guard,
    `capacity_market_records.rs` expects over checked-in constant data).
    The residue is reported as findings (P2 panic-path finding).
14. **Marker sweep.** `TODO`/`FIXME`/`todo!`/`unimplemented!`: **zero
    hits** in the six in-scope crates. `DEFERRED`/`NOT implemented` hits
    are all recorded honesty statements (SoH estimation deferral, rainflow
    seam, mTLS deployment requirement) — by design, not findings.
15. **Composition-root sweep.** `CommandGates`, `CommandExpiryService`,
    `DispatchSimulator*`, `ApprovalWorkflowEngine`, `FallbackEngine` are
    constructed only inside `flexcore-application`'s own tests — no
    production composition root exists (see FRV1-R1-004).

## 3. Checks performed, all consistent

1. `mod`/`pub use` trees vs actual files — all six crates, exact match.
2. Cross-crate `use` resolution — clean (macro-generated IDs accounted for).
3. 14-state/27-edge frozen table vs `try_transition` — exact, including
   terminal set, CANCELLED reachable only from APPROVED/ISSUED, SUPERSEDED
   from exactly six states, EXPIRED from exactly four, REJECTED from exactly
   the three gates.
4. Three-gate separation — structural in the domain engine
   (`record_validation/record_feasibility/record_authorization`, each
   state-pinned) and re-enforced in the application gate
   (`CommandGates::record_command_approval` is the only APPROVED producer;
   `journal_evidence` pulls covered validation + feasibility evidence from
   the journal, not caller input).
5. Storage `COMMAND_STATES`/gates/evidence/reason rules vs domain engine
   behaviour — consistent; also matches `command_record.rs
   ::replay_full_lifecycle`'s per-edge replay (reason vs evidence per
   destination state).
6. `replay_full_lifecycle` covers every one of the 27 frozen edges and
   fails closed on anything else — enumerated all 27 match arms; the
   fallback arm maps FAILED/EXPIRED→FALLBACK; SUPERSEDED parses the
   `command:<uuid>` backward link.
7. Command-store `ensure_schedule_revision_usable` ↔ command-gate
   `schedule_revision_state` seam — the seam doc mandates delegation to the
   storage check; the `ScheduleRevisionUsability` vocabulary (Active/
   Superseded/Unknown) matches the two storage error variants plus Ok.
8. §14.3 flat record (`S143_FIELD_GROUPS`, 17 groups) vs `DispatchCommand`
   accessors used in `from_command` — every accessor exists; field names
   match the table.
9. Frozen event vocabulary (26) vs `event_name`, serde tag rendering, and
   the emit sites in command_gates/command_expiry/dispatch_simulator —
   only existing frozen variants are constructed; payloads match the
   `dispatch_payload!` shape (identity + time only; actor/reason stay in
   the journal, the recorded gap is stated in three module headers).
10. Idempotency-key derivation — `event_assembly::idempotency_key` =
    `<snake_case frozen name>:<correlation>`; the two-events-on-one-receipt
    pairings in `reject_through_feasibility_gate` use distinct event names,
    so keys stay unique per event instance.
11. `TenantContext` construction — single path `from_authenticated_claims`,
    no `From`/`Default`, private fields; ports' `TenantContextSource`
    documents the not-yet-authenticated stand-in honestly.
12. SOC/units discipline — [0,1] fraction at every API boundary touched in
    scope (contracts `StateOfCharge`, rolling-horizon carried SOC, dispatch
    SOC targets, §14.3 rendering "SOC fraction … [0,1]").
13. Solver-class vocabulary — no SOCP/MISOCP claim anywhere in the six
    crates; degradation cost curves are linear/piecewise-linear INPUT DATA
    with explicit non-convex refusal (`validate_convex_for_minimization`)
    and the ADR-010 statement carried in the provenance record; the
    objective module labels everything SYNTHETIC.
14. `flexcore-ports` vendor-grep test table — 15 `include_str!` entries for
    15 source files (length matches; the earlier recorded mismatch is fixed).
15. Fakes vs port traits — full method coverage, arity checked by eye
    against each trait definition.

## 4. Findings

### FRV1-R1-001 [P1] [phase-03] Mid-interval execution boundary yields a RollPlan whose projection can never succeed

- Status: OPEN
- Location: crates/flexcore-application/src/rolling_horizon.rs:675-683, 743, 916-940
- Issue: `plan_rolling_horizon_roll` freezes intervals with
  `interval_start < executed_until` (start-inclusive, D-P3.24-3) and sets
  `reoptimize_start = new_horizon_start.max(executed_until)` (line 743).
  For a mid-horizon trigger there is **no check** that `executed_until`
  lies on the interval grid (the alignment check at lines 729-733 exists
  only for `HorizonAdvance`). If the boundary falls strictly inside an
  interval — the exact case the module doc says the convention supports
  ("the remaining part of the in-progress interval is covered by the next
  roll", lines 30-33) — then `project_rolling_schedule` requires the first
  future interval to start exactly at `cursor = executed_until`
  (lines 920-931) **and** to satisfy
  `(start − new_horizon_start) % interval_millis == 0` (lines 883-890).
  Both cannot hold at once, so every projection fails with `ProjectionGap`
  ("expected the next future interval at <executed_until>"). The plan is
  accepted and the projection is unsatisfiable — a validated input the
  documented behaviour cannot consume.
- Recommendation: either reject a mid-interval boundary at plan time
  (`MisalignedInterval`) or set `reoptimize_start` to the next grid
  boundary at/after `executed_until` (the in-progress interval is already
  frozen, so the gap between the boundary and the next boundary is
  intentionally realised history — but the doc must then say so).
  Minimal fix: one alignment guard in the mid-horizon branch.

### FRV1-R1-002 [P1] [phase-04] `CommandExpiryService::run_sweep` stamps every swept command's events with ONE caller-supplied `EventScope`

- Status: OPEN
- Location: crates/flexcore-application/src/command_expiry.rs:460-474 (with
  419-452 and 118-126)
- Issue: `EventScope` carries the portfolio/site/asset identity of ONE
  command ("a wrong scope on an event would break tenant routing",
  D-P4.15-4, line 57-61). `apply_expiry` correctly takes it per call, but
  `run_sweep` takes a single `scope: EventScope` and applies it to **every**
  decision in the batch (`self.apply_expiry(ctx, scope, decision.command_id,
  actor)?`, line 471). `ExpiryCandidate` (lines 227-234) carries no scope,
  so a heterogeneous batch — the normal case for a validity-window sweep —
  emits `DispatchExpired` events whose portfolio/site/asset IDs belong to
  some other command. The module's own honesty rule refuses to guess a
  scope the caller cannot read; the batch API silently reuses one scope for
  many commands, which is worse.
- Recommendation: carry the scope on the candidate
  (`ExpiryCandidate { …, scope: EventScope }`) or change `run_sweep` to
  accept `&[(ExpiryCandidate, EventScope)]`; alternatively return the
  decisions and let the caller apply per command (the pure sweep already
  returns `Vec<ExpiryDecision>`).

### FRV1-R1-003 [P1] [phase-15/phase-02] `WeatherDataProvider` and `PriceSignalSource` ports have no implementation anywhere; `TelemetryIngestStore`/`TelemetryReader` have no real adapter

- Status: OPEN
- Location: crates/flexcore-ports/src/weather_data.rs:267; crates/flexcore-ports/src/price_signal.rs:317; crates/flexcore-ports/src/telemetry_ingest.rs:163; crates/flexcore-ports/src/telemetry_read.rs:410; crates/adapters/postgres/src/telemetry.rs:16-60
- Issue: Repo-wide `impl … for` grep: `WeatherDataProvider` (P15.03) and
  `PriceSignalSource` (P15.04) have **zero** implementations — no adapter,
  no fake, no simulator stand-in; the P15.03/04 ingestion surfaces are
  currently unreachable from any composition. For the telemetry ports,
  the Postgres adapter implements the equivalent operations as inherent
  `PgUnitOfWork` methods (`insert_telemetry`, `insert_telemetry_batch`,
  `query_telemetry`, `latest_telemetry_per_asset`, `quarantine_telemetry`,
  `list_telemetry_quarantine`) over a crate-local
  `NormalizedTelemetryRecord` mirror, explicitly noting the port contract
  "is being implemented concurrently" — so `TelemetryIngestStore` and
  `TelemetryReader` are implemented only by the test fakes
  (`flexcore-ports/src/fakes.rs:937,1056`). The telemetry gap is recorded
  in the ports lib.rs ("the recorded P2.06-adjacent gap") — honest, but it
  remains an open seam at release time.
- Recommendation: before the first public release, either land the
  `TelemetryIngestStore`/`TelemetryReader` impls on `PgUnitOfWork`
  (the operations already exist — the work is the trait mapping and
  dropping the crate-local record mirror) and add at minimum test fakes
  for the two Phase-15 ports, or re-record the gaps with their current
  status in the release notes so the unwired seams are visible.

### FRV1-R1-004 [P1] [phase-04] The P4.11–P4.18 command core is constructed by no composition root — test-only wiring

- Status: OPEN
- Location: crates/flexcore-application/src/command_gates.rs:957 (`CommandGates`), command_expiry.rs:287 (`CommandExpiryService`), dispatch_simulator.rs, fallback_policy.rs (`FallbackEngine`), command_gates.rs:2395 (`ApprovalWorkflowEngine`)
- Issue: Repo-wide grep: `CommandGates {`, `CommandExpiryService`,
  `DispatchSimulator*`, `ApprovalWorkflowEngine`, `FallbackEngine` are
  constructed only inside `crates/flexcore-application/tests/*`. No
  composition root exists in `crates/edge-agent`, `crates/flexcore-simulator`,
  or `crates/adapters` — i.e. the entire protected command path
  (creation → validation → feasibility → authorization → issue → ack →
  tracking → expiry/cancel/supersede) is exercised exclusively by tests.
  The six injected seams of `CommandGates` (store, envelopes, models,
  assets, feasibility runner, authz evaluator) therefore have no
  production implementations anywhere either (the seams are trait-only by
  design). This is consistent with the phased delivery plan (the control-plane
  API/server is a later phase), but for a "first public release" claim the
  core services shipping test-only must be stated, not implied.
- Recommendation: record the composition-root plan for the 0.x release
  explicitly (which binary will construct these, or that v0.x exposes
  none of them), so the release does not imply an operable command path.

### FRV1-R1-005 [P2] [phase-15] `flexcore-domain` lib.rs: the doc comment above `pub mod bess_model;` describes the wrong module

- Status: OPEN
- Location: crates/flexcore-domain/src/lib.rs:38-44
- Issue: The comment block above `pub mod bess_model;` reads
  "P15.09 — BMS-reported SoH ingestion vocabulary (additive): provenance and
  staleness rules … platform-side SoH estimation is DEFERRED" — a verbatim
  duplicate of the `bess_soh` comment below it. `bess_model.rs` is actually
  the P3.05/P3.06/P3.07 BESS capability/degradation/reserve model (its own
  header, lines 1-8, is correct). Comment drift only; the module list and
  code are unaffected.
- Recommendation: replace the `bess_model` comment with the P3.05/P3.06/
  P3.07 one-liner (matching the module header).

### FRV1-R1-006 [P2] [phase-06/P6.06+P15.15+P16.01] `flexcore-storage` lib.rs module table is missing three modules

- Status: OPEN
- Location: crates/flexcore-storage/src/lib.rs:15-28 (the `| Module | Content |` table)
- Issue: The crate-level doc table lists 9 modules; `edge_enrollment`
  (P6.06), `interconnection_store` (P15.15) and `market_records_store`
  (P16.01) were added later and are documented only in per-`pub mod`
  comments (lines 61-108), not in the table. Everything else in the crate
  follows the table-plus-comment pattern, so the table is now incomplete.
- Recommendation: add the three rows to the table (one line each; the
  per-mod comments already carry the text).

### FRV1-R1-007 [P2] [phase-04] `CommandGateStoreError` is documented as a 1:1 mapping of `CommandStoreError`, but the mapping cannot be 1:1

- Status: OPEN
- Location: crates/flexcore-application/src/command_gates.rs:199-200 ("The composition root implements it over `flexcore_storage::command_store::CommandRecordStore` (the error vocabulary maps 1:1; see [`CommandGateStoreError`]"), 290-309; crates/flexcore-storage/src/command_store.rs:141-191
- Issue: `CommandStoreError` has 8 variants; `CommandGateStoreError` has 6.
  `IllegalTransition` and `InvalidRecord` have no seam counterpart (the
  seam's `journal_transition` takes the domain `CommandTransition`, whose
  conversion to `CommandTransitionRecord` can fail with `InvalidRecord`,
  and the caller of the seam has no way to express it except as
  `Backend`), and `TransitionConflict` drops `actual_state`. The composition
  root can map these only approximately, contradicting "maps 1:1".
- Recommendation: reword the doc to name the actual mapping (e.g.
  `IllegalTransition`/`InvalidRecord` → `Backend` fail-closed, with the
  recorded code), or add the missing variants to the seam error.

### FRV1-R1-008 [P2] [phase-04] `CommandRecordFields::validate_common`: always-empty `if` block in the supersedes group

- Status: OPEN
- Location: crates/flexcore-application/src/command_record.rs:540-549
- Issue: The group-17 check is an `if self.supersedes_command_id.is_none()
  && self.superseded_by_command_id.is_none() { /* comments only */ }` — the
  branch never returns or errors; the comment explains both-absent is legal.
  The code reads as an unfinished check and a reader can reasonably assume
  validation happens here when it does not.
- Recommendation: replace the empty `if` with a plain comment above the
  group (or a `debug_assert!`-free comment block), keeping the explanation
  without the dead branch.

### FRV1-R1-009 [P2] [phase-16] `BaselineEngineError::EmptyField` is used for a non-empty-field violation

- Status: OPEN
- Location: crates/flexcore-application/src/baseline_engine.rs:204-208
- Issue: `BaselineGapRecord::new` rejects `recorded_at_unix_ms < 0` with
  `BaselineEngineError::EmptyField { field: "recorded_at_unix_ms" }` — a
  negative timestamp is not an empty field. Callers matching on the variant
  (and the rendered message "must be non-empty") will mis-describe the
  failure. Minor naming/taxonomy drift in an otherwise clean error catalog.
- Recommendation: add a `NegativeTimestamp { field }` variant or reuse an
  `InvalidField`-style variant.

### FRV1-R1-010 [P2] [phase-03/phase-04] Panics on fallible paths in non-test library code

- Status: OPEN
- Location: crates/flexcore-application/src/scheduling_objective.rs:582-583, 648; crates/flexcore-application/src/dispatch_security.rs:501, 536; crates/flexcore-application/src/rollout_orchestrator.rs:575, 584
- Issue: `SyntheticPriceCurve::v1_default` and
  `SyntheticSeries::v1_default_flat_zero` `assert!` on zero interval
  count/seconds and panic on caller error, while the same module's
  validation path has named errors for exactly these conditions
  (`NonPositiveIntervalSeconds`, `EmptySeries`). The mutex-lock
  `lock().expect("poisoned …")` paths in `dispatch_security` and
  `rollout_orchestrator` panic on poisoning. All are low-risk and
  message-justified, but they are the only panic-on-input paths found in
  the six crates' non-test code (check 13 of §2).
- Recommendation: for the two `v1_default` constructors, either return
  `Result` (the error variants already exist) or document the panic
  contract on the signature; the mutex expects are conventional and may
  stand, but a one-line note in the crate README would make the policy
  explicit.

## 5. Release-readiness verdict for this area

The core layers are in unusually strong shape for a first public release.
The frozen invariants are not merely present but structurally enforced: the
14-state/27-edge command lifecycle is a code oracle with exhaustive negative
tests and is re-validated at the storage row level; the three gates are three
separate functions with separate evidence surfaces; the six-state quality
vocabulary and the canonical units appear everywhere they should, with no
silent conversions; the honesty vocabulary (DEFERRED, ASSUMPTION, recorded
gap notes, SYNTHETIC labels, test-grade digests) is consistent and pervasive;
and no solver-class, protection-grade, or certification claim anywhere
contradicts the baseline. Before release, the P1 items need attention: the
rolling-horizon mid-interval boundary makes a documented scenario
uncomputable (a real logic gap, not polish), the expiry sweep's single-scope
batch API can stamp wrong scope IDs on lifecycle events, the Phase-15 ports
and the telemetry port pair have no real implementations, and the command
core ships test-only with no composition root — all four are either small
fixes or recording obligations, and none undermines the frozen invariants.
The P2 items are documentation drift and minor taxonomy polish, safe to
batch into a single cleanup pass. Market/EV/forecast application modules
were intentionally left to review R7.
