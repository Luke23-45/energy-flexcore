# Benchmarks — the P3.27 measurement harness

Everything measured here is **simulation-only** (the entire 0.x line,
AGENTS.md rule 5) over the **synthetic** Phase 3 portfolio
(`scenarios/synthetic-portfolio/scenarios-phase3/` — synthetic fixtures, not
field data). No market, field, or deployment claim exists or is implied.

## What the benchmark is

The P3.27 subtask (ledger
`ledger/phase-03-flexibility-scheduling/ledger_phase_3.md` §P3.27; report
§19.3, §19.4) measures, per pinned P3.26 scenario × portfolio size:

| Field | Meaning |
|---|---|
| `replay_runtime` | Wall-clock seconds of one sequential run of the replay pipeline (scenario load → canonical records). Single run: no repetitions, no warmup — the raw figure of THAT run. |
| `solver_runtime` | Scheduling-solve seconds. **`not_measured`** until the VERIFY-BUILD G5 real-solve legs land (see `ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md` §3). |
| `objective_value` | §19.4 scheduling metric. **`not_measured`** until a real solve exists. |
| `feasible_interval_count` | §19.4 scheduling metric. **`not_measured`** until a real solve exists. |
| `schedule_deviation` | §19.4 scheduling metric. **`not_measured`** until a real solve exists. |

Every record also carries, captured at run time:

- the **hardware/container description**: OS/arch (`std::env::consts`),
  CPU model (`/proc/cpuinfo` on Linux; `unavailable:` elsewhere — record it
  manually), logical cores (`std::thread::available_parallelism`), memory
  (`/proc/meminfo` on Linux), and a container flag (the
  `FLEXCORE_BENCHMARK_CONTAINER` env override, the standard indicator files,
  or an explicit `no-indicator-found` that is NOT proof of a non-container
  run). Anything the process cannot observe is recorded `unavailable:`,
  never guessed.
- the **scenario pin** (id, version, seed, forecast input, solver pins —
  verbatim from the artefact) and the **scaling-copy spec** (for probe
  points).
- a fail-loud **completeness** summary: a field that was not measured says
  so with its reason. **There is no default zero** — an unmeasured field
  never renders as `0`, and `--require-complete` fails the run if any §19.4
  field is unmeasured.

## The no-targets rule (report §19.4 — binding)

> "Do not set arbitrary universal targets such as “zero violations in 1,000
> cases proves safety” or “ADMM converges in 30 iterations.” Set acceptance
> thresholds after defining the model, service, benchmark, and operational
> risk."

Consequently:

1. **No universal runtime target exists anywhere** — not in the harness, not
   in CI, not in the report template. No CI step gates on a runtime value.
   A benchmark "regression" is a P3.26 expectation mismatch or a
   benchmark-record STRUCTURAL failure (missing fields, unparsable record) —
   never a runtime threshold.
2. The measured figures are the **v0.2.0 BASELINE** for later comparison.
   They are not targets, not capabilities, and not deployment figures.
3. **Scaling probe:** portfolio copies ×2 and ×4 are recorded as RAW
   NUMBERS ONLY. Ledger P3.27 step 2, verbatim: *"do not claim a scaling
   law from two points — report the raw numbers."* No ratio, exponent, or
   trend may be derived in any document in this repository from those
   points.
4. **A developer-laptop or CI-runner figure is not a deployment figure.**
   The hardware/container block exists precisely so no one can present one
   as the other.

## How to run it

Baseline run (the human's single recorded run — see
`ledger/phase-03-flexibility-scheduling/27_benchmark_report.md` for the
template the results are transcribed into):

```sh
# One scenario, base portfolio only:
cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-a-normal-day.json \
  --code-version <your-code-version> \
  --out benchmark-record-scenario-a.json

# One scenario with the ×2/×4 scaling probe (three records in one file):
cargo run -p flexcore-sim-runner -- benchmark \
  --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-c-dropout.json \
  --scaling 2,4 \
  --code-version <your-code-version> \
  --out benchmark-record-scenario-c.json
```

Exit codes: 0 = record(s) produced; 1 = completeness/structural failure
(only with `--require-complete`); 2 = usage/config error.

`--require-complete` fails the run when any §19.4 measured field is
`not_measured` (i.e. until the G5 real-solve legs land, it fails — by
design: the empty cell must be filled with a measurement or stay an honest
gap, never a zero).

The scaling copies are generated deterministically in memory (deep copy of
every portfolio site, suffixed `--x<factor>-<k>`); they are part of the
record (`scaling` block: factor, method, copied site keys, asset-key
count) and are not committed scenario files.

## CI

`.github/workflows/ci.yml` runs the P3.26 scenario suite
(`apps/sim-runner/tests/phase3_scenarios.rs`) as a regression gate and the
benchmark harness as a **record producer**: the `benchmark-record` artifact
is uploaded for inspection, and only record STRUCTURE is asserted. CI
records come from shared GitHub-hosted runners — they are context for
regression triage, **not baseline figures and not comparable across
commits**. The baseline is the human's single recorded run with its
hardware/container block filled in.
