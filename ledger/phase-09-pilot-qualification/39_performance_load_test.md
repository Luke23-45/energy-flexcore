# P9.39 — Pilot performance and load test: the TEST TEMPLATE + EMPTY run record (WRITTEN-UNRUN — no measurement exists)

Date: 2026-11-14. Subtask: P9.39 (`ledger_phase_9.md` §P9.39). Owner role:
platform engineering. Report trace: §23 Stage 9; §19.2(8) (performance/load
tests: telemetry ingest rate, scheduling runtime, query latency, command
fan-out, recovery); §19.4 (platform metrics: availability, throughput, p95/p99
API/ingest latency, recovery). Inputs per the ledger: **P9.01** (the pilot
load profile: sites, assets, telemetry rate — the scale this template is
parameterized by, never assumed), **P9.14** (lab results), **P9.17** (shadow
operation). Precedents reused, not reinvented: the Phase 6 perf harness
(`ledger/phase-06-edge-protocol/19_23_22_conformance_security_performance.md`
§5 — the P6.22 load harness over the REAL edge composition, structural-only
assertions, no arbitrary timing threshold), `crates/edge-agent/src/health.rs`
and `crates/edge-agent/src/clock_quality.rs` (the closed health and
clock-quality vocabularies the under-load clock metric reads), and the
P8.20 X-8 pre-registration discipline
(`ledger/phase-08-advanced-coordination/20_reproducibility.md`).

## 0. Run-status honesty — READ FIRST

**The load test has NOT been run.** Nothing was executed in this loop (the
loop bans; ENTRY_NOTE.md boundary 6: every execution-type subtask — P9.39
included — delivers a TEMPLATE + procedure; the run is the human gate). **No
telemetry sample was ingested, no command was fanned out, no restart was
performed, no percentile was computed.** Every measured cell in §7 is
deliberately EMPTY (`(EMPTY — UNRUN)`); no figure is fabricated, estimated,
or extrapolated anywhere in this document. The "what is measured / at which
seam" statements in §3 are derived FROM READING the named code on
2026-11-14 (each citation names the file and the mechanism verified to
exist by reading) — they are DESIGN statements, not evidence that anything
performs to any level. Numeric acceptance thresholds are NOT recorded here
at all — see §5 (X-8: they are registered BEFORE the run, in a dated
amendment, never fitted to a result).

## 1. What this subtask is — and what it is not

§19.2(8) names the load-test layer: **telemetry ingest rate, scheduling
runtime, query latency, command fan-out, recovery**. No earlier P9 subtask
covers these at pilot scale: **P7.26 measured the plugin host only, in the
lab** (a different component and a different scale — this subtask does not
re-quote any P7.26 figure, and none exists to quote). The Phase 6 P6.22
harness measured the EDGE composition (ingest → durable buffer → forward)
with structural-only assertions; its harness parameters were explicitly
NOT a pilot load profile. This subtask defines the PILOT-scale measurement
plan and leaves the run to the human gate.

**What this test is NOT (stated before anything else):**

- Not a field capacity claim. A load test executed on the pilot rig (lab or
  pre-production hardware) establishes behaviour **of that rig under that
  profile** — it is not evidence of field capacity, and it is not a claim
  about production hardware behaviour under production conditions.
- Not a fleet-capacity claim. §19.3: "Do not claim that the first
  distribution feeder tests establish capability for a 10,000-bus utility
  network" — the same rule applies here: no extrapolation beyond the tested
  profile (ledger step 4), in any direction.
- Not a safety, certification, or control-grade claim (AGENTS.md §3;
  ENTRY_NOTE.md boundary 4). FlexCore is supervisory orchestration; a load
  result says nothing about protection, primary response, or hard
  real-time behaviour.
- Not a substitute for P9.17. The field evidence for the pilot is the
  shadow/field acceptance record; the load test is platform-engineering
  evidence about the deployment's own capacity at the pilot profile.

## 2. Preconditions and inputs (recorded honestly)

| Input | Required for this test | Status |
|---|---|---|
| P9.01 pilot scope | The load profile inputs of §4 (sites, assets, telemetry rate, schedule frequency, concurrent operators, command fan-out per dispatch) come from the frozen pilot scope — never invented | P9.01 ticked; the PROFILE VALUES themselves are PLACEHOLDER until the human gate copies them from `01_pilot_scope_freeze.md` into §4 |
| P9.14 lab results | The lab-level behaviour baseline the pilot-scale run is compared against | OPEN (P9.14 unchecked) — the pilot run does not start before it |
| P9.17 shadow operation | The pilot deployment running in read-only/shadow so the load is realistic | OPEN (P9.17 unchecked) — the ledger's dependency order is respected; this template exists now, the run comes after |
| §19.4 preconditions | "Set acceptance thresholds after defining the model, service, benchmark, and operational risk" — the model/service/risk definitions for THIS deployment | **STILL MISSING — no numeric threshold is pre-registered anywhere in this repository** (§5) |
| Frozen versions | Control-plane, edge-agent, adapter, OpenEMS, grid-model (P9.03) versions frozen for the run | To be recorded in §7.0 at run time — a version change mid-run invalidates the affected cases (the P9.16 rule, applied here by reference) |

## 3. The metric set and the code seams it is measured at

Every metric below names the actual code seam it is observed at (paths read
2026-11-14). A metric with no seam is not invented; a seam that only exists
in the composition is labelled as such.

### M-1 Telemetry ingest rate and ingest latency (p95/p99)

| Seam | What it provides |
|---|---|
| `crates/edge-agent/src/ingest.rs` | The edge read path: `EmsAdapter.read` → `AntiCorruptionLayer.map_reading` → canonical `EdgeTelemetrySample`; `event_time` from the source, `ingest_time` stamped at the edge from the `Clock` (the P0.09 honesty pair — the pair the latency is computed from) |
| `crates/edge-agent/src/runner.rs` | The poll → buffer → forward supervision loop; the buffer-first order (at-least-once, §15.4); the audited backpressure path (a full buffer PAUSES polling — never drops) |
| `crates/edge-agent/src/buffer_file.rs` | The durable journal (`FLEXCORE-EDGE-TELEMETRY-JOURNAL`, length-prefixed CRC-32 records); the configured capacity bound and the audited overflow policy; the recovery replay |
| `crates/flexcore-ports/src/telemetry_ingest.rs` | The central ingest port the edge uplink feeds |
| `apps/control-plane/src/metrics.rs` | The P2.21 telemetry metric families: ingestion delay as a per-BATCH summary (`_sum`/`_count` — per-asset breakdown to logs, never labels), per-asset staleness series while stale, per-STATE quality counts (all six canonical states, zero included), quarantine counts by closed reason-class |
| `apps/control-plane/src/metrics_route.rs` | The `/metrics` scrape surface the load-test observer reads |

Measured: steady-state samples/s at the pilot profile; edge ingest latency
(poll-start to sample buffered) and end-to-end ingest latency (event_time →
centrally queryable), both as p95/p99 per §19.4. Recorded per §10.4 quality
state — a load profile that pushes quality states other than `GOOD` is
recorded AS THAT FACT, never averaged away.

### M-2 Query latency (p95/p99)

| Seam | What it provides |
|---|---|
| `apps/control-plane/src/metrics.rs` | The P1.20 per-request recorder middleware (`track_metrics`): every served request emits one JSON log line (trace id, method, route pattern, status, tenant, actor) and updates the request/error families |
| `apps/control-plane/src/routes.rs` (+ the per-phase route modules) | The operator-console API surface the percentiles are taken over |

Measured: p95/p99 per route-class (portfolio/site/asset/schedule/dispatch/
audit reads) at the pilot's concurrent-operator count. Never across tenants:
each percentile is measured within one tenant context (the §16.2 isolation
is not a load-test variable).

### M-3 Command end-to-end latency percentiles

| Seam | What it provides |
|---|---|
| `crates/edge-agent/src/command_path.rs` | The composed edge pipeline: P6.14 four-condition gate → P6.15 verifier (`command_verify.rs`) → P6.17 policy re-check → P6.16 forwarder (`command_forward.rs`) |
| `crates/edge-agent/src/command_verify.rs` | The four checks (signature, duplicate, sequence, expiry); the expiry check trusts only `Synchronized` clock quality (the P6.31 seam) |
| `crates/edge-agent/src/command_forward.rs` | The separately-gated `CommandForwarder` port; production composition fail-closed |
| §14.1 lifecycle timestamps | The latency is measured over the §14.1 states the pilot actually exercises — in the load test, as in P9.19, NO command reaches ISSUED on the field path; the load exercise runs the gate/validate/feasible/approve transitions in shadow, and the ACKNOWLEDGED latency is measured only in the shadow/test-lab composition the deployment actually has (recorded honestly per run — if the pilot composition cannot reach ACKNOWLEDGED, the acknowledgement-latency cell records exactly that, not a zero) |

Measured: per-stage latency (validation, feasibility, authorization, edge
gate+verify, forward) and the §19.4 dispatch metric **acknowledgement
latency** where the composition can express it; percentiles p95/p99. A
command rejected by any gate is recorded as a REJECTION with its named
reason (`signature`/`duplicate`/`sequence`/`expiry`/`allowlist`/`scope`),
counted in the throughput figure — rejections under load are behaviour, not
noise to be hidden.

### M-4 Audit-buffer throughput

| Seam | What it provides |
|---|---|
| `crates/edge-agent/src/audit_buffer.rs` | `FileAuditBuffer`: the durable, BOUNDED, append-oriented audit journal (`FLEXCORE-EDGE-AUDIT-JOURNAL`, CRC-32 framing), in-order upload on reconnect; same mechanics family as the P6.09 telemetry journal |
| `crates/edge-agent/src/runner.rs` | The `AuditSink` the degraded-mode transitions and backpressure pauses are written through |

Measured: audit records/s sustained at the pilot profile (the load test
generates rejections, transitions, and pauses on purpose — the audit path
must absorb them), journal growth rate, upload drain time after a
disconnect window, and the overflow policy's observed behaviour (COUNTED,
audited — an overflow that is neither counted nor audited is a FAILING cell,
never a pass).

### M-5 Clock-sync behaviour under load

| Seam | What it provides |
|---|---|
| `crates/edge-agent/src/clock_quality.rs` | The `ClockQualityMonitor`: closed vocabulary `Unknown`/`Synchronized`/`Degraded`/`Unsynchronized`; offset estimated as the MEDIAN of a bounded sample window of event_time-vs-ingest_time pairs; a pair beyond `MAX_ABS_OFFSET_MILLIS` refused as corrupt; PILOT-PROFILE DEFAULTS recorded in the module (100 ms tolerance, 30 000 ms sync timeout — DEFAULTS, not measurements, per the module header) |
| `crates/edge-agent/src/health.rs` | The health seam: `DEGRADED_CLOCK_DRIFT` for Degraded/Unsynchronized, `DEGRADED_UNVERIFIED` for Unknown — never healthy-by-default |
| `crates/edge-agent/src/command_verify.rs` | The hold-on-drift consumer: expiry checks trusted ONLY on `Synchronized` (§17.4, §17.5 rule 5) |

Measured, specifically under load (this is the M-1/M-3 interaction §19.4
does not split out but the pilot needs): do time-sample pairs still arrive
within the sync timeout at the profile rate; does the median offset stay
inside the configured tolerance; does the hold-on-drift rule trigger under
load (and if it does, commands are HELD fail-closed and audited — that is
the correct behaviour and is recorded as such); do any spurious
`DEGRADED_CLOCK_DRIFT` transitions appear. These measurements are the
load-side input to P9.18 acceptance criterion (b) ("measured clock offset is
within the bound the command expiry assumes") — P9.18 owns that acceptance;
this test supplies its measurements.

### M-6 Scheduling runtime

| Seam | What it provides |
|---|---|
| `crates/flexcore-optimization-api/src/port.rs` | The `Optimizer` port (`solve(problem, config)`) — scheduling depends on this contract, never on a solver directly (§5.2) |
| `crates/flexcore-optimization-api/src/result.rs` | The result contract records runtime in the provenance block (§9.2) — the runtime figure travels with the solve, it is not re-derived |
| The pilot composition | Which provider(s) serve the pilot scheduling path is a deployment composition fact, recorded per run in §7.0 with versions |

Measured: scheduling runtime per cycle at the pilot's schedule frequency and
asset count (the §19.4 scheduling metric), recorded with the provider, tier
(Tier 1 LP/QP/MIP via HiGHS; Tier 2 continuous conic via Clarabel — the
solver-class discipline of AGENTS.md §3 rule 4 applies to every recorded
row), problem size, and time limit/config digest. Never quoted across
problem classes.

### M-7 Recovery time after a control-plane restart

| Seam | What it provides |
|---|---|
| `apps/control-plane/src/probes.rs` | Liveness (`/health`) vs readiness (`/ready`) — deliberately distinct; readiness names its dependency checks (database, broker) so the recovery measurement can attribute the gap |
| `crates/edge-agent/src/runner.rs` | The edge DEGRADED state machine: on central unavailability the edge keeps polling + buffering (audited); on recovery it exits degraded mode (audited) and drains |
| `crates/edge-agent/src/buffer_file.rs` | The post-restart journal replay (truncate-and-audit on a corrupt tail) and the drain of the outage window |

Measured: time from restart start to `/ready` true (with the failing
dependency named if any); edge-side time in DEGRADED (from the audit
timestamps — the audit trail IS the evidence); post-outage buffer drain
time at the recorded outage depth; audit-journal upload drain time (pairs
with M-4). Every recovery cell names the trigger actually injected (a
restart is not a link loss is not a broker outage).

### M-8 The degraded case (ledger step 3) — ingest at the profile rate while one worker/solver is slow or down

| Seam | What it provides |
|---|---|
| `crates/edge-agent/src/runner.rs` | Backpressure replaces loss: a full buffer PAUSES polling, audited — never a silent drop |
| `crates/edge-agent/src/buffer_file.rs` | The configured capacity bound; the overflow policy (drop-oldest, COUNTED and audited — the P6.22 harness's audited drop-oldest counter is the structural check) |
| The scheduling composition (M-6 seam) | The slow/down worker: its requests time out or fail with named errors; the schedule path records the failure (§12.6 closed status vocabulary) — it never silently relaxes |

Measured: queue depth over time, backpressure-pause count and durations,
drop-oldest counts (zero is a RESULT, recorded — it is never assumed),
rejection counts, and the recovery drain after the worker is restored.
**The pass rule is structural, not numeric:** no silent drop anywhere; every
loss (if any) is counted and audited with its reason. Whether the observed
depths/durations are acceptable is a §5 threshold question, decided before
the run.

## 4. Workload definitions (ramp / soak / spike) — the pilot profile

The profile parameters come from P9.01 (frozen pilot scope) and are copied
into this table by the human gate BEFORE the run. Until then every value is
PLACEHOLDER — the P6.22 rule applies verbatim: values chosen to exercise
code paths in a lab harness are NOT the pilot load profile and must never
be quoted as one.

| Profile parameter | Value (from P9.01) |
|---|---|
| Pilot sites under load | PLACEHOLDER pending P9.01 input |
| Assets per site (total) | PLACEHOLDER pending P9.01 input |
| Telemetry rate (samples per asset per interval; interval) | PLACEHOLDER pending P9.01 input |
| Scheduling frequency (cycles/hour) | PLACEHOLDER pending P9.01 input |
| Concurrent operator sessions | PLACEHOLDER pending P9.01 input |
| Command fan-out per dispatch (assets per dispatch decision) | PLACEHOLDER pending P9.01 input |
| Edge buffer depth (records, configured) | PLACEHOLDER — the deployment configuration value |
| Burst factor / channel conditions | PLACEHOLDER pending P9.01 input |

The three workload shapes, run IN ORDER, each at the profile above:

| # | Shape | Definition | What it isolates |
|---|---|---|---|
| W-1 | **Ramp** | Ingest/command load increased stepwise from a fraction of the profile to the full profile, holding a fixed window at each step | The engagement point of each bounded mechanism (buffer backpressure, audit overflow, queue growth) — where each mechanism STARTS acting, observed, not assumed |
| W-2 | **Soak** | The full pilot profile held steady for the soak duration (recorded before the run; long enough to expose slow accumulation: journal growth, memory-visible growth via external observation, error-rate windows) | Steady-state p95/p99 stability; drift or accumulation that a short run hides |
| W-3 | **Spike** | The full profile plus a defined burst (the burst factor above) for a short window, then back to the profile | Transient behaviour: how far queues/backpressure travel and how fast they recover — with every drop/pause counted and audited |

Then the degraded runs of M-8 (worker/solver slow, then down) at the W-2
profile. The order (ramp → soak → spike → degraded) is fixed before the run
and recorded in §7.0; a different order is a NEW run row, never an
overwrite.

## 5. Acceptance thresholds — NOT pre-registered (X-8); the registration block

§19.4 (verbatim rule): **"Do not set arbitrary universal targets such as
'zero violations in 1,000 cases proves safety' or 'ADMM converges in 30
iterations.' Set acceptance thresholds after defining the model, service,
benchmark, and operational risk."** The §19.4 preconditions the ledger
carries as still missing remain missing: no numeric threshold for ingest
rate, latency percentile, throughput, or recovery time is pre-registered
anywhere in this repository, and none is invented here.

**The X-8 discipline, applied to this test (the P8.20 standard, dated):**

1. Thresholds are registered in the block below, with a REGISTRATION DATE,
   BEFORE the first run — derived from the deployment's defined model,
   service, and operational risk (P9.01 scope, P9.07 risk positions), never
   from a preliminary measurement.
2. The registration is an AMENDED RECORD: the amendment names the date, the
   definer, and the basis of each threshold. It never edits §7's measured
   cells; measured cells are filled only from run output.
3. A threshold set or changed AFTER a result was known is INVALID — the
   affected run is discarded and re-run under a re-registered threshold
   (the P9.16 "thresholds fitted to observed results" failure mode, applied
   here by name).
4. The re-run verification (the ledger's method: "Re-run the load test and
   compare against the recorded numbers within stated tolerance") needs a
   tolerance — that tolerance is ALSO pre-registered in this block before
   the first run, or the comparison is refused (`TOLERANCE_NOT_PRE_
   REGISTERED`, the P8.20 comparator rule carried over).

**Threshold-registration block — all cells EMPTY until the dated amendment:**

| Metric | Threshold (value + basis) | Registered by | Registration date |
|---|---|---|---|
| M-1 ingest rate / ingest latency p95/p99 | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-2 query latency p95/p99 | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-3 command-path / acknowledgement latency p95/p99 | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-4 audit throughput / drain time | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-5 clock-sync under load (offset vs tolerance; transitions) | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-6 scheduling runtime | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-7 recovery time | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| M-8 degraded-case bounds (queue depth, pause duration, drops) | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| Re-run comparison tolerance | (EMPTY — not registered) | (EMPTY) | (EMPTY) |

The M-8 structural rule (no silent drop; every loss counted and audited) is
NOT a threshold and needs no registration — it holds by construction and is
checked on every run.

## 6. Environment requirements (what the run must record to count)

1. **Frozen versions travel with every number** (the P6.22 conditions rule):
   control-plane build/version, edge-agent build/version, adapter versions,
   OpenEMS version (where the site EMS is in the loop), grid-model version
   (P9.03), the load-profile values of §4 in force, and the composition's
   provider set for M-6.
2. **Hardware honesty (the P6.22 lab-vs-gateway rule, restated):** a number
   produced on a laptop or CI runner is a LAB number, labelled as such — it
   is never presented as a pilot-rig number. The pilot-rig hardware (CPU,
   memory, disk, network path between edge and central) is recorded in
   §7.0. Where the run is executed on the actual pilot deployment
   hardware, that fact is recorded; where it is not, the record says so.
3. **External observation for CPU/memory/disk:** an in-process harness
   cannot measure its own CPU/memory/disk honestly (the P6.22
   `not_measured_by_this_harness` pattern) — external observation (OS
   tooling at the rig) is a human-gate input, recorded alongside.
4. **Control-authority mode recorded per §21.4:** the load test runs in
   read-only/shadow (or the lab/test-lab composition) — NO field write
   authority is exercised or granted by this test (ENTRY_NOTE boundary 3).
   The mode in force is a §7.0 field.
5. **No production tenant data in the load generator:** synthetic load data
   only, with quality states and units per §10.2/§10.4 — a load test that
   exercises the ACL with malformed input does so as a DECLARED case
   (counted rejections), never as accidental noise.
6. **Clock condition recorded:** the sync source in force, the configured
   tolerance/timeout (the pilot-profile configuration values, which may
   differ from the module's recorded DEFAULTS — the actual configured
   values are what §7 reports for M-5).

## 7. RUN RECORD — ALL MEASURED CELLS EMPTY. NOTHING HAS BEEN MEASURED.

### 7.0 Run conditions (one row per run; filled by the human gate from the run itself)

| Run | Date | Workload shape (W-1/W-2/W-3/degraded) | Profile values in force (§4 ref) | Control-authority mode (§21.4) | Rig (pilot hardware / lab — named) | Versions (control plane / edge / adapters / OpenEMS / grid model) | Operator | Order deviation, if any |
|---|---|---|---|---|---|---|---|---|
| EMPTY | (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

### 7.1 Baseline and percentile measurements (M-1 … M-7 at the W-2 profile)

| Metric | Measured value (p50) | p95 | p99 | Conditions ref (§7.0 run id) |
|---|---|---|---|---|
| Telemetry ingest rate (samples/s, steady state) | (EMPTY — not measured) | n/a | n/a | (EMPTY) |
| Edge ingest latency (poll → buffered) | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| End-to-end ingest latency (event_time → centrally queryable) | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Query latency, per route-class: (EMPTY rows added per class at the run) | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Command path per-stage latency: validation / feasibility / authorization / edge gate+verify | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Acknowledgement latency (ONLY if the composition can express it — else the cell records that fact) | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Audit throughput (records/s) / journal growth | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Clock-sync: median offset; sample-pair arrival vs sync timeout; drift-transition count | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |
| Scheduling runtime (per cycle, provider + tier + problem size recorded) | (EMPTY — not measured) | (EMPTY) | (EMPTY) | (EMPTY) |

### 7.2 Degraded-case measurements (M-8, ledger step 3)

| Degraded case (worker/solver slow; worker/solver down) | Queue depth (max / sustained) | Backpressure pauses (count, total duration) | Drop-oldest count (audited; zero is a recorded result) | Rejections (count, by named reason) | Recovery drain time | Conditions ref |
|---|---|---|---|---|---|---|
| (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |
| (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

### 7.3 Recovery measurements (M-7)

| Trigger actually injected | Restart → `/ready` true (failing dependency named if any) | Edge time in DEGRADED (from audit timestamps) | Post-outage buffer drain (at recorded depth) | Audit-journal upload drain | Conditions ref |
|---|---|---|---|---|---|
| (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

### 7.4 External observation (the P6.22 rule — not measurable in-process)

| Observation | Value | Method/tool | Conditions ref |
|---|---|---|---|
| CPU utilisation under the profile | (EMPTY — not measured) | (EMPTY) | (EMPTY) |
| Memory (RSS) under the profile | (EMPTY — not measured) | (EMPTY) | (EMPTY) |
| Disk (journal write rate / growth) | (EMPTY — not measured) | (EMPTY) | (EMPTY) |

## 8. What a passing load test would and would not establish

**Would establish (exactly):** that THIS deployment build, at the frozen
versions, on the recorded rig, at the §4 pilot profile, sustained the
recorded ingest/query/command/audit/scheduling load with the recorded
percentiles and the recorded degraded behaviour; that no silent drop
occurred at any exercised mechanism (every loss counted and audited); and
— once §5 is registered — that the recorded values compare within the
pre-registered thresholds and re-run tolerance. It establishes a
REPEATABLE measurement (the ledger's verification method is a re-run
against the recorded numbers), not a promise.

**Would NOT establish:**

- **Not a field capacity claim.** The test rig — even the pilot deployment
  hardware — is not the field environment: the production network path,
  concurrent customer load, and site conditions differ. Field behaviour
  evidence comes from P9.17 shadow operation, not from this rig.
- **Not a fleet-capacity claim.** No extrapolation beyond the tested
  profile (§19.3; ledger step 4): nothing here says anything about a
  second site, a larger asset count, or a higher telemetry rate.
- **Not a safety/certification/control-grade claim** (AGENTS.md §3): load
  behaviour says nothing about protection grade, primary response, or
  certification status.
- **Not solver-class or optimality evidence** (AGENTS.md §3 rule 4): M-6
  runtimes are per recorded provider/tier/problem; they never imply an
  SOCP result from an LP/MIP tool or a global-optimality guarantee from a
  local solver.
- **Not a substitute for the functional gates:** a load pass does not
  validate a schedule, authorize a command, or close any P9 gate other
  than P9.39's own criteria.

## 9. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) BEFORE the first run: copy the P9.01 profile values into §4; append
#     the dated threshold amendment to §5 (thresholds + re-run tolerance,
#     each with basis, definer, date). No run starts without this.
#     Evidence: the dated amendment text in this file.

# (c) THE RUN (human gate; W-1 ramp → W-2 soak → W-3 spike → M-8 degraded,
#     in the recorded order), at the frozen versions and the recorded rig,
#     with external CPU/memory/disk observation:
#     - drive the load per §4's profile; capture the /metrics scrapes
#       (apps/control-plane/src/metrics_route.rs surface) and the edge
#       audit trail;
#     - fill §7.0–§7.4 FROM THE RUN OUTPUT ONLY — one §7.0 row per run,
#       every number carrying its conditions reference; a number without
#       its §7.0 row is not a record (the P6.22 conditions rule);
#     - rejections and drops are recorded AS THE COUNTED OUTCOME with
#       their named reasons; a silent drop is a FAILING cell, never a pass.

# (d) THE LEDGER'S VERIFICATION METHOD (independent re-run): a second
#     engineer re-runs the load test from §4/§6 alone and compares against
#     the recorded numbers within the §5-registered tolerance. The
#     comparison verdict is recorded in §7 (a re-run row), never by
#     editing the original row.

# (e) honesty grep — no fabricated figure may exist anywhere in this
#     record's history before the run: the EMPTY labels of §7 must be
#     present in the committed template (they are, verbatim).
```

A green (a) with (b) pending leaves this record WRITTEN-UNRUN (the current
state). The ledger tick may advance past "implemented, awaiting human
build/test" only when §7 is filled from actual runs, §5 is registered, and
the (d) re-run comparison is recorded.

## 10. Open risks carried to P9.27 (the residual-risk register input)

| # | Risk | Why it is open |
|---|---|---|
| R-1 | **No numeric threshold is pre-registered** (§5 EMPTY): until the dated amendment, no run result can be judged pass/fail — only recorded. The pilot's operational risk definition (the §19.4 precondition) is a human/product input still outstanding | Recorded here; P9.27 carries it until the amendment exists |
| R-2 | **The load profile is PLACEHOLDER until P9.01's values are copied** (§4): a run against a guessed profile measures the wrong thing and proves nothing about the pilot scale | The P9.01 copy step is gate input (b) |
| R-3 | **Lab-rig numbers misread as pilot numbers** — the named ledger risk. Mitigated by the §6 hardware-honesty rule and the §7.0 rig column, but the discipline is human-enforced at fill time | The rig column is mandatory per row |
| R-4 | **Degraded-case coverage depends on the pilot composition**: the slow/down-worker cell needs a worker seam the deployment actually has (M-8); where the composition differs, the cell records the composition's actual failure path — a composition that silently drops is a FINDING against the deployment, not a template gap | Verified at run time; the structural no-silent-drop rule is the check |
| R-5 | **Acknowledgement latency may be unexpressible in the pilot composition** (M-3): if the pilot runs read-only/shadow throughout, the ACKNOWLEDGED stage has no field-path instance — the cell records that fact rather than a proxy figure | Recorded honestly per run |
| R-6 | **Clock-sync under load interacts with command expiry** (M-5): if load delays sample pairs beyond the sync timeout, the edge holds commands fail-closed — correct behaviour with an operational cost; the pilot's expiry bound vs observed offset is P9.18's acceptance, and this test only supplies the load-side measurements | Cross-reference recorded; P9.18 owns the acceptance |

## 11. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| The load test itself (any cell of §7) | **NOT RUN — banned in this loop; human-gate input; all measured cells EMPTY** |
| Threshold registration (§5) | **NOT REGISTERED — §19.4 preconditions still missing; X-8: registration is a dated amendment BEFORE the run** |
| Any performance figure anywhere in this record | **NONE EXISTS — nothing measured, nothing extrapolated, nothing fabricated** |
