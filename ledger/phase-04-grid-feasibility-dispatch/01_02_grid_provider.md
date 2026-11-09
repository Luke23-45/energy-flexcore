# P4.01 + P4.02 — Decision note: the primary grid-model provider and the grid-model port / adapter contract

- **Date:** 2026-11-07.
- **Subtasks:** P4.01 (ledger_phase_4.md §P4.01) and P4.02 (§P4.02). Report trace: §12.4, §25.2, §23 Stage 4, §9.1, §5.2, §8.1, §7.2.
- **Deliverable crate:** `crates/flexcore-grid-api` (core crate; NEW).
- **D4 note:** pandapower is BSD-3-Clause (project repository; RESEARCH.md §1). Recorded here as D4-audit input; the formal D4 license/dependency audit is OPEN and owned by legal + engineering — this note is input to it, not the audit.

## 1. D-P4.01-1 — Provider selection (a RECOMMENDATION, never a D3 decision)

**Decision.** The primary grid-model provider is **pandapower**, per the
recommended default (report §25.2: "pandapower is the primary power-flow
provider"; `.agent/DECISIONS.md` D3 row: "0.x balanced-radial default is a
RECOMMENDATION, not a decision"). This is recorded as a **RECOMMENDATION
pending confirmation by the D3 gate** — it is not a D3 decision.

**D3 remains OPEN, verbatim** (report §25.2):

> Network class and model fidelity | Balanced radial distribution, unbalanced
> three-phase, and meshed transmission require different models and solvers |
> Phase 0; power systems

Recommended defaults pending confirmation (report §25.2, verbatim): "v0.x
targets balanced radial distribution feeders; pandapower is the primary
power-flow provider; unbalanced three-phase modelling enters the track from
v0.4.0; meshed transmission operation is out of scope." The Phase 4
implementation encodes these defaults as **declared, machine-checked scope**,
not as silent assumptions — see §4.

**Why by case fit, not familiarity.** The recorded risk (P4.01: "choosing a
provider by familiarity rather than by case fit") is mitigated structurally:
the primary path covers exactly the v0.3.0 workload named by the release
definition (§2.1, §21.4) — balanced AC power-flow evaluation of synthetic
feeder candidates — and the addition rule in §3 forces any second provider to
name a case the primary cannot handle.

## 2. Version pin

| Item | Value | Source |
|---|---|---|
| Primary provider | pandapower **3.5.6**, released 7 October 2026 | `ledger/phase-04-grid-feasibility-dispatch/RESEARCH.md` §1 (PyPI release files; changelog; repository — read 2026-11-07) |
| Class | Balanced AC power flow, SOME three-phase/asymmetric calculations. **Not a universal validator. Its OPF capability is NOT the feasibility gate.** | report §12.4 [R20]; RESEARCH.md §1/§7 |
| License | **BSD-3-Clause** | RESEARCH.md §1 (project repository) |
| Upcoming | 4.0.0 listed with no release date — **do not plan against it** (RESEARCH.md §6 task 4; re-check at worker-adapter build time) | RESEARCH.md §1 |

**Pin mechanics (P4.01 step 4 — the exact steps, since this subtask cannot
run cargo or edit manifests):**

1. **Python worker requirements pin** — in the numerical worker's
   requirements/lockfile (`apps/worker`, the P3.19 pattern; outside this
   subtask's file ownership), pin exactly `pandapower==3.5.6`. The
   coordinator handover note in `crates/flexcore-grid-api/Cargo.toml`
   records this step.
2. **Workspace manifest** — the coordinator adds `crates/flexcore-grid-api`
   to `[workspace] members` and `[workspace.dependencies]`, and registers
   the crate in `tools/check_dependency_rule.py` (MEMBER_DIRS, CORE_CRATES,
   `ALLOWED_INTERNAL_EDGES = {"flexcore-contracts"}`); exact lines in the
   crate manifest header.
3. **Pin on every result** — the port's result type
   (`flexcore_grid_api::port::PowerFlowResult`) carries `provider_version`
   as a **required field** (`validate_complete` rejects an empty value),
   and the provider stamps the recorded pin on every result. The worker's
   reported installed version is cross-checked against the pin at mapping
   time; a mismatch is a backend contract violation, never a warning
   (tested).

## 3. D-P4.01-3 — Provider-addition rule

A second provider (**OpenDSS, GridLAB-D, PyPSA**) is added **only** when a
**named test case** cannot be expressed or validated with pandapower. The
case is recorded in this ledger directory (a new dated decision note) with:
the case, the failing pandapower expression/verification, and the candidate
provider's verified version + license. **No familiarity-based additions.**

Pre-recorded context (from RESEARCH.md, not a decision): OpenDSS is the
likely trigger for unbalanced feeder studies (the IEEE 13-bus unbalanced case,
§19.3, may be the named case — decided at P4.01 only if the case actually
arises; it has NOT arisen in v0.3.0). PyPSA is never the distribution
feasibility validator (RESEARCH.md §4: "do not route a feeder feasibility
check through PyPSA"). GridLAB-D is a candidate only for unbalanced/
time-series cases.

## 4. D-P4.01-2 — Provider boundary (§12.4)

The provider is the **balanced AC power-flow path with SOME
three-phase/asymmetric calculations** — it is **not a universal validator**
(report §12.4: "verify model and solver limitations for each case").

The boundary is recorded as a **mandatory, machine-checked scope declaration**
(`flexcore_grid_api::scope::ScopeDeclaration`; adapter's declaration:
`flexcore_grid_api::adapter::balanced_worker_scope()`):

- **Phase:** balanced (positive-sequence) only. Unbalanced three-phase is
  *declared not-modelled*; it enters from v0.4.0 per the recommended default
  (D3 OPEN).
- **Topology:** radial and meshed (the balanced solver covers both; the
  v0.x workload is radial).
- **Modelled equipment:** lines, two-winding transformers, load elements,
  static generators, external grid.
- **Declared NOT modelled (the limitation list):** unbalanced three-phase
  asymmetry; protection coordination; dynamic/transient behaviour; sub-cycle
  control; optimal power flow.
- The scope's `not_modelled` list is **mandatory and non-empty** (a scope
  claiming to model everything is a construction error); the list surfaces
  verbatim as **model-coverage limitations on every result** (§12.5 step 6).
- pandapower's OPF capability is not the feasibility gate and is not wired
  in as one without a new decision (§12.5; RESEARCH.md §7).
- Solver-class discipline: the gate runs an **AC power-flow evaluation** of
  specified injections/controls. It is **not AC-OPF**, and no other solver
  class is conflated with it (AGENTS.md rule; ledger CONTEXT.md §5).

## 5. D-P4.02-1 — Provider path: transport-neutral `GridModelBackend` seam (the D-P3.17-1 pattern)

**Decision.** The AC power-flow evaluation executes in the **Python
numerical worker** (§7.2: Python is the power-system simulation/numerical
worker; it is never the command authority) via the pinned provider version,
behind an internal, **gRPC-free, transport-neutral Rust trait**
`flexcore_grid_api::backend::GridModelBackend`. The worker adapter that
implements the trait (in-process bridge for tests, §15.1 Protobuf/gRPC for
deployment once the Phase 7 protobuf contracts land) is a **later-wave
deliverable** — the P3.19 precedent. No transport dependency exists in the
crate (enforced by the manifest-scan test).

The Rust side owns: scope gating, the canonical-units translation layer
(owned, tested code in both directions — §10.2 rules, hand-computed
round-trip fixtures), result construction and provenance, the pin
cross-check, and the completeness gate. The worker owns the physics and
reports its installed provider version.

**VERIFY-BUILD items (recorded in `crates/flexcore-grid-api/src/backend.rs`,
all confirmed at worker-adapter build time against the pinned version):**
result-table fields (per-bus `vm_pu`, per-branch kA currents and percent
loading, per-transformer loading and side currents); convergence-flag
location/semantics including failed-run table behaviour; iteration-count
source; residual availability (mismatch residuals are `NotAvailable` with a
recorded reason until a documented source is verified); worker-side element
sign conventions (generation-type positive = injection) and their
materialisation of the translated injection; confirmation that the declared
modelled set matches what the worker actually exercises.

## 6. Port contract summary (P4.02 deliverable)

| Element | Where | Notes |
|---|---|---|
| `GridModelProvider` port (§9.1) | `src/port.rs` | `capabilities()` (identity + pinned version + mandatory scope), `run_power_flow(model, injections)`, `evaluate_dispatch(...)` defaulting to run + port-level limit evaluation. Object-safe, sync — no async runtime on the port |
| Versioned network model IR | `src/port.rs` | `VersionedNetworkModel` + `GridModelRef` (id/version/content hash carried; versioning/hash/health is P4.03's deliverable). Structural validation: unique ids, existing bus refs, positive parameters, exactly one slack |
| Candidate injections | `src/port.rs` | Canonical contract types (`ActivePowerMW`, `ReactivePowerMVAr`) + the DECLARED `ReactiveSignConvention` (no default) |
| Power-flow result | `src/port.rs` | Bus voltages in BOTH representations (kV at nominal base + per-unit), branch currents (A)/loadings (fraction of declared rating), transformer loadings, closed convergence vocabulary (`converged / did_not_converge / provider_reported_error`), residuals (available or not-available WITH reason), mandatory coverage limitations, applied-injection records, runtime; `validate_complete` (pin required) |
| Limit verdict (§12.5 steps 5, 7) | `src/port.rs` | `evaluate_limits` is a pure function of result + limits; non-convergence/errored/incomplete → `Inconclusive` (fail-closed, never a pass, §17.5 invariant 9). The §12.5 step 8 reject/constrain/return policy is P4.07, not here |
| Scope declaration | `src/scope.rs` | Mandatory; undeclared-scope adapter = construction error; scope gate refuses out-of-scope requests; non-empty `not_modelled`; AC-PF-not-AC-OPF solver statement enforced |
| Backend seam | `src/backend.rs` | `GridModelBackend` + worker-contract payload vocabulary + VERIFY-BUILD items |
| Translation layer | `src/translation.rs` | Canonical ↔ worker model, both directions, round-trip tested with hand-computed fixtures |
| Provider | `src/adapter.rs` | Rust-side provider logic (scope gate → translation → backend → mapping → completeness gate); names the provider; carries the pin, boundary, D3-OPEN, and addition-rule statements |

**Contract tests** (`tests/`): the same candidate through the port produces
the same verdict across two fully independent adapters (the pandapower-path
provider over a scripted backend double, and an in-crate stub second
provider with a different declared scope — the port is real, tested);
undeclared-scope construction errors; scope-gate refusals; pin recording and
cross-check; fail-closed non-convergence; applied-injection records;
translation round trips both directions; dependency-direction (port-surface
source scan + exact manifest set + port-only consumer).

## 7. D3 status, restated

D3 stays **OPEN**. Nothing in this note or crate decides it. The balanced
default and the provider choice are recommendations encoded as declared
scope, and the scope declaration is the mechanism that keeps D3 open: a
future unbalanced adapter is a NEW scope declaration plus a recorded
decision, not a silent default change.
