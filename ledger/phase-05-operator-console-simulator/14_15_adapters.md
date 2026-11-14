# P5.14 + P5.15 — Simulator adapter integrations: asset simulator and grid simulator

Date: 2026-11-11. Status: **implemented, awaiting human build/test** — the
Phase 5 loop bans hold: NOTHING was built, nothing was run, nothing was
installed (no `cargo build`/`check`/`test`, no `pip install`, no worker
execution, no docker). Every code path below is written, never executed;
every expected outcome is a pre-registered expectation, not a measured
figure. The first `cargo build`/`test --workspace` is the human's gate.

Report trace: §19.1 (simulation strategy — asset sim + distribution
quasi-static tracks integrated in Phase 5), §12.4 (pandapower row), §8.1
(`apps/sim-runner`), §24 vertical slice steps 5–8 (extended composition),
§21.4 (simulation-only 0.x).

---

## P5.14 — Asset simulator integration (§19.1, §8.1)

### What was integrated

The full scenario-run composition lives in
`apps/sim-runner/src/scenario_run.rs` (`ScenarioRunHarness` + the
`ScenarioRunRecord`), wired into the sim-runner CLI as the new additive
`scenario-run` subcommand (`apps/sim-runner/src/main.rs`). One invocation
executes the §24 slice against the REAL Phase 2 simulator and the REAL
Phase 4 adapter chain, on the scenario clock only:

1. **Telemetry ingest** — the REAL `SimulatorTelemetrySource` (Phase 2
   simulator crate) through the `TelemetrySource` port, tenant-scoped
   subscription, bounded pull-based emission. This is the same provider the
   control-plane ingestion composes (`apps/worker` `compose_ingestion`) —
   there is NO console-side or runner-side simulator mock (the P5.14
   mitigation).
2. **Schedules → dispatch** — the REAL gate chain (P4.11 creation from the
   seeded schedule revision, P4.11/P4.12 validation, the §12.5 feasibility
   gate, the P4.13 REQUIRED authorization with no auto-approve) over
   in-memory SEAM-LEVEL stores, then the REAL `SimulatorDispatchAdapter`
   (P4.16) over the REAL `SimulatorTelemetryTransport` — a
   composition-root transport implemented over the simulator crate itself,
   with NO transport fake.
3. **Tracking** — observations served from the REAL pumped telemetry
   stream; GOOD/SUSPECT usable, BAD/MISSING/STALE skipped, never coerced
   (`TRACKING_QUALITY_RULE`).
4. **Dropout path** — drop windows come from the scenario's OWN fault plan
   (configuration, P2.15). A recorded dropout is visible in the run record
   (availability change + evidence kind `fault_plan_drop_window`) and
   triggers the REAL P3.24 `plan_rolling_horizon_roll` (prior-revision
   fixture supplied by the composition; realised state from REAL telemetry;
   the past is never re-solved as if it had not happened).
5. **Versions per run** (P5.14 step 3 / P5.19 input): scenario id + version,
   master seed, code version (`--code-version` → `FLEXCORE_CODE_VERSION` →
   `dev-unrecorded`), simulator crate version (`SIMULATOR_CRATE_VERSION`,
   compile-time), per-asset model identities, simulator source id
   `sim://<scenario_id>`.

### The adapter inventory (what the run record names — real vs seam)

| Leg | Adapter | Real/Seam | Detail |
|---|---|---|---|
| telemetry_ingest | `SimulatorTelemetrySource` via `TelemetrySource` port | REAL | the Phase 2 simulator; same provider the worker composes |
| dispatch | `SimulatorDispatchAdapter` (P4.16) over `SimulatorTelemetryTransport` | REAL | transport implemented over `flexcore-simulator`; no transport fake |
| command_gates | `CommandGates` (P4.11–P4.13) over in-memory stores | SEAM | store/outbox/envelope/model-version/asset-capability/authz seams are named in-memory composition support; the gate logic is the real application code |
| grid_feasibility | `run_feasibility_gate` (P4.04–P4.07) + `PandapowerGridProvider` over a selectable backend | REAL pipeline; backend per selection | `scripted` = SEAM (fixture outcome, NOT a computed power flow); `worker_process` = REAL leg (P5.15, UNVERIFIED-API) |
| schedule_roll | `plan_rolling_horizon_roll` (P3.24) | REAL | realised state from REAL telemetry; the re-solve itself is the optimizer port (out of this composition's scope) |

The `ScenarioRunRecord.adapters` array carries exactly this inventory per
run, plus the recorded statements (simulation-only boundary, solver class,
provider boundary, tracking-quality rule, seam inventory) — the P5.14
acceptance evidence "the run record proves which adapters were real vs
seam-level".

### Decisions

- **D-P5.14-1 (DEFAULT):** the composition is a sim-runner module + CLI
  subcommand, not a control-plane route. The control plane's Phase 4
  dispatch surface is read-only (P4.30, no generic execute endpoint §15.2),
  so the scenario-run path is the dispatch integration surface; the console
  (P5.08 Simulation Lab) consumes the run record via the read APIs later in
  the phase.
- **D-P5.14-2 (DEFAULT):** the stand-in identity derivation (asset keys →
  `AssetId`, tenant, portfolio/site, schedule/command fixture UUIDs) is the
  recorded P2.24 replay discipline — deterministic FNV-based stand-ins,
  NOT registry-issued IDs. The mapping key → registry ID exists only at
  registration (P2.25 step 2); a run is not a registration.
- **D-P5.14-3 (DEFAULT):** the gates run AT THE ISSUE INSTANT (the clock
  and the simulator pump advance together first) so the feasibility
  ramp-baseline seam reads telemetered states that actually exist and are
  fresh at the gate instant. Running the gates on an unpumped stream would
  have produced a guaranteed seam error — a composition bug, not an honest
  gate outcome.
- **D-P5.14-4 (DEFAULT):** the run record is emitted regardless of the
  verdicts the REAL gates reached (a rejection/dropout refusal is an honest
  outcome of the slice, exit 0); only composition/config errors exit 2.

### Honest limitations

- The gate stores are in-memory seams; the Postgres/broker-backed stores
  (Phase 1/4 adapters) are NOT wired here — the record names them Seam.
- The grid feasibility leg's fixture is the three-bus synthetic feeder; NO
  IEEE feeder was used or claimed (§19.3 discipline; P5.20 owns the
  per-case record).
- Nothing was built or run: the tests in
  `apps/sim-runner/tests/scenario_run_p5_14.rs` are pre-registered
  expectations, UNRUN.

---

## P5.15 — Grid simulator adapter integration (§12.4, §19.1)

### What was integrated

1. **The Python worker leg (NEW):** `apps/worker/python/grid_worker.py` +
   `requirements.txt` (`pandapower==3.5.6`, the D-P4.01-1 pin — NEVER
   INSTALLED) + `README.md`. The worker reads ONE
   `flexcore-grid-worker/1` JSON request on stdin (model IR + translated
   injections + options — the `WorkerRequest` shapes of
   `crates/flexcore-grid-api/src/worker_backend.rs`), builds the pandapower
   network from the IR, runs **`pp.runpp` — an AC power-flow evaluation of
   specified injections, NOT an AC-OPF** (solver-class discipline,
   AGENTS.md rule 4, stated in the worker's honesty header, its status
   text, and the requirements/README), and emits ONE JSON response (the
   `BackendPowerFlowOutcome` serde shape behind the protocol tag).
   Malformed request ⇒ nothing on stdout + non-zero exit (the Rust side
   fail-closes); an in-evaluation failure ⇒ in-band
   `converged:false + reported_error` — never a crash, never a retry
   (NO_RETRY_POLICY, D-P4.05-2).
2. **The Rust process backend:** `crates/flexcore-grid-api/src/worker_backend.rs`
   (`WorkerProcessBackend`) — the REAL `GridModelBackend` over
   `std::process::Command` + one JSON document each way, bounded timeout
   (helper thread + `recv_timeout`; hung worker killed ⇒ Unavailable).
   Fail-closed on EVERY failure: spawn failure, non-zero exit with no
   output, timeout, empty/unparseable output, wrong protocol tag, missing
   field, non-finite number — all map through
   `GridPortError::BackendUnavailable`/`BackendContractViolation` to the
   P4.08 ProviderError trigger. No path from a worker failure to a pass.
3. **Scenario-run wiring:** the sim-runner `scenario-run --grid-backend
   worker` selects the REAL worker-process leg for the feasibility gate
   (program/script/timeout are CLI options); `--grid-backend scripted`
   (the default) keeps the seam-level fixture. The run record names which
   backend ran (`grid_checks[].backend` = `scripted` | `worker_process`,
   with the provider id+version `pandapower-worker@3.5.6`) — a run can
   never silently substitute one for the other. The existing verdict and
   fail-closed logic of flexcore-grid-api was NOT modified (additive
   backend only).
4. **The P4.02 pipeline, not a reimplementation:** the scenario feasibility
   leg calls `run_feasibility_gate` steps 1–4 + the P4.06/P4.07 limit
   check through the existing `PandapowerGridProvider` (scope gate,
   translation, result mapping, completeness gate, pin cross-check). No
   power-flow or limit logic exists in the sim-runner (the P5.15 mitigation:
   "the scenario path calls the platform pipeline").

### The pandapower-worker UNVERIFIED-API honesty note

**The Python worker was WRITTEN, never executed.** pandapower 3.5.6 was
NEVER installed; no worker process ever ran against it; no power-flow
result anywhere in this repository was computed by the worker. Its
pandapower API usage follows the provider's DOCUMENTED API as of the
2026-11-07 research cut-off and is **UNVERIFIED** until the human
build/test pass executes it against the pinned version. Recorded mapping
assumptions (each stated in `grid_worker.py`'s header table):

- branch absolute ohms → unit-length line representation
  (`r_ohm_per_km = r_ohm`, `length_km = 1.0`);
- `rated_current_a` → `max_i_ka` (A → kA);
- `vk_r_percent` → pandapower `vkr_percent`;
- no-load transformer data absent from the IR ⇒ `pfe_kw=0`, `i0_percent=0`
  — a recorded model simplification, surfaced in the outcome warnings;
- injection-positive payload: p ≥ 0 → `sgen`, p < 0 → `load` with the
  reactive sign flipped so the convention is preserved.

The honesty note travels with the backend identity (`describe()` notes:
"WRITTEN, never executed … UNVERIFIED until a human build/test pass runs
it"), in the worker header, in `apps/worker/python/README.md`, and in the
grid-feasibility adapter-trace detail when the `worker_process` backend is
selected. The transport's own payload checks (protocol tag, JSON shape,
finiteness, row completeness at the adapter, pin cross-check) contain any
API mismatch fail-closed.

### Decisions

- **D-P5.15-1 (DEFAULT):** the transport is one JSON document each way over
  `std::process` with a bounded timeout — the simplest transport that adds
  NO new dependency to the CORE crate (no tokio/gRPC client). A gRPC/
  Protobuf transport remains the deployment-stage upgrade path recorded in
  `backend.rs`; the payload shapes are unchanged by it.
- **D-P5.15-2 (DEFAULT):** CVXPY is deliberately NOT a dependency of the
  grid worker: this leg is AC power flow only, no optimization. The CVXPY
  pin stays a Phase 3 matter (its 1.9.x solver-default matrix is an open
  verification task there); no version is pinned here from an unverified
  source.
- **D-P5.15-3 (DEFAULT):** the scripted fake-worker doubles in
  `crates/flexcore-grid-api/tests/worker_backend_p5_15.rs` prove the wire
  contract and the fail-closed paths WITHOUT executing the worker (and
  without any platform-dependent hanging process — the live timeout branch
  is implemented and reviewed, not executed in this loop; recorded in that
  file's header).
- **D-P5.15-4 (DEFAULT):** the worker reports its installed pandapower
  version; the ADAPTER cross-checks it against the recorded pin (existing
  P4.01 pin discipline, untouched). The worker itself never refuses a
  mismatch.

### Feeder-case record (P5.15 step 3 — the honest entry)

No IEEE 13/34/123-bus case was run or claimed. The scenario feasibility leg
evaluates the SYNTHETIC three-bus radial fixture
(`scenarios/synthetic-portfolio` + `tests_support::three_bus_model`
magnitudes). IEEE feeder licensing/model-availability confirmation is the
P5.20 per-case record's obligation; nothing here presupposes it.

### Honest limitations

- Nothing was built, run, or installed: the worker was never executed, the
  Rust backend never spawned it, and both test files are pre-registered
  expectations (UNRUN).
- The live timeout path and the real pandapower behaviour are unverified by
  construction in this loop; the fail-closed boundary means an unverified
  worker degrades to HOLD/fail-closed outcomes, never to a pass.
- Per-subtask ledger ticks are "implemented, awaiting human build/test
  (decision record: `14_15_adapters.md`)" — never "tested/verified/passed".

---

## Files touched (this subtask pair)

| File | Change |
|---|---|
| `apps/sim-runner/src/scenario_run.rs` | P5.14 composition (pre-existing from this wave): + `DispatchSliceOutcome`, per-slice grid-check summary naming the backend/provider, gates-at-issue-instant fix, monotonic `run_to_horizon_end`, public `deterministic_uuid` |
| `apps/sim-runner/src/main.rs` | additive `scenario-run` CLI subcommand wiring the harness (all dispatch parameters are CLI options — deployment configuration, D-P4.16-3) |
| `apps/sim-runner/src/lib.rs` | module registration (pre-existing from this wave) |
| `apps/sim-runner/Cargo.toml` | composition dependencies (pre-existing from this wave) |
| `apps/worker/python/grid_worker.py` | NEW — the P5.15 Python worker leg (UNVERIFIED-API) |
| `apps/worker/python/requirements.txt` | NEW — the recorded `pandapower==3.5.6` pin (never installed) |
| `apps/worker/python/README.md` | NEW — honest status + the human run procedure |
| `crates/flexcore-grid-api/src/worker_backend.rs` | the REAL process backend (pre-existing from this wave; untouched here) |
| `apps/sim-runner/tests/scenario_run_p5_14.rs` | NEW — P5.14 composition integration suite (UNRUN) |
| `crates/flexcore-grid-api/tests/worker_backend_p5_15.rs` | NEW — P5.15 wire-contract + fail-closed suite, scripted fake-worker payloads (UNRUN) |

Verification-tool discipline (AGENTS.md §6): `python tools/verify_docs.py`
and `python tools/validate_agent.py` were run at close and recorded in the
HANDOVER; `python -I tools/check_dependency_rule.py` was run for the new
app→crate edges.
