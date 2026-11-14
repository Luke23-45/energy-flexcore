<!-- P9.03 — Pilot network model build and validation (decision record). Model-build PLAN only: no model built, no validation run, every run cell EMPTY. D3 carried OPEN (RECOMMENDATION only). -->
# P9.03 — Pilot Network Model Build and Validation (Model-Build Plan)

Date: 2026-11-14. Subtask: P9.03 (`ledger_phase_9.md` §P9.03).
Owner role: Power systems engineering. Report trace: §23 Stage 9; §12.4;
§12.5; §25.2 (D3 — network class and model fidelity).
Inputs: P9.02 reconciled registry (template — no clean pass exists,
`02_site_asset_reconciliation.md`); customer/DSO network data (feeder
topology, conductor/transformer parameters, operating limits) — **does not
exist**; D3 position — **OPEN** (§2).

## 1. Honest status — the plain statement

**No grid model has been built and none is claimed.** No pilot feeder
exists to model, no DSO data has been received, and no validation power
flow has run. This record is the model-build PLAN: data sources, the D3
position carried open, the build mapped onto the platform's existing grid
seams (by citation), the §12.4/§12.5 validation procedure, and the
validation/coverage tables whose result cells are EMPTY. Everything
build/validation-shaped in this loop is a plan; the build and the
validation runs are HUMAN gates.

## 2. D3 status — carried OPEN (RECOMMENDATION only)

D3 (network class and model fidelity) is agent-undecided (ENTRY_NOTE
boundary 2). Recorded position:

- **RECOMMENDATION (not a decision):** balanced radial distribution
  feeders as the default network class for 0.x (§25.2), which matches the
  platform's current provider (§4.2: pandapower, balanced power flow).
- **Condition that would change it (recorded per ledger step 1):** any
  pilot site with significant phase imbalance, single- or two-phase
  laterals carrying the pilot assets, or a DSO requirement for unbalanced
  analysis → unbalanced three-phase class with a corresponding provider
  (§4.3). The interim class stands only until the real site data arrives;
  the change condition is evaluated against the P9.02-reconciled site
  records, not assumed.
- The pilot build MUST record the actual class per §5 step 1 of the
  ledger; this plan pre-records neither the class nor the data that would
  justify it, because neither exists.

## 3. Data sources (all NOT YET RECEIVED)

| Source | Supplies | Status |
|---|---|---|
| Customer/DSO feeder topology | buses, branches, transformers, switches, connectivity | NOT RECEIVED — `<EMPTY>` |
| Conductor / transformer parameters | r/x (Ω), ratings (MVA), tap/voltage base (kV) | NOT RECEIVED — `<EMPTY>` |
| Operating limits | voltage limits (per-unit and kV at nominal base), thermal limits (MVA) | NOT RECEIVED — `<EMPTY>` |
| P9.02 registry extract | asset bus references, phases, rated power (MW/MVAr)/energy (MWh), PCC | TEMPLATE ONLY — no clean pass |
| Recorded historical operating points | measured PCC power (MW/MVAr), voltages where available | NOT RECEIVED — `<EMPTY>` |

A build started before all five rows are filled from primary sources is
invalid (the "estimated impedances presented as measured" failure mode).

## 4. Build plan — mapped to the platform's existing seams (by citation)

### 4.1 Model representation seam (§10.1/§10.2)

The model is built in the platform's existing grid-model IR, not in a new
format: `crates/flexcore-grid-api/src/port.rs` — `VersionedNetworkModel`
with `BusSpec`, `BranchSpec`, `TransformerSpec`, `ExternalGridSpec`,
`GridModelRef` (model version/reference ID), and base values declared
explicitly (voltage kV at the nominal base, per-unit in solver
representation; power MW/MVAr; `P > 0` = export at the point of
connection). Coverage gaps are first-class: `CoverageLimitation` exists in
the same port — §7 uses it.

### 4.2 Provider for the recommended class (balanced power flow)

The existing provider seam: `crates/flexcore-grid-api/src/{backend.rs,
worker_backend.rs, adapter.rs}` dispatching to the numerical worker
`apps/worker/python/grid_worker.py` — pandapower 3.5.6 (the recorded pin),
power flow only. Standing caveats carried verbatim into this plan: the
worker is **UNVERIFIED-API and NEVER RUN** (never installed; every
pandapower call follows the documented-API form); it is a steady-state
power-flow evaluation of specified injections and controls — **not an
AC-OPF**, not a dynamic/stability tool (solver-class discipline; §19.1:
steady-state power flow is not transient-stability evidence).

### 4.3 Provider for the unbalanced class (if the change condition fires)

Where unbalanced three-phase is required, §12.4 names OpenDSS or GridLAB-D
as the provider class. **No such provider exists in the repository**; this
plan only records that a class change requires selecting, pinning, and
conformance-checking one of them — a separate, later build task, never a
silent substitution of the balanced provider.

### 4.4 Feasibility-gate seam the model must serve (§12.5)

The built model is consumed by the existing feasibility gate:
`crates/flexcore-grid-api/src/feasibility.rs` and its contract tests
`crates/flexcore-grid-api/tests/feasibility_gate.rs` — `NetworkLimits`,
`FeasibilityVerdict`, `PowerFlowStatus`, `PowerFlowResiduals`,
`AppliedInjectionRecord`; with the application-side seams
`crates/flexcore-application/src/feasibility_return.rs` and
`command_gates.rs` (validation, feasibility, and authorization remain
distinct gates, §14.1). A model that cannot populate these types with
declared base values and limits is not buildable for the pilot.

## 5. Validation procedure (§12.4/§12.5) — human-executed

1. Record the network class chosen for the pilot and the D3 basis for it
   (§2); if D3 is still open at build time, record the interim class and
   restate the change condition.
2. Build the model in the §4.1 IR with a model version, source-data
   references (§3 rows), and base values declared explicitly.
3. Select the provider per the declared class (§4.2/§4.3) and record why
   it fits the class.
4. Set the validation thresholds **in advance and in writing, before any
   run** (residual bounds on PCC P/Q in MW/MVAr and voltage magnitude in
   per-unit). Fitting thresholds after seeing results invalidates the run.
5. Run power flow against at least one recorded historical operating point
   per site; record residuals in §6 via the `PowerFlowResiduals` shape.
6. Where reverse-power-flow conditions are plausible at the site, validate
   at least one such point explicitly — the §12.5 SOCP-relaxation caution
   applies at reverse power flow, and a model validated only at one benign
   operating point must not be trusted at the extremes (ledger risk row).
7. Write §7 coverage limitations from the actual build — never "none".
8. Freeze the model version used for pilot acceptance; any later change is
   a new version requiring full re-validation (§8).

## 6. Validation-run table — ALL CELLS EMPTY

| Site ID | Model version (GridModelRef) | Provider (version) | Class (balanced/unbalanced) | Operating point (date/source) | PCC P residual (MW) | PCC Q residual (MVAr) | V residuals (p.u.) | Pre-set threshold (P/Q/V) | Pass/fail |
|---|---|---|---|---|---|---|---|---|---|
| `<SITE-ID-1>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` (set before run) | `<EMPTY>` |

Independent verification (ledger): a second engineer re-runs the
validation power flow from the frozen model and the recorded inputs and
compares residuals — a run result nobody can reproduce is not evidence.

## 7. Coverage-limitations table — VALUE CELLS EMPTY (structure mandated)

| Limitation class | Present in this model? | Detail (what is not represented) | Consequence declared for the pilot |
|---|---|---|---|
| Unmodelled branches | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Estimated (not measured) impedances | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Missing phase data | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Other (unmodelled protection, switching states, …) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

A "none" in every row is a claim requiring evidence; the default until a
real build exists is that limitations are UNKNOWN, not absent.

## 8. Model freeze rule

The frozen model version ID is referenced by the pilot's feasibility
configuration and by P9.14/P9.15/P9.19. Any change to topology,
parameters, limits, or base values produces a new `GridModelRef` version
and voids prior validation — affected subtasks re-run. No version is
frozen by this record; the freeze happens at the human build, after §6
passes its pre-set thresholds.

## 9. What this plan does NOT claim

- No model exists; no model version is frozen; no residual has ever been
  computed against real site data.
- The pandapower pin (3.5.6) is RECORDED, not installed or run; its API
  mapping remains UNVERIFIED (`grid_worker.py` header caveat).
- Power-flow validation is steady-state evidence only. It is not
  transient-stability, protection, or certification evidence, and no
  solver class is conflated (power flow ≠ AC-OPF; pandapower balanced
  power flow ≠ unbalanced analysis; HiGHS LP/QP/MIP and Clarabel conic
  are scheduling solvers, not grid-model providers).
- D3 is not closed here or anywhere else by agents.

## 10. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| D3 OPEN — balanced-radial is a RECOMMENDATION; a wrong class invalidates the built model | §2 |
| Estimated impedances presented as measured | §3 rule; no DSO data received yet to prevent it |
| Unbalanced network modelled as balanced | §2 change condition; §4.3 provider not implemented |
| Single-benign-point validation trusted at reverse-power-flow extremes | §5 step 6; §12.5 SOCP-relaxation caution |
| Grid worker NEVER RUN (UNVERIFIED-API) — the first real build is also the first execution of the provider path | §4.2; Phase 5 standing caveat |
| No DSO data received; no site exists to request it from | §3 |

## 11. Reviewer verification procedure

1. Confirm §6/§7 cells are `<EMPTY>` and no model version, residual, or
   threshold value appears anywhere in this record.
2. Trace §4 citations to the named files (`port.rs`, `feasibility.rs`,
   `feasibility_gate.rs`, `grid_worker.py`, `feasibility_return.rs`,
   `command_gates.rs`); confirm no new provider or IR was invented.
3. Confirm §2 states a RECOMMENDATION with an explicit change condition,
   not a D3 closure.
4. Confirm §5 step 4 requires thresholds set before the run.
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the plan is written; it must never be reported as
   "built", "validated", or "passed".

## 12. Filling commands (for the human engineer)

- Build: construct the `VersionedNetworkModel` from the §3 sources in the
  `crates/flexcore-grid-api` IR; assign the model version ID
  (`GridModelRef`); the build/execute commands belong to the platform's
  own build runbook — this loop built and ran nothing.
- Validate: execute §5; fill §6 and §7; freeze per §8.
- Ledger tick (after the frozen model and validation report are filed):
  `[x]` on P9.03 is justified by the ledger acceptance criteria; today's
  `[x]` means **implemented, awaiting human build/test**.

Status: **WRITTEN-UNRUN.** No pilot exists yet and none is claimed; no
model has been built, no provider has been executed, and no validation run
has occurred or is scheduled.
