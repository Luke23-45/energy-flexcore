# P8.17 — Dynamic evaluation: research controllers under disturbances (TEMPLATE — every measured cell EMPTY)

Date: 2026-11-13. Subtask: P8.17 (`ledger_phase_8.md` §P8.17).
Owner role: Research engineer (power systems). Report trace: §19.1;
§19.3–§19.4; §13.3. Inputs: P8.16 integration
(`16_andes_integration.md`); the P8.17 pure evaluation harness
(`apps/worker/python/andes_worker/evaluation_harness.py`); P8.09/P8.15
results (other waves — NOT consumed here: their evaluation cells are
EMPTY by the same ENTRY_NOTE boundary).

## 1. Status of this record — a TEMPLATE, nothing measured, nothing fabricated

**Every measured cell in this document is EMPTY.** Per the phase
boundaries (ENTRY_NOTE boundary 4: "Every measurement is an EMPTY cell")
the ANDES runs are **human/lab gate inputs**: no figure, seed result,
benchmark number, or "plausible" value appears anywhere below, and none
was computed by any code in this loop (nothing was installed, built, or
run). What exists and is verified to the extent the loop allows:

- the **pure metric definitions** (nadir, zenith, max ROCOF, damping
  ratio by log decrement, settling time) in
  `apps/worker/python/andes_worker/evaluation_harness.py`, each pinned by
  HAND-COMPUTED self-tests (analytic decaying sinusoid with δ = 1.0 →
  ζ = 1/√(4π²+1) ≈ 0.157177; exact-slope ROCOF ramps; band-inclusive
  settling cases) — the harness was **written, never executed**;
- the **worker** that will produce the traces
  (`andes_dynamic_worker.py` — UNVERIFIED-API, ANDES never installed);
- the **filling procedure** (§5) — exact commands the human/lab pass
  runs, with the cells they fill.

The tick on P8.17 means *implemented, awaiting human build/test*. Until a
human run fills the cells, **no dynamic-evaluation claim of any kind
exists**.

## 2. Disturbance set definition (ledger step 1) — declared BEFORE any run

Each disturbance names its case file, parameters, and simulation settings
as TEMPLATE fields the human pass fills/points at real files; the case
file paths are `RECORD-AT-FIRST-RUN` placeholders, never invented.

| # | Disturbance kind | Case file | Disturbance parameters (recorded at run) | Simulation settings (solver, step, end time) | Model scope |
|---|---|---|---|---|---|
| D-1 | Fault (declared model's fault object, cleared) | `RECORD-AT-FIRST-RUN` (P8.16 §3 candidates: Kundur two-area / IEEE 14-bus w/ dynamic data — ASSUMPTION to verify) | fault type, bus, from_s, to_s: `RECORD-AT-RUN` | solver, step_seconds, end_time_seconds: `RECORD-AT-RUN` | ANDES models list: `RECORD-AT-RUN` |
| D-2 | Line trip | `RECORD-AT-FIRST-RUN` | line id, trip at_s: `RECORD-AT-RUN` | `RECORD-AT-RUN` | `RECORD-AT-RUN` |
| D-3 | Generator outage | `RECORD-AT-FIRST-RUN` | machine id, trip at_s: `RECORD-AT-RUN` | `RECORD-AT-RUN` | `RECORD-AT-RUN` |
| D-4 | Load step | `RECORD-AT-FIRST-RUN` | load id, ΔP (MW), ΔQ (MVAr), at_s: `RECORD-AT-RUN` | `RECORD-AT-RUN` | `RECORD-AT-RUN` |

Selection discipline (the ledger's named risk — "disturbances chosen
because the research controller handles them well"): this set is declared
BEFORE any run and is frozen when the first run happens; adding a
disturbance after seeing results requires a dated amendment to this
table with the reason.

## 3. Per-disturbance result table — ALL MEASURED CELLS EMPTY

One row per disturbance × controller proposal; every measured cell is to
be filled ONLY by executing §5's commands on real ANDES output. Each
filled row must cite the model/case reference (case file + ANDES version
+ model list) in the Model/case ref column — a number without its model
scope is invalid and must not be entered (ledger acceptance: "every
number carries its model/case reference").

| Disturbance | Controller proposal (provenance ref) | Baseline controller (deterministic, ref) | Freq nadir (Hz) | Max ROCOF (Hz/s) | Settling time (s) | Voltage extremes (pu) | Damping ratio (from decaying window) | Instability observed? (yes/no/model-scoped) | Model/case ref |
|---|---|---|---|---|---|---|---|---|---|
| D-1 | _(EMPTY — run pending)_ | _(EMPTY)_ | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-2 | _(EMPTY — run pending)_ | _(EMPTY)_ | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-3 | _(EMPTY — run pending)_ | _(EMPTY)_ | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-4 | _(EMPTY — run pending)_ | _(EMPTY)_ | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | `RECORD-AT-RUN` |

Rules for whoever fills this table (recorded now, before any number):

- Cells are filled from the worker's result envelopes (§5), never from
  memory, never estimated. An in-band study failure
  (`termination_status: "failed"`) is recorded AS A FAILURE in the cell
  (e.g. `FAILED: <diagnostic>`), never as a blank and never as a number.
- A trace without a computable metric (e.g. no decaying-oscillation
  window → the harness refuses a damping ratio) is recorded as
  `N/A (reason)` — the harness's fail-closed refusals are honest answers,
  not gaps to paper over. A GROWING oscillation is recorded as an
  instability observation, never as a damping ratio.
- The comparison column (baseline controller) uses the SAME disturbance,
  case, and settings; a comparison against a different run is invalid.
- The units are the canonical ones: frequency Hz, ROCOF Hz/s, voltage pu
  (solver representation), time s, power MW/MVAr; P > 0 = export
  (§10.2).

## 4. Model-limits note (ledger step 4 — written BEFORE the runs, as required)

- The dynamic model is a **declared model, not the real grid**: its
  results are evidence **about the model**, stated as such (§19.1). No
  result in §3 may be phrased as a statement about grid stability.
- A finite set of disturbances with favourable outcomes is **not** a
  safety or robustness guarantee (§13.3); the model limits travel with
  every number via the Model/case ref column.
- Steady-state power-flow results are **never** quoted as transient-
  stability evidence, and dynamic results are **never** quoted as
  quasi-static feasibility evidence (P8.16 §5; the boundary statements
  ride in every worker result envelope).
- The proposals evaluated here are research outputs (ADR-010): they were
  applied to the simulated system in shadow, never to devices; the
  worker that produced them has no field-write path
  (`16_andes_integration.md` §4).
- Metric definitions and their hand-computed verification are pinned in
  `evaluation_harness.py` (`run_self_tests()`); a metric definition must
  not change after results exist — a change is a new harness version and
  re-fills the affected cells.

## 5. Exact filling commands (the human/lab gate inputs)

Recorded verbatim; all from the repository root, all requiring the
isolated research environment from P8.16's procedure (ANDES installed per
`requirements-andes.txt` AFTER gates G-4 and the P8.36 advisory check):

```sh
# 0. Metric definitions self-check (no ANDES needed):
python apps/worker/python/andes_worker/evaluation_harness.py
#    expected: every hand-computed self-test PASSes (nadir 49.2 Hz /
#    depth 0.8 Hz; ROCOF 0.5 and 2.0 Hz/s cases; damping zeta = 1/sqrt(4*pi^2+1)
#    = 0.157177 (6 dp); settling 4.0 s / 4.75 s / 0.0 s cases).

# 1. Pure-logic test suites (no ANDES needed):
PYTHONPATH=apps/worker/python/andes_worker python -m pytest \
  apps/worker/python/andes_worker/tests -v

# 2. One study per disturbance x controller (the ANDES run itself):
#    build the request document per the worker README's transport contract
#    (case, models, simulation, monitored, event, evaluation, provenance),
#    then:
PYTHONPATH=apps/worker/python/andes_worker python \
  apps/worker/python/andes_worker/andes_dynamic_worker.py < request-D1.json \
  > result-D1-controller.json
#    record: the result envelope's model_scope, outputs.traces, metrics,
#    provenance.termination_status + diagnostics — INTO the §3 cells.
#    Repeat per disturbance (D-2..D-4) and per controller proposal, and
#    once per disturbance for the deterministic baseline controller.

# 3. Fill the §3 table from the result envelopes. Paste the model/case
#    reference (case_path + case_version + andes_version_reported +
#    declared_models) into the last column of every filled row.
```

Cells the loop did NOT and COULD NOT touch: all of §2's
`RECORD-AT-RUN` fields, all of §3, and the case-file existence itself
(the P8.16 case candidates are recorded ASSUMPTIONS until the first
install verifies them).

## 6. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| All disturbances run and reported | **UNRUN** — template with EMPTY cells (§2/§3); runs are the human/lab gate |
| Every number carries its model/case reference | Rule recorded (§3); no number exists yet to carry one |
| Baseline comparison on the same disturbances | Column defined (§3); EMPTY |
| Model limits stated | §4, written before any run |

## 7. Contract gaps → Phase 7 follow-ups (recorded, not invented)

- Same seams as `16_andes_integration.md` §7 (gRPC payload binding,
  research payload types, unit vocabulary); no new gaps found in the
  harness itself — its contract is its own recorded metric definitions.
