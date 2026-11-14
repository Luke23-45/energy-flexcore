# P5.23 — Benchmark runs and evaluation metrics (pre-registered TEMPLATE — the v0.3.0 baseline; measured cells EMPTY)

- **Date:** 2026-11-11 (template authored; the measured run has NOT happened).
- **Subtask:** P5.23 (ledger_phase_5.md §P5.23). Report trace: §19.3–§19.4,
  §22.2 (reproducible benchmarks), §22.3 (release gates).
- **Method source:** the P3.27 benchmark template
  (`ledger/phase-03-flexibility-scheduling/27_benchmark_report.md`) and its
  Phase 4 re-application (`ledger/phase-04-grid-feasibility-dispatch/
  35_gate_benchmark_report.md`) are REUSED AS THE METHOD — same discipline,
  same binding rules, same honesty conventions, same hardware capture. This
  is NOT a new measurement method; it is the same method applied to the
  Phase 5 scenario-library cases (P5.20) and the scenario-run composition
  legs (P5.14–P5.16). Where a metric was already covered by P3.27 (the
  replay/scheduling runtime fields) or P4.35 (the feasibility-gate
  durations), THIS template references that method and does not re-measure
  differently.

## 0. Status — read first

**This file is a TEMPLATE. Every measured cell below is EMPTY. No measured
figure exists anywhere for the Phase 5 library cases yet, and none is
claimed.** The authoring followed the loop bans (nothing built, nothing
run); the human's single recorded run (§5 commands) fills the tables in §7.
The P3.27/P4.35 binding rules apply verbatim, restated:

1. **Fill a cell only from the produced run record.** Never transcribe a
   number from memory, another document, or a different run. Never write
   `0` for an unmeasured field — an unmeasurable field is recorded
   `not_measured` WITH its reason (fail-loud); an empty cell that stays
   empty is an honest gap, a written zero is a false claim.
2. **The figures are the v0.3.0 BASELINE — never targets.** Report §19.4
   verbatim: *"Do not set arbitrary universal targets such as 'zero
   violations in 1,000 cases proves safety' or 'ADMM converges in 30
   iterations.' Set acceptance thresholds after defining the model,
   service, benchmark, and operational risk."* No runtime threshold is
   evaluated by this template, by the harness, or in CI. The one
   exception the ledger orders is the regression INVESTIGATION FLAG of §8
   — a flag to look, never a pass/fail gate and never a performance
   target. The baseline is the FIRST HUMAN RUN; the regression factors
   are CONFIGURATION recorded at baseline time (§8), never numbers
   pre-written here.
3. **Raw numbers only.** No ratio, exponent, trend, or "scales with
   portfolio size" statement may be derived from any column — not here,
   not anywhere. The rows are raw observations of ONE run on ONE machine.
4. **CI runs are not baseline figures.** Any CI-executed run produces
   triage-context records from shared runners; the baseline is the
   human's single recorded run with §6 filled in.
5. **Conditions travel with every figure** (§6 block). Solver-class
   discipline applies to the figures too: the grid feasibility leg is an
   **AC power-flow evaluation of specified injections — NOT an AC-OPF
   solve** (§12.5, §7.4), HiGHS is LP/QP/MIP — never SOCP, Clarabel is
   convex conic — never MISOCP, and no timed duration is ever described
   as "optimization time" unless a real optimizer produced it.
6. **Every published figure is labelled simulated** with its scenario id
   + version (§20.2; the Simulation Lab display, P5.08, renders the
   labels structurally).

## 1. The §19.4 metrics enumeration (cited, never from memory)

Report §19.4 (evaluation metrics), verbatim area table — the areas and
metrics THIS template covers for the library cases. The same enumeration
is mirrored in the agent digest (`.agent/CONTEXT.md`, report §16–§19
security/edge/simulation coverage); the REPORT is the authority.

| Area | Metrics (report §19.4) | Covered for the library cases by |
|---|---|---|
| Scheduling | objective value, schedule deviation, feasible interval count, runtime | the P3.27 `BenchmarkRecord` measured block (reused, §4); solver-side fields stay `not_measured` until a real solve is wired (the VERIFY-BUILD G5 rule carries forward) |
| Asset model | SOC/power violations, ramp violations, availability prediction error | the pre-registered expected outcomes of the library records (P5.20) + the scenario-run record's quality accounting — **measurement procedure in §3.2, cells EMPTY** |
| Grid feasibility | voltage violations, branch/transformer overload, AC power-flow convergence, residuals | the P4.35 template method applied per case (§4); the scenario-run record's grid-check summaries (verdict/reason) |
| Flexibility | available vs delivered MW/MWh, uncertainty calibration, shortfall | **not measurable in this composition** — no flexibility envelope delivery path runs in the sim-runner scenario composition in 0.3.0; recorded `not_measured` with the reason (§3.4) |
| Dispatch | acknowledgement latency, tracking error, failure/expiry rate, recovery time | the scenario-run record's command summaries (issue outcome, tracking verdict, final state) — **per-command latency figures require the timing surface of §3.3; cells EMPTY** |
| ADMM / GNN / MARL | (report §19.4 rows) | **out of scope for 0.3.0** — no ADMM/GNN/MARL module exists in any phase; the rows are recorded here only to state the exclusion honestly (nothing measured, nothing claimed) |
| Platform | availability, throughput, p95/p99 API/ingest latency, recovery, tenant isolation | split across P5.24 (the load-measurement template, `24_load_measurement.md` — throughput/latency/recovery) and the §24 demonstration (availability/isolation evidence); THIS template does not re-measure them |

**No arbitrary targets (binding):** the enumeration above fixes WHAT is
recorded, never WHAT a good number is. No acceptance threshold exists in
this template; §8 owns the post-baseline regression-factor configuration
mechanism.

## 2. What is measured per case (and what honestly is not)

The timing surface: the P5.23 additive module (`apps/sim-runner/src/
metrics.rs`) records per-leg wall-clock durations beside the run record
(`ScenarioRunRecord.timings`, schema `flexcore-scenario-run-timings/v1`):
`telemetry_ingest_pump`, `dispatch_slice`, `horizon_sweep`,
`fault_probes`, `record_build`. This is the same shape of gap P4.35 §2
recorded for its limit-check columns, closed here for the scenario-run
composition legs. Untimed legs are ABSENT (never zero); the measurement
clock is wall-clock (`std::time::Instant`), the run's STATE clock is the
scenario clock (P0.09) — the two are never mixed.

| Metric | Existing surface at authoring (read, nothing run) | Status |
|---|---|---|
| Replay-pipeline runtime per case | P3.27 `BenchmarkRecord.measured.replay_runtime` (`benchmark` subcommand) | **Measured by the P3.27 method** — REUSED, not re-measured |
| Per-leg composition wall clock (ingest pump / dispatch slice / horizon sweep / record build) | `ScenarioRunRecord.timings` (P5.23 `metrics.rs`, wired into `scenario-run`) | **Measured** — new surface, same discipline |
| Feasibility-gate solve duration + attempt wall clock | P4.35's method over `PowerFlowResult.runtime` + the attempt stamps | **REUSED from P4.35** — the P4.35 template owns these figures; this template's grid rows cite that record, they do not re-time it |
| Limit-check / margin-diagnostics duration | NONE (the P4.35 §2 gap stands — `check_limits` is a pure function with no timer; the surface is outside this subtask's ownership) | **NOT measured — recorded gap, reason carried in the cell** |
| Scheduling solver runtime / objective value / feasible interval count / schedule deviation | NONE in the composition (no real solve wired — the P3.27 G5 rule) | **NOT measured — recorded gap, reason carried in the cell** |
| Dispatch acknowledgement latency / tracking error / failure-expiry rate | the run record's command summaries carry the OUTCOMES (issue outcome, tracking verdict, final state); no latency timer exists on the issue path in this composition | **Outcomes measured (counts); latencies NOT measured — recorded gap** |
| Recovery time after restart | none in the harness | **NOT measured — P5.24's template owns the pre-registered manual procedure** |
| Convergence status / residuals (grid) | the worker backend's result fields (P4.35 method) | **Measured when available**; `NotAvailable` diagnostics stay `not_measured (reason)` |
| Quality-state counts (per run) | `ScenarioRunRecord.quality_counts` | **Measured** (a factual count, not a performance figure) |

## 3. Measurement methods (each REUSED — no method is invented here)

### 3.1 Replay-pipeline runtime — the P3.27 method, unchanged
`std::time::Instant` around the full replay pipeline (scenario load
through canonical record rendering), a single sequential run, no
repetitions, no warmup — the raw figure of THIS run. Scaling probes (the
deterministic ×2/×4 portfolio copies) follow the P3.27 generator
exactly (`benchmark.rs scaling_scenario_json`) where a case's scale class
warrants a probe; the copies are part of the record.

### 3.2 Asset-model outcomes — the P2.25 expected-quality accounting
The per-fault stored-quality outcomes are verified by the P2.25
expected-quality plan tied to the P2.15 fault types (the replay/coverage
accounting), and the run record's `quality_counts` (exactly one state per
value: GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE, §10.4) is the factual
per-run count. SOC/power/ramp violations are recorded as the counts the
pre-registered expected outcomes name — never a derived "violation rate"
and never a target ("zero violations proves safety" is explicitly
forbidden, report §19.4). The availability-prediction-error metric has NO
measurement surface in 0.3.0 (no prediction model runs in the
composition) — `not_measured` with that reason.

### 3.3 Dispatch outcomes — counts only in this template
Failure/expiry counts come from the run record's command summaries
(final_state, issue_outcome, tracking_verdict — the same fields the
Simulation Lab's failure journal renders). Acknowledgement-latency and
tracking-error FIGURES have no timer in the composition (the issue path
is deterministic scenario-clock work; a wall-clock latency timer is a
composition change outside this subtask's ownership) — the cells stay
EMPTY with that reason until a subtask that owns the composition adds
the surface.

### 3.4 Flexibility metrics — honestly out of scope
No flexibility envelope delivery path runs in the sim-runner composition
in 0.3.0 (available-vs-delivered comparison needs the P3.12 envelope +
delivery measurement path wired end-to-end). All flexibility rows are
`not_measured (no delivery measurement path in the 0.3.0 composition)`.

## 4. The cases, pins, and conditions

- **Cases:** the P5.20 scenario library (§19.3), `scenarios/library/
  lib-01…lib-08` — each a ScenarioRecord v3 with pinned inputs, seeds,
  tolerances, pre-registered expected outcomes, its per-case IEEE record,
  and its DECLARED (never measured) runtime class. The per-case table
  lives in `scenarios/library/INDEX.md` and is REUSED here by reference —
  this template does not restate the library.
- **Master seeds:** each record's own `seeds.master_seed` (pinned; the
  record verifies them fail-loud at replay — P5.18/P5.19). No seed is
  invented here.
- **Versions:** each run names its code version (`--code-version`), the
  simulator crate version, the pandapower provider pin (3.5.6 —
  cross-checked per P4.35), and the HELICS version only for federation
  cases (3.7.0 pin; the worker leg is UNVERIFIED until executed).
- **Backend per case:** `scripted` or `worker_process` per the record
  (INDEX.md). The scripted backend is a recorded FIXTURE, not a computed
  power flow — its durations are composition timings, never
  power-flow-solve figures, and no cell may present them as such.
- **Hardware/container block:** the P3.27 `capture_hardware` block, and
  the §6 table below records it AT RUN TIME (never reconstructed from
  memory).

## 5. The run commands (the human's single recorded run)

```sh
# Per library case (repeat for each record the operator chooses to
# benchmark; the case set is the P5.20 library, lib-01 … lib-08):
cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/library/lib-01-minimal-single-asset.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out benchmark-record-lib-01.json

# The scenario-run composition legs (per-leg timings; the fault journal
# and probes where the case declares faults):
cargo run -p flexcore-sim-runner -- scenario-run \
  --scenario scenarios/library/lib-07-stress-asset-dropout.json \
  --fault-probes \
  --code-version <record-your-code-version> \
  --out scenario-run-record-lib-07.json
```

One `benchmark` invocation emits one JSON array of `BenchmarkRecord`s
([×1, ×2, ×4] per the `--scaling` list); one `scenario-run` invocation
emits one `ScenarioRunRecord` whose `timings` block carries the per-leg
durations. The §7 tables are filled FROM these records only.

## 6. Hardware / container description — RECORD AT RUN TIME (empty now)

Copy the `hardware` block of the produced records here verbatim (the
P3.27 rule: DO NOT reconstruct from memory; anything the harness leaves
`unavailable:` the human records manually).

| Field | Value (fill at run time) |
|---|---|
| Run date (UTC) | |
| OS / arch (from the record) | |
| CPU model (record or manual entry) | |
| Logical cores (record) | |
| Total memory (record or manual entry) | |
| Container? (record `container_flag` + how detected) | |
| Confine/limits (cgroup, VM, CI runner? — manual) | |
| Code version used | |
| Rust toolchain (rust-toolchain.toml channel) | |
| Build profile (dev/release; `cargo run` default = dev) | |
| Python worker runtime + pandapower installed version (cross-checked vs pin 3.5.6) | |

## 7. Measured results — ALL CELLS EMPTY (fill from the records only)

Transcribe each value from the matching record field exactly (full
precision; do not round). A cell whose record field is `not_measured`
stays EMPTY or is written `not_measured (…reason…)` — never zero, never a
guess. Conditions = §6 (one run, one machine).

### Table A — per-case replay-pipeline runtime (the P3.27 method; one row per benchmarked case)

| Case (record id) | scale class | ×1 runtime [s] | ×2 runtime [s] | ×4 runtime [s] | replay record count (×1) | not_measured cells + reasons |
|---|---|---|---|---|---|---|
| lib-01-minimal-single-asset | minimal | | | | | |
| lib-02-small-portfolio | small | | | | | |
| lib-03-medium-multi-site-meshed | medium | | | | | |
| lib-06-stress-pv-ramp | stress | | | | | |
| lib-07-stress-asset-dropout | stress | | | | | |
| lib-08-stress-reverse-power-flow | stress | | | | | |

(lib-04/lib-05 run the `worker_process` backend — their benchmark rows
belong to the P4.35 feasibility record once that leg executes; until then
their cells stay EMPTY with the reason `worker leg UNVERIFIED — see
scenarios/library/INDEX.md`.)

### Table B — per-case composition legs (the P5.23 `timings` block; one row per scenario-run case)

| Case (record id) | telemetry_ingest_pump [s] | dispatch_slice [s] | horizon_sweep [s] | fault_probes [s] | record_build [s] |
|---|---|---|---|---|---|
| lib-01-minimal-single-asset | | | | | |
| lib-02-small-portfolio | | | | | |
| lib-03-medium-multi-site-meshed | | | | | |
| lib-06-stress-pv-ramp | | | | | |
| lib-07-stress-asset-dropout | | | | | |
| lib-08-stress-reverse-power-flow | | | | | |

(A leg absent from a record's `timings` block stays EMPTY with its
reason — an untimed leg is never a zero. The scripted-backend rows time
composition work over a recorded fixture, NOT a power-flow solve.)

### Table C — per-case §19.4 outcome metrics (counts and facts; not performance)

| Case | scheduling: solver runtime | scheduling: objective value | scheduling: feasible intervals | scheduling: schedule deviation | grid: convergence | grid: voltage/thermal violations (counts) | asset-model: SOC/power/ramp violations (counts) | dispatch: failure/expiry counts | dispatch: ack latency | dispatch: tracking error | flexibility rows |
|---|---|---|---|---|---|---|---|---|---|---|---|
| lib-01 | not_measured (no real solve — G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (no latency timer — §3.3) | not_measured (§3.3) | not_measured (§3.4) |
| lib-02 | not_measured (G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (§3.3) | not_measured (§3.3) | not_measured (§3.4) |
| lib-03 | not_measured (G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (§3.3) | not_measured (§3.3) | not_measured (§3.4) |
| lib-06 | not_measured (G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (§3.3) | not_measured (§3.3) | not_measured (§3.4) |
| lib-07 | not_measured (G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (§3.3) | not_measured (§3.3) | not_measured (§3.4) |
| lib-08 | not_measured (G5) | not_measured (G5) | not_measured (G5) | not_measured (G5) | | | | | not_measured (§3.3) | not_measured (§3.3) | not_measured (§3.4) |

(The empty cells of the grid/asset/dispatch-count columns fill from the
records' verdicts and summaries; a case whose backend is `scripted`
records its counts from the FIXTURE outcome and names the backend — a
fixture count is a composition fact, never a physics result.)

### Reading rules for §7 (binding)

- Raw numbers only (rule 3 in §0): the ×1/×2/×4 columns authorize no
  scaling claim. Qualitative cross-reference only ("larger/smaller/
  comparable on this machine").
- A non-converged or failed case's duration is still recorded (a factual
  measured duration) but is NEVER averaged with passing cases, and its
  outcome cells carry their fail-closed reasons.
- These figures describe ONE run on ONE machine against ONE pinned
  toolchain. They authorize no conclusion about any other hardware, load,
  model, or deployment, and no latency guarantee of any kind (§14.5).
- Every figure published onward carries the SIMULATED label and its
  scenario id + version (the Simulation Lab renders this structurally).

## 8. Regression check (the ledger's rule) — pre-registered design, factor is CONFIGURATION

The P4.35 §7 design is REUSED verbatim: the regression flag fires only on
IDENTICAL inputs (same record id + version, same input hash, same pinned
provider/backend, same build profile); the factor is a recorded
CONFIGURATION value set WITH the baseline — NOT written here in advance,
because pre-writing a factor would be exactly the arbitrary universal
target §19.4 forbids. The value(s) are recorded in the §7 addendum at
baseline time with the reviewing engineers' rationale. The flag is an
investigation marker, never a gate; no runtime threshold is evaluated in
CI (the P3.27 rule, carried forward unchanged).

## 9. Publication and simulation honesty

- The measured figures (when they exist) publish in the Simulation Lab
  (P5.08) with the per-run simulated label + scenario/version, and in the
  phase handover (P5.30) as the v0.3.0 baseline — figures WITH their §6
  conditions, never targets.
- The P5.23 checkbox is evidenced by THE HUMAN'S RUN OUTPUT (plus this
  template's §8 addendum), never by this template itself.

## 10. VERIFY-BUILD items

1. The §5 commands are pre-registered shapes; the `benchmark` subcommand
   exists (P3.27) and the `scenario-run` subcommand exists (P5.14) with
   the `--fault-probes` flag (P5.21) — the first build is the real proof;
   expected mechanical fixes only.
2. The §3.3 latency surface (ack latency / tracking error figures) needs
   a composition timing change owned by a composition-owning subtask;
   until then those cells stay `not_measured`.
3. The lib-04/lib-05 `worker_process` rows need the executed worker leg
   (pandapower 3.5.6, never installed in the authoring loop).
