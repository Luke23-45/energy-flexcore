# P5.16 — HELICS co-simulation federates (`apps/worker/python/helics_federates/`)

Status: **WRITTEN, NEVER EXECUTED — UNVERIFIED-API.** The Phase 5 loop bans
install/build/run: `helics==3.7.0` (`requirements-helics.txt`) was NEVER
installed, no broker was ever started, no federate ever joined a
federation, and no exit report was ever produced. Every helics call in
these scripts follows the DOCUMENTED 3.7.0 API as of the 2026-11-07
research cut-off; the mapping/assumption table in `federate_common.py`
names every call. The first federation run belongs to the human
build/test pass (see "How to run" below).

## What HELICS is here — and what it is not

HELICS **coordinates** the federation: it grants time steps to the
federates and carries their value publications/subscriptions. It **does
not replace the federates' physics models** (report §12.4, §19.1). The
physics stay in the existing models:

- the asset federate publishes the **Phase 2 asset simulator's** records
  (the canonical `ReplayRecord` feed) — it computes nothing;
- the grid federate drives the **UNMODIFIED P5.15 pandapower worker**
  (`../grid_worker.py`, the `flexcore-grid-worker/1` protocol) through a
  subprocess, one JSON document each way — it computes no power flow
  itself, and an AC power flow is NOT an AC-OPF;
- the control federate publishes **pre-computed schedule-fixture
  setpoints** — there is no optimizer behind it.

A completed co-simulation run is **not a power-system validation by
itself** (§19.1), and a steady-state power-flow result is never
transient-stability evidence. Everything here is simulation-only (the 0.x
line, §21.4): no field read, no field write, no control authority.

## Files

| File | Content |
|---|---|
| `federate_common.py` | Shared plumbing: config loading, the helics import guard (fail-closed — the gate that keeps this loop honest: without the installed binding NO exit report can ever be produced), the helics call/assumption table, exit-report emission |
| `broker.py` | The broker stand-alone script (in-process pyhelics broker, supervision loop, broker exit report) |
| `asset_federate.py` | The asset/end-use federate: publishes the Phase 2 simulator's records; records received setpoints |
| `grid_federate.py` | The grid federate: spawns the unmodified P5.15 worker per time step; publishes bus voltages (pu) / branch loadings (percent) |
| `control_federate.py` | The FlexCore/control federate: publishes pre-computed setpoint-fixture values; records received measurements |
| `federation_config.example.json` | The federation configuration TEMPLATE (three federates, pinned 15 s time step, broker address, publications/subscriptions) — the document the Rust orchestrator validates |
| `requirements-helics.txt` | The recorded `helics==3.7.0` pin (NEVER installed; D4 audit input; pyhelics-binding match is RESEARCH.md open task 3, still OPEN) |

## The federation topology (the record names it verbatim)

```text
control_federate --setpoint_p_mw (MW)--> asset_federate
asset_federate   --p_mw (MW) / q_mvar (MVAr)--> grid_federate (+ control_federate subscribes measurements)
grid_federate    --vm_pu (pu) / loading_percent (percent)--> control_federate
```

All power values are **injection-positive at the point of connection:
P > 0 means export** (§10.2). Time is HELICS-coordinated on the pinned
`time_step_seconds` (15 s in the template); federates request time,
receive granted times, and the orchestrator verifies — from the federates'
exit reports, at run time — that every federate's final granted time
equals the configured `stop_time_seconds` with equal granted-step counts
before it may record the time synchronisation as verified.

## Config discipline

- The federation config JSON is **scenario-level versioned data** (P5.16
  step 1; P5.18 consumes it as the scenario record's federation field).
- The **default scenario runs WITHOUT HELICS** (direct adapters); co-
  simulation is **opt-in**: `flexcore-sim-runner scenario-run
  --helics-federation <config.json> [--helics-execute]`.
- The **broker address is deployment configuration** (the config's
  `broker.address`), never a hard-coded constant.
- The config is validated by the Rust orchestrator (`apps/sim-runner/src/
  helics_orchestration.rs`) BEFORE any process is launched: protocol tag,
  pinned time step, broker address shape, the three-federate minimum,
  unique publication topics, every subscription matched to exactly one
  publication, canonical units (MW, MVAr, MVA, MWh, kV, A, pu, percent),
  and the per-role options. A validation failure is exit 2 — the
  federation never launches half-configured.

## How to run (the human build/test pass)

1. Confirm the pyhelics binding version matching HELICS 3.7.0
   (RESEARCH.md open task 3), then create an ISOLATED environment and
   install `helics==3.7.0` per `requirements-helics.txt`. NEVER install
   into a system environment; the archived `helics-apps` package must
   not be used.
2. Prepare the federation's data inputs and point the config at them:
   - the records feed (`run --out` output of `flexcore-sim-runner run`),
     with the `asset_key_map` filled from the records' actual asset
     UUIDs (the deterministic stand-in identities);
   - the grid model IR JSON (`VersionedNetworkModel` serde shape);
   - the setpoint fixture (asset_key → intervals).
3. Validate without executing:
   `flexcore-sim-runner scenario-run --scenario <path> --helics-federation apps/worker/python/helics_federates/federation_config.example.json`
   — this composes and records the co-sim topology in the run record
   WITHOUT launching anything (`execution_mode: "composed"`).
4. Execute the federation (adds `--helics-execute`): the orchestrator
   launches broker + federates concurrently with the bounded timeout and
   records every outcome fail-closed. Any federate failure, timeout, or
   contract violation is recorded as a fail-closed outcome in the run
   record — there is no path from a federate failure to a verified
   co-simulation record.

## Honest limitations

- Nothing in this directory has ever executed; the helics API usage is
  UNVERIFIED until the first federation run against the pinned version.
- The example config's data-input paths (`replay-records.json`,
  `grid-model.json`, `setpoints-fixture.json`) are TEMPLATES — the
  artefacts they name must be produced and referenced by the operator;
  the orchestrator validates only the config's structure, and the
  federates fail closed if a path is unreadable at run time.
- The exit reports are the federates' SELF-reported evidence
  (granted steps, final granted time, exchange counts). The orchestrator
  cross-checks final granted times and step counts across federates, but
  a deeper time-synchronisation audit (message-level timestamps) is a
  future task, not claimed here.
