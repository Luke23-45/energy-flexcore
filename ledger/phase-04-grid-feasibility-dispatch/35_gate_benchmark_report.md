# P4.35 — Feasibility-gate runtime and diagnostics report (pre-registered TEMPLATE — the v0.3.0 feasibility baseline)

- **Date:** 2026-11-10 (template authored; the measured run has NOT happened).
- **Subtask:** P4.35 (ledger_phase_4.md §P4.35). Report trace: §21.3 (solver
  duration/status), §19.4 (grid feasibility metrics), §12.5.
- **Method source:** the P3.27 benchmark template
  (`ledger/phase-03-flexibility-scheduling/27_benchmark_report.md`) is REUSED
  AS THE METHOD — same discipline, same binding rules, same honesty
  conventions. This is NOT a re-measurement of P3.27 and NOT a second
  method; it is the same method applied to the feasibility gate
  (power-flow solve + limit checks) per model size, which §21.3 requires
  so the "solver duration/status" measurement covers the feasibility
  solver and not only the scheduler.

## 0. Status — read first

**This file is a TEMPLATE. Every measured cell below is EMPTY. NO
measured figure exists anywhere for the feasibility gate yet, and none is
claimed.** The authoring followed the loop bans (nothing built, nothing
run); the human's single recorded run (§4 commands) fills the cells in
§6. The P3.27 binding rules apply verbatim, restated for this gate:

1. **Fill a cell only from the produced run record.** Never transcribe a
   number from memory, another document, or a different run. Never write
   `0` for an unmeasured field — an unmeasurable field is recorded
   `not_measured` WITH its reason (fail-loud); an empty cell that stays
   empty is an honest gap, a written zero is a false claim.
2. **The figures are the v0.3.0 feasibility BASELINE — never targets.**
   Report §19.4 verbatim: *"Do not set arbitrary universal targets such
   as 'zero violations in 1,000 cases proves safety' or 'ADMM converges
   in 30 iterations.' Set acceptance thresholds after defining the model,
   service, benchmark, and operational risk."* No runtime threshold is
   evaluated by this template. The one exception the ledger explicitly
   orders is the regression INVESTIGATION FLAG of §7 — a flag to look,
   never a pass/fail gate and never a performance target.
3. **Raw numbers only.** No ratio, exponent, trend, or "scales with
   model size" statement may be derived from the size columns — not
   here, not anywhere. The per-model-size rows are raw observations.
4. **CI runs are not baseline figures.** Any CI-executed run of the
   §4 commands produces triage-context records from shared runners; the
   baseline is the human's single recorded run with §5 filled in.
5. **Conditions travel with every figure.** A developer-laptop figure is
   a developer-laptop figure; the §5 block exists so no environment is
   ever misrepresented as another. Solver-class discipline applies to the
   figures too: what is timed is an **AC power-flow evaluation of
   specified injections** — NOT an AC-OPF solve, and the timed duration
   must never be described as "optimization time" (§12.5, §7.4).

## 1. What is measured (and what honestly is not, yet)

The ledger P4.35 step 1 asks for per-check durations: power-flow solve,
limit checks, margin diagnostics, plus solver/provider version, model
version + size, convergence status, iteration count, residuals. What the
code actually exposes at authoring (verified by reading the sources,
nothing run):

| Measured quantity | Existing timing surface | Status at authoring |
|---|---|---|
| Power-flow SOLVE duration (§12.5 step 4) | `PowerFlowResult.runtime: Duration` — a REQUIRED, completeness-validated field of every result (`crates/flexcore-grid-api/src/port.rs`) | **Measured** — recorded by the provider path per run. |
| Whole-ATTEMPT duration (steps 3–4 wall clock) | `FeasibilityAttemptRecord.started_at_millis / finished_at_millis` (`feasibility.rs`) — caller-supplied gate clock stamps | **Measured** — but millisecond resolution and clock-stamped, coarser than `result.runtime`; both are recorded and never mixed. |
| Limit-check duration (§12.5 step 5) | NONE — `check_limits` (`limits.rs`) is a pure function with no timing parameter or output | **NOT measured — recorded gap** (see §2). |
| Margin-diagnostics duration (§12.5 step 7) | NONE — diagnostics are computed inside `check_limits` with no separate timer | **NOT measured — recorded gap** (see §2). |
| Convergence status | `PowerFlowStatus` (closed vocabulary) on every result | **Measured** (a factual status, not a performance figure). |
| Iteration count + residuals | `PowerFlowResiduals` via `Diagnostic<PowerFlowResiduals>` (available, or `NotAvailable` WITH the provider's reason) | **Measured when available**; a `NotAvailable` diagnostic stays `not_measured (reason)` in the cell — never zero. |
| Model size (bus/branch/transformer count) | Derived from the model IR: `VersionedNetworkModel.buses/branches/transformers` lengths | **Measured** (a factual count, not a performance figure). |
| Solver/provider version | `provider_version` — REQUIRED field, cross-checked against the pin (pandapower 3.5.6, P4.01) | **Measured** (pin identification). |

## 2. The recorded timing-surface gap (honest limitation)

**The limit check and margin diagnostics have no timing hook.** Ledger
P4.35 step 1 asks for per-check durations over "power-flow solve, limit
checks, margin diagnostics"; the existing surface times only the solve
(`PowerFlowResult.runtime`) and the whole attempt (the clock stamps).
`check_limits` is deliberately a pure provider-independent function
(P4.06 contract) and carries no timer.

The gap is recorded here rather than papered over. Closing it needs a
SMALL additive timing surface (e.g. the composition root clocking the
`check_limits` call and recording the duration beside the
`LimitCheckReport`, or an additive field on the report's provenance) — a
pure, no-infrastructure change, but one that touches
`crates/flexcore-grid-api/src/limits.rs` or its caller, which is OUTSIDE
this subtask's file ownership. Until that surface exists, the §6
limit-check columns stay EMPTY with the reason `not_measured (no timing
surface — see §2)`, and the solve column is the gate's measured core.
Adding the surface is left as a VERIFY-BUILD-adjacent open item for a
subtask that owns the grid-api crate.

## 3. Scenarios, model sizes, and pins

- **Scenarios (ledger step 2):** the §24 vertical-slice landing points
  (steps 5–8) over the versioned synthetic scenarios — `scenarios/
  synthetic-portfolio/scenario-v1.json` (the P2.13 exit-gate scenario:
  1 tenant / 1 portfolio / 2 sites / BESS + PV + flexible load + EVSE per
  site, 24 h, 1-minute cadence, master seed with recorded per-asset
  derivation) and `scenario-v2.json`. The second ledger leg — "the
  largest scenario-library case available (P5.20)" — is a PHASE 5
  deliverable and does not exist yet; the template records this honestly
  and the P5.20 row is pre-registered EMPTY (see §6, table C).
- **Master seed:** 20260601 (the scenario's recorded master seed; the
  feasibility gate itself is DETERMINISTIC — no randomness — so the seed
  pins the scenario data, not the solve).
- **Clock configuration:** the gate clock is the caller-supplied
  `now_millis` (UTC millis, §10.3) — the gate never reads a wall clock.
  The benchmark harness therefore supplies a FIXED controlled `now_millis`
  per run, recorded in the run record; the solve duration comes from
  `PowerFlowResult.runtime` (provider-measured), NOT from the controlled
  clock.
- **Provider pin:** pandapower 3.5.6 (P4.01 pin discipline; the pin is
  cross-checked against the worker's reported version and a mismatch is a
  contract violation — a run with a mismatched pin is discarded, not
  recorded).
- **Solver-class statement (binding):** the measured solve is an AC
  power-flow evaluation of specified injections and controls (HiGHS/
  Clarabel are NOT in this path — the feasibility gate runs no
  optimization; the scheduler's HiGHS 1.15.1 / Clarabel.rs 0.11.1 pins
  from P3.27 do not apply here and are not restated as if they did).
  Model class: BALANCED per P4.34 (`34_unbalanced_deferral.md`) — the
  figures describe a balanced power flow only.
- **Model sizes:** the model-size probe follows the P3.27 scaling
  discipline — the SAME feeder model deep-copied ×2 and ×4 (deterministic
  suffix `--x<factor>-<k>` on every bus/branch/transformer id, copies
  generated in memory per the P3.27 pattern, the copy spec recorded in
  the run record). One invocation per scenario emits `[×1, ×2, ×4]`
  records.

## 4. The run commands (the human's single recorded run)

The harness subcommand mirrors the P3.27 shape (a `feasibility-benchmark`
invocation over the sim-runner/worker path; the exact harness wiring is
VERIFY-BUILD — see §8):

```sh
# Per scenario (repeat for scenario-v2.json); --scaling 2,4 adds the probe points.
# One invocation emits one JSON array: [x1 record, x2 record, x4 record].
cargo run -p flexcore-sim-runner -- feasibility-benchmark \
  --scenario scenarios/synthetic-portfolio/scenario-v1.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out feasibility-benchmark-record-v1.json

cargo run -p flexcore-sim-runner -- feasibility-benchmark \
  --scenario scenarios/synthetic-portfolio/scenario-v2.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out feasibility-benchmark-record-v2.json
```

Each record carries: scenario id, model size block (bus/branch/transformer
counts, scaling factor), provider id + pinned version (cross-checked),
model id + version + content hash, convergence status, solve runtime
(from `PowerFlowResult.runtime`), attempt wall-clock duration (from the
attempt stamps), iteration count and residuals (`not_measured (reason)`
where the provider's diagnostic is `NotAvailable`), and the hardware
block. Limit-check / diagnostics duration fields are emitted as
`not_measured (no timing surface — see §2)` until the §2 surface exists.

## 5. Hardware / container description — RECORD AT RUN TIME (empty now)

Copy the `hardware` block of the produced records here verbatim (same
rule as P3.27 §4: DO NOT reconstruct from memory; anything the harness
leaves `unavailable:` the human records manually).

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
| Build profile (dev/release; e.g. `cargo run` default = dev) | |
| Python worker runtime + pandapower installed version (cross-checked vs pin 3.5.6) | |

## 6. Measured results — ALL CELLS EMPTY (fill from the records only)

Transcribe each value from the matching record field exactly (keep full
precision; do not round). A cell whose record field is `not_measured`
stays EMPTY or is written as `not_measured (…reason…)` — never zero,
never a guess.

### Table A — §24 vertical-slice scenario, `scenario-v1.json`

| Model size | buses | branches | trafos | solve runtime [ms] (PowerFlowResult.runtime) | attempt wall clock [ms] | limit-check duration [ms] | diagnostics duration [ms] | convergence | iterations | residuals |
|---|---|---|---|---|---|---|---|---|---|---|
| ×1 | | | | | | | | | | |
| ×2 | | | | | | | | | | |
| ×4 | | | | | | | | | | |

### Table B — `scenario-v2.json`

| Model size | buses | branches | trafos | solve runtime [ms] (PowerFlowResult.runtime) | attempt wall clock [ms] | limit-check duration [ms] | diagnostics duration [ms] | convergence | iterations | residuals |
|---|---|---|---|---|---|---|---|---|---|---|
| ×1 | | | | | | | | | | |
| ×2 | | | | | | | | | | |
| ×4 | | | | | | | | | | |

### Table C — largest scenario-library case (P5.20) — PRE-REGISTERED, NOT AVAILABLE IN PHASE 4

The P5.20 scenario library is a Phase 5 deliverable; no Phase 4 run can
fill this table. The table is pre-registered so the P5.20 close-out fills
it by the same method and under the same rules (§0) — not re-measured
differently.

| Model size | buses | branches | trafos | solve runtime [ms] | attempt wall clock [ms] | limit-check duration [ms] | diagnostics duration [ms] | convergence | iterations | residuals |
|---|---|---|---|---|---|---|---|---|---|---|
| ×1 | | | | | | | | | | |
| ×2 | | | | | | | | | | |
| ×4 | | | | | | | | | | |

### Reading rules for §6 (binding)

- The ×1/×2/×4 rows are raw numbers only (rule 3 in §0). The ONLY
  permitted cross-referencing is qualitative: e.g. "the ×4 solve runtime
  was larger/smaller/comparable to ×1 on this machine" — no ratios, no
  curve fitting, no extrapolation to other model sizes.
- A non-converged run's duration is still recorded (it is a factual
  measured duration of a non-converged evaluation), but it is NEVER
  averaged together with converged runs, and a non-convergence row
  carries its `not_measured` limit fields (no limit check runs on a
  non-converged result — the P4.06 fail-closed rule).
- These figures describe ONE run on ONE machine against ONE pinned
  provider. They authorize no conclusion about any other hardware, load,
  model, or deployment, and no latency guarantee of any kind (the §14.5
  timing-honesty boundary applies to feasibility figures too).
- Objective/quality claims are out of scope: no figure here says
  anything about field behaviour, safety, or certification.

## 7. Regression check (ledger step 4) — pre-registered design, factor is CONFIGURATION

The ledger orders: "a feasibility run exceeding its recorded baseline by
more than a stated factor on identical inputs is flagged for
investigation (not silently accepted)." Pre-registered here:

- **Inputs identity rule:** the flag fires only on IDENTICAL inputs —
  same scenario, same model content hash, same pinned provider version,
  same build profile. A flag on differing inputs is meaningless and is
  not emitted.
- **The factor is a recorded CONFIGURATION value, set WITH the baseline**
  (P5.24 discipline: conditions travel with the figure). It is NOT
  written here in advance as a number, because writing a factor before
  the baseline exists would be exactly the "arbitrary universal target"
  §19.4 forbids. The value is recorded in this file's §6 addendum at
  baseline time, with the rationale the power-systems + QA reviewers sign.
- **The flag is an investigation marker, never a gate:** a flagged run
  is looked at (provider version drift? model change? machine load?); it
  does not fail a build by itself, and no runtime threshold is evaluated
  anywhere in CI (the P3.27 rule, carried forward unchanged).

## 8. VERIFY-BUILD items

1. The `feasibility-benchmark` harness invocation of §4 is authored as a
   pre-registered command shape; the harness wiring (sim-runner
   subcommand or worker-side runner) lands with a subtask that owns those
   files. First `cargo build`/`cargo test` of that wiring is the real
   proof — expected mechanical fixes.
2. The §2 timing surface (limit-check + diagnostics durations) must land
   before those columns become fillable; until then they stay
   `not_measured (no timing surface — see §2)`.
3. The §4 commands, §5 hardware block, and §6 tables are filled by the
   human's single recorded run; the P4.35 checkbox is evidenced by THAT
   output (plus the CI run log for the regression-check wiring), never by
   this template.
4. The figures enter the Phase 4 handover (P4.32) as the v0.3.0
   feasibility baseline — as measured numbers WITH their §5 conditions,
   never as targets.
