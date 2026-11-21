# apps/worker/python — the Python numerical workers

This directory is the Python leg of the worker area: the P5.15
grid-simulator adapter worker (`grid_worker.py`, the process behind the
`WorkerProcessBackend` in `crates/flexcore-grid-api/src/worker_backend.rs`),
the P5.16 HELICS co-simulation federates, and the Phase 8 / Phase 15
research workers with their pure support tooling. **Everything here was
WRITTEN, NEVER EXECUTED** — the loop bans install/build/run, so the first
execution of any module in this tree is a recorded human gate (each
package's README names its own gate and honesty header).

## Contents

| Path | Owning phase | Content |
|---|---|---|
| `grid_worker.py` | P5.15 | The grid-api worker backend process: one `flexcore-grid-worker/1` JSON request on stdin → one AC-power-flow result envelope on stdout (pandapower `runpp`, guarded import) |
| `helics_federates/` | P5.16 | The HELICS co-simulation federates (broker + asset/grid/control roles, coordination-only) and the example federation config; `helics==3.7.0` recorded, never installed |
| `forecast_worker/` | P15.02 (completed by P15.17) | The statistical baseline forecast worker: frozen no-skill baseline rule sets → one `flexcore-forecast-worker/1` request on stdin → one proposal envelope on stdout; numpy is the guarded import |
| `gnn_worker/` | P8.11 (evaluation P8.12) | The GNN warm-start/screening inference worker (torch/PyG DECISION-POINT, guarded imports, no inference output exists) and `gnn_evaluation.py`, the pure end-to-end-metric harness |
| `marl_worker/` | P8.13/P8.14 | The MARL simulation/shadow worker (Ray/RLlib DECISION-POINT): pure filter-probe path, CBF safety filter, shadow log, evaluation harness; no policy output exists |
| `andes_worker/` | P8.16 (metrics P8.17; cross-checks P8.39) | The ANDES dynamic-simulation research worker (ANDES 2.0.0 recorded, GPLv3-isolated, guarded import), the pure P8.17 response-metrics harness, and the P8.39 cross-check module |
| `portfolio_analytics/` | P8.22 | Pure-stdlib portfolio analytics: derived reporting over injected data, all measured cells EMPTY |
| `reproducibility/` | P8.20 | Pure-stdlib reproducibility tooling: record schema validator, seed derivation, rerun comparator |
| `drill/` | P8.24 | Fault-injection scaffolding for the research-module robustness drill (pure stdlib over injected callables; every drill cell UNRUN) |
| `training-data-manifest-v1.json` | P8.35 | The GNN training-data family assignment (findings F-1/F-2; every per-instance row EMPTY until the human/lab generation gate) |
| `training-data-manifest-v2.json` | P15.18 | The forecasting dataset-registration discipline; carries the v1 GNN assignment forward BY REFERENCE (v1 is never edited) |
| `requirements.txt` | P5.15 | The recorded pin `pandapower==3.5.6` (D-P4.01-1) — **NEVER INSTALLED** in this loop |
| `requirements-dev.txt` | P8.37 | The recorded test-tooling pin `pytest==9.2.0` (test-only; SECONDARY-SOURCE PIN; RE-VERIFY AT FIRST INSTALL) — never installed in this loop |
| `otel_instrumentation.py` | FIM-F09.01 | The shared stdlib-only OTEL trace-context module: strict W3C `traceparent` parse/serialize, JetStream-header/envelope/`TRACEPARENT` extraction, schema-v1 (P11.07) span-carrying log lines, and the P11.08 emit-time redaction (closed vocabulary mirrored from `python/workers/worker_template/redaction.py` plus the workers' `FORBIDDEN_RESULT_KEYS`); guarded OTel-SDK import, no-op fallback DEFAULT; in-module `--self-test` |
| `tools_fim/` | FIM-F09.03 | `validate_collector_config.py`: STRUCTURAL validation of `deploy/compose/otel-collector-config.yaml` (three signals wired, references resolved) via a documented minimal YAML-subset reader — a static tool, NO network, NO Collector run; `--self-test` pins included |
| `tests_fim_otel/` | FIM-F09.04 | `selftest.py` (runnable `python -I`): pinned propagation round-trips, malformed refusals, exact Rust-side header-name extraction, seeded-secret redaction, no-op fallback equality (with and without the SDK import guarded out), and the validator's run against the checked-in config |

Each package's README documents its own files, honesty header, and human
gate; this top-level README does not repeat them.

## Transport and self-test conventions

- **Transport** (the five result-emitting workers: `grid_worker.py`,
  `forecast_worker`, `gnn_worker`, `marl_worker`, `andes_worker`): ONE
  JSON request document on stdin → ONE JSON result envelope on stdout.
  Malformed input → NOTHING on stdout, a reason on stderr, exit 2. An
  in-band refusal is a result envelope with `termination_status:
  "failed"` — never a crash, never a fabricated figure.
- **`--self-test` flag**: the gnn, marl, and forecast workers' `__main__`
  runs the in-module hand-computed self-tests when invoked with
  `--self-test` (exit 0/1) and its documented stdin→stdout transport
  otherwise (bare invocation = transport). The andes worker's `__main__`
  is the bare transport (`raise SystemExit(main())`); its in-module
  self-tests live in `evaluation_harness.py` / `cosim_crosschecks.py`,
  which carry the same `--self-test` flag. `grid_worker.py`'s `__main__`
  runs the transport directly. The pytest suites under each package's
  `tests/` mirror the same expectations.
- **Guarded imports**: every framework (pandapower, numpy, torch/PyG,
  Ray/RLlib/Gymnasium, ANDES, helics) is imported INSIDE a function that
  raises a NAMED deployment requirement — the pure-logic core imports,
  runs, and self-tests with NOTHING installed, which is also the loop's
  honesty gate.
- **Proposal-only / ADR-010**: the research workers' output trees are
  scanned by `assert_proposal_only` before emission; no
  authorization/command-shaped key can leave a worker. The four research
  workers are mode-locked to simulation/shadow (a constant, never a
  request-selectable field); the grid worker computes a power-flow
  statement about the declared model only — neither it nor any worker
  here carries a field-write path (simulation-only 0.x line).

## FIM-F09 instrumentation (the F-09 Python OTEL wave; WRITTEN-UNRUN)

Every package here except `grid_worker.py` (outside the F-09 wave's
file set) imports `otel_instrumentation.py` guarded in its entrypoint(s)
and emits ONE schema-v1 worker-start trace line on stderr carrying the
W3C `traceparent` context (resolved from the message envelope / the
`TRACEPARENT` env var; absent context stays EMPTY, never synthesized).
The `drill` package has no README of its own: its
`fault_injection.py` `__main__` carries the same guarded import and
entry-start line. Honesty boundary: the OpenTelemetry SDK is OPTIONAL
and NOT installed (never added to any requirements file); the no-op
fallback is the DEFAULT posture and the SDK-present OTLP export path is
UNPROVEN until a human runs it with the SDK installed. A RUNNING
Collector gateway and FIRING SLO alerts are HUMAN GATES (F-09
acceptance criteria 1-3 of
`docs/releases/final-implementation-before-publication.md` §5 need live
infrastructure this repository has never had); the checked-in Collector
config is validated STRUCTURALLY by `tools_fim/validate_collector_config.py`
— never executed. Each package README's FIM-F09 section names its own
hook sites; the wave-7 self-test suite runs with
`python -I apps/worker/python/tests_fim_otel/selftest.py`.

## Honest status: `grid_worker.py` (the entry point the Rust backend spawns)

1. **The worker was written, never executed.** pandapower 3.5.6 was never
   installed, no worker process was ever spawned against real pandapower,
   and no power-flow result anywhere in this repository was computed by it.
   Every pandapower call in `grid_worker.py` follows the provider's
   documented API as of the 2026-11-07 research cut-off and is
   **UNVERIFIED** until the human's build/test pass executes the worker
   against the pinned version. The Rust backend's own payload checks
   (protocol tag, JSON shape, finiteness, row completeness at the adapter)
   are the fail-closed boundary that contains any mismatch.
2. **Solver class:** AC power flow (Newton–Raphson, `runpp`) of SPECIFIED
   injections — not AC-OPF, not a state estimator, not a stability tool
   (AGENTS.md rule 4; §12.5; §19.1).
3. **Scope:** balanced positive-sequence, radial distribution model.
   Unbalanced three-phase, protection, dynamics, and sub-cycle behaviour
   are not modelled (D3 OPEN; the v0.4.0 unbalanced deferral record).
4. **Test coverage:** the Rust side is tested against SCRIPTED fake worker
   payloads (a self-spawning test-double subprocess, not pandapower) — see
   `crates/flexcore-grid-api/tests/worker_backend_p5_15.rs`. The scripted
   doubles prove the transport and fail-closed paths; they prove NOTHING
   about pandapower behaviour.

The same written-never-run posture holds for every package above — each
package README's status line is the authoritative statement for that
package.

## How the human build/test pass runs things (the recorded procedures)

The runtime pin lives in `requirements.txt` (grid worker only); the
test-only tooling pin lives in `requirements-dev.txt` (pytest, for every
package's `tests/` suite). Both are RECORDED, NEVER INSTALLED in this
loop — installation happens at the human gate, into an isolated venv,
after re-verifying the pins per each file's header:

```sh
python -m venv .venv-grid-worker          # never committed
.venv-grid-worker/Scripts/pip install -r apps/worker/python/requirements.txt
# then point a scenario run at the real backend, e.g.:
# flexcore-sim-runner scenario-run --scenario scenarios/synthetic-portfolio/scenario-v2.json \
#     --grid-backend worker --worker-program .venv-grid-worker/Scripts/python \
#     --worker-script apps/worker/python/grid_worker.py --worker-timeout-ms 30000
```

```sh
# the test-tooling pin (pytest) for every package's tests/ suite:
pip install -r apps/worker/python/requirements-dev.txt
# per-package example (each package README names its own command):
PYTHONPATH=apps/worker/python/gnn_worker python -m pytest apps/worker/python/gnn_worker/tests -v
# the in-module hand-computed self-tests run WITHOUT pytest:
python apps/worker/python/gnn_worker/gnn_inference_worker.py --self-test
```

The scenario-run record names the backend (`worker_process`) and carries the
worker's reported pandapower version, so a run can never silently substitute
the scripted seam backend for the real one — the two are distinct, named
adapters in the run record's adapter inventory.
