# P2.25 / P2.31 — Exit-gate synthetic portfolio and versioned corrections + as-of replay (design + decisions)

| Field | Value |
|---|---|
| Subtasks | P2.25 — Exit-gate synthetic portfolio (BESS + PV + load + EVSE, 24-hour horizon); P2.31 — Versioned telemetry corrections and as-of (point-in-time) replay |
| Report trace | §23 Stage 2 (exit gate); §24 steps 1–2, 10; §10.4, §16.1; P0.07, P0.08, P0.09; **foundations v1.0 §6.4 (governs, report rank 2)** |
| Inputs | P2.02–P2.06 (registry, telemetry model/storage/repositories); P2.08–P2.10 (quality, dedup, ingestion); P2.13–P2.15 (scenario model, simulator, fault plan); P2.18–P2.19 (current-state, historical view); P2.24 (deterministic replay) |
| Status | Implementation record for the P2.25/P2.31 decisions the ledger asked to make once. Code lives in `scenarios/synthetic-portfolio/scenario-v2.json` (the exit-gate artefact), `crates/adapters/postgres/migrations/0012_correction_versions_p2_31.{up,down}.sql`, `crates/adapters/postgres/src/corrections.rs` (+ minimal additive edits in `telemetry.rs`), `apps/sim-runner/src/as_of.rs` (+ the `as-of` subcommand), `apps/control-plane/src/routes_phase2.rs` (the `as_of` query parameter), and the test suites `crates/adapters/postgres/tests/corrections.rs`, `apps/sim-runner/tests/as_of.rs`, `apps/control-plane/tests/exit_portfolio.rs` |
| Execution status, recorded honestly | The loop bans of this subtask forbid running the API, the simulator pipeline, cargo, and any test runner. The exit-gate REGISTRATION + 24-hour RUN + INGESTION are therefore executed by a human via `25_exit_gate_runbook.md` (P2.25 steps 2–3); the tests and tooling here are the verification instruments. Additionally, at the time of writing the worker composition (`apps/worker` `compose_ingestion`) still returns `None` — no `TelemetrySource` is linked at composition (the recorded P2.10/P2.14 wiring status) — so the runbook names exactly what must land before the run step executes |
| Decisions | D-E1..D-E4 below (P2.25); D-C1..D-C8 below (P2.31), made once for Phase 2 |

---

## 1. P2.25 — The exit-gate portfolio decisions

### D-E1 — The exit-gate scenario is `scenario-v2.json`; the version bump is recorded (decided once)

P2.25 step 1 verified `scenario-v1.json` against the P2.13 rule and the
P2.25 shape: scenario id, version, seed + derivation rule, cadence, 24-hour
horizon, two sites each with the five asset types, capability-profile
references, bus references, and the P2.15 fault plan were all present. ONE
item was missing: a **per-asset expected-quality plan tied to the P2.15
fault types** — the accounting input the coverage check needs so injected
MISSING intervals are distinguishable from unexplained gaps (P2.25 step 4
and the ledger's named fault-vs-bug risk). Per the P2.13 versioning rule
(a content change creates a NEW version, never an in-place edit of a
released version), the extension is **`scenario-v2.json`**:

- `meta.version` = `v2`; everything else from v1 is byte-identical
  (portfolio, horizon, cadence, seeds, fault injections).
- The new top-level `expected_quality` section carries, per injected asset:
  the window, the fault type, the expected stored quality state inside the
  window, and `coverage_in_window` = `expected_gap` (the `drop` fault —
  covered only by the gap detector's synthesized `MISSING` markers,
  P2.04 D-T6) or `covered` (all six other fault types still emit one value
  per tick, so they do not reduce coverage).
- The simulator tolerates the section (unknown top-level keys are ignored;
  only behaviour-profile `params` objects are strictly validated), so the
  P2.24 `run`/`replay` CLI accepts v2 unchanged. The P2.24 replay
  regression tests keep pinning v1; the exit gate (P2.25/P2.26) runs **v2**.

### D-E2 — Registration, run, and ingestion are the human's execution via the runbook (decided once, honestly recorded)

P2.25 steps 2–3 (registration through the API, not SQL; the 24-hour run
ingested through the pipeline) could not be executed under this subtask's
loop bans. The deliverable is `25_exit_gate_runbook.md`: the exact command
sequence (Compose stack, migrations, the API registration calls with the
full request bodies derived from `scenario-v2.json`, the runbook's UUID
capture step, the simulator/ingest invocation, and the per-asset
per-quantity expected counts). **Recorded blocker at writing time:** the
worker's `compose_ingestion` (apps/worker) returns `None` — the simulator
source is not yet linked at composition — so the run step of the runbook is
executable only after that recorded P2.10/P2.14 wiring lands. The runbook
states this in place and does not paper over it.

Expected counts (faults disabled; derived from the recorded per-type
emission sets of P2.13 §5 and the D-SC3 cadence — 1440 ticks):

| Asset type | Quantities emitted | Records per asset |
|---|---|---|
| BESS | active_power, reactive_power, soc, availability, operating_mode, health | 6 × 1440 = 8 640 |
| PV | active_power, reactive_power, availability | 3 × 1440 = 4 320 |
| Flexible load | active_power, baseline_reference | 2 × 1440 = 2 880 |
| EVSE | active_power, soc, energy, connection_state | 4 × 1440 = 5 760 |
| Site meter | active_power, reactive_power | 2 × 1440 = 2 880 |

Two sites × 5 assets → **48 960 records** for the full horizon. The
site-south EVSE's second window is inside the horizon; no per-asset cadence
override is used in v2, so every (asset, quantity) pair expects exactly
1440 records.

### D-E3 — Coverage accounting with expected-gap accounting (decided once)

The coverage tool is pure logic + a DB-gated proof
(`apps/control-plane/tests/exit_portfolio.rs`): per (asset, quantity), each
expected tick is classified as

| Tick state | Classification |
|---|---|
| A value record exists | covered by data |
| Only a MISSING marker exists, inside an injected window | covered by expected gap (the fault plan accounts for it) |
| Only a MISSING marker exists, OUTSIDE any injected window | unexpected gap marker — gate-failing |
| No record, inside an injected window | expected gap without its marker yet — visible, not silent |
| No record, outside any injected window | **unexplained gap — gate-failing** (fault vs bug is distinguishable) |

The DB-gated test runs the same function on records read back through the
Postgres stored-telemetry read path (the port-only rule holds: no simulator
dependency in control-plane tests; the real 48 960-record volume is
checked by the runbook's SQL counts + this accounting on a sample).

### D-E4 — Canonical checks are fixture-based (decided once)

The canonical-value checks (units from the frozen quantity map, exactly one
of the six quality states, value totality, SOC ∈ [0,1], P > 0 = export)
run over hand-written canonical fixtures in the control-plane test suite —
worker/control-plane tests never depend on the simulator (the recorded
port-only rule, P2.10). The fixtures encode the rules; the runbook's run
produces the real volume the gate asserts them against, plus the runbook's
SQL spot-checks on the stored rows.

---

## 2. P2.31 — Correction and as-of decisions

### D-C1 — Storage shape: new rows in the same table, version column, extended PK (decided once)

Migration 0012 extends `telemetry` ADDITIVELY (never recreating it):

| Column | Meaning |
|---|---|
| `telemetry_version integer NOT NULL DEFAULT 1` | 1 = the original record (every pre-0012 and pipeline-written row is an original with zero pipeline change); ≥ 2 = corrections |
| `correction_of uuid NULL` | the superseded row's `telemetry_id` |
| `correction_reason text NULL` | why (mandatory, non-blank, for versions ≥ 2) |
| `corrected_by text NULL` | the actor/source (mandatory, non-blank, for versions ≥ 2) |
| `corrected_at timestamptz NULL` | the correction's own timestamp — when it became known (foundations §6.4); the as-of visibility key for versions ≥ 2 |

A correction is a NEW ROW carrying the superseded record's storage identity
legs `(asset_id, event_time, source_sequence, quantity)`. The PRIMARY KEY
is extended to
`(asset_id, event_time, source_sequence, quantity, telemetry_version)` —
**the P2.05 D-P3 semantics stay intact** (the partition key `event_time`
remains in the PK, which PostgreSQL requires) — and the extended PK is the
unique constraint that permits multiple versions of one identity
distinguished by version. The constraint NAME `telemetry_pkey` is retained
so the P2.09 duplicate classification (which matches it by name) keeps
working unchanged. Append-only holds at the storage level: no runtime role
holds UPDATE or DELETE (D-P6 grants unchanged).

There is deliberately **no foreign key** on `correction_of`: `telemetry_id`
alone is not unique (the PK is composite) and PostgreSQL cannot enforce an
FK omitting PK legs. The linkage is enforced by the correction procedure,
which reads the original row inside the SAME transaction before inserting —
a stronger guarantee for this path.

### D-C2 — The correction procedure (decided once)

`PgUnitOfWork::insert_telemetry_correction` (`corrections.rs`), in order:
(1) the original (version 1) must exist and belong to the caller's tenant —
otherwise `OriginalMissing` (the caller stores through the normal ingest
path; see D-C3); (2) the new version is `latest + 1`, and `corrected_at`
must not precede the superseded version's `corrected_at` (the per-identity
as-of ordering must stay monotone, otherwise "latest version ≤ as-of"
becomes ambiguous); (3) the corrected content is re-validated through the
IDENTICAL record rules as any ingested record (tenant match, time ordering,
value-kind pairing, SOC range, finiteness — the shared `validate_record`);
(4) the insert is savepoint-guarded; a PK collision means a concurrent
correction created a newer version first — a rejected retry, never an
overwrite. The original row remains queryable unchanged
(`query_telemetry_versions` lists every version ascending).

A correction with content identical to the current version is NOT silently
deduplicated — it is an explicit actor action with a reason and becomes a
new version (append-only, auditable). Recorded on purpose.

### D-C3 — Late-arrival rule and its wiring point (decided once, honestly recorded)

The rule (ledger step 2): a late record for an already-covered interval is
stored as a versioned correction — or as the FIRST record if the interval
was not covered by a value record (only a derived MISSING marker or
nothing) — never silently overwriting the value a past decision used.
Encoded as the pure function `classify_late_arrival(facts)` with exactly
three outcomes: `AcknowledgeExactDuplicate` (P2.09, unchanged),
`StoreAsFirstValueRecord` (normal ingest path; the gap marker stays as
history), `RecordAsVersionedCorrection` (the explicit correction path,
D-C2).

**Wiring point, recorded honestly:** the P2.09 duplicate classification
lives in `telemetry.rs`'s insert path (`ConflictingDuplicate` → the P2.10
pipeline quarantines). The worker ingestion pipeline is OUTSIDE this
subtask's file ownership, and a conflicting duplicate arriving unannounced
must keep being quarantined — the pipeline must never auto-promote a
conflicting re-emission to a correction. The recorded wiring is therefore:
the pipeline keeps P2.09 behaviour unchanged; `classify_late_arrival` names
the decision the pipeline reports for a late arrival, and an authorized
actor submits the record through the correction path (reason + actor
mandatory), which is where the third branch is implemented. Hooking the
classifier into the worker's duplicate branch is the recorded follow-up
(one call site; the function is pure and adapter-public).

### D-C4 — As-of query semantics (decided once)

A query naming an as-of time returns, per storage identity
`(asset_id, event_time, source_sequence, quantity)`, the latest version
whose `corrected_at` ≤ the as-of time; the original (version 1) is ALWAYS
visible — an as-of view never un-sees that a record existed, it only
decides WHICH version was current. The default (no as-of) returns the
latest version. Implemented as an anti-join (`NOT EXISTS` a
strictly-newer version visible at the as-of instant), served by the
extended PK index prefix — no new index. The granularity is the storage
identity (which includes `source_sequence`); under the recorded P2.04
D-T2 grain (one row per identity per source sample) this is equivalent to
the ledger's "(asset, event_time, quantity)" wording, and corrections share
the full identity of the row they supersede.

Assumption, recorded: a derived MISSING gap marker and a measured value
sharing one (asset, event_time, quantity) with equal versions are
disambiguated by the higher `source_sequence`; the gap detector's markers
are assumed to carry sequence 0. VERIFY-BUILD: confirm the gap detector's
sequence convention when the P2.08 marker path is exercised on PostgreSQL.

### D-C5 — As-of on the API: the `as_of` parameter (decided once)

`GET /api/v1/telemetry/query` (P2.12) and `GET /api/v1/telemetry/history`
(P2.19, raw mode) accept an optional `as_of` query parameter (RFC 3339
UTC; malformed → 422 with the stable envelope, never a silent "latest").
Semantics: D-C4. Constraints, decided once:

- `as_of` + `cursor` on the query route → 422 (an as-of page is a complete
  snapshot bounded by `limit`; the keyset cursor is a latest-version
  parameter).
- `as_of` + `bucket` → 422 (record-level versions have no bucket semantics
  in Phase 2; extending aggregation over versions is an explicit future
  decision, not a silent reinterpretation of D-H2).
- The as-of read is served by the Postgres adapter's as-of methods inside
  the same unit-of-work transaction as the historical view (`RLS` applied,
  `spawn_blocking` hosting rule); the Test backend has no versioned storage
  and fails closed (infrastructure error), consistent with the recorded
  telemetry-port wiring status.
- Without `as_of`, both routes now return the LATEST version per identity
  (the visibility predicate with a NULL as-of) — a response can never show
  a superseded value next to its correction.

### D-C6 — Current-state reads the latest version only (additive change, recorded)

`latest_telemetry_per_asset` (P2.18) gained one ORDER BY leg:
`telemetry_version DESC` after `(event_time DESC, source_sequence DESC)`.
A superseded version of the same storage identity can therefore never win
the `DISTINCT ON`; with no corrections present, behaviour is byte-identical
to before (every row is version 1). Asserted by the DB-gated test
`superseded_version_never_reaches_the_current_state`.

### D-C7 — As-of replay entrypoint: the `as-of` subcommand (decided once)

`flexcore-sim-runner as-of --versions <path> [--as-of <unix-ms>]
--decision-input <path> [--report <path>]` is DISTINCT from the P2.24
simulator replay: it re-derives the decision's input snapshot from the
VERSIONED RECORD HISTORY (per identity, the latest version visible at the
as-of instant — the same rule as D-C4, pure, in
`apps/sim-runner/src/as_of.rs`) and compares it against the records file
the decision actually used, on the P2.24 comparison tuple
`(asset_id, event_time, quantity, value, quality, source_sequence)` —
never on `ingest_time` (P2.13 §3; correction metadata is view-selection
input, never compared). Exit codes: the recorded P2.24 contract (pass 0 /
mismatch 1 / usage 2). Data source: records FILES — the recorded sim-runner
dependency surface has no database/broker infrastructure, so the runbook
carries the SQL export that produces the versions file (the canonical
record shape plus `corrected_at_unix_ms`, `corrected_by`,
`correction_reason`); a future `TelemetryReader`-wired variant maps onto
the pure selection without a redesign (the recorded P2.24 pattern).

### D-C8 — Migration down-path is honest about data loss (decided once)

0012 down removes versioned corrections IRREVERSIBLY (the 4-leg PK cannot
hold two rows of one identity) and touches the RLS FORCE flag for the
duration of the migration so the owning role's DELETE is not silently
filtered to zero rows by the tenant policy (the fail-closed policy would
otherwise make the down a no-op that then fails on the PK re-add). Original
rows are never touched.

---

## 3. Acceptance-criteria coverage

| Ledger acceptance | Where satisfied |
|---|---|
| P2.25: ≥ 2 sites, each with the five asset types, registered via the API | `exit_portfolio.rs` shape test + runbook registration sequence (D-E2; execution is the human's) |
| P2.25: full 24-hour horizon ingested; per-asset coverage verified | Runbook expected-count table (D-E2) + the coverage accounting (D-E3) |
| P2.25: all records canonical | Canonical fixture checks (D-E4) + runbook SQL spot-checks |
| P2.25: injected faults accounted (MISSING listed, not unexplained) | D-E3 expected-gap accounting |
| P2.31: a correction never mutates the original row | D-C1/D-C2 + `correction_creates_a_new_version_and_the_original_stays_intact` |
| P2.31: as-of at T1 pre-correction, T2 corrected | D-C4 + `as_of_query_returns_pre_correction_at_t1_and_corrected_at_t2` |
| P2.31: current-state reads the latest version only | D-C6 + `superseded_version_never_reaches_the_current_state` |
| P2.31: as-of replay reproduces a past decision's inputs (diff on mismatch) | D-C7 + `apps/sim-runner/tests/as_of.rs` |
| P2.31: unique-constraint behaviour | `the_extended_pkey_still_classifies_plain_ingest_duplicates` + the monotonicity rejection test |

## 4. VERIFY-BUILD items (loop bans — the human's runs confirm)

1. Migration 0012 up/down replay on PostgreSQL 18 (partitioned-table PK
   drop/re-add; the down's RLS FORCE toggle) — exercised by the adapter
   integration suite's migration replay.
2. The `NOT EXISTS` visibility predicate's plan shape (PK-prefix index use)
   on representative volumes — the EXPLAIN run is the human's, as for
   D-H4.
3. The concurrent-correction PK collision classification (two simultaneous
   `insert_telemetry_correction` calls on one identity) — the single-thread
   tests prove the constraint path; the true race needs a parallel run.
4. The D-C4 gap-marker sequence-0 assumption (marker vs measured tie-break)
   against the P2.08 gap detector's actual marker identity.
5. The `as-of` CLI end-to-end against the runbook's SQL export (the export
   query's JSON shape matches `VersionedRecord`).
