# P2.23/P2.24 — Fault-injection suite and deterministic replay

| Field | Value |
|---|---|
| Subtasks | P2.23 — Phase 2 fault-injection suite (ingestion + storage faults); P2.24 — deterministic replay (scenario → stored telemetry → replay output) |
| Report trace | P2.23: §19.2 (test layer 6), P1.24, P2.15. P2.24: §23 Stage 2 (exit gate "replayed deterministically"), §24 step 10, §19.2, P2.13 |
| Inputs | P2.10 (ingestion service), P2.14–P2.15 (simulator + fault plan), P1.24 (outbox/broker fault pattern), P2.13 (determinism scope and versioning rule) |
| Status | Code + tests WRITTEN, NOT RUN (loop bans). Every "expected outcome" below is what the tests encode; the OBSERVED outcomes are recorded by the human's run (`cargo test -p flexcore-worker --test fault_suite`, `cargo test -p flexcore-sim-runner`) |
| Scope note | All artefacts are simulation-only (the entire 0.x line). The fault kinds are platform-handling tests and simulator faults — not claims about real device failure modes. No safety, certification, or control-authority claim anywhere |

---

## 1. P2.23 — the fault-injection suite

Suite location: `apps/worker/tests/fault_suite.rs`, driving the SAME
ingestion pipeline the worker hosts (`flexcore_worker::ingestion`), on real
PostgreSQL (the P1.13 harness convention, `FLEXCORE_TEST_DATABASE_URL`), per
the recorded P2.06 seeding pattern. Every test asserts the fault condition
FIRST, then the handling (ledger step 5).

### 1.1 Results log — expected outcomes (tests written, not run)

| # | Fault | Driver | Assertion (fault first) | Expected outcome |
|---|---|---|---|---|
| 1 | Simulator stops mid-batch | Partial batch (first 6 of 12 horizon ticks) through `process_batch` | `stored == 6` and `count == 6` (< horizon) | Partial batch stored, no ambiguous rows; remainder retried after resume → 12 stored, 0 duplicates, 0 quarantined |
| 2 | Ingestion service restart mid-batch | Fresh `IngestionTask` over the SAME store + durable inbox; the redelivered full horizon | `count == 6` after the first instance "dies" | Overlap detected by the inbox (`redeliveries_skipped == 6`), remainder stored (`stored == 6`), exactly one row per record (12), 0 quarantined |
| 3 | NATS down during ingestion | Outbox-only ingestion of 24 records with NO publisher connected (P1.24 pattern); broker connects for the recovery leg | `count == 24` while pending outbox entries for the asset ≥ 24 | State + outbox rows written while the broker is unreachable; `drain_once` on recovery empties the asset's pending set; rows delivered, never deleted (`count(*) FROM outbox` unchanged) |
| 4 | PostgreSQL down during ingestion | A store pointed at an unreachable endpoint (`127.0.0.1:1`); the fault surfaces at `begin` (or already at `connect`) | `process_batch` returns `Err` | Clean failure: no partial batch (only the baseline row), failed record neither stored nor quarantined, committed outbox entries not lost (present in ANY delivery state) |
| 5 | Partition missing for the ingest frontier | event_time in June 2031 (deliberately unpartitioned) | `report.out_of_range_quarantined == 1` | Clear per-record error ("outside every existing telemetry partition"), quarantined as evidence (stage `5-duplicate-handling`), counted under `out_of_range_event_time`, current state untouched |
| 6 | Clock fault — event_time far future | The versioned scenario's `horizon.start` moved to 2099-06-01 (scenario-config driver; ticks at the scenario cadence) | all 10 records rejected | Every record quarantined with the partition reason; nothing stored (`count == 0`); no GOOD current state for the asset (latest-per-asset read is empty) |
| 7 | Clock fault — event_time far past | The versioned scenario's `horizon.start` moved to 2020-01-01 (scenario-config driver) | `staleness(event_time, now, 60 s limit)` is STALE for every record BEFORE ingestion | Records store as valid history; the P2.08 staleness signal fires for all 10 (`stale_observations == 10`); stored quality NOT rewritten (`quality_states[GOOD] == 10`, `[STALE] == 0`); the latest-per-asset record is stale under the same limit — never silently GOOD as current state |
| 8 | Volume fault — burst above the backpressure limit | 64 frames offered into a capacity-8 bounded channel, no consumer running | `accepted == 8`, `shed == 56`, `frames_shed == 56` | Shed policy observed (retryable `FrameShed`), metric incremented; the pipeline drains exactly the buffered 8 (`frames_processed + drained == 8`); memory bound = the recorded capacity; the repeats deduplicate (1 stored row) |

### 1.2 Recorded constraints and honest boundaries

- **Clock-fault driver constraint.** The ingestion service depends only on
  the `TelemetrySource` port (P2.10 acceptance), so `apps/worker/tests` cannot
  import `flexcore-simulator`. The clock-fault event times are DERIVED from
  the versioned scenario configuration (the `horizon.start` mutation, the
  cadence read from `cadence.default_seconds`) — the tick sequence the P2.14
  engine emits for that configuration, not arbitrary hand-picked values. The
  fully engine-driven end-to-end run is the Compose-stack exit-gate path
  (P2.25/P2.26).
- **Clock-fault handling split, per the P2.08 rules in `ingestion.rs`:**
  far-future (outside every partition) → quarantined with a clear reason;
  far-past (storable history) → stored as measured history, with the
  staleness signal + the view-time STALE rule preventing a silently-GOOD
  current state. The pipeline never rewrites stored quality (P2.08).
- **SUSPECT path via scenario faults.** The out_of_range fault
  (source code `OUT_OF_RANGE` → SUSPECT through the declared quality
  mapping) is exercised end-to-end at the source level by P2.15's tests and
  through the pipeline by the P2.16 quality-event tests; the fault-suite
  table above covers the ingestion/storage/clock/volume faults the ledger
  names for P2.23.
- **NATS-down modelling.** The pipeline is outbox-only (report §6.3), so a
  broker outage cannot touch the state write path; the test models the
  outage as "no publisher connected" (P1.24 pattern) and drains on
  recovery, including provisioning the `FLEXCORE_TELEMETRY` stream (the
  four Phase 1 streams do not include it).
- **Storage faults on real PG.** The `TelemetryIngestStore` is hard-wired to
  the Postgres unit of work by design (P2.10), so the storage-fault tests
  are DB-gated; there is no fake unit-of-work path for the worker's
  pipeline (the fakes cover the P2.09/P2.12 seams, already tested).

## 2. P2.24 — deterministic replay

### 2.1 DECISION — the replay entrypoint (ledger step 2, made once)

**A minimal `apps/sim-runner` CLI crate is the replay entrypoint**, adopting
the ledger's default: replay is a DISTINCT entrypoint from background
ingestion (different trigger, different consumer, different failure
surface), so it is not a worker task and not a control-plane route. The
crate is a composition-root binary (`flexcore-sim-runner`) over a thin
library (`src/lib.rs`) so the behaviour is integration-testable.

| Subcommand | Behaviour |
|---|---|
| `run --scenario <path> [--seed <n>] [--out <path>]` | Executes the scenario version + seed through the deterministic simulator (via the `TelemetrySource` port: `pull_frame` + `decode` — no engine internals) and emits the canonical replay records as a JSON array (file or stdout). This output IS the reference-file form |
| `replay --scenario <path> --reference <path> [--seed <n>] [--code-version <string>] [--report <path>]` | Re-runs the scenario and compares against the reference records file; emits the replay report (JSON + human summary; report JSON to `--report` when given) |

Dependency surface: flexcore-simulator, flexcore-contracts,
flexcore-domain, flexcore-ports, serde/serde_json. NO database, NO broker.
The coordinator adds `apps/sim-runner` to the workspace members (the root
`Cargo.toml` was not edited by this subtask — see the manifest header note).

### 2.2 DECISION — the comparison fields (ledger step 3, frozen by type)

**Comparison is on exactly `(asset_id, event_time, quantity, value, quality,
source_sequence)` — NEVER on `ingest_time` (P2.13 §3).** The comparison
tuple is frozen structurally: the `ReplayRecord` type has no `ingest_time`
field, so a comparison that touches it cannot compile. `ingest_time` is
read from the decoded records only to be dropped at the record boundary
(`record_from_normalized`). The dedicated test rebuilds one run's records
with a +37 h ingest-time shift and proves the comparison still passes with
zero mismatches (`two_runs_with_different_ingest_times_still_match`).

Multiset semantics: duplicate identities (the `duplicate` /
`conflicting_duplicate` fault kinds, P2.15) pair up in deterministic sorted
order, so faulted scenarios replay identically and a de-duplicated
reference surfaces the surplus explicitly. Values compare with exact f64
equality (byte-identical determinism, P2.13 §3 — nothing is rounded into
plausibility).

### 2.3 DECISION — the replay report and exit codes (ledger step 4)

The report (`src/report.rs`) carries: `scenario_id`, `scenario_version`,
`seed`, `code_version`, `reference_count`, `candidate_count`,
`match_count`, `mismatch_count`, `verdict` (`"pass"`/`"fail"`), and
`mismatches` — each mismatch naming the diverging field (`value`,
`quality`, `missing_in_candidate`, `missing_in_reference`) and carrying both
records where both exist. Rendered as JSON (machine-readable) plus a human
summary (verdict line, counts, up to 20 rendered diffs).

**Exit-code contract (recorded):** replay pass → **0**, replay mismatch →
**1**, usage/config error → **2**. The mapping is a pure,
fail-closed function (`exit_code_for_verdict`: an unknown verdict maps to
2, never to a pass). CI's P2.27 scenario-regression gate asserts the
report's VERDICT, not just the exit code.

The code version travels from `--code-version`, else the
`FLEXCORE_CODE_VERSION` environment, else the literal `dev-unrecorded` — an
unrecorded code version is stated, never faked (P2.13 §4: replay always
names scenario version + seed + code version).

### 2.4 DECISION — the reference surface (recorded follow-up)

In Phase 2, `replay` compares against a **records file** (the `run --out`
form). The stored-telemetry comparison path — reading the reference through
the `TelemetryReader` port (`flexcore-ports::telemetry_read`) against the
Postgres adapter — is DESIGNED but not wired here: the Postgres read
adapter is under concurrent change (P2.19 historical queries), and wiring
it prematurely would couple this subtask to an in-flight surface. The
comparison is record-based, so a stored reference is a mapping onto
`compare()` (reader → `Vec<ReplayRecord>` → `compare`), not a redesign.
Recorded as the wiring follow-up; no dead config path ships in the CLI.

### 2.5 The stand-in identity (recorded)

The scenario speaks stable asset keys; the key → `AssetId` mapping happens
at registration through the API (P2.25 step 2). The sim-runner registers
nothing, so it derives a DETERMINISTIC stand-in UUID per asset key (two
FNV-1a-64 digests over the key and a domain-separated prefix — the same
standard constants the simulator's recorded derivation rule uses) and a
fixed stand-in tenant scope. Stable across runs — which is what the
comparison needs — and clearly labelled a stand-in, never presented as a
registry-issued ID. Tenant is not a comparison field.

### 2.6 Tests (apps/sim-runner/tests/replay.rs — pure, no DB)

| Test | Ledger acceptance | Expected outcome |
|---|---|---|
| `replay_of_the_exit_gate_scenario_passes_with_zero_mismatches` | Replay of the exit-gate scenario passes with zero mismatches | Two independent runs of `scenario-v1.json` compare equal; `match_count == record count` |
| `the_reference_file_round_trip_preserves_replay_equality` | (file-mode support) | The JSON reference form round-trips exactly and compares pass; the file contains no `ingest_time` |
| `a_tampered_seed_fails_with_a_per_record_diff` | Changed-seed replay fails with a diff | Seed 20260602 via the seed override → verdict fail, mismatch list non-empty with per-record diffs incl. value diffs |
| `a_generation_altering_change_breaks_replay_and_reports_it` | A generation change fails honestly (expected, not hidden) | One value altered between reference and fresh run → verdict fail, the diff names the exact record and field |
| `two_runs_with_different_ingest_times_still_match` | Comparison excludes `ingest_time` | One run's records rebuilt with +37 h ingest times (the disagreement asserted first) → comparison passes with zero mismatches |
| `the_replay_report_carries_every_required_field_and_verdict` / `a_failing_report_lists_the_mismatches_with_diffs` | Report contains scenario version, seed, code version, counts, verdict | All required fields present in the JSON rendering; diffs serialize with both records |
| `the_exit_code_contract_is_pass_zero_mismatch_one_usage_two` | Exit-code contract | 0 / 1 / 2, fail-closed on an unknown verdict |
| `the_replay_records_are_canonical_units_and_six_state_vocabulary` | Canonical-value sanity (P0.07/P0.08) | Finite powers, SOC in [0,1], reactive values always carry their declared convention, quantities in the frozen vocabulary, all-GOOD for the disabled-plan v1 run |

## 3. Acceptance-criteria mapping

| Ledger criterion | Where satisfied |
|---|---|
| P2.23: all fault cases pass with recorded outcomes | §1.1 table (expected); the human's run records the observed outcomes |
| P2.23: no partial batch left ambiguous | Faults 1, 2, 4 (all-or-committed; store-down rolls back everything) |
| P2.23: clock-skew records flagged, not stored as GOOD | Faults 6, 7 (quarantine; staleness signal + view-time rule) |
| P2.23: each test asserts the fault condition first | Every test's FAULT FIRST block (§1.1) |
| P2.24: replay entrypoint decided | §2.1 (minimal `apps/sim-runner` CLI) |
| P2.24: comparison on the frozen tuple, never ingest_time | §2.2 (+ the structural no-field argument and the shift test) |
| P2.24: report with version/seed/code/counts/verdict | §2.3, `src/report.rs` |
| P2.24: exit codes recorded | §2.3 (0/1/2, fail-closed) |
| P2.24: tampered-seed and generation-change tests | §2.7 (both fail WITH diffs) |

## 4. VERIFY-BUILD items and follow-ups

- **VERIFY-BUILD:** the PG-down fault uses
  `postgresql://127.0.0.1:1/...?connect_timeout=1` — the fast-fail relies
  on sqlx honoring `connect_timeout` as a URL parameter; if not honored the
  assertions still hold (the fault surfaces at the pool's acquire timeout,
  slower). Marked at the test site.
- **Coordinator wiring (not done here, per ownership):** add
  `apps/sim-runner` to the root `[workspace] members`; the manifest header
  records it.
- **Follow-up wiring:** the stored-telemetry reference through the
  `TelemetryReader` port (§2.5), once the P2.19 Postgres read adapter
  stabilizes.
- **Not run:** both suites are written but NOT executed (loop bans). The
  run commands are `cargo test -p flexcore-worker --test fault_suite` (DB
  service required; the NATS case additionally needs
  `FLEXCORE_TEST_NATS_URL`) and `cargo test -p flexcore-sim-runner` (pure).
