# P5.24 — Performance and load measurement (pre-registered LOAD-measurement plan + TEMPLATE — the v0.3.0 load baseline; measured cells EMPTY)

- **Date:** 2026-11-11 (template authored; the measured run has NOT happened).
- **Subtask:** P5.24 (ledger_phase_5.md §P5.24). Report trace: §19.2 layer 8
  (the platform's performance envelope), §19.4 (platform metrics:
  availability, throughput, p95/p99 API/ingest latency, recovery, tenant
  isolation), §21.3 (observability), §21.4 (browser matrix), §22.3
  (release gates — full load tests are a release-gate activity).
- **Method source:** the P3.27/P4.35 measurement discipline is REUSED (the
  P5.23 template, `23_benchmark_report.md`, states it once for Phase 5):
  fill a cell only from the produced record; unmeasured fields are
  `not_measured` WITH their reason, never zero; raw numbers only; CI runs
  are not baseline figures; conditions travel with every figure; no
  arbitrary universal targets (report §19.4 verbatim). The hardware block
  is the same P3.27 capture, so conditions stay comparable across P3.27 /
  P4.35 / P5.23 / P5.24 records.

## 0. Status — read first

**This file is a TEMPLATE. Every measured cell below is EMPTY. No load
figure exists anywhere for the platform yet, and none is claimed.** The
authoring followed the loop bans (nothing built, nothing run, nothing
installed — the load harness was written, never executed). The human's
recorded run (§4 procedure A) fills §6.

**The scope honesty block (binding, recorded ON every produced
`LoadMeasurementRecord` as `scope_notes`):**

1. **Console browser-side performance is NOT measured in this loop** —
   no browser automation exists in this phase. The initial-bundle-load
   and workspace-render figures are procedure B below: a MANUAL,
   pre-registered procedure recorded PER §21.4 browser, and no figure is
   ever claimed for a browser that was not tested. Nothing in the console
   tests (vitest, server-rendered strings) measures render time.
2. **The harness load is ONE sequential client.** It does not model
   concurrent users, backpressure, broker/database contention, or any
   production traffic shape. A figure from this harness is a
   single-client figure with its conditions; no capacity or concurrency
   claim may be attached to it.
3. **Command fan-out time and recovery time after a service restart have
   NO harness in 0.3.0** — the record emits them `not_measured` with the
   reason, and procedures C and D below are the pre-registered MANUAL
   paths. A fan-out or recovery figure is never synthesized from other
   measurements.
4. **No threshold anywhere gates on a load figure** (report §19.4: no
   arbitrary universal targets). The figures are the v0.3.0 load baseline
   inputs for later comparison, never targets; the regression-factor
   mechanism is the P5.23 §8 design (factor = CONFIGURATION set at
   baseline time, investigation flag never a gate).

## 1. The five ledger measures (P5.24 step 1) — coverage map

| Measure | Harness coverage in 0.3.0 | Procedure | What is NOT covered (honest) |
|---|---|---|---|
| Telemetry ingest rate | `load-run --ingest-scenario … --ingest-iterations N`: N sequential runs of the REAL deterministic replay pipeline (Phase 2 simulator → canonical records), timed per iteration; rate = total records / total wall clock | A (harness) | the POSTGRES/broker ingestion path is not exercised by this harness — the figures describe the replay pipeline, and the record's method string says exactly that |
| Scheduling runtime | REUSED — the P3.27 method owns the replay/scheduling runtime figures (`benchmark` subcommand; the solver-side fields stay `not_measured` until a real solve is wired, the G5 rule). NOT re-measured here differently. | P5.23 §5 | nothing new; this row exists to satisfy the ledger's "reuse, do not re-measure differently" rule |
| API query latency (p95/p99) | `load-run --query-url http://… --query-count M`: M sequential HTTP GET round trips (fresh TCP connection per trip, Connection: close), nearest-rank p50/p95/p99 over the measured samples | A (harness) | TLS endpoints are a NAMED error (plain HTTP only, std-only client); authenticated flows, POST/PUT latencies, and concurrent sessions are not exercised |
| Command fan-out time | NO harness — `not_measured` on every record | C (manual, pre-registered) | nothing measured; a concurrent-issue harness is a composition change no subtask owns yet |
| Recovery time after a service restart | NO harness — `not_measured` on every record | D (manual, pre-registered) | nothing measured; restart orchestration is out of the harness's scope |

Platform metrics the ledger P5.24 step 1 does not name but §19.4 lists
(availability, tenant isolation): availability evidence belongs to the
§24 exit-gate demonstration (P5.30) and tenant isolation to the P5.26 API
tests — THIS template does not re-measure them, and no figure here is an
availability or isolation claim.

## 2. The harness (what exists at authoring — written, never run)

`flexcore-sim-runner load-run` (P5.24, `apps/sim-runner/src/load_run.rs`,
wired additively in `main.rs`):

- **Load spec (validated fail-loud):** at least one leg configured;
  iteration counts ≥ 1; `http://` URLs only (an `https` URL is a named
  error — no silent fallback); measuring nothing is exit 2, never an
  empty record.
- **Ingest leg:** reads the pinned scenario, runs N sequential
  replay-pipeline iterations, records per-iteration wall clock; emits
  `telemetry_ingest_rate` (records/s), `telemetry_ingest_iteration_p95_ms`
  (nearest-rank p95), `telemetry_ingest_iteration_max_ms` (a raw
  observation) — each with its method string.
- **Query leg:** M sequential HTTP GET round trips against the configured
  endpoint (bounded 16 MiB read, Connection: close); emits
  `query_latency_p50_ms` / `p95` / `p99` — nearest-rank percentiles over
  the MEASURED samples only, no interpolation, no fitting (the method is
  documented in the module and pinned by tests).
- **Record:** `LoadMeasurementRecord` (schema
  `flexcore-load-measurement-record/v1`): the declared load spec, the
  load CONDITIONS (scenario id, iterations, record counts, URL, trip
  count, the single-client concurrency statement), the P3.27 hardware
  block, the results (measured / not_measured with reasons), the scope
  notes, the pre-registered manual procedures, and the run context.
- **Percentile method (recorded once):** nearest-rank — rank =
  ceil(fraction × n) clamped to [1, n] over the ascending-sorted samples;
  reproducible from the samples alone (pinned by tests).

What the harness deliberately does NOT contain: no concurrency, no
ramping, no authentication, no result thresholds, no averaging across
runs.

## 3. What is realistic for 0.x (and what would be overreach)

The 0.x line is a single-tenant, simulation-only line whose §24 vertical
slice is one portfolio of ten assets at a 1-minute cadence (the library
README and the P3.27 scaling probe define the scale classes). A
realistic 0.x load question is: "does the replay pipeline and a control
plane read endpoint answer a single sequential client in a stable,
reproducible envelope on the recorded hardware?" It is NOT: "what
throughput does a 10,000-bus utility deployment sustain?" — the library
README's scale-limit statement applies to load figures with the same
force (the first feeder tests do not establish capability for a 10,000-
bus utility network, §19.3). No load figure in this phase may be quoted
as a deployment capacity, a sizing input, or an SLA.

## 4. Procedures (pre-registered; each fills its own §6 table)

### Procedure A — the harness run (the human's single recorded run)

```sh
# The ingest leg (offline; no services needed):
cargo run -p flexcore-sim-runner -- load-run \
  --ingest-scenario scenarios/library/lib-02-small-portfolio.json \
  --ingest-iterations 10 \
  --code-version <record-your-code-version> \
  --out load-record-ingest-lib-02.json

# The query leg (needs the Phase 1-4 services running on the recorded
# host; the URL is the deployment's, recorded verbatim in the record):
cargo run -p flexcore-sim-runner -- load-run \
  --query-url http://127.0.0.1:<port>/api/v1/tenants \
  --query-count 100 \
  --code-version <record-your-code-version> \
  --out load-record-query.json
```

Record the §6 hardware block from the produced records verbatim.

### Procedure B — console performance (MANUAL; no browser automation exists)

Per §21.4 browser (current + previous major: Chrome, Edge, Firefox,
Safari — record PER browser, never a combined figure):

1. Serve the built console bundle the way the deployment does (control
   plane or CDN, §21.4) with the browser's cache DISABLED (a cold-load
   figure; state the choice).
2. Record: initial bundle load (network transfer + parse) and the render
   time of ONE workspace (the Overview workspace is the default), using
   the browser's own performance tooling (e.g. the navigation timing
   entry). State the tool used.
3. Repeat on the browser matrix actually tested; a browser not tested
   gets NO figure — the row says `not tested (browser unavailable)`.

The CI level for the browser matrix is whatever P5.12 records (the full
matrix is a §22.3 release-gate activity) — record which level ran; the
console performance figures themselves are MANUAL in this phase either
way.

### Procedure C — command fan-out (MANUAL, pre-registered; NOT harness-measured)

Definition (pre-registered so the procedure cannot drift): the wall
clock from the approval of K commands (K = the §24 slice's fleet size —
the scenario's dispatchable assets) to the last simulator ACKNOWLEDGED
journal entry, measured from the audit trail's timestamps (the §15.3
command events) — acknowledgement is application-layer acceptance, never
physical delivery (§14.1). Run it on the §24 slice scenario with the
recorded seeds; record K, the scenario id, and the figure together.

### Procedure D — recovery after restart (MANUAL, pre-registered; NOT harness-measured)

Definition (pre-registered): the wall clock from a controlled stop of ONE
service (record WHICH service and HOW it was stopped) to the first
successful tenant-scoped read through the public API after restart. The
console's own disconnect state (P5.25) is the user-visible counterpart —
its behaviour is tested there, not timed here. Record the stop method,
the service, and the figure together; a restart under load is NOT this
procedure.

## 5. Conditions — travel with every figure (binding)

Every figure published from this template carries: the hardware block
(§6), the load spec verbatim (scenario, iterations, URL, counts), the
concurrency statement (single sequential client), the code version, the
build profile, and (for procedure B) the browser + version + tool. A
figure without its conditions is not a measurement — it is an anecdote,
and this template does not publish anecdotes.

## 6. Measured results — ALL CELLS EMPTY (fill from the records / procedure notes only)

### Table A — the harness run (procedure A; transcribe from the LoadMeasurementRecord)

| Field | Value (fill at run time) |
|---|---|
| Run date (UTC) | |
| Ingest scenario (id + version) | |
| Ingest iterations | |
| telemetry_ingest_rate [records/s] | |
| telemetry_ingest_iteration_p95_ms | |
| telemetry_ingest_iteration_max_ms | |
| Query URL (verbatim) | |
| Query count | |
| query_latency_p50_ms | |
| query_latency_p95_ms | |
| query_latency_p99_ms | |
| command_fan_out_time | not_measured (procedure C, manual) |
| recovery_time_after_restart | not_measured (procedure D, manual) |
| Hardware block (copy verbatim) | |

### Table B — console performance per browser (procedure B; one row per BROWSER ACTUALLY TESTED)

| Browser + version | Bundle load (cold) | Overview render | Tool used | Notes |
|---|---|---|---|---|
| Chrome | | | | |
| Edge | | | | |
| Firefox | | | | |
| Safari | | | | |
| (any browser not tested) | not tested (browser unavailable) | not tested | — | no figure is claimed |

### Table C — manual procedures C and D

| Procedure | Service / scenario / K | Figure (wall clock) | Stop method (D) | Notes |
|---|---|---|---|---|
| C — command fan-out | | | — | acknowledgement = application-layer acceptance |
| D — recovery after restart | | | | single-service restart; no restart under load |

### Reading rules for §6 (binding)

- Raw numbers only; no ratio, trend, or scaling claim across rows.
- The harness figures describe ONE sequential client on ONE machine —
  the concurrency statement travels with them wherever they are quoted.
- The p50/p95/p99 are nearest-rank figures of THE recorded samples; no
  re-derivation from a different sample set is comparable, and two runs
  are never averaged.
- No figure here is an availability claim, a tenant-isolation claim, a
  capacity claim, or an SLA; and no figure is a target.

## 7. CI wiring (P5.24 step 4 — the smoke level)

The ledger orders the load tests in CI at a SMOKE level (full load tests
are a release-gate activity, §22.3). Pre-registered: the CI smoke step is
ONE short `load-run` ingest invocation (the minimal library case,
lib-01, few iterations) whose PASS criterion is "the record was produced
and every configured leg measured" — NEVER a duration threshold (the
P3.27 rule; a slow runner produces a slow-but-valid record). The wiring
lands with the CI-owning subtask (P5.28's pipeline is the console-side
one; the sim-runner CI steps follow the existing `scenario-regression`
job pattern). Recorded honestly: the CI step is DESIGNED here, wired
elsewhere, and has never run.

## 8. Publication

The measured figures (when they exist) publish in the phase handover
(P5.30) as the v0.3.0 load baseline WITH their §5 conditions, and the
platform-relevant ones (ingest rate, query latency) surface in the
Simulation Lab alongside the P5.23 benchmark display — labelled
simulated/measured exactly as the record labels them. The P5.24 checkbox
is evidenced by THE HUMAN'S RUN OUTPUT + the procedure-B/C/D notes, never
by this template.

## 9. VERIFY-BUILD items

1. The §4 procedure-A commands are pre-registered shapes over the
   written-but-never-run harness; the first `cargo build` + run is the
   real proof (expected mechanical fixes only).
2. The query leg's figures require the Phase 1–4 services actually
   running (the authoring loop never ran them); the URL is the
   deployment's, not a default.
3. Procedures B/C/D are MANUAL — nothing in the repo automates them, and
   no subtask in this loop executed them.
