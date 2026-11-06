# Ledger — Phase 2: Asset and Telemetry Foundation

| Field | Value |
|---|---|
| Phase | Phase 2 — Report Stage 2: Asset and telemetry foundation |
| Report trace | §23 Stage 2; §10–§11; §15; §19; §4 (Asset Registry, Telemetry); §6.5; §16.1–§16.3; §24 steps 1–2, 10 |
| Status | Not started — planning ledger only; no code written by this ledger |
| Simulation status | Entire 0.x line is simulation-only. Phase 2 ingests simulated telemetry only; it adds no device writes, no field reads, and no control authority above Simulation (§21.4). The simulator telemetry source is a stand-in for future site adapters, not a site integration |
| Authority | Report v1.1 is the product baseline; `docs/foundations/` v1.0 governs where the two conflict. Phase 0 freeze record and Phase 1 foundation are the input contract sets |
| Spelling | "license" (US spelling); "acknowledgement" (noun) — per `.agent/WRITING.md` |

## How to use this ledger

- Subtask IDs `P2.01`…; every subtask traces to a report section, has inputs,
  steps, a concrete deliverable, testable acceptance criteria, an owner role,
  dependencies, a verification method, and risks.
- Phase 0 contracts are cited by ID (P0.07 units, P0.08 quality, P0.09 time,
  P0.10 asset vocabulary, P0.14 events). Phase 1 artefacts are cited by ID
  (P1.05 contracts crate, P1.06 domain, P1.12–P1.13 schema/adapter, P1.14–P1.16
  JetStream/outbox/inbox, P1.17 API, P1.25 Compose). Do not re-define them.
- The Stage 2 exit gate (§23): *synthetic BESS/PV/load/EVSE portfolio
  telemetry is ingested, normalized, queried, and replayed deterministically.*
  Every subtask serves that gate or the handover to Stage 3.
- Determinism is a first-class requirement: same scenario + same versions +
  same seed = same stored telemetry and same replay output (P2.24, P2.26).

## P2.01 — Confirm Phase 1 exit gate passed and handover pack received

- **Objective:** Prove Phase 2 starts from a working foundation, not from an unfinished one.
- **Report trace:** §23 Stage 1 → Stage 2; P1.29, P1.31
- **Inputs / prerequisites:** Phase 1 handover pack (P1.29); Stage 1 exit-gate review record (P1.31); P1.26 demonstration record.
- **Steps:**
  1. Read the Stage 1 exit-gate record; confirm pass or pass-with-conditions, and list conditions.
  2. Verify the handover artefacts exist: workspace + toolchain pins, crates (contracts/domain/ports/application/events/storage/audit/authz), Postgres adapter + migrations, NATS adapter + outbox publisher, idempotent consumers, API + OpenAPI, worker, Compose stack, CI.
  3. Re-run the P1.26 end-to-end workflow (tenant → portfolio → site → asset) on a clean Compose stack; confirm it still passes.
  4. Record the implemented-API-subset gap from P1.29 (only tenants/portfolios/sites/assets routes exist) and the Phase 2 routes that must be added (P2.12).
  5. If the gate did not pass, stop: Phase 2 does not start.
- **Deliverable / output:** Phase 2 entry note: gate outcome, conditions, handover inventory, P1.26 re-run result.
- **Acceptance criteria / exit check:**
  - [ ] Stage 1 gate outcome is pass or pass-with-conditions, with the record cited.
  - [ ] P1.26 workflow re-runs green on a clean stack.
  - [ ] The implemented-API-subset gap is recorded.
- **Owner role:** Principal engineering
- **Dependencies:** P1.29, P1.31 (Phase 1)
- **Verification method:** Handover walk-through + P1.26 re-run output.
- **Risks / failure modes:** Building the registry on a foundation whose exit gate never passed. Mitigation: hard entry gate — this subtask.

## P2.02 — Design the registry domain model (tenant/portfolio/site/asset)

- **Objective:** Design the registry entities, relationships, and invariants from the Phase 0 vocabulary before writing schema or code.
- **Report trace:** §10.1, §4 (Portfolio & Sites, Asset Registry), §1.4; P0.10
- **Inputs / prerequisites:** P2.01; P0.10; P1.06 (domain entities for Tenant/Portfolio/Site/Asset already exist in skeleton form).
- **Steps:**
  1. Define the entity graph: Tenant 1—* Portfolio; Portfolio 1—* Site; Site 1—* Asset; Site 1—1 PCC (point of common coupling, §10.1); Site *—1 GridModel reference (model + version); Asset *—1 bus reference.
  2. Define Asset fields (§10.1): `asset_id`, site, type, vendor/model, rated power/energy, phase, bus reference, capability profile. Define the asset-type enum from §2.2: BESS, PV inverter, flexible C&I load, EVSE/EV fleet, site meter, wind (generic interface), thermal storage/heat pump (generic envelope). Backup generation is NOT a first-class type — record its exclusion.
  3. Define Site fields (§10.1): `site_id`, portfolio, location, timezone, PCC, grid-model reference, edge identity (edge identity is a reference only in Phase 2 — no edge agent exists; record that the field is reserved, not populated by a live enrollment).
  4. Define Portfolio fields (§10.1): `portfolio_id`, tenant, operator, market region, aggregation rules. Record that market region is a label in Phase 2 (D1 open) and must not drive market logic.
  5. Define registry invariants: asset IDs unique within a tenant; site belongs to exactly one portfolio; portfolio belongs to exactly one tenant; deleting a tenant/portfolio/site follows a recorded rule (cascade vs restrict — decide once, record it; audit trail preserved either way).
  6. Define the capability-profile reference: each asset carries a capability-profile reference (P0.10 vocabulary); the profile's contents are implemented in Phase 3 — Phase 2 stores the reference and the asset's rated power/energy only.
- **Deliverable / output:** Registry domain-model design document (entity graph, fields, invariants, deletion rule).
- **Acceptance criteria / exit check:**
  - [ ] Entity graph covers Tenant/Portfolio/Site/Asset/PCC/GridModel reference with cardinalities.
  - [ ] Asset-type enum matches §2.2; backup generation excluded with the reason recorded.
  - [ ] Edge identity is marked reserved/not populated; market region is marked a label, not logic.
  - [ ] Deletion rule decided and recorded.
  - [ ] Capability profile is a reference only in Phase 2.
- **Owner role:** Power systems + platform engineering (Rust)
- **Dependencies:** P2.01
- **Verification method:** Design review against §10.1, §2.2, P0.10.
- **Risks / failure modes:** Market region driving logic before D1 closes; edge identity faked as a live enrollment. Mitigation: explicit "label/reserved" markings in the design.

## P2.03 — Implement the registry in the domain and application layers

- **Objective:** Implement registry use cases (register/update/deactivate tenant, portfolio, site, asset) on the Phase 1 foundation.
- **Report trace:** §4 (Asset Registry, Portfolio & Sites), §6.1–§6.3, §3.4 (workflow step 1)
- **Inputs / prerequisites:** P2.02; P1.06–P1.08 (domain, ports, application).
- **Steps:**
  1. Extend `flexcore-domain` with the registry aggregates and invariants from P2.02 (asset-type enum, PCC, grid-model reference, capability-profile reference, deletion rule).
  2. Extend `flexcore-application` with use cases: register tenant/portfolio/site/asset; update asset metadata (vendor/model, rated power/energy, capability-profile reference); deactivate asset (with reason + audit); query registry (tenant-scoped).
  3. Emit the §15.3 events on each registry change: `AssetRegistered`, `AssetCapabilityUpdated`, `AssetAvailabilityChanged` — via the outbox (P1.13/P1.15 path), never by direct NATS publish from the use case.
  4. Enforce tenant scoping: every use case requires a `TenantContext` (P1.11); cross-tenant registry access is rejected.
  5. Add tests (fake ports): happy path per use case; duplicate asset ID rejected; asset registered under a non-existent site rejected; cross-tenant access rejected; deactivation emits the event + audit record.
- **Deliverable / output:** Registry domain + application implementation with tests.
- **Acceptance criteria / exit check:**
  - [ ] `cargo test -p flexcore-domain -p flexcore-application` passes.
  - [ ] All three registry events are emitted via the outbox path (test asserts outbox records, not NATS calls).
  - [ ] Duplicate asset ID, missing-site, and cross-tenant cases are rejected in tests.
  - [ ] No SQLx/NATS/Axum import in domain/application crates (P1.04 check still passes).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.02
- **Verification method:** `cargo test`, P1.04 dependency check, outbox assertions.
- **Risks / failure modes:** Registry use case publishing directly to NATS (bypassing the outbox); tenant scoping enforced only at the API layer. Mitigation: outbox-only emission + application-layer tenant tests.

## P2.04 — Design the canonical telemetry model and units

- **Objective:** Design the canonical telemetry record: fields, units, quality, time, source identity, sequence.
- **Report trace:** §10.2–§10.4, §3.4 (workflow steps 3–4), §10.1 (AssetState); P0.07–P0.09
- **Inputs / prerequisites:** P2.02; P0.07–P0.09; P1.05 (contracts crate types).
- **Steps:**
  1. Define the telemetry record fields: `telemetry_id` (or composite key — decide once, record it), `tenant_id`, `asset_id` (and site/portfolio denormalized or joined — decide once, record it), `event_time`, `ingest_time`, `source_sequence`, `source_id` (source identity), `quality` (exactly one of the six P0.08 states), `source_quality_code` (original source code as diagnostic metadata), `value` + `unit` (canonical MW/MVAr/MVA/MWh per P0.07; SOC as fraction [0,1]), `clock_quality` (synchronisation status if available).
  2. Define the measured quantities per asset type: BESS (P, Q, SOC, availability, operating mode, health); PV (P, Q, availability); flexible load (P, baseline reference where applicable); EVSE (P, connection state, SOC/energy where applicable); site meter (P/Q at PCC). Each quantity names its canonical unit.
  3. State the unit rule: adapters (including the simulator source, P2.14) convert to canonical units at the boundary and record the unit + sign-mapping profile; persisted values use canonical units only.
  4. State the sign rule: P > 0 = export at the PCC; Q per the declared convention carried on the record (or the asset's profile reference).
  5. State the time rule: `event_time` is when the measurement occurred; `ingest_time` is when FlexCore received it; UTC internally; site timezone retained for display only.
  6. State the identity rule: every record carries source identity and source sequence (P2.09 handles duplicates/out-of-order).
- **Deliverable / output:** Canonical telemetry model design document (record fields, per-asset quantities, units, rules).
- **Acceptance criteria / exit check:**
  - [ ] Every field from step 1 is defined with type and nullability; no field is left "TBD".
  - [ ] Every per-asset quantity names its canonical unit; no kW/kWh in any persisted field.
  - [ ] P > 0 = export and the per-profile Q convention are stated.
  - [ ] `event_time` vs `ingest_time` distinguished; UTC-internal stated.
  - [ ] Source identity + source sequence are mandatory fields.
- **Owner role:** Power systems + platform engineering (Rust)
- **Dependencies:** P2.02
- **Verification method:** Design review against §10.2–§10.4 and P0.07–P0.09.
- **Risks / failure modes:** kW persisted as canonical; Q sign assumed; event/ingest time conflated. Mitigation: design review + P2.06 contract tests.

## P2.05 — Design the telemetry storage: partitioned tables, indexes, retention

- **Objective:** Design the telemetry tables as PostgreSQL partitioned tables with indexes and a retention mechanism.
- **Report trace:** §16.1 (partitioned telemetry tables), §16.3 (retention), §6.5 (time-series tables); `RESEARCH.md` (partitioning)
- **Inputs / prerequisites:** P2.04; P1.12 (schema discipline); `RESEARCH.md`.
- **Steps:**
  1. Design the telemetry table(s): one partitioned table for normalized telemetry (decide: one table for all quantities vs one per quantity — decide once, record the rationale; the default recommendation is one table with a `quantity` column, partitioned by `event_time` range).
  2. Specify partitioning: `PARTITION BY RANGE (event_time)`; partition interval (daily vs weekly vs monthly — decide from the expected synthetic volume in P2.14 and record the reasoning; monthly is the default for the synthetic portfolio, with a note that production volume would revisit it). Record the honesty note from `RESEARCH.md` §1: partitioning pays only when a table would otherwise be very large, and the Phase 2 synthetic size does not justify it on measured volume — the partitioned shape is designed for the volume trajectory (§16.1), not claimed as a measured win.
  3. Specify the primary key: it must include the partition key (`event_time`) — e.g. `(telemetry_id, event_time)` or `(asset_id, event_time, source_sequence)` — decide once, record it.
  4. Specify indexes: `(asset_id, event_time DESC)` for current-state/history queries; `(tenant_id, asset_id, event_time DESC)` for tenant-scoped queries; a partial index for the latest value per asset if the current-state view (P2.18) needs it — decide once.
  5. Specify partition management: pre-create partitions ahead of the ingest frontier (PostgreSQL does not auto-create them); a maintenance job/script creates the next partitions; rows outside all partitions error — the ingestion path must handle that error (P2.10).
  6. Specify retention: retention is configuration per data class (§16.3), implemented as detach/drop of whole partitions; no universal retention period in code; the Phase 2 default configuration value (if any) is recorded as configuration, not schema, with its source.
  7. Specify RLS: `tenant_id` on every row; RLS policy per P1.12 discipline; the ingestion role and the query role are distinct and neither owns the tables.
  8. Record the TimescaleDB decision: not used in Phase 2 (optional deployment/profile only, §16.1); revisit only on measured volume (P2.26 performance note).
- **Deliverable / output:** Telemetry storage design document (tables, partitioning, keys, indexes, partition management, retention, RLS).
- **Acceptance criteria / exit check:**
  - [ ] Partitioning method, interval, and rationale are recorded.
  - [ ] The honesty note is recorded: partitioning is the designed shape for the volume trajectory, not a measured Phase 2 win (synthetic size does not justify it on its own).
  - [ ] Primary key includes the partition key.
  - [ ] Indexes cover the P2.18/P2.19 query patterns (current-state + history).
  - [ ] Partition pre-creation and the out-of-range error path are specified.
  - [ ] Retention is configuration, not schema; no universal period hard-coded.
  - [ ] TimescaleDB non-use is recorded with the revisit trigger (measured volume).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.04
- **Verification method:** Design review against §16.1/§16.3 and the PostgreSQL partitioning docs; `EXPLAIN` review of the planned queries (P2.19).
- **Risks / failure modes:** Primary key without the partition key (PostgreSQL rejects it); no pre-creation (ingestion errors at month boundaries); retention hard-coded. Mitigation: design review + P2.06 migration tests.

## P2.06 — Implement telemetry migrations, repositories, and RLS

- **Objective:** Implement the telemetry tables, repositories, and RLS on the Phase 1 Postgres adapter.
- **Report trace:** §16.1–§16.2, §6.3; P2.05, P1.13
- **Inputs / prerequisites:** P2.05; P1.13.
- **Steps:**
  1. Write the migrations: extend the Phase 1 registry tables (tenants/portfolios/sites/assets, created by P1.12/P1.13) — do not recreate them — adding the asset-type enum, the capability-profile reference, and the PCC/grid-model reference (P2.02); add the telemetry partitioned table + indexes + RLS policies; extend outbox/inbox (already present from Phase 1) only if the telemetry flow needs new columns, and record any change.
  2. Implement the telemetry repository in `crates/adapters/postgres/`: insert (single + batch), query by asset + time range, latest-value-per-asset query, all tenant-scoped via `TenantContext`.
  3. Implement the ingestion write path's duplicate handling at the repository level (P2.09): idempotency key / (asset_id, event_time, source_sequence) uniqueness — decide the key once (P2.09) and enforce it here with a unique constraint.
  4. Add integration tests on real PostgreSQL 18: migrations up/down; partition routing (a row lands in the correct monthly partition — verify via the partition's own table); partition pruning (`EXPLAIN` shows pruning for a time-bounded query); RLS isolation (tenant A cannot read tenant B's telemetry); unique-key duplicate rejection; out-of-range event_time error handling.
- **Deliverable / output:** Migrations + telemetry repository + integration tests.
- **Acceptance criteria / exit check:**
  - [ ] Migrations run up/down cleanly from an empty database.
  - [ ] Registry tables are extended, not recreated (no duplicate tenants/portfolios/sites/assets migration; the Phase 1 tables are the base).
  - [ ] Partition-routing test passes (row in the correct partition).
  - [ ] `EXPLAIN` for a time-bounded asset query shows partition pruning.
  - [ ] RLS test passes for telemetry rows.
  - [ ] Duplicate insert is rejected by the unique constraint (test).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.05
- **Verification method:** Integration tests on PostgreSQL 18; `EXPLAIN` output in the test log.
- **Risks / failure modes:** Partition pruning failing (query missing the partition key); RLS bypassed via the ingestion role. Mitigation: EXPLAIN tests + per-role RLS tests.

## P2.07 — Define the telemetry ingestion pipeline (validate → normalize → store → publish)

- **Objective:** Define the ingestion pipeline stages and the evidence each stage records, per §6.4 (pipeline pattern).
- **Report trace:** §6.4 (Pipeline), §3.4 (workflow steps 3–4), §15.3 (`TelemetryReceived`, `TelemetryQualityChanged`)
- **Inputs / prerequisites:** P2.04–P2.06; P0.08.
- **Steps:**
  1. Define the stages: (1) receive (source identity + authentication of the source — simulator source in Phase 2, real source auth in later phases); (2) schema validation (record shape, required fields, types); (3) unit/sign normalization to canonical (P2.04); (4) quality assignment (P2.08 — the quality rules); (5) duplicate/out-of-order handling (P2.09); (6) store (P2.06 repository); (7) publish `TelemetryReceived` (and `TelemetryQualityChanged` when the quality state changes for an asset) via the outbox.
  2. State the pipeline rule (§6.4): each stage records evidence; failure does not silently skip the next gate — a failed record is quarantined with a reason, counted, and (where safe) retried; it is never silently dropped or silently stored as GOOD.
  3. Define the quarantine: a quarantine table/queue for records failing validation, with the raw payload, the failure reason, source identity, and timestamps; quarantine is queryable (P2.12 API or an admin view — decide once).
  4. Define the ordering guarantee: ingestion tolerates out-of-order arrival (P2.09); the pipeline never assumes arrival order equals event order.
  5. Define the backpressure rule: ingestion is bounded (channel/queue limits); when the store is slow, the pipeline applies backpressure rather than unbounded buffering — record the limit and the behaviour at the limit.
- **Deliverable / output:** Ingestion-pipeline design document (stages, evidence, quarantine, backpressure).
- **Acceptance criteria / exit check:**
  - [ ] All seven stages are defined with inputs/outputs.
  - [ ] The no-silent-skip rule and the quarantine design are stated.
  - [ ] Backpressure limit and at-limit behaviour are stated.
  - [ ] `TelemetryReceived`/`TelemetryQualityChanged` publication via the outbox is stated per stage.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.04–P2.06
- **Verification method:** Design review against §6.4 and §3.4.
- **Risks / failure modes:** A failed record silently stored as GOOD; unbounded buffering under a slow store. Mitigation: quarantine + backpressure tests (P2.10, P2.24).

## P2.08 — Implement data-quality assignment and stale detection

- **Objective:** Implement the quality rules: per-value quality assignment and staleness detection with per-use-case freshness limits.
- **Report trace:** §10.4, §3.4, §17.4 (stale-data gates), §22.3 (stale/missing telemetry handled); P0.08
- **Inputs / prerequisites:** P2.04, P2.07; P0.08.
- **Steps:**
  1. Implement the quality-assignment rules as a pure function (unit-testable, no I/O): range checks per quantity (e.g. SOC in [0,1] — outside → BAD or SUSPECT per the rule below; P outside rated limits → SUSPECT); plausibility checks (e.g. BESS P and SOC consistency — a discharge while SOC is 0 is SUSPECT); source-reported invalid → BAD; missing interval → MISSING (generated by the staleness/gap detector, not by the ingestion of a value); derived/interpolated/substituted → ESTIMATED (only where a derivation rule exists — Phase 2 derives nothing by default; ESTIMATED appears only if a derivation is explicitly configured and recorded).
  2. Decide the SUSPECT-vs-BAD boundary once and record it: hard validation failure (schema, type, SOC outside [0,1], source reports invalid) → BAD; plausibility/range/consistency failure on an otherwise well-formed value → SUSPECT.
  3. Implement staleness detection: per-asset expected interval (from the asset's telemetry configuration — the simulator's cadence in Phase 2); a value is STALE when its age exceeds the use-case freshness limit; freshness limits are per use case (current-state view vs historical view — P2.18/P2.19 define their limits) and are configuration, not hard-coded constants.
  4. Implement gap detection: expected intervals with no value produce MISSING markers (as derived records, clearly marked as derived, not as measured values).
  5. Emit `TelemetryQualityChanged` when an asset's quality state changes (via the outbox).
  6. Add tests: each rule with a passing and a failing case; STALE boundary test (age == limit vs limit + 1s); MISSING gap test; SUSPECT-vs-BAD boundary tests; ESTIMATED never appears without an explicit derivation rule (test).
- **Deliverable / output:** Quality-assignment module + staleness/gap detector + tests.
- **Acceptance criteria / exit check:**
  - [ ] Every quality rule has a unit test (pass + fail case).
  - [ ] SUSPECT-vs-BAD boundary is recorded and tested.
  - [ ] STALE boundary test passes (age == limit is not STALE; limit + 1s is).
  - [ ] MISSING markers are derived records, distinguishable from measured values (test).
  - [ ] ESTIMATED cannot appear without an explicit derivation rule (test).
  - [ ] `TelemetryQualityChanged` is emitted on quality-state change (test).
- **Owner role:** Power systems + platform engineering (Rust)
- **Dependencies:** P2.04, P2.07
- **Verification method:** Unit tests; boundary tests.
- **Risks / failure modes:** STALE computed from ingest_time instead of event_time; ESTIMATED silently substituted for missing data; SUSPECT/BAD conflated. Mitigation: boundary tests + the P0.08 vocabulary as the test oracle.

## P2.09 — Implement duplicate and out-of-order handling (sequence, idempotency)

- **Objective:** Implement duplicate tolerance and out-of-order handling using source sequence, event time, ingestion time, and idempotency keys (§6.3).
- **Report trace:** §6.3, §15.4, §6.4 (outbox/inbox)
- **Inputs / prerequisites:** P2.04, P2.06–P2.07.
- **Steps:**
  1. Define the identity of a telemetry record for deduplication: `(asset_id, event_time, source_sequence)` + idempotency key — decide once, record it, enforce it with the P2.06 unique constraint.
  2. Implement duplicate handling: an exact duplicate (same identity, same values) is acknowledged and not stored twice; a conflicting duplicate (same identity, different values) is quarantined with a reason, not silently overwritten and not silently accepted.
  3. Implement out-of-order handling: a late-arriving record with an older `event_time` is stored (it is valid history); the current-state view (P2.18) is recomputed from the latest `event_time`, not the latest arrival — a late record with a newer `event_time` than the current state updates the state; a late record with an older `event_time` does not regress the current state.
  4. Implement the consumer-side idempotency (reuse P1.16 inbox): a redelivered ingestion message is detected and not re-stored.
  5. Add tests: exact duplicate → one row; conflicting duplicate → quarantine; out-of-order late record → stored, current state not regressed; redelivery → one row.
- **Deliverable / output:** Deduplication/out-of-order module + tests.
- **Acceptance criteria / exit check:**
  - [ ] Exact-duplicate test: one row stored, second insert acknowledged without a second row.
  - [ ] Conflicting-duplicate test: quarantined with a reason; stored value unchanged.
  - [ ] Out-of-order test: late older record stored; current state unchanged; late newer record updates state.
  - [ ] Redelivery test: one row after redelivery.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.04, P2.06–P2.07
- **Verification method:** Integration tests on PostgreSQL + NATS.
- **Risks / failure modes:** Last-write-wins on conflict (silent data corruption); current state regressed by a late record. Mitigation: quarantine-on-conflict + event_time-ordered current state.

## P2.10 — Implement the ingestion service (pipeline + quarantine + backpressure)

- **Objective:** Implement the ingestion service that runs the P2.07 pipeline against the P2.06 repository.
- **Report trace:** §3.4, §6.4, §15.3; P2.07–P2.09
- **Inputs / prerequisites:** P2.07–P2.09; P1.19 (worker hosting).
- **Steps:**
  1. Create the ingestion service (a worker task in `apps/worker` or a dedicated ingestion binary — decide once, record the justification; the default is a worker task, reusing P1.19).
  2. Implement the pipeline stages from P2.07 end-to-end: receive → validate → normalize → quality-assign → dedup → store → publish events via outbox.
  3. Implement quarantine writes (P2.07) and a quarantine count metric.
  4. Implement backpressure (P2.07): bounded channel; at the limit, the service sheds load by a recorded policy (e.g. reject with a retryable error + metric) — never by unbounded buffering or silent dropping.
  5. Implement the ingestion API boundary: the service accepts telemetry from a `TelemetrySource` port (P1.07 trait) — the simulator source (P2.14) is the first implementation; no ingestion code depends on the simulator's internals.
  6. Add integration tests: happy-path batch; invalid batch (mixed valid/invalid — valid stored, invalid quarantined, counts correct); duplicate batch; out-of-order batch; backpressure test (slow store → bounded memory, shed policy observed); source-authentication failure → rejected (simulator source auth is a stub in Phase 2 — record its status honestly).
- **Deliverable / output:** Ingestion service + integration tests.
- **Acceptance criteria / exit check:**
  - [ ] Happy-path, mixed-validity, duplicate, out-of-order, and backpressure tests pass on the Compose stack.
  - [ ] Quarantine count metric increments for invalid records; no invalid record is stored as GOOD.
  - [ ] Ingestion depends only on the `TelemetrySource` port (no simulator import in the service).
  - [ ] Source-auth status is recorded honestly (stub vs real).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.07–P2.09
- **Verification method:** Integration tests on the Compose stack.
- **Risks / failure modes:** Ingestion coupled to the simulator; invalid records stored as GOOD; unbounded buffering. Mitigation: port-only dependency + quarantine tests + backpressure test.

## P2.11 — Define the telemetry source contract (TelemetrySource port, simulator as first provider)

- **Objective:** Define the `TelemetrySource` contract precisely, with the simulator as its first provider and real adapters as future providers.
- **Report trace:** §9.1 (`TelemetrySource`: subscribe/decode), §5.2 (capability seam), §5.3 (telemetry source category)
- **Inputs / prerequisites:** P1.07 (ports crate); P2.04.
- **Steps:**
  1. Define the port in `flexcore-ports`: `subscribe(query) -> TelemetryStream` and `decode(payload) -> NormalizedTelemetry` (per §9.1), plus a `source_id()` and a `health()` for the provider.
  2. Define the query type: asset set (tenant-scoped), time range, quantities — all tenant-scoped; a query without a tenant context is rejected.
  3. Define the normalized output: the P2.04 canonical record — a provider that cannot produce a canonical record (missing quality, missing sequence) fails decoding, it does not emit a partial record.
  4. State the provider rule: every provider declares its cadence, its quality semantics (how it maps its source states to the six canonical states, P0.08), and its clock-quality behaviour.
  5. Record the Phase 2 provider set: exactly one — the simulator telemetry source (P2.14). Real adapters (MQTT, Modbus, OpenEMS) are later phases.
- **Deliverable / output:** `TelemetrySource` port definition + provider-rule document.
- **Acceptance criteria / exit check:**
  - [ ] The port has subscribe/decode/source_id/health with tenant-scoped queries.
  - [ ] Decode failure (non-canonical output) is specified, not silent.
  - [ ] Every provider must declare cadence, quality mapping, and clock behaviour.
  - [ ] The Phase 2 provider set is exactly one (simulator).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.07, P2.04
- **Verification method:** Port review against §9.1; provider-rule checklist.
- **Risks / failure modes:** A provider emitting partial records; a query without tenant scope. Mitigation: port-level tests (P2.14).

## P2.12 — Extend the API: registry + telemetry query endpoints (OpenAPI)

- **Objective:** Extend the Axum API with registry CRUD and telemetry query endpoints, all tenant-scoped, with OpenAPI.
- **Report trace:** §15.1–§15.2, §20.1 (Assets/Sites/Portfolios workspaces — API only in this phase); P0.15
- **Inputs / prerequisites:** P2.03, P2.06, P2.10; P1.17.
- **Steps:**
  1. Add routes: `POST/GET /api/v1/tenants`, `/portfolios`, `/sites`, `/assets` (full CRUD where Phase 1 left stubs — complete the set; DELETE follows the P2.02 step 5 deletion rule — cascade vs restrict as decided and recorded there, audit trail preserved either way); `GET /api/v1/telemetry/query` (asset, time range, quantities, quality filter, pagination); `GET /api/v1/assets/{id}/state` (current state, P2.18); quarantine read endpoint (P2.07 — decide the path, e.g. `GET /api/v1/telemetry/quarantine`, tenant-scoped, admin role).
  2. Enforce tenant scoping + authorization on every route via `flexcore-authz` (P1.11); cross-tenant access denied per the P1.17 recorded choice (403 vs 404).
  3. Enforce the query contract: time range required; maximum range/page size enforced (record the limits); quality filter accepts only the six canonical states (a seventh value → 422).
  4. Regenerate OpenAPI with utoipa; commit the updated spec.
  5. Add API tests: CRUD happy path + validation errors; telemetry query by asset/time/quality; pagination; cross-tenant denial; seventh-quality-value → 422; oversized range → 422.
- **Deliverable / output:** Extended API routes + updated OpenAPI spec + API tests.
- **Acceptance criteria / exit check:**
  - [ ] All routes in the P0.15 list that belong to Stage 2 exist; no route outside the list.
  - [ ] DELETE behaviour follows the recorded P2.02 step 5 deletion rule (test: deleting a site/portfolio behaves per the rule, audit trail preserved).
  - [ ] Telemetry query enforces time range, page size, and the six-state quality filter (tests).
  - [ ] Cross-tenant denial tested per route.
  - [ ] OpenAPI spec regenerated and committed.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.03, P2.06, P2.10
- **Verification method:** API tests on the Compose stack; OpenAPI diff review.
- **Risks / failure modes:** Unbounded telemetry query (full-table scan via API); quality filter accepting a non-canonical value. Mitigation: enforced limits + 422 tests.

## P2.13 — Define the simulator scenario model (assets, profiles, seeds, determinism)

- **Objective:** Define the simulator's scenario model: what a scenario is, how it is seeded, and what determinism means.
- **Report trace:** §19.1–§19.3, §24 steps 1–2, 10; §5.8 (`sim-lab` profile)
- **Inputs / prerequisites:** P2.02, P2.04; P0.23 (test strategy).
- **Steps:**
  1. Define a scenario: a named, versioned artefact containing the portfolio (sites, assets, rated power/energy, capability-profile references), the simulation horizon and cadence, the per-asset behaviour profiles (load shapes, PV shapes, BESS dispatch shapes — synthetic, documented as synthetic), random seeds, and the expected telemetry cadence per asset.
  2. Define determinism: same scenario version + same seed + same code version = byte-identical telemetry records (same event_time, values, quality, sequence). Record the determinism scope: values and timestamps are deterministic; `ingest_time` is not (it depends on wall-clock ingestion) — replay comparisons use `event_time` and values, never `ingest_time`.
  3. Define the scenario versioning rule: a scenario change creates a new version; replay (P2.24) always names the scenario version + seed + code version.
  4. Define the asset behaviour profiles for the exit-gate portfolio: BESS (charge/discharge shape respecting SOC [0,1] and rated limits), PV (diurnal shape, zero at night, curtailment capability noted but not exercised in Phase 2), flexible load (baseline + reducible range), EVSE (connection windows + charging shape). Each profile states its assumptions (e.g. "synthetic clear-sky PV shape, no weather model").
  5. Store scenarios under `scenarios/synthetic-portfolio/` (per §8.1 target tree).
- **Deliverable / output:** Scenario-model definition + the exit-gate scenario (versioned).
- **Acceptance criteria / exit check:**
  - [ ] Scenario schema defined (portfolio, horizon, cadence, profiles, seeds, versions).
  - [ ] Determinism scope stated, with `ingest_time` explicitly excluded from replay comparison.
  - [ ] All four exit-gate asset profiles defined with stated assumptions.
  - [ ] Scenario stored under `scenarios/synthetic-portfolio/` with a version.
- **Owner role:** Power systems + platform engineering (Rust)
- **Dependencies:** P2.02, P2.04
- **Verification method:** Scenario review against §19.3 and §24; determinism test (P2.24).
- **Risks / failure modes:** A profile presented as a validated physical model (it is synthetic); `ingest_time` compared in replay (non-deterministic by construction). Mitigation: assumptions stated per profile; replay compares event_time + values only.

## P2.14 — Implement the simulator telemetry source (synthetic BESS/PV/load/EVSE)

- **Objective:** Implement the first `TelemetrySource` provider: a deterministic simulator emitting canonical telemetry for the four exit-gate asset types.
- **Report trace:** §5.8 (`sim-lab` profile), §19.1, §24 steps 1–2; P2.11, P2.13
- **Inputs / prerequisites:** P2.11, P2.13; P1.07 (ports).
- **Steps:**
  1. Create the simulator as a built-in Rust provider (`plugins/builtin/asset-simulator/` per §8.1, or a `crates/` provider — decide once, record it; it implements the `TelemetrySource` port and nothing else).
  2. Implement generation per the P2.13 scenario: per-asset cadence, per-asset behaviour profiles, seeded RNG (seed from the scenario; record the RNG algorithm + seed handling so a re-run reproduces the stream).
  3. Emit canonical records only (P2.04): canonical units, P > 0 = export, SOC [0,1], exactly one quality state per value (GOOD by default; the simulator must also be able to emit SUSPECT/BAD/MISSING/STALE on demand for testing — P2.15), source sequence monotonically increasing per asset, `event_time` on the scenario clock, `ingest_time` set at emission.
  4. Implement the simulator's quality-mapping declaration (P2.11): how each emitted state maps to the canonical six.
  5. Implement start/stop/pause and a bounded emission rate (backpressure-aware: the simulator respects the ingestion service's backpressure, P2.10).
  6. Add tests: determinism (same scenario + seed → identical records across two runs); SOC stays in [0,1] across a full run; P respects rated limits; sequence is monotonic per asset; quality states emitted on demand match the request.
- **Deliverable / output:** Simulator telemetry source + determinism tests.
- **Acceptance criteria / exit check:**
  - [ ] Determinism test: two runs, same scenario + seed, produce identical records (event_time, values, quality, sequence).
  - [ ] SOC in [0,1] and P within rated limits across a full 24-hour run (test).
  - [ ] Sequence monotonic per asset (test).
  - [ ] The simulator implements only the `TelemetrySource` port (no direct DB/NATS access — `cargo tree` check).
- **Owner role:** Platform engineering (Rust) + power systems (profiles)
- **Dependencies:** P2.11, P2.13
- **Verification method:** Determinism + invariant tests; `cargo tree`.
- **Risks / failure modes:** Non-deterministic RNG (time-seeded); simulator writing directly to the database; SOC drifting outside [0,1]. Mitigation: seeded RNG + port-only access + invariant tests.

## P2.15 — Implement simulator fault injection (stale, missing, suspect, bad, duplicates)

- **Objective:** Give the simulator a fault-injection mode so quality/stale/duplicate handling is testable end-to-end.
- **Report trace:** §19.2 (test layer 6: fault injection), §24 step 8 (partial failure, timeout — simulator side)
- **Inputs / prerequisites:** P2.14; P2.08–P2.09.
- **Steps:**
  1. Define the fault-injection controls in the scenario: per-asset, per-interval faults — drop values (→ MISSING), delay values past the freshness limit (→ STALE), emit out-of-range values (→ SUSPECT), emit source-invalid flags (→ BAD), emit duplicate records, emit out-of-order records, emit conflicting duplicates.
  2. Implement the controls as scenario configuration (versioned with the scenario), not as ad-hoc code paths.
  3. Add end-to-end tests per fault: each fault produces the expected quality state / quarantine outcome in the stored telemetry (via P2.08–P2.10).
  4. Record the fault-injection scope: these are simulator faults for testing the platform's handling; they are not claims about real device failure modes.
- **Deliverable / output:** Simulator fault-injection controls + end-to-end fault tests.
- **Acceptance criteria / exit check:**
  - [ ] Each of the seven fault types has an end-to-end test with the expected stored outcome.
  - [ ] Faults are scenario configuration, not code branches (review).
  - [ ] No fault test depends on wall-clock timing (use scenario-clock + freshness-limit arithmetic).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.14, P2.08–P2.09
- **Verification method:** End-to-end fault tests on the Compose stack.
- **Risks / failure modes:** Fault tests that are timing-flaky; a fault mode that bypasses the pipeline (emitting directly to the DB). Mitigation: scenario-clock arithmetic; faults flow through the ingestion pipeline.

## P2.16 — Implement telemetry event publication (TelemetryReceived, TelemetryQualityChanged, AssetAvailabilityChanged)

- **Objective:** Wire telemetry ingestion to the §15.3 events via the outbox, with correct ordering and idempotency.
- **Report trace:** §15.3, §15.5, §6.3; P2.07, P1.15–P1.16
- **Inputs / prerequisites:** P2.10; P1.09, P1.15.
- **Steps:**
  1. Publish `TelemetryReceived` per ingested batch (or per record — decide once, record the granularity and the rationale; batch-level is the default to bound event volume, with the batch's record IDs included).
  2. Publish `TelemetryQualityChanged` when an asset's quality state changes (P2.08), including the previous and new states.
  3. Publish `AssetAvailabilityChanged` when an asset's availability changes (from the asset's telemetry-derived availability — define the availability rule once: e.g. availability derived from the latest quality + connectivity signals, recorded in the design).
  4. Publish all three via the outbox (state + outbox in one transaction, P1.13) — never by direct NATS publish from the ingestion service.
  5. Add tests: batch ingestion produces the expected events in the outbox; quality change produces the event with previous/new states; availability change produces the event; redelivery does not duplicate events (inbox, P1.16).
- **Deliverable / output:** Telemetry event wiring + tests.
- **Acceptance criteria / exit check:**
  - [ ] Event granularity decision recorded (batch vs per-record).
  - [ ] All three events have tests with expected payloads.
  - [ ] Events are published via the outbox only (test asserts outbox records; no direct NATS call in the ingestion service — code inspection).
  - [ ] Redelivery test: no duplicate events.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.10
- **Verification method:** Outbox assertions + integration tests.
- **Risks / failure modes:** Events published directly to NATS (dual-write hole); event volume unbounded (per-record for high-frequency telemetry). Mitigation: outbox-only + recorded granularity.

## P2.17 — Implement the asset registry API and views (portfolio/site/asset reads)

- **Objective:** Implement the registry read surface: portfolio, site, and asset views with tenant scoping.
- **Report trace:** §20.1 (Portfolios, Sites, Assets workspaces — API/views only), §15.2; P2.03, P2.12
- **Inputs / prerequisites:** P2.03, P2.12.
- **Steps:**
  1. Implement the read models: portfolio view (composition, aggregation rules, commitments placeholder), site view (overview, PCC, grid-model reference, edge connectivity — edge connectivity is "not connected / simulator" in Phase 2, stated honestly), asset view (state, capability reference, telemetry quality, limits, maintenance/outage status — maintenance/outage status is a stored field, populated by the registry, not by a live system).
  2. Implement list/get endpoints with pagination and tenant scoping (P2.12 limits apply).
  3. State the display rule for later (Phase 5): measured vs estimated vs stale vs simulated values are distinguished — in Phase 2, the API marks every value with its quality state and a `simulated: true` flag so no consumer mistakes simulator data for measured data.
  4. Add API tests: list/get per entity; pagination; cross-tenant denial; `simulated` flag present on all simulator-derived values.
- **Deliverable / output:** Registry read endpoints + API tests.
- **Acceptance criteria / exit check:**
  - [ ] All three views exist with the specified fields.
  - [ ] Edge connectivity honestly reports simulator/not-connected status.
  - [ ] Every simulator-derived value carries `simulated: true` + quality state (test).
  - [ ] Pagination + cross-tenant tests pass.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.03, P2.12
- **Verification method:** API tests.
- **Risks / failure modes:** Simulator data presented as measured; edge connectivity faked as live. Mitigation: `simulated` flag + honest connectivity status, both tested.

## P2.18 — Implement the current-state view (latest value per asset, with freshness + quality)

- **Objective:** Implement the current-state query: the latest value per asset, with freshness and quality attached, never a stale value presented as current.
- **Report trace:** §20.1 (Overview, Assets), §10.4 (STALE), §6.4 (CQRS selective — read model); P2.08
- **Inputs / prerequisites:** P2.06, P2.08–P2.09, P2.12.
- **Steps:**
  1. Define the current-state rule: for each asset + quantity, the current value is the record with the latest `event_time` (not the latest `ingest_time`); if no record exists, the state is MISSING; if the latest record's age exceeds the current-state freshness limit, the state is STALE (the last known value is shown with the STALE flag, never as GOOD).
  2. Implement the query: latest-per-asset via the P2.05 index (or a materialized current-state table updated on ingestion — decide once, record the rationale; the default is a query over the partitioned table using the index, with a materialized table as a later optimisation only on measured need).
  3. Implement the freshness limits as configuration per use case (P2.08): the current-state view's limit is recorded in configuration.
  4. Implement the response shape: value + unit + quality + event_time + ingest_time + age + `simulated` flag.
  5. Add tests: latest-by-event_time (not ingest_time) with an out-of-order late record; STALE boundary; MISSING (no records); ESTIMATED never presented as measured (quality preserved in the response).
- **Deliverable / output:** Current-state query + API endpoint (`GET /api/v1/assets/{id}/state`, P2.12) + tests.
- **Acceptance criteria / exit check:**
  - [ ] Latest-by-event_time test passes (out-of-order record does not regress state).
  - [ ] STALE boundary test passes.
  - [ ] MISSING test passes (no records → MISSING, not a zero value).
  - [ ] Response carries quality + age + `simulated` flag (test).
  - [ ] Query uses the P2.05 index (`EXPLAIN` shows index use, not a full scan).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.06, P2.08–P2.09, P2.12
- **Verification method:** Unit + API tests; `EXPLAIN` output.
- **Risks / failure modes:** Current state from latest arrival (regression on late records); STALE shown as GOOD; missing shown as zero. Mitigation: event_time ordering + boundary tests + MISSING semantics.

## P2.19 — Implement the historical view (time-range queries, aggregation, pagination)

- **Objective:** Implement the historical telemetry query: time-range, per-asset/per-quantity, with aggregation and pagination.
- **Report trace:** §15.2 (`/telemetry/query`), §16.1 (time-window queries), §20.1 (Audit & Reports — data only)
- **Inputs / prerequisites:** P2.06, P2.12; P2.05 (partition pruning).
- **Steps:**
  1. Implement the query: asset (+ quantity filter), time range (`event_time` bounded, required), quality filter (six states only), pagination (cursor or offset — decide once, record it; cursor is the default for time-series).
  2. Implement aggregation: downsampling/bucketing (e.g. mean/min/max per interval — decide the supported aggregations once, record them; raw + bucketed are both available).
  3. Enforce limits: maximum time range, maximum page size, maximum rows per request (record the values as configuration); oversized requests → 422 with a stable error.
  4. Verify partition pruning: `EXPLAIN` on representative queries shows pruning; record the `EXPLAIN` outputs.
  5. Add tests: range query returns exactly the expected records; quality filter; aggregation correctness against a hand-computed fixture; pagination (no duplicates, no gaps across pages); oversized range → 422; cross-tenant denial.
- **Deliverable / output:** Historical query implementation + tests + `EXPLAIN` evidence.
- **Acceptance criteria / exit check:**
  - [ ] Range/quality/aggregation/pagination tests pass.
  - [ ] Aggregation matches a hand-computed fixture exactly.
  - [ ] Pagination has no duplicates or gaps (test walks all pages).
  - [ ] `EXPLAIN` shows partition pruning for time-bounded queries.
  - [ ] Oversized-range → 422 test passes.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.06, P2.12
- **Verification method:** API + unit tests; `EXPLAIN` outputs.
- **Risks / failure modes:** Unbounded query (full-table scan); pagination gaps/duplicates; aggregation rounding changing meaning (P0.07: do not round in a way that changes meaning). Mitigation: enforced limits + pagination test + fixture test.

## P2.20 — Implement retention and partition maintenance (detach/drop, pre-creation job)

- **Objective:** Implement the retention and partition-maintenance mechanics from the P2.05 design.
- **Report trace:** §16.3, §21.2 (schema migrations with compatibility windows); P2.05
- **Inputs / prerequisites:** P2.05–P2.06.
- **Steps:**
  1. Implement the partition pre-creation job: creates the next N partitions ahead of the ingest frontier on a schedule (or on ingestion when the next partition is missing — decide once; scheduled pre-creation is the default, with the ingestion path returning a clear error if a partition is still missing).
  2. Implement retention execution: detach (then drop, or archive first — decide once, record it) partitions older than the configured retention per data class; retention execution is a manual/approved operation in Phase 2 (not an automatic delete), with an audit record per detached/dropped partition.
  3. Implement the safety rule: retention never drops the current or the in-flight partition; a drop requires the partition to be fully outside the retention window.
  4. Add tests: pre-creation creates the expected partitions; retention detaches only partitions older than the window; dropping the current partition is refused; audit records exist per retention action.
  5. Define and record the no-retention-configured behaviour (decide once): when no retention value is configured for a data class, no deletion occurs — partitions accumulate unboundedly — and a metric/alert records the unconfigured state so the accumulation is visible, not silent. (A configured default is the alternative; either way the behaviour is written down, not left to whoever reads the code later.)
- **Deliverable / output:** Partition-maintenance job + retention execution + tests.
- **Acceptance criteria / exit check:**
  - [ ] Pre-creation test: partitions exist ahead of the frontier.
  - [ ] Retention test: only out-of-window partitions are detached/dropped.
  - [ ] Current-partition drop is refused (test).
  - [ ] The no-retention-configured behaviour is recorded and tested (no deletion; accumulation is visible via metric/alert, or a configured default exists).
  - [ ] Every retention action has an audit record (test).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.05–P2.06
- **Verification method:** Integration tests on PostgreSQL 18.
- **Risks / failure modes:** Automatic deletion of in-window data; a missing partition crashing ingestion. Mitigation: manual/approved retention + pre-creation + error path test.

## P2.21 — Extend observability for telemetry (ingestion delay, staleness, queue depth, quality counts)

- **Objective:** Extend the Phase 1 observability with telemetry-specific metrics and alerts.
- **Report trace:** §21.3, §17.4 (stale-data gates); P1.20
- **Inputs / prerequisites:** P2.10, P2.08; P1.20.
- **Steps:**
  1. Add metrics: ingestion delay (ingest_time − event_time, per asset/quantity); stale-telemetry count per asset; queue depth (ingestion backlog); outbox pending (already present — extend with telemetry event counts); quarantine count (P2.10); quality-state counts per asset (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE).
  2. Add tracing: ingestion spans carry tenant/site/asset IDs, source ID, event_time, quality (P1.20 correlation rule).
  3. Add the staleness signal: a per-asset staleness metric that the (future) operator views and the (future) feasibility gate will consume — in Phase 2 it is a metric + log, not an alerting rule (alerting thresholds are a deployment concern, not hard-coded).
  4. Verify the no-secrets rule still holds for telemetry logs (source IDs are not secrets; no credential appears in any telemetry log line — test).
- **Deliverable / output:** Telemetry metrics + tracing + no-secrets re-test.
- **Acceptance criteria / exit check:**
  - [ ] `/metrics` exposes all six metric families from step 1.
  - [ ] Ingestion spans carry the P1.20 correlation fields + quality.
  - [ ] No-secrets test passes for telemetry log lines.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.10, P2.08
- **Verification method:** `/metrics` scrape; log inspection; no-secrets test.
- **Risks / failure modes:** Ingestion delay computed from the wrong timestamps; quality counts missing STALE. Mitigation: metric inventory test (each metric asserted present).

## P2.22 — Write the Phase 2 security test slice (registry + telemetry isolation, source auth)

- **Objective:** Extend the Phase 1 security suite to the registry and telemetry surfaces.
- **Report trace:** §19.2 (test layer 7), §17.4, §16.2; P1.23
- **Inputs / prerequisites:** P2.03, P2.06, P2.10, P2.12; P1.23.
- **Steps:**
  1. Tenant-isolation tests: every new route (registry CRUD, telemetry query, current-state, historical, quarantine) — tenant A cannot read/write tenant B's data.
  2. Registry tests: an asset cannot be registered under another tenant's site; a portfolio cannot be moved across tenants.
  3. Telemetry tests: telemetry cannot be ingested for another tenant's asset (source identity is tenant-scoped); telemetry query cannot cross tenants.
  4. Source-auth tests: the simulator source authenticates per the recorded stub status (P2.10); an unauthenticated source is rejected; a source presenting another tenant's identity is rejected.
  5. Record the not-yet-covered items (protocol-adapter auth, edge enrollment — later phases) with owning phases.
- **Deliverable / output:** Phase 2 security test suite + coverage note.
- **Acceptance criteria / exit check:**
  - [ ] Tenant-isolation tests exist per new route and pass.
  - [ ] Cross-tenant registry/telemetry cases are rejected in tests.
  - [ ] Unauthenticated/wrong-tenant source tests pass.
  - [ ] Not-yet-covered list names owning phases.
- **Owner role:** Platform engineering (Rust) + security (reviewer)
- **Dependencies:** P2.03, P2.06, P2.10, P2.12
- **Verification method:** `cargo test` security suite.
- **Risks / failure modes:** Telemetry ingestion without tenant scoping (a source writing to any asset). Mitigation: source identity tenant-scoped + tests.

## P2.23 — Write the Phase 2 fault-injection suite (ingestion + storage faults)

- **Objective:** Extend fault-injection testing to ingestion and storage faults.
- **Report trace:** §19.2 (test layer 6); P1.24, P2.15
- **Inputs / prerequisites:** P2.10, P2.14–P2.15; P1.24.
- **Steps:**
  1. Ingestion faults: simulator stops mid-batch (partial batch stored, remainder retried, no duplicates); ingestion service restarts mid-batch (same); NATS down during ingestion (state + outbox written, events drain on recovery — P1.24 pattern, now with telemetry volume).
  2. Storage faults: PostgreSQL down during ingestion (clean failure, no partial batch, no lost outbox entries); a partition missing for the ingest frontier (clear error, no silent drop — P2.05/P2.20).
  3. Clock faults: simulator clock skew (event_time far future/past) → SUSPECT/quarantine per P2.08, never silently stored as GOOD current state.
  4. Volume fault: a burst above the backpressure limit → shed policy observed, metric incremented, no unbounded memory (P2.10).
  5. Each test asserts the fault actually occurred before asserting the handling.
- **Deliverable / output:** Phase 2 fault-injection suite + results log.
- **Acceptance criteria / exit check:**
  - [ ] All fault cases pass with recorded outcomes.
  - [ ] No partial batch is left in an ambiguous state (all-or-quarantined, verified).
  - [ ] Clock-skew records are flagged, not stored as GOOD.
  - [ ] Each test asserts the fault condition first.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.10, P2.14–P2.15
- **Verification method:** Compose-stack fault tests; results log.
- **Risks / failure modes:** Fault tests that pass because the fault never happened; partial batches left ambiguous. Mitigation: fault-condition assertions + batch-atomicity checks.

## P2.24 — Implement deterministic replay (scenario → stored telemetry → replay output)

- **Objective:** Implement deterministic replay: re-running a scenario reproduces the stored telemetry and the replay output.
- **Report trace:** §23 Stage 2 (exit gate: "replayed deterministically"), §24 step 10, §19.2 (deterministic scenario tests); P2.13
- **Inputs / prerequisites:** P2.13–P2.14, P2.10; P1.19 (worker/sim-runner hosting — see note).
- **Steps:**
  1. Define replay: given a scenario version + seed + code version, re-run the simulator and compare the produced records (event_time, values, quality, sequence) against the originally stored records; report match/mismatch per record with a diff.
  2. Implement the replay entrypoint: a CLI command or worker job (per §8.1, `apps/sim-runner/` is the scenario-execution entrypoint — decide whether Phase 2 creates a minimal `sim-runner` or hosts replay in the worker; decide once, record it; the default is a minimal `sim-runner` CLI, since replay is a distinct entrypoint from background ingestion).
  3. Implement the comparison: compare on (asset_id, event_time, quantity, value, quality, source_sequence) — never on `ingest_time` (P2.13).
  4. Implement the replay report: scenario version, seed, code version, record counts, match count, mismatch list (with diffs), and a pass/fail verdict.
  5. Add tests: replay of the exit-gate scenario passes; a tampered scenario (changed seed) fails with a diff; replay after a code change that alters generation fails honestly (the test documents that generation changes break replay — this is expected, not a bug to hide).
- **Deliverable / output:** Replay entrypoint + comparison + report + tests.
- **Acceptance criteria / exit check:**
  - [ ] Replay of the exit-gate scenario passes with zero mismatches.
  - [ ] Changed-seed replay fails with a diff (test).
  - [ ] Comparison excludes `ingest_time` (test: two runs with different ingest times still match).
  - [ ] Replay report contains scenario version, seed, code version, counts, and verdict.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.13–P2.14, P2.10
- **Verification method:** Replay tests on the Compose stack.
- **Risks / failure modes:** Replay compared on `ingest_time` (always fails); a generation change silently breaking replay with no diff. Mitigation: comparison-field test + diff report.

## P2.25 — Build the exit-gate synthetic portfolio (BESS + PV + load + EVSE, 24-hour horizon)

- **Objective:** Build the concrete synthetic portfolio the Stage 2 exit gate runs against.
- **Report trace:** §23 Stage 2 (exit gate), §1.4, §2.2, §24 steps 1–2; P2.13
- **Inputs / prerequisites:** P2.02–P2.03, P2.13–P2.14.
- **Steps:**
  1. Create the portfolio: one tenant, one portfolio, several simulated sites (record the count — the §24 slice says "several"; the exit-gate portfolio uses at least two sites to prove multi-site handling), each site with at least one asset of each of the four types (BESS, PV, flexible load, EVSE) plus a site meter.
  2. Register the full portfolio through the API (P2.12), not by direct SQL.
  3. Run the simulator for a 24-hour horizon at the scenario cadence; ingest all telemetry through the pipeline (P2.10).
  4. Verify: every asset has telemetry for the full horizon (except injected MISSING intervals, which are accounted for); every record is canonical (P2.04); quality states are as expected per the fault plan (P2.15).
  5. Record the portfolio as a versioned scenario artefact (P2.13) so the gate can be re-run.
- **Deliverable / output:** Exit-gate synthetic portfolio (versioned scenario + registration record + ingestion record).
- **Acceptance criteria / exit check:**
  - [ ] ≥2 sites, each with BESS + PV + load + EVSE + site meter, registered via the API.
  - [ ] Full 24-hour horizon ingested; per-asset coverage verified (counts per asset).
  - [ ] All records canonical (unit/quality/time checks pass on a sample + counts).
  - [ ] Injected faults accounted for (MISSING intervals listed, not unexplained gaps).
- **Owner role:** Power systems + platform engineering (Rust)
- **Dependencies:** P2.02–P2.03, P2.13–P2.14
- **Verification method:** Coverage counts per asset; canonical-value checks.
- **Risks / failure modes:** A single-site portfolio (multi-site handling unproven); gaps unexplained (fault vs bug indistinguishable). Mitigation: ≥2 sites + fault accounting.

## P2.26 — Prove the Stage 2 exit gate (ingest, normalize, query, replay deterministically)

- **Objective:** Demonstrate the §23 Stage 2 exit gate end-to-end on the synthetic portfolio.
- **Report trace:** §23 Stage 2 (exit gate)
- **Inputs / prerequisites:** P2.01–P2.25 (all Phase 2 work).
- **Steps:**
  1. Run the exit-gate demonstration on a clean Compose stack: register the P2.25 portfolio via the API; run the simulator for 24 hours; ingest through the pipeline; verify stored telemetry (counts, canonical units, quality states).
  2. Query: current-state view per asset (P2.18) and historical view per asset (P2.19) — verify both return the expected values.
  3. Replay: re-run the scenario (P2.24) and verify the replay report passes with zero mismatches.
  4. Run the fault variant: the same portfolio with the P2.15 fault plan — verify quality/stale/duplicate handling end-to-end.
  5. Record the run: commands, outputs, timings, counts, versions (toolchain, Postgres, NATS, scenario version, seed, code version). Include a performance note: ingestion rate and query latency observed (no target claimed — §19.4 forbids arbitrary targets; the numbers are observations, not promises).
- **Deliverable / output:** Exit-gate demonstration record.
- **Acceptance criteria / exit check:**
  - [ ] Ingestion, normalization, query (current + historical), and deterministic replay all demonstrated on a clean stack.
  - [ ] Current-state and historical queries return expected values (verified against the scenario).
  - [ ] Replay passes with zero mismatches.
  - [ ] Fault variant passes with expected quality outcomes.
  - [ ] Run record lists exact versions + observed (not target) performance numbers.
- **Owner role:** Principal engineering (demonstrator); power systems (witness)
- **Dependencies:** P2.01–P2.25 (all)
- **Verification method:** The demonstration record; re-run by a second person.
- **Risks / failure modes:** A demo that only works on the author's machine; performance numbers presented as targets. Mitigation: clean-checkout re-run; numbers labelled observations.

## P2.27 — Extend CI for Phase 2 (migration tests, scenario regression, telemetry contract tests)

- **Objective:** Extend the Phase 1 CI with the Phase 2 gates.
- **Report trace:** §22.2 (migration tests, scenario regression suite); P1.27
- **Inputs / prerequisites:** P2.06, P2.13, P2.24; P1.27.
- **Steps:**
  1. Add migration tests for the telemetry/registry migrations (up/down on real PostgreSQL 18, including partition creation).
  2. Add the scenario regression suite: the exit-gate scenario runs in CI and its replay passes (P2.24); a generation change that breaks replay fails CI honestly (with a diff, not a silent pass).
  3. Add telemetry contract tests: every adapter/provider passes the same interface suite (§19.2 layer 3 — the `TelemetrySource` contract suite from P2.11, run against the simulator now and against future providers later).
  4. Keep all Phase 1 gates green; record any gate that is still not applicable (Python Ruff/pytest, frontend, plugin/WIT — later phases).
- **Deliverable / output:** Extended CI workflow + gate inventory update.
- **Acceptance criteria / exit check:**
  - [ ] Migration tests run in CI and pass.
  - [ ] Scenario regression runs in CI and passes (replay verdict in the log).
  - [ ] Telemetry contract suite runs in CI against the simulator.
  - [ ] All Phase 1 gates still pass.
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.06, P2.13, P2.24
- **Verification method:** CI run history.
- **Risks / failure modes:** Scenario regression that passes by comparing `ingest_time` (always fails) or by skipping the comparison (always passes). Mitigation: replay-verdict assertion in CI.

## P2.28 — Write the Phase 2 handover pack for Phase 3

- **Objective:** Assemble the exact inputs Phase 3 (flexibility + scheduling) consumes.
- **Report trace:** §23 Stage 2 → Stage 3; §11 (capability models — Phase 3 implements them)
- **Inputs / prerequisites:** P2.01–P2.27.
- **Steps:**
  1. List the handover artefacts: registry domain model + implementation (P2.02–P2.03); canonical telemetry model (P2.04); telemetry storage + repositories (P2.05–P2.06); ingestion pipeline + quality/stale/dedup (P2.07–P2.10); `TelemetrySource` contract (P2.11); API extensions (P2.12, P2.17); scenario model + simulator (P2.13–P2.15); event wiring (P2.16); current-state + historical views (P2.18–P2.19); retention/partition maintenance (P2.20); observability (P2.21); exit-gate portfolio + demonstration (P2.25–P2.26); CI extensions (P2.27); versioned corrections + as-of replay (P2.31).
  2. For each, record location, version, and status.
  3. State the Phase 3 entry conditions: Stage 2 exit gate passed; capability-profile references exist on assets but their contents are Phase 3 work; the simulator's behaviour profiles are synthetic and documented as such.
  4. List open questions Phase 3 must not answer by itself (D1–D3 where still open; baseline method — depends on D1/D2; retention values — configuration).
  5. State the forecast non-delivery explicitly: no Phase 2 subtask produces a ForecastBundle or any forecast provider, and no forecast artefact is handed over. Phase 3's envelope and scheduling subtasks that name forecast inputs consume the Phase 3 synthetic forecast fixture (P3.31), not a Phase 2 output — §23 Stage 2 deliverables do not include forecasts, so this is a stated boundary, not a gap to be filled by expanding Stage 2.
- **Deliverable / output:** Handover pack index for Phase 3.
- **Acceptance criteria / exit check:**
  - [ ] Every P2.01–P2.27 deliverable appears with location + status.
  - [ ] Capability-profile-reference status is stated (reference exists, contents are Phase 3).
  - [ ] Forecast non-delivery is stated: no forecast artefact is handed over; Phase 3 consumes its own synthetic fixture (P3.31).
  - [ ] Open questions list the gate owning each.
- **Owner role:** Principal engineering
- **Dependencies:** P2.01–P2.27
- **Verification method:** Phase 3 lead confirms each artefact before Phase 3 starts.
- **Risks / failure modes:** Phase 3 treating simulator profiles as validated physical models. Mitigation: synthetic status stated in the handover.

## P2.29 — Hold the Stage 2 exit-gate review

- **Objective:** Run the formal §23 Stage 2 exit gate and record its outcome.
- **Report trace:** §23 Stage 2 (exit gate)
- **Inputs / prerequisites:** P2.01–P2.28, P2.31; `CHECKLIST.md` for this phase.
- **Steps:**
  1. Convene the reviewers (engineering, product, power systems — plus security for the P2.22 items).
  2. Walk `CHECKLIST.md` item by item with evidence (test outputs, CI runs, the P2.26 demonstration record, the P2.24 replay report).
  3. Re-run the P2.26 demonstration (or review a fresh recorded run) during the review, including the replay verdict.
  4. Record the outcome: pass, pass with conditions (listed with owners/dates), or fail (blocking items listed).
- **Deliverable / output:** Exit-gate review record.
- **Acceptance criteria / exit check:**
  - [ ] Every `CHECKLIST.md` item has an evidence reference or a recorded exception.
  - [ ] The exit-gate demonstration + replay were shown (live or fresh recorded run).
  - [ ] The outcome is recorded with conditions or blocking items.
- **Owner role:** Principal engineering (chair)
- **Dependencies:** P2.01–P2.28, P2.31
- **Verification method:** The review record itself.
- **Risks / failure modes:** Gate passed without the replay verdict. Mitigation: replay report is a required exhibit.

## P2.30 — Record what Phase 2 explicitly did NOT build (anti-scope audit)

- **Objective:** Close Phase 2 with an explicit negative list.
- **Report trace:** §23 Stage 2; §2.1 (v0.1.0 contents)
- **Inputs / prerequisites:** P2.01–P2.29.
- **Steps:**
  1. List what was not built: no flexibility envelopes or capability-model computation (Phase 3); no scheduling/optimization (Phase 3); no grid feasibility or command lifecycle beyond the domain state machine (Phase 4); no operator console (Phase 5); no edge agent or protocol adapters (Phase 6); no plugin host/WASM (Phase 7); no Python workers; no market integration; no real site/field telemetry ingestion (simulator only).
  2. List what was not decided: D1–D6 where still open; no baseline method (depends on D1/D2); no retention values as code (configuration only); no TimescaleDB adoption (measured-volume trigger not met).
  3. List what was not claimed: no field-read capability; no device-write capability; no control authority above Simulation; no conformance/certification claim; no performance target (observed numbers only, P2.26); simulator profiles are synthetic, not validated physical models.
  4. Verify the repository state: only Phase 2 artefacts added (registry/telemetry code, scenarios, migrations, CI extensions); `docs/foundations/*` untouched; the report untouched; nothing committed or pushed by this ledger work (per task instruction).
  5. File the negative list and reference it from the Phase 3 handover pack (P2.28).
- **Deliverable / output:** "What Phase 2 did not build / decide / claim" list.
- **Acceptance criteria / exit check:**
  - [ ] The negative list covers code, decisions, and claims per steps 1–3.
  - [ ] Repository state verified: no Phase 3+ artefacts present; foundations and report untouched.
  - [ ] The list is referenced from the handover pack.
- **Owner role:** Principal engineering
- **Dependencies:** P2.01–P2.29
- **Verification method:** `git status`; file listing.
- **Risks / failure modes:** Simulator telemetry mistaken for field telemetry in a status report; capability profiles assumed implemented because the reference exists. Mitigation: the negative list states both explicitly.

---

## P2.31 — Versioned telemetry corrections and as-of (point-in-time) replay (foundations §6.4)

- **Objective:** Implement the foundations §6.4 correction rule that no earlier Phase 2 subtask carries: telemetry correction and late arrival must be append-only or versioned, so historical decisions can be replayed using the data available at decision time. P2.24 replays by re-running the simulator; this subtask is the storage/replay-semantics half — what a correction is, and how a past decision is re-derived from past data.
- **Report trace:** Foundations v1.0 §6.4 (governs — report rank 2); §10.4 (quality states); §19.2 (deterministic scenario tests); P2.07–P2.10, P2.24.
- **Inputs / prerequisites:** P2.04–P2.06 (telemetry model, storage, repositories); P2.09 (duplicate/late handling); P0.08 (quality vocabulary).
- **Steps:**
  1. Define the correction record: a correction is a new versioned record (new row/version), never an in-place update of the original value; it carries the corrected value, the original record's identity, the correction reason, the actor/source, and the correction timestamp. The original row remains queryable.
  2. Define the late-arrival rule consistently with P2.09: a late record for an already-covered interval is stored as a versioned correction (or as the first record, if the interval was MISSING) — never silently overwriting the value a past decision used.
  3. Define as-of query semantics: a query names an as-of time; it returns, per (asset, event_time, quantity), the latest version whose correction timestamp is ≤ the as-of time. The default (no as-of) returns the latest version. Record the semantics in the repository contract and in the API query parameters (P2.12/P2.19).
  4. Implement the storage: version column + correction linkage on the telemetry table (extend the P2.06 migrations — do not recreate the table); a unique constraint that permits multiple versions of the same (asset, event_time, quantity) distinguished by version; the current-state view (P2.18) reads the latest version only.
  5. Implement the as-of replay entrypoint (distinct from the P2.24 simulator replay): given a past decision's timestamp, re-derive the decision's input snapshot from the as-of view and compare it with the snapshot the decision actually used; report match/mismatch per record with a diff.
  6. Add tests: correction creates a new version and leaves the original intact; as-of query at T1 returns the pre-correction value and at T2 the corrected value; current-state view is unaffected by a superseded version; as-of replay reproduces the original decision inputs.
- **Deliverable / output:** Correction/versioning design + migrations + as-of query + as-of replay entrypoint + tests.
- **Acceptance criteria / exit check:**
  - [ ] A correction never mutates the original row (test: original value still queryable after correction).
  - [ ] As-of queries return the correct version per correction timestamp (T1 pre-correction, T2 post-correction — test).
  - [ ] The current-state view reads the latest version only (test).
  - [ ] As-of replay reproduces a past decision's input snapshot from the as-of view (test with a diff on mismatch).
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P2.04–P2.06, P2.09, P2.18, P2.24
- **Verification method:** Integration tests on PostgreSQL 18; as-of query tests; replay-diff test.
- **Risks / failure modes:** A correction silently rewriting history so a past decision can no longer be explained; as-of and latest-version queries confused in the API. Mitigation: append-only versions + the as-of test pair (T1/T2) as a gate.

---

## Traceability summary

| Report section | Subtasks |
|---|---|
| §4 (Asset Registry, Telemetry) | P2.02–P2.03, P2.10 |
| §6.4–§6.5 (pipeline, storage strategy) | P2.05–P2.07 |
| §9.1 (TelemetrySource) | P2.11, P2.14 |
| §10 (units, time, quality, entities) | P2.02, P2.04, P2.08 |
| §11 (capability vocabulary, reference only) | P2.02 |
| §15 (APIs, events) | P2.12, P2.16–P2.17 |
| §16 (storage, tenancy, retention) | P2.05–P2.06, P2.20 |
| §17 (security) | P2.22 |
| §19 (simulation, testing, benchmarks) | P2.13–P2.15, P2.23–P2.24, P2.27 |
| §20.1 (workspaces, API/views only) | P2.17–P2.19 |
| §21.3 (observability) | P2.21 |
| §22.2 (CI gates) | P2.27 |
| §23 Stage 2 (exit gate) | P2.25–P2.26, P2.29 |
| §24 steps 1–2, 10 | P2.13, P2.25, P2.24 |
| Foundations v1.0 §6.4 (versioned corrections, as-of replay) | P2.31 |

Subtask count: **31** (P2.01–P2.31).
