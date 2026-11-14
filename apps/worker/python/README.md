# P5.15 — the Python grid numerical worker leg

This directory is the Python leg of the P5.15 grid-simulator adapter
integration: the process behind the `WorkerProcessBackend`
(`crates/flexcore-grid-api/src/worker_backend.rs`). The Rust grid-api
backend spawns this script, writes ONE `flexcore-grid-worker/1` JSON request
on its stdin, and reads ONE JSON response on its stdout (the
`BackendPowerFlowOutcome` shape behind the protocol tag).

## Contents

| File | Content |
|---|---|
| `grid_worker.py` | The worker entrypoint: grid-api model IR + translated injections → pandapower AC power flow → result payload |
| `requirements.txt` | The recorded pin `pandapower==3.5.6` (D-P4.01-1) — **NEVER INSTALLED** in this loop |

## Honest status (do not skip)

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

## How the human build/test pass runs it (the recorded procedure)

```sh
python -m venv .venv-grid-worker          # never committed
.venv-grid-worker/Scripts/pip install -r apps/worker/python/requirements.txt
# then point a scenario run at the real backend, e.g.:
# flexcore-sim-runner scenario-run --scenario scenarios/synthetic-portfolio/scenario-v2.json \
#     --grid-backend worker --worker-program .venv-grid-worker/Scripts/python \
#     --worker-script apps/worker/python/grid_worker.py --worker-timeout-ms 30000
```

The scenario-run record names the backend (`worker_process`) and carries the
worker's reported pandapower version, so a run can never silently substitute
the scripted seam backend for the real one — the two are distinct, named
adapters in the run record's adapter inventory.
