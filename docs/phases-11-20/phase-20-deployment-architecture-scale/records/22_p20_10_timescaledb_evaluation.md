<!-- P20.10 — TimescaleDB evaluation record. ADOPTION-CRITERIA-FIRST record
     (X-8: criteria registered BEFORE any evaluation run that could fill
     them). The evaluation is EMPTY — no volume evidence exists, no
     TimescaleDB instance was installed, built, run, or benchmarked, and
     the trigger (telemetry volume evidence) is UNFIRED. The current 0010
     partitioning is the design of record; this record extends it, never
     silently replaces it. -->
# P20.10 — TimescaleDB Evaluation Record (criteria registered FIRST; evaluation EMPTY; trigger UNFIRED; no instance exists)

| Field | Value |
|---|---|
| Subtask | P20.10 — TimescaleDB evaluation record (`ledger_phase_20.md` §P20.10) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **EVALUATION RECORD — CRITERIA REGISTERED, EVALUATION EMPTY.** No TimescaleDB extension, container, or instance exists or was installed; no compression was enabled anywhere; no benchmark or evaluation run of any kind was executed; no volume figure exists. The trigger — telemetry volume evidence — is UNFIRED. |
| Trigger (named, UNFIRED) | Telemetry volume evidence [S-66]: the demonstration, on MEASURED partition volumes, that the 0010 partitioning's maintenance or query cost is no longer adequate. The trigger condition is defined by the criteria in §2; the evidence would come from the P20.01/P20.02 measurement layer (Agent A, consumed by citation) and from the P2.20 maintenance job's operational record — neither exists. |
| The seam of record | `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` — D-P7 states verbatim in substance: "TimescaleDB is NOT used (revisit trigger: measured volume; P2.05 §10)". This record is that revisit's decision machinery — it does NOT overturn D-P7, and the partitioned shape remains the design of record until the criteria below are met AND the human owner accepts an adoption. |
| Sources | [S-66] (TimescaleDB docs — mechanism-level capability classes only: compression, continuous aggregates; no benchmark or comparison figure is cited); migration 0010 (D-P1..D-P5, D-P7); `crates/adapters/postgres/src/partition_maintenance.rs` (the P2.20 monthly pre-creation + retention detach/drop path); P20.01/P20.02 (Agent A — the measurement layer whose figures would fill the evidence cells; cited by expected path, see the close-out reconciliation note) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No volume evidence exists; no evaluation has run; no TimescaleDB
component of any kind is present in this repository or its deployment
shape.** The single-host Compose stack runs PostgreSQL 18 (UNVERIFIED pin,
P10.13) with the 0010 monthly RANGE partitions pre-created by the P2.20
maintenance job — no auto-creation, no TTL, no scheduled delete (0010
D-P5). This record delivers exactly what plan §2 assigns: the adoption
criteria registered BEFORE any evaluation run (§2), the named-consumer
demand register for continuous aggregates with every demand cell EMPTY
(§3), the migration path from the 0010 partitioning with what moves,
what stays, and the rollback (§4), and the EMPTY evaluation record
structure (§5). The evaluation STAYS EMPTY until evidence exists — an
unfilled criterion is a non-adoption, which is the honest default.

## 2. Adoption criteria — registered BEFORE any evaluation run (X-8)

Adoption of TimescaleDB for the domain telemetry store requires ALL of
the following, evidenced on real measured volumes — each criterion was
registered here before any run that could fill it, and none is met
because no measurement exists:

| # | Criterion | Evidence required (from a real deployment's clocks, never estimated) | Status |
|---|---|---|---|
| C-1 | Compression need DEMONSTRATED on partition volumes: measured storage footprint of the retained `telemetry` partitions (per the deployment's signed retention configuration — no period exists while D1 is OPEN, P9.10 §6) exceeds the recorded storage envelope, AND measured ingest/query cost on those volumes misses a pre-registered threshold | Measured footprint figures + measured ingest/query timings + the pre-registered thresholds (X-8) they were judged against | `<EMPTY — UNMEASURED>` |
| C-2 | Continuous-aggregates demand from NAMED consumers: at least one recorded consumer (§3) whose measured query pattern is demonstrably served better by a continuous aggregate than by the existing P2.18/P2.19 composite-index + partition-pruning query shape | The consumer's measured query timings, both shapes, on real volumes | `<EMPTY — UNMEASURED>` |
| C-3 | The migration path of §4 has been rehearsed restore-first: the 0010→hypertable conversion exercised on a restored copy in isolation (the P12.03/P9.24 §5 restore-into-isolation discipline, cited), with the rollback path (§4.3) executed and verified | Drill transcript, all evidence cells filled FROM THE RUN | `<EMPTY — UNRUN>` |
| C-4 | The extension's licensing and packaging decision is recorded by the human owner (TimescaleDB's community license is UNVERIFIED — verify against the primary source at evaluation time; no license claim is made here) | Owner decision record with the primary-source citation | `<EMPTY — NOT STARTED>` |
| C-5 | The dependency-rule and migration-policy impact is reviewed: an extension is a DEPLOYMENT artifact (no new Rust crate, no workspace dependency — hard ban 7); the `flexcore_migrator` role gains extension-management DDL, which the P11.18 migration-policy audit (cited) must record | Review record | `<EMPTY — NOT STARTED>` |

A criterion left unfilled blocks adoption — fail closed. The criteria
never relax after a run begins: a worse-than-criterion result is a
FINDING with a disposition (the inherited P9.24 §4 judgement rule,
cited from the P12.04 register §4), never a re-worded criterion.

## 3. Continuous-aggregates demand register — named candidate consumers, demand cells EMPTY

The named consumers below are the recorded query-shape owners that COULD
demand continuous aggregates; the demand column is EMPTY everywhere —
naming a consumer is not evidence of demand, and none of these records
asks for TimescaleDB today:

| Named consumer (recorded) | Query shape of record (cited) | Aggregate demand | Evidence |
|---|---|---|---|
| Portfolio/asset views (P2.18/P2.19 query patterns) | Composite DESC indexes + partition pruning (`telemetry_asset_time_idx`, `telemetry_tenant_asset_time_idx`, `telemetry_asset_quantity_time_idx` — 0010) | `<EMPTY>` | `<EMPTY — UNMEASURED>` |
| P15 forecasting inputs (aggregated intervals per the P1.10 data-class vocabulary: `aggregated_intervals`) | The forecast-worker input pipeline (`apps/worker/python/forecast_worker/`) as recorded | `<EMPTY>` | `<EMPTY — UNMEASURED>` |
| P16 market/settlement interval aggregations (5-min/15-min class shapes) | The P16 market-record seams (cited for interplay; the settlement store design is the P16 record's, not redefined here) | `<EMPTY>` | `<EMPTY — UNMEASURED>` |
| Console dashboards over the `/metrics` registry | NOT a telemetry-store consumer — dashboards read the P11 metrics store (P11.10), not the `telemetry` table; recorded to prevent the classic conflation | n/a — out of scope | n/a |

## 4. Migration path from the 0010 partitioning — what moves, what stays, rollback

Recorded BEFORE any evaluation, so a future evaluation runs against a
reviewed path rather than improvising one. Nothing below has executed.

### 4.1 What moves

| Element | Movement |
|---|---|
| The `telemetry` parent table | Converted from `PARTITION BY RANGE (event_time)` to a TimescaleDB hypertable with the SAME partition key semantics (space partitioning, if used, is a re-derivation — never assumed) |
| Existing monthly partitions | Folded into the hypertable as chunks along the SAME monthly boundaries — the D-P2 boundary semantics (`[first day of month, first day of next month)`) are preserved so every query that prunes today prunes identically after conversion |
| The `telemetry_quarantine` table | NOT moved — it is not a time-series bulk table; it stays a plain RLS table (0010 design, unchanged) |

### 4.2 What stays (non-negotiable invariants)

| Invariant | Statement |
|---|---|
| D-P1 row grain | One row per (asset_id, event_time, quantity); the PK `(asset_id, event_time, source_sequence, quantity)` survives conversion (the partition-key-in-PK rule maps to the chunking key) |
| D-P5 retention | NO retention value, TTL, or scheduled-delete rule enters the DDL from TimescaleDB retention policies — retention remains per-data-class deployment configuration executed as an audited decision; any TimescaleDB retention/drop-policy mechanism is a deployment choice the owner records, never a silent capability adoption |
| D-P6 roles + RLS | `flexcore_ingest`/`flexcore_query` grants, ENABLE + FORCE RLS, and the exact P1.12 §5.1 policy text are preserved byte-for-byte through the conversion; the post-conversion drill re-verifies the policies (fail closed on any difference) |
| D-P4 indexes | The three composite DESC indexes are recreated on the hypertable unless the evaluation MEASURES that a TimescaleDB-native index shape serves the same P2.18/P2.19 patterns — a measured replacement, never a silent swap |
| P2.20 maintenance job | The monthly pre-creation loop is SUPERSEDED by chunk auto-creation ONLY if C-1..C-5 are all met and the owner accepts; until then the job stays the mechanism of record |

### 4.3 Rollback

The rollback is restore-based, never an improvised down-conversion: the
pre-conversion base backup + WAL (the P12.01/P12.02 posture, cited)
restores the pre-conversion cluster in isolation; the conversion is a
recorded, owner-approved change that appears in the migration-policy
audit (P11.18 discipline) as a named event. **No down-migration exists
for a hypertable conversion in this design** — the same honesty as the
P10.13 no-downgrade rule (cited from `deployment-upgrade-model.md` §3.3's
irreversible-migration framing): a conversion that fails its post-checks
is rolled back by restore, and the criteria of §2 include the rehearsed
rollback (C-3) precisely so the path is proven before any production
conversion is attempted.

## 5. The evaluation record structure — EMPTY

| Field | Value |
|---|---|
| Evaluation date / executor | `<EMPTY>` |
| Instance under evaluation (version, packaging, license verified against the primary source) | `<EMPTY>` |
| Dataset (real measured volumes; production-shaped SYNTHETIC data only per the P20.02 policy — never customer material) | `<EMPTY>` |
| C-1 storage-footprint figures | `<EMPTY — UNMEASURED>` |
| C-1 ingest/query timing figures vs pre-registered thresholds | `<EMPTY — UNMEASURED>` |
| C-2 consumer query timings (both shapes) | `<EMPTY — UNMEASURED>` |
| C-3 conversion + rollback drill transcript | `<EMPTY — UNRUN>` |
| C-4 owner license/packaging decision | `<EMPTY>` |
| C-5 migration-policy review | `<EMPTY>` |
| Verdict (meets criteria / does not) | `<EMPTY>` — never pre-judged |

## 6. What this record does NOT establish

1. No TimescaleDB component exists anywhere; no evaluation ran; no
   benchmark figure, comparison number, or compression ratio is recorded
   or hinted at — every cell is EMPTY.
2. It does not overturn D-P7: the 0010 partitioned shape is the design of
   record; this record registers the criteria that would justify a
   revisit, nothing more.
3. It does not close any E-decision; adoption is a human owner decision
   gated on the §2 criteria AND the owner's acceptance.
4. It adds no dispatch authority (ADR-010) and touches no runtime code,
   no migration, and no workspace dependency (hard bans 6/7).
5. §13.3 semantics: a future passing evaluation is a measured-day
   outcome on a finite dataset, never a guarantee about production
   behavior.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every §2/§3/§5 evidence cell is
   EMPTY/UNMEASURED/UNRUN, with the trigger recorded UNFIRED.
2. Confirm §2's criteria were registered BEFORE any evaluation run (X-8)
   and include the fail-closed rule; confirm the D-P7 citation in the
   header.
3. Confirm §3 names consumers with citation-resolvable query shapes and
   that no demand is asserted; confirm the dashboard row records the
   metrics-store/telemetry-store distinction.
4. Confirm §4.2's invariants (row grain, no-TTL, RLS byte-preservation,
   index-measured-replacement, P2.20 supersession rule) and §4.3's
   restore-based rollback with no down-migration.
5. Confirm no ledger checkbox was ticked by this agent and
   `docs/foundations/*`, `docs/plan/*`, `.agent/*`, and migration 0010
   are untouched.

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.10-1 | [S-66] is cited mechanism-level only (compression, continuous aggregates are capability CLASSES of the extension) | No capability, performance, or licensing claim is made; the community-license question is explicitly UNVERIFIED and gated on C-4 |
| A-P20.10-2 | The evidence that could fill C-1/C-2 would come from Agent A's P20.01/P20.02 measurement layer | First cited by expected path; reconciled to the landed records `records/01_p20_01_capacity_model_framework.md` and `records/02_p20_02_load_test_plan.md` during the same wave (see the close-out record) |
| F-P20.10-1 | The trigger is UNFIRED and the evaluation is EMPTY by design | Binding honesty statement |
| F-P20.10-2 | No retention period exists while D1 is OPEN, so even the C-1 footprint criterion cannot be scoped until the signed retention configuration exists | Carried, not hidden — the P9.10 §6 / P12.01 §5 no-basis rule governs |

Status: **WRITTEN-UNRUN, TRIGGER UNFIRED** — criteria cannot evaluate by
being registered; the volume evidence, the evaluation run, and the
owner's adoption decision are the human gates and have not happened and
are not scheduled.
