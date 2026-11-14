# Decision Record — P5.18 (Scenario definition and replay), P5.19 (Replay
# reproducibility verification), P5.20 (Scenario library: benchmark cases)

- **Date:** 2026-11-11 (Phase 5 wave 4a; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.18 (§19.2 layer 5, §24 step 10, §19.3), P5.19 (§24
  step 10, §22.3), P5.20 (§19.3, §19.4).
- **Loop status:** NOTHING was installed, built, run, or executed — no
  `cargo build`/`check`/`test`, no `pip install`, no worker process, no
  docker, no Python worker. Every code path and test file below is
  complete source written only; every test file carries an UNRUN header;
  **the P5.19 double-run proof and the negative control are UNRUN — they
  are the human build/test pass's pre-registered run**, and no measured
  figure, verdict, or runtime exists anywhere in this record. No ledger
  checkbox claims tested/verified/passed.
- **Building on:** P5.14–P5.16 (`14_15_adapters.md`, `16_helics_cosim.md`,
  `17_scoping_decision.md`), the P3.26 pinned-record pattern
  (`scenarios/synthetic-portfolio/scenarios-phase3/`), the P3.25/P3.26
  pre-registration discipline
  (`ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md`),
  and the P4.35 measurement-template pattern
  (`ledger/phase-04-grid-feasibility-dispatch/35_gate_benchmark_report.md`).

---

## 1. P5.18 — Scenario definition and replay

### What was implemented

1. **The ScenarioRecord v3** (`apps/sim-runner/src/replay_record.rs`, NEW)
   — the additive evolution of the pinned-record pattern: the `phase3`
   pin block of the Phase 3 scenario files, promoted to a first-class,
   versioned, fail-loud record schema (`schema_version: "3"`). Every
   record pins: the telemetry scenario (path + id + version), the forecast
   fixture (when a solve leg consumes one), the NAMED synthetic grid-model
   fixture, the objective configuration (from the declared set — never
   free-text), the dispatch/tracking parameters (deployment configuration,
   D-P4.16-3), the backend selection (`scripted` / `worker_process`) with
   the provider pin, the master seed with the recorded derivation rule,
   the clock configuration, the pre-registered tolerances, the
   pre-registered expected-outcome slot (P3.26 discipline), the
   nondeterminism inventory (P5.19), the solver pins with their classes
   (AGENTS.md rule 4), the expected runtime class, and the per-case IEEE
   record (P5.20).
2. **The `replay-run` path** — a new CLI subcommand of
   `apps/sim-runner` (additive; all existing subcommands unchanged):
   `flexcore-sim-runner replay-run --record <path> [--double-run |
   --baseline <path>] [--out <path>] [--report <path>]`. It re-executes a
   stored ScenarioRecord from the recorded inputs ONLY and emits the
   **ReplayComparisonRecord**: the input-hash match (recomputed over the
   run-input sections vs the recorded hash — a mismatch is a TAMPER
   finding, never a silent re-hash), the per-leg comparisons with NAMED
   diffs, and the reproducibility verdict.
3. **The replay rule, implemented as validation (the P5.18 acceptance
   core):** any input that cannot be reproduced from the record is a
   NAMED error, never a silent default. Verified pins: the scenario file
   exists, loads all-or-nothing (P2.13), and carries EXACTLY the pinned
   id + version; the scenario's master seed EQUALS the pinned seed (the
   seed-override helper `scenario_json_with_seed` is deliberately NOT on
   this path — replay never re-seeds a pinned record); the clock pins
   (start / cadence / horizon) match the artefact; a `worker_process`
   selection carries its worker pin; `thread_count` is 1; the
   expectation slot is pre-registered; an unknown dispatch asset is a
   named error.
4. **The P4.37 closed loop, wired at the scenario level** (the P4.37 open
   item "closed-loop verification is Phase 5"). `ScenarioGridFeasibilityRunner`
   now captures, per dispatch slice, the pipeline outcome (steps 1–4) and
   the step-5 limit report (`ClosedLoopObservation` — observation only;
   the command gate's own verdict is UNCHANGED). After the slice, the
   captured evidence is routed through the FROZEN D-P4.07-1 rule
   (`flexcore_grid_api::verdict::route_pipeline_outcome` — the real grid
   crate's pure function, not a reimplementation) and fed to the FROZEN
   P4.37 consumption contract
   (`flexcore_application::feasibility_return::plan_consumption` — the
   real contract, imported, not mirrored). The consumption decision is
   RECORDED on the run record (`ScenarioRunRecord.consumption_decisions`).
   The re-scheduling consumer stays the existing P3.24 rolling-horizon
   trigger path — the frozen trigger set (`SCHEDULE_ROLL_TRIGGER_STRINGS`)
   is NOT modified and NOT extended; the trigger gap travels with the
   `ReOptimizationInput` per the P4.37 `OPEN_ITEM_STATEMENT`, restated on
   every recorded summary.
5. **The meshed two-feeder fixture** (additive; `GridFixtureKind`) — the
   §19.3 "multiple feeders/sites connected through a common substation
   constraint" case at fixture scale, BALANCED only, a NAMED fixture under
   the adapter's DECLARED `TopologyScope::RadialAndMeshed` (the P4.34
   record). The default `ScenarioRunHarness::new` behaviour is unchanged
   (three-bus radial); the named fixture is opt-in via
   `new_with_grid_fixture` and pinned per record.

### Decisions

- **D-P5.18-1 (DEFAULT):** the record schema is a NEW additive file
  (`replay_record.rs`), not an edit of the Phase 2 scenario loader or the
  frozen scenario artefacts. The pinned scenario files
  (`scenarios/synthetic-portfolio/**`) are consumed UNCHANGED — no
  existing `scenarios/*.json` was touched.
- **D-P5.18-2 (DEFAULT):** paths in a record resolve relative to the
  RECORD file's directory; the worker `script_path` is repo-root-relative
  (the CLI convention). Both conventions are stated in the library README
  and the CLI help.
- **D-P5.18-3 (DEFAULT):** the input hash is a double FNV-1a-64 digest
  over the canonical JSON of the RUN-INPUT sections (inputs, backend,
  seeds, tolerances, nondeterminism controls) — the repo's recorded
  derivation discipline. It is an identity TAG, not a cryptographic
  checksum; the pre-registration text (purpose, expected outcomes,
  references) is NOT hashed (changing it is a documentation change, not a
  scenario change).
- **D-P5.18-4 (DEFAULT):** the closed-loop capture is an additive
  observation on the existing runner, not a second verdict path. The
  command gate's verdict conversion and the §14.1 gate order are
  untouched.
- **D-P5.18-5 (DEFAULT):** the meshed fixture maps site 1's assets to
  bus_3 (feeder-a) and site 2's to bus_5 (feeder-b) — deterministic per
  site order; the radial fixture keeps every asset at bus_3 (unchanged).

### Honest limitations

- Nothing was built or run: the replay path, the closed-loop wiring, and
  all tests are UNRUN (pre-registered expectations; the first
  `cargo build`/`test` is the human's gate).
- On the scripted backend a grid check is a recorded FIXTURE outcome —
  never a computed power flow. A RETURN/CONSTRAIN outcome therefore only
  arises on a leg whose power flow actually violates a limit (the
  worker_process leg); the scripted-backend library runs route
  `within_limits` and record that honestly.
- The worker_process legs were NEVER executed (the P5.15 UNVERIFIED-API
  note carries forward unchanged); the library's stress records
  pre-register their fail-closed expectations for the human pass.
- A pipeline-level verdict that routed before an attempt exists carries
  empty candidate identity; the contract's structural validation refuses
  the conversion and the summary records `not_planned` with the reason —
  never silently dropped.

## 2. P5.19 — Replay reproducibility verification

### The verification method (§24 step 10)

The SAME scenario + the SAME versions + the SAME seeds produce the SAME
results WITHIN A STATED, PRE-REGISTERED tolerance. The checker
(`apps/sim-runner/src/reproducibility.rs`, NEW) implements two comparison
regimes and the verdict builder; the double-run execution itself is the
human's pre-registered run (UNRUN here — never executed by the authoring
loop).

### The tolerance table (pre-registered BEFORE any run — the anti-fitting rule)

| Output type | Rule | Relative | Absolute floor |
|---|---|---|---|
| Telemetry records (scripted deterministic legs) | `byte_identical` | — | — |
| ScenarioRunRecord (scripted grid backend) | `byte_identical` | — | — |
| ScenarioRunRecord (worker_process backend, numeric fields) | `relative_1e-9` | 1e-9 | 1e-12 |
| Telemetry records (worker_process legs, numeric fields) | `relative_1e-9` | 1e-9 | 1e-12 |
| Objective values (real-solve legs, VERIFY-BUILD) | `relative_1e-9` | 1e-9 | 1e-12 |
| Runtimes / iteration counts / ingest times / hardware | `never_compared` | — | — |

Both 1e-9 (relative) and 1e-12 (floor) are **DEFAULT** values, registered
in code (`DEFAULT_VALUE_RELATIVE_TOLERANCE` / `DEFAULT_VALUE_ABSOLUTE_FLOOR`),
mirrored in every library record's `tolerances` pin, and consistent with
the P3.26 pin block's `replay_tolerance: 1e-9`. A change is a recorded
decision — never a post-hoc fit. Quality states and sign conventions are
NEVER tolerance-bounded (a tolerance applies to numbers only).

### The nondeterminism-source inventory (each listed + neutralized)

| Source | Neutralization |
|---|---|
| wall_clock | no wall clock is read in any run path; every instant is the scenario clock via the controlled RunClock (P0.09 §5); `ingest_time` is excluded from comparison BY RECORD SHAPE (P2.13 §3) |
| hash_map_iteration_order | comparison orders are deterministic (compare.rs sorts on the frozen identity tuple); run-record maps are BTreeMap; asset identities derive in scenario-file order |
| thread_count | pinned `thread_count = 1` in the solver configuration (the P3.26 pin block, `scenarios/synthetic-portfolio/scenarios-phase3/` — CITED, not re-pinned); the sim-runner composition is single-threaded in-process; a record with a different value is rejected fail-closed |
| ambient_entropy | all run identities are deterministic FNV-1a-64 derivations; the pinned master seed is the only randomness source |
| floating_point_backend | the scripted backend is a byte-stable fixture; a worker leg is tolerance-bounded (1e-9 / 1e-12 DEFAULT) with the provider pin recorded per run |
| helics_federation_execution | the HELICS leg is OPT-IN (P5.16); a co-sim leg is checked only against the SAME federation config + versions; its failures are fail-closed |

### The pre-registered verification plan (the human's runs; UNRUN)

The expected identical-output property: for each `scripted` library case,
`replay-run --double-run` yields verdict **`identical`** — every leg
byte-identical (telemetry on the frozen P2.24 tuple; the whole
ScenarioRunRecord JSON) and `input_hash_match: true`. For a
`worker_process` case, numeric fields may differ within relative 1e-9
(floor 1e-12); anything beyond is `mismatch` with named diffs.

```sh
# The proof (double-run, both legs) — per scripted case:
cargo run -p flexcore-sim-runner -- replay-run \
  --record scenarios/library/lib-02-small-portfolio.json \
  --double-run \
  --code-version <record-your-code-version> \
  --report replay-proof-lib-02.json
# (repeat for lib-01, lib-03, lib-06, lib-07, lib-08)

# The baseline form (a stored run record; telemetry leg honestly
# `not_compared` there):
cargo run -p flexcore-sim-runner -- scenario-run \
  --scenario scenarios/synthetic-portfolio/scenario-v2.json \
  --out baseline-run-record.json --code-version <code-version>
cargo run -p flexcore-sim-runner -- replay-run \
  --record scenarios/library/lib-02-small-portfolio.json \
  --baseline baseline-run-record.json \
  --report replay-baseline-lib-02.json

# The NEGATIVE CONTROL (pre-registered, ledger P5.19 step 2): change ONE
# recorded input (a seed, a scenario version, a dispatch target) and
# verify the replay either ERRORS loudly at pin verification or produces
# a NAMED diff — never a silent reproduction:
#   e.g. pin seeds.master_seed = 999999 in a copy of lib-02 →
#   replay-run exits 2 with "the pinned scenario's master seed is 20260601
#   but the record pins 999999; ... replay never re-seeds a pinned record"
#   e.g. doctor one value in the stored baseline JSON → replay-run exits 1
#   with the mismatched path named in the run_record leg's diffs.
```

Acceptance evidence (ledger P5.19 exit check): the two clean runs match
within the pre-stated tolerances (the verdicts above), the negative
control behaves as specified, and the verdict is visible to the
Simulation Lab (P5.08, which consumes the ReplayComparisonRecord
verdict). **None of this has been executed** — the CI wiring note
(ledger P5.19 step 4) is recorded for P5.28 (which owns CI files; the
reproducibility check joins the scenario regression suite there, per
§22.2).

### Honest limitations

- The proof runs are UNRUN: no reproducibility verdict exists anywhere in
  this record; the checker and the plan are implemented and unit-tested
  (UNRUN tests), nothing more.
- The tolerance-bounded regime has never compared a real worker leg (the
  worker was never executed); the 1e-9 DEFAULT is the P3.26-inherited
  pre-registration, not an empirically fitted bound.

## 3. P5.20 — Scenario library: benchmark cases

### The §19.3 mapping (read from the report §19.3 enumeration — cited, not invented)

Report §19.3: *"Start with a small set of recognised cases and add scale
gradually"* — (1) IEEE 13/34/123-bus distribution feeders where licensing
and model availability are confirmed; (2) IEEE 39/118-bus transmission
systems for selected grid studies; (3) synthetic portfolios with BESS, PV,
flexible load, and EVSE; (4) multiple feeders/sites connected through a
common substation constraint; (5) stressed scenarios such as PV ramps,
forecast error, communication loss, asset dropout, reverse power flow,
and topology change. Scale limit (verbatim discipline): *"Do not claim
that the first distribution feeder tests establish capability for a
10,000-bus utility network."*

| Library record | §19.3 bullet | Scale | Backend | IEEE record |
|---|---|---|---|---|
| `lib-01-minimal-single-asset.json` | (3) synthetic portfolios — single-asset scale | minimal | `scripted` | excluded |
| `lib-02-small-portfolio.json` | (3) synthetic portfolios — full fixture scale (BESS+PV+flexible load+EVSE+meter, two sites) | small | `scripted` | excluded |
| `lib-03-medium-multi-site-meshed.json` | (4) multiple feeders/sites through a common substation constraint (fixture scale, balanced) — also the topology-change coverage at fixture scale | medium | `scripted` | excluded |
| `lib-04-stress-worker-timeout.json` | (5) stressed scenarios — solver-timeout analogue (the P5.21 injection suite owns the full fault set) | stress | `worker_process` (UNRUN) | excluded |
| `lib-05-stress-nonconvergence.json` | (5) stressed scenarios — non-convergence (the §19.4 AC-power-flow-convergence metric) | stress | `worker_process` (UNRUN) | excluded |
| `lib-06-stress-pv-ramp.json` | (5) stressed scenarios — PV ramps, forecast error (the P3.26 scenario (b) pairing) | stress | `scripted` | excluded |
| `lib-07-stress-asset-dropout.json` | (5) stressed scenarios — asset dropout (the P3.26 scenario (c) pairing); communication loss analogue at the telemetry layer | stress | `scripted` | excluded |
| `lib-08-stress-reverse-power-flow.json` | (5) stressed scenarios — reverse power flow | stress | `scripted` | excluded |

IEEE bullets (1) and (2): **excluded in every record** — no IEEE
13/34/123-bus or 39/118-bus model is available in this repository and
neither licensing nor model availability is confirmed (the mandatory
per-case record carries the reason; the same honest entry as the
`14_15_adapters.md` feeder-case record). A future inclusion requires the
confirmed license/availability IN the per-case record first, and the
record validation refuses an inclusion whose reason does not record the
confirmation.

### What was built

- `scenarios/library/` (NEW): 8 ScenarioRecord v3 cases + the README (the
  synthetic-only statement, the path conventions, the scale-limit
  statement) + `INDEX.md` (per-case purpose, scale, expected runtime
  CLASS — never a measured number, backend dependency).
- `scenarios/library/fixtures/scenario-minimal-single-asset.json` (NEW):
  the smallest replayable P2.13 scenario (one tenant, one site, ONE BESS,
  24 h, 15-min cadence, seed 20260601) — the minimal case's pinned
  telemetry artefact.
- The schema-validation tests
  (`apps/sim-runner/tests/scenario_library_p5_20.rs`, NEW, UNRUN): every
  record parses + validates; every pinned input exists and its identity
  matches; every expectation slot is pre-filled with status UNRUN; the
  IEEE exclusions are recorded with reasons; the INDEX names every record;
  the scale-limit statement is present.

### Decisions

- **D-P5.20-1 (DEFAULT):** the library starts at fixture scale (one asset
  → the two-site fixture → the meshed two-feeder fixture) with stress
  cases as pinned records — "start small and add scale gradually", no
  scaling claim of any kind.
- **D-P5.20-2 (DEFAULT):** the reverse-power-flow case pins the EXPORT
  dispatch target and records the scripted-backend limitation honestly
  (the scripted backend CANNOT produce a reverse-power-flow violation);
  the violation expectations are pre-registered for the worker leg
  (UNRUN).
- **D-P5.20-3 (DEFAULT):** the topology-change stressor is covered by the
  meshed fixture case — a fixture-level topology class under the DECLARED
  scope, never a runtime switching model.
- **D-P5.20-4 (DEFAULT):** the communication-loss stressor is covered as
  the telemetry-layer analogue (the drop window emits no frames); the
  full §19.2 layer 6 fault set is P5.21's (cross-referenced, not
  duplicated).

### Honest limitations

- No library case has been RUN; every expected outcome is pre-registered
  and UNRUN. The P5.23 benchmark fills its measured figures per case
  under the P4.35 rules (conditions travel; raw numbers; no targets) —
  nothing is pre-measured here.
- The worker_process cases cannot run until the human build/test pass
  executes the worker (UNVERIFIED pandapower API).
- The forecast fixtures' consumption is the real-solve leg (VERIFY-BUILD):
  the sim-runner composition does not solve, so the pins validate
  presence + version only.

## 4. Files touched (this subtask set)

| File | Change |
|---|---|
| `apps/sim-runner/src/replay_record.rs` | NEW — the ScenarioRecord v3 schema + validation, the input hash, the `replay-run` execution path, the ReplayComparisonRecord |
| `apps/sim-runner/src/reproducibility.rs` | NEW — the P5.19 checker: the tolerance table, the nondeterminism inventory, the tolerance/structural comparisons, the verdict builder |
| `apps/sim-runner/src/feasibility_consumption.rs` | NEW — the P4.37 closed loop at the scenario level (frozen routing + frozen contract + the recorded summary) |
| `apps/sim-runner/src/scenario_run.rs` | additive — `GridFixtureKind` (+ the meshed two-feeder fixture), `new_with_grid_fixture`, the `ClosedLoopObservation` capture, the `consumption_decisions` field + `ConsumptionDecisionSummary` on the run record |
| `apps/sim-runner/src/main.rs` | additive — the `replay-run` subcommand; the closed-loop summary attached to the scenario-run record + its summary line; fixed a pre-existing stray-brace syntax error and a duplicated EXIT-CODES paragraph in the usage text |
| `apps/sim-runner/src/lib.rs` | module registration (coexisting with the wave-4b modules) |
| `scenarios/library/**` | NEW — 8 records + README + INDEX + the minimal fixture |
| `apps/sim-runner/tests/scenario_replay_p5_18.rs` | NEW — UNRUN |
| `apps/sim-runner/tests/reproducibility_p5_19.rs` | NEW — UNRUN |
| `apps/sim-runner/tests/scenario_library_p5_20.rs` | NEW — UNRUN |
| `ledger_phase_5.md` | P5.18/P5.19/P5.20 objective boxes ticked "implemented, awaiting human build/test" |
| `HANDOVER.md` | the wave 4a row + the implemented count |

Verification-tool discipline (AGENTS.md §6): `python tools/verify_docs.py`
and `python tools/validate_agent.py` were run at close and their output is
recorded in HANDOVER.md; `python -I tools/check_dependency_rule.py` was
run for the new edges (the sim-runner is a composition root: the
grid-api/application imports are existing allowed edges; no new crate
dependency was added to any Cargo.toml).
