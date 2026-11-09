# P3.27 — Benchmark report (pre-registered TEMPLATE — the v0.2.0 baseline)

- **Date:** 2026-11-07 (template authored; the measured run has NOT happened).
- **Subtask:** P3.27 (ledger_phase_3.md §P3.27). Report trace: §19.3, §19.4,
  §22.2 (scenario regression suite).
- **Deliverables:** `apps/sim-runner/src/benchmark.rs` (the harness),
  `apps/sim-runner/src/main.rs` + `src/lib.rs` (additive `benchmark`
  subcommand), `apps/sim-runner/tests/benchmark.rs` (harness tests),
  `benchmarks/README.md`, this template, and the additive CI wiring.

## 0. Status — read first

**This file is a TEMPLATE. Every measured cell below is EMPTY.** The
harness was authored without running cargo (loop-rules), so NO measured
figure exists anywhere in this repository yet, and none is claimed. The
human's single recorded run (§3 commands) fills the cells in §5. Rules that
bind whoever fills this in:

1. **Fill a cell only from the corresponding `BenchmarkRecord` JSON field,
   produced by the §3 commands.** Never transcribe a number from memory,
   another document, or a different run. Never write `0` for an unmeasured
   field — the harness records unmeasurable fields as `not_measured` with a
   reason (fail-loud); an empty cell that stays empty is an honest gap, a
   written zero is a false claim.
2. **The figures are the v0.2.0 BASELINE for later comparison — never
   targets.** Report §19.4, verbatim: *"Do not set arbitrary universal
   targets such as “zero violations in 1,000 cases proves safety” or “ADMM
   converges in 30 iterations.” Set acceptance thresholds after defining
   the model, service, benchmark, and operational risk."* No runtime
   threshold exists in the harness, in CI, or in this template.
3. **Raw numbers only.** Ledger P3.27 step 2, verbatim: *"Run a scaling
   probe: portfolio size ×2 and ×4 (synthetic copies) and record runtime
   growth; do not claim a scaling law from two points — report the raw
   numbers."* No ratio, exponent, trend, or "scales linearly" statement may
   be derived from the ×1/×2/×4 columns — not here, not anywhere.
4. **CI records are not baseline figures.** The `benchmark-record` CI
   artifact comes from shared GitHub-hosted runners and is for regression
   triage only; the baseline is the human's single recorded run with §4
   filled in.
5. A developer-laptop figure is a developer-laptop figure. The
   hardware/container block (§4) exists so no environment is ever
   misrepresented as another.

## 1. What is measured (and what honestly is not, yet)

| Record field | §19.4 metric | Harness status at authoring |
|---|---|---|
| `measured.replay_runtime` | runtime | Measured: one sequential run of the replay pipeline (scenario load → canonical records), `std::time::Instant`, no repetitions, no warmup. |
| `measured.solver_runtime` | runtime (solve) | `not_measured` — the real-solve legs are VERIFY-BUILD-gated (G5, `25_26_tests_scenarios.md` §3). |
| `measured.objective_value` | objective value | `not_measured` (G5). |
| `measured.feasible_interval_count` | feasible interval count | `not_measured` (G5). |
| `measured.schedule_deviation` | schedule deviation | `not_measured` (G5). |
| `measured.replay_record_count` | — | Measured; a factual deterministic count, not a performance figure. |

When the G5 real-solve legs land, the harness's `MeasuredBlock` fields are
filled by the solve wiring and §5's solver columns become fillable the same
way. Until then they stay EMPTY.

## 2. Scenarios and scaling probe

The pinned scenarios are the P3.26 suite
(`scenarios/synthetic-portfolio/scenarios-phase3/`, SYNTHETIC — see that
README): (a) normal day, (b) PV ramp / forecast error, (c) asset dropout
mid-horizon; 24 h / 15-min grid; master seed 20260601; pinned solvers
(HiGHS 1.15.1, Clarabel.rs 0.11.1) travel in each file's `phase3` pin
block, copied verbatim into every record's `scenario_pin`.

The scaling probe measures the SAME scenario with the portfolio deep-copied
×2 and ×4 (deterministic suffix `--x<factor>-<k>` on every site and asset
key; the copy spec travels inside each record's `scaling` block; the copies
are generated in memory, not committed as scenario files). Scenario (c)'s
fault injection is remapped to the copy-1 key, so the dropout semantics
survive scaling.

## 3. The run commands (the human's single recorded run)

```sh
# Per scenario (repeat for b and c); --scaling 2,4 adds the probe points.
# One invocation emits one JSON array: [x1 record, x2 record, x4 record].
cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-a-normal-day.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out benchmark-record-a.json

cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-b-pv-ramp.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out benchmark-record-b.json

cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-c-dropout.json \
  --scaling 2,4 \
  --code-version <record-your-code-version> \
  --out benchmark-record-c.json
```

Exit codes: 0 = records produced; 1 = completeness/structural failure; 2 =
usage/config error. (`--require-complete` exists and currently fails by
design while the G5 solver fields are unmeasured — do not use it for the
baseline run.)

## 4. Hardware / container description — RECORD AT RUN TIME (empty now)

Copy the `hardware` block of the produced records here verbatim (the
harness fills what it can; anything `unavailable:` the human records
manually — the README lists which). DO NOT reconstruct from memory.

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

## 5. Measured results — ALL CELLS EMPTY (fill from the records only)

Transcribe each value from the matching `BenchmarkRecord` field exactly
(keep full precision; do not round). A cell whose record field is
`not_measured` stays EMPTY (or is written as `not_measured (G5)`) — never
zero, never a guess.

### Scenario (a) — normal day (`scenario-a-normal-day.json`)

| Portfolio | replay_runtime [s] | solver_runtime [s] | objective_value | feasible_interval_count | schedule_deviation | replay_record_count |
|---|---|---|---|---|---|---|
| ×1 | | | | | | |
| ×2 | | | | | | |
| ×4 | | | | | | |

### Scenario (b) — PV ramp / forecast error (`scenario-b-pv-ramp.json`)

| Portfolio | replay_runtime [s] | solver_runtime [s] | objective_value | feasible_interval_count | schedule_deviation | replay_record_count |
|---|---|---|---|---|---|---|
| ×1 | | | | | | |
| ×2 | | | | | | |
| ×4 | | | | | | |

### Scenario (c) — asset dropout mid-horizon (`scenario-c-dropout.json`)

| Portfolio | replay_runtime [s] | solver_runtime [s] | objective_value | feasible_interval_count | schedule_deviation | replay_record_count |
|---|---|---|---|---|---|---|
| ×1 | | | | | | |
| ×2 | | | | | | |
| ×4 | | | | | | |

### Reading rules for §5 (binding)

- The ×1/×2/×4 columns are raw numbers only (rule 3 in §0). The ONLY
  permitted cross-referencing is qualitative: e.g. "the ×4 replay runtime
  was larger/smaller/comparable to ×1 on this machine" — no ratios, no
  curve fitting, no extrapolation to other portfolio sizes.
- Objective values are comparable ONLY within one scenario at the same
  portfolio factor AND only once a real solve exists (they are synthetic
  objectives, D1/D2 OPEN — never a market interpretation).
- These figures describe ONE run on ONE machine. They authorize no
  conclusion about any other hardware, load, or deployment.

## 6. CI wiring (§22.2, additive)

`.github/workflows/ci.yml` gains an additive `phase3-scenario-regression`
job: (1) the P3.26 suite (`cargo test -p flexcore-sim-runner --test
phase3_scenarios`) runs as the regression gate — a P3.26 expectation
mismatch is a regression; (2) the benchmark harness runs over all three
scenarios with `--scaling 2,4` and the records are uploaded as the
`benchmark-record` artifact (naming follows the existing artifact
pattern); (3) only record STRUCTURE is asserted (schema tag, pin fields,
hardware fields, `measured.replay_runtime.status == "measured"`,
completeness present). **No runtime threshold is evaluated anywhere** — a
benchmark regression is an expectation mismatch or a structural record
failure, never a slow run.

## 7. VERIFY-BUILD items

1. First `cargo test -p flexcore-sim-runner --test benchmark` — the tests
   were authored without a build; compile fixes are expected mechanical.
2. The harness record's `not_measured` fields (solver_runtime,
   objective_value, feasible_interval_count, schedule_deviation) become
   measurable only when the G5 real-solve legs land
   (`25_26_tests_scenarios.md` §3/§6); this template's solver columns stay
   EMPTY until then.
3. The §3 commands, §4 hardware block, and §5 tables are filled by the
   human's single recorded run; the P3.27 checkbox is evidenced by THAT
   output (plus the CI run log), never by this template.
