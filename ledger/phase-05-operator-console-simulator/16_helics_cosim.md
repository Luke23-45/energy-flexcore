# P5.16 — HELICS co-simulation integration (§12.4, §19.1)

Date: 2026-11-11. Status: **implemented, awaiting human build/test** — the
Phase 5 loop bans hold: NOTHING was built, NOTHING was run, NOTHING was
installed (no `cargo build`/`check`/`test`, no `pip install`, no helics
download, no broker, no federate, no docker). `helics==3.7.0` is the
RESEARCH.md §1 pin (2026-11-07) and was **NEVER installed**. Every helics
API call in this subtask follows the DOCUMENTED 3.7.0 API as of the
research cut-off and is **UNVERIFIED** until the human's build/test pass
executes the federation against the pinned version. The pyhelics binding
version matching 3.7.0 remains RESEARCH.md open task 3 (still OPEN).

Report trace: §12.4 (HELICS row: "It coordinates federates; it does not
replace their physics models" [R16]), §19.1 (multi-domain co-simulation
track; "Do not use HELICS itself as the power-system physics model"),
§23 Stage 5, §7.4 (simulator environment), §8.1 (`apps/sim-runner`).

---

## The recorded role statement (carried in every record and document)

**HELICS coordinates the federation: it grants the time steps and carries
the value exchange between federates. It does NOT replace the federates'
physics models (§12.4, §19.1).** The physics stay in the EXISTING models —
the Phase 2 asset simulator's behaviour profiles and the Phase 4 pandapower
AC power-flow pipeline (driven through the UNMODIFIED P5.15 worker
protocol, one JSON document each way). A completed co-simulation run is
**not a power-system validation by itself**, and a steady-state power-flow
result is **never transient-stability evidence** (§19.1 — the P5.17
constraint statement). Everything is simulation-only (§21.4): the
federation exchanges federation VALUES (setpoint-fixture values,
measurements from the existing models), never commands, and nothing here
implies field control, safety, certification, or any protection-grade
function.

## The composed topology (the run record names it verbatim)

```text
control_federate --setpoint_p_mw (MW)--> asset_federate
asset_federate   --p_mw (MW) / q_mvar (MVAr)--> grid_federate
                    (+ control_federate subscribes measurements)
grid_federate    --vm_pu (pu) / loading_percent (percent)--> control_federate
```

Three value federates + one broker, the P5.16 step-1 minimum: a grid
federate (the Phase 4 pandapower pipeline), an asset/end-use federate
(the Phase 2 simulator's records feed), and a FlexCore/control federate
(pre-computed schedule-fixture setpoints — NO optimizer behind it, the
same optimizer-port seam discipline the P5.14 composition records). All
power values are injection-positive at the point of connection: P > 0
means export (§10.2); voltage is per-unit on the declared nominal base.

The physics flow, federate by federate:

| Federate | Physics source (existing model) | What the federate itself does |
|---|---|---|
| `asset` | Phase 2 simulator profiles, via the canonical `ReplayRecord` feed (`run --out`) | Replays records onto the HELICS clock; publishes usable samples (GOOD/SUSPECT only; BAD/MISSING/ESTIMATED/STALE skipped and COUNTED, never coerced); records received setpoints |
| `grid` | Phase 4 pandapower AC power flow, via the UNMODIFIED `grid_worker.py` (`flexcore-grid-worker/1`, one JSON each way) | Spawns the worker per time step, publishes bus voltages / branch loadings; a non-converged outcome is an IN-BAND result (publishes nothing that step, counts the skip, never fabricates a voltage); a worker transport failure is fail-closed |
| `control` | The pinned schedule fixture | Publishes fixture setpoints; records received measurements; contains no optimization and no physics |

## What was built

1. **The federation config** (`apps/worker/python/helics_federates/federation_config.example.json`)
   — the scenario-level versioned data (P5.16 step 1): federation name,
   broker address AS DEPLOYMENT CONFIGURATION, pinned 15 s time step,
   stop time, per-federate script paths + interpreters + options, and the
   full publication/subscription list with declared canonical units
   (MW, MVAr, MVA, MWh, kV, A, pu, percent — MW/MVAr/pu/percent used here).
   The P5.18 scenario record consumes this document as its federation
   field.
2. **The Python federate scripts** (`apps/worker/python/helics_federates/`):
   `federate_common.py` (shared plumbing + the helics call/assumption
   table + the FAIL-CLOSED helics import guard — without the installed
   binding NO exit report can ever be produced, so a co-simulation record
   can never exist without helics having actually run), `broker.py`
   (in-process pyhelics broker, supervision loop, broker exit report),
   `asset_federate.py`, `grid_federate.py`, `control_federate.py`. On
   SUCCESS each federate emits ONE `flexcore-helics-federate/1` exit
   report (granted steps, final granted time, exchange counts, the role
   statement); every failure path prints NOTHING on stdout and exits
   non-zero — no false reports (the grid_worker.py fail-closed pattern).
3. **The Rust orchestration module** (`apps/sim-runner/src/helics_orchestration.rs`,
   NEW, additive): config parsing + fail-closed validation (protocol tag,
   pinned time step, broker address shape/scheme/port, the three-federate
   minimum, unique names, globally unique publication topics, every
   subscription matched to exactly one publication, canonical units,
   `double` kind, per-role REAL-data-input options), the launch plan
   (broker FIRST, then federates in config order — pure composition,
   nothing spawned), the REAL `ProcessFederateLauncher` (all processes
   spawned concurrently, per-process bounded wait, hung process KILLED,
   fail-closed on spawn failure/timeout/non-zero exit/malformed report —
   the P5.15 worker-process pattern widened to a process group; helper
   thread + `recv_timeout`, no async runtime), exit-report parsing
   (protocol tag, kind, process identity, `ok`, finiteness), the
   time-synchronisation verification (every federate's final granted time
   == the configured stop time within 1e-6 s AND equal granted-step
   counts — anything else is recorded as NOT synchronised, never coerced),
   and the `HelicsCoSimRecord`.
4. **The CLI wiring** (`apps/sim-runner/src/main.rs`, additive):
   `scenario-run --helics-federation <config.json>` composes and records
   the co-sim topology (`execution_mode: "composed"`); adding
   `--helics-execute` actually launches the federation
   (`execution_mode: "executed"`), with `--helics-timeout-ms` and
   `--helics-broker-program` as deployment configuration. When the flag
   is absent the output is byte-compatible with the P5.14/P5.15 record;
   when present the output is an envelope `{"scenario_run": …,
   "helics_cosim": …}` — the run record names the co-sim topology
   (P5.16 step 1; the P5.18 scenario-record field is this envelope's
   `helics_cosim` document).
5. **The test suite** (`apps/sim-runner/tests/helics_orchestration_p5_16.rs`,
   NEW, UNRUN): 16 pre-registered tests over the scripted federate
   launcher (a composition double over the `FederateLauncher` seam — NO
   process is ever spawned, NO helics anywhere), covering every
   validation rule, the launch plan, exit-report fail-closed parsing, and
   the execution/record honesty (all-ok federation verifies sync; a
   timed-out, missing-output, short-time, or diverging-count federation
   NEVER verifies).

## Decisions

- **D-P5.16-1 (DEFAULT): orchestration layer, not a replacement.** HELICS
  wraps the EXISTING adapters (P5.14 simulator, P5.15 worker pipeline);
  no physics moves into any federate script, and no existing adapter code
  was modified. The grid federate re-uses the P5.15 worker through its
  own subprocess and its exact protocol; the asset federate consumes the
  simulator's own records feed.
- **D-P5.16-2 (DEFAULT): no Rust helics binding.** No `helics`/`heliocs`
  crate is added and none exists in this repository: an unverified
  dependency would violate the loop bans and the dependency policy. The
  Rust side composes and launches PROCESSES (the grid_worker.py
  subprocess pattern); the helics binding stays a Python-side, pinned,
  never-installed dependency (`requirements-helics.txt`, separate from
  `apps/worker/python/requirements.txt`).
- **D-P5.16-3 (DEFAULT): co-simulation is opt-in.** The default scenario
  runs WITHOUT HELICS (direct adapters — the P5.14/P5.15 composition,
  unchanged); HELICS enters only via `--helics-federation`, and the
  federation only launches with `--helics-execute`. Composition
  (`composed`) and execution (`executed`) are distinct, recorded
  execution modes — a composed record never implies a federation ran.
- **D-P5.16-4 (DEFAULT): time synchronisation is verified from the
  federates' exit reports at run time** (final granted times + step
  counts), and the deeper audit (message-level timestamps) is recorded as
  a future task in the README — claimed nowhere.
- **D-P5.16-5 (DEFAULT): exit reports are SELF-reported evidence.** The
  orchestrator validates shape/identity/consistency fail-closed, but it
  cannot observe the broker's wire traffic; the record says what the
  federates reported, and the first real federation run is the human's
  verification point.

## Honest limitations (the UNVERIFIED-API block)

- **Nothing was executed.** No helics installation, no broker, no
  federate, no exit report ever produced. Every helics call (listed in
  the `federate_common.py` table) is a DOCUMENTED-API form, marked
  UNVERIFIED; the recorded assumptions (e.g. the `--broker=<address>`
  core-init string) are named there and re-checked at the human's first
  federation run.
- **The tests are UNRUN** (pre-registered expectations, scripted doubles
  — no helics, no processes). The first `cargo test --workspace` is the
  human's gate.
- **The example config's data-input paths are TEMPLATES** (records feed,
  model IR JSON, setpoint fixture): the orchestrator validates structure;
  the artefacts must be produced and referenced by the operator, and the
  federates fail closed on an unreadable path at run time.
- The pinned version provenance (`HELICS_VERSION_PROVENANCE`) travels on
  every record so no run can lose it: helics 3.7.0 per RESEARCH.md,
  NEVER installed, re-verify at the first federation run (with RESEARCH.md
  open task 3, the pyhelics binding match — the archived `helics-apps`
  package must not be used).
- Per the ledger discipline, the P5.16 tick is "implemented, awaiting
  human build/test" — NEVER "tested/verified/passed".

## Files touched (this subtask)

| File | Change |
|---|---|
| `apps/worker/python/helics_federates/federate_common.py` | NEW — shared plumbing, helics call/assumption table, fail-closed import guard, exit-report emission |
| `apps/worker/python/helics_federates/broker.py` | NEW — the broker stand-alone script (UNVERIFIED-API) |
| `apps/worker/python/helics_federates/asset_federate.py` | NEW — the asset/end-use federate (UNVERIFIED-API) |
| `apps/worker/python/helics_federates/grid_federate.py` | NEW — the grid federate over the UNMODIFIED P5.15 worker (UNVERIFIED-API) |
| `apps/worker/python/helics_federates/control_federate.py` | NEW — the FlexCore/control federate (setpoint fixture; no optimizer) |
| `apps/worker/python/helics_federates/federation_config.example.json` | NEW — the federation configuration TEMPLATE (scenario-level versioned data) |
| `apps/worker/python/helics_federates/requirements-helics.txt` | NEW — the recorded `helics==3.7.0` pin (NEVER installed; D4 audit input) |
| `apps/worker/python/helics_federates/README.md` | NEW — honest status, topology, config discipline, the human run procedure |
| `apps/sim-runner/src/helics_orchestration.rs` | NEW — the orchestration module (validation, plan, launcher, reports, record) |
| `apps/sim-runner/src/lib.rs` | additive module registration + doc row |
| `apps/sim-runner/src/main.rs` | additive `--helics-*` CLI options + the co-sim leg in `scenario_run_command` + envelope output |
| `apps/sim-runner/tests/helics_orchestration_p5_16.rs` | NEW — 16 pre-registered tests, scripted launcher (UNRUN) |

Verification-tool discipline (AGENTS.md §6): `python tools/verify_docs.py`,
`python tools/validate_agent.py`, and `python -I
tools/check_dependency_rule.py` were run at close and recorded in the
HANDOVER (the run outputs are in the P5.16/P5.17 wave's handover entry).
