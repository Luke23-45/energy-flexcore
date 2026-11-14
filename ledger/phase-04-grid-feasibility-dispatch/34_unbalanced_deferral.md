# P4.34 — Unbalanced three-phase power-flow deferral and trigger record

- **Date:** 2026-11-10 (record authored; nothing built, nothing run — loop rules).
- **Subtask:** P4.34 (ledger_phase_4.md §P4.34). Report trace: §12.5 step 4,
  §12.4, §25.2 (D3), §19.3.
- **Deliverable/output:** this deferral/trigger note. The ledger's other
  deliverable — "model-class statement in feasibility results" — ALREADY
  EXISTS in code from P4.05 (cited in §3 below); nothing new needed to be
  implemented for it.

## 0. Status of the decision gate this record does NOT touch

**D3 (network class / model fidelity) remains OPEN.** This file is a
**scoped Phase-4 deferral record**, not a D3 resolution and not a D3
recommendation of its own. The balanced default it records is the
*recommended default of report §25.2*, restated here for Phase 4's scope
only; deciding D3 stays with the Phase 0 gate, owner power systems (per
`.agent/DECISIONS.md` and decision note
`01_02_grid_provider.md` D-P4.01-1). Nothing in this record closes,
narrows, or "governs" D3 itself — only Phase 4's use of the open default.

## 1. The decision recorded (scoped to Phase 4 / v0.3.0)

**Phase 4 (release v0.3.0, simulation and shadow) uses the BALANCED
power-flow model only. No unbalanced three-phase run is performed in this
phase.** Concretely:

1. The only Phase 4 grid-model adapter — the pandapower worker path
   (`crates/flexcore-grid-api/src/adapter.rs`, provider id
   `pandapower-worker`, pinned pandapower **3.5.6**) — declares
   `PhaseScope::Balanced` in its mandatory scope declaration
   (`adapter::balanced_worker_scope` → `scope::ScopeDeclaration::new`,
   decision note D-P4.01-1/D-P4.02-1).
2. No adapter declaring `PhaseScope::Unbalanced` exists in this phase:
   the `PhaseScope::Unbalanced` variant carries the recorded statement
   "NOT declared by any v0.3.0 adapter: unbalanced three-phase modelling
   enters the track from v0.4.0 per the recommended default (§25.2;
   D3 OPEN)" (`scope.rs`).
3. The declared topology scope of the adapter is
   `TopologyScope::RadialAndMeshed` — that is the ADAPTER's declared
   capability. The Phase 4 synthetic feeder models are radial, which is
   the D3 recommended default's topology class; a meshed Phase 4 case
   would be a named case under the same addition rule as below, not a
   silent scope use. The deferral DECIDED here is about the PHASE class
   (balanced vs unbalanced), which is what §12.5 step 4 names.
4. §12.5 step 4's alternative — "or three-phase unbalanced" — is thereby
   a **governed choice**: the unbalanced branch of the pipeline is
   deferred with the triggers of §4 below, not left as an unexamined
   default.

## 2. Why the deferral is legitimate (the report/decision chain)

| Source | What it records |
|---|---|
| §12.5 step 4 | "run the appropriate AC power-flow (or three-phase unbalanced) model" — the unbalanced path is ALTERNATIVE, taken when the network demands it |
| §25.2 / D3 | balanced radial is the RECOMMENDED default; D3 OPEN |
| §2.1 (v0.3.0 row) | "validation, feasibility, command lifecycle, approvals, audit" — no unbalanced modelling named for this release |
| CONTEXT.md §8 (D3 row) | "unbalanced three-phase enters from v0.4.0" |
| D-P4.01-3 (addition rule) | a second provider is added ONLY for a named test case the primary provider cannot express or validate |
| P4.01 acceptance criteria | "D3 still recorded as OPEN (not silently decided)" — this record inherits that obligation |

## 3. The boundary recorded in every feasibility result (already implemented, cited)

The ledger's deliverable "model class statement in feasibility results"
does not need new code — P4.05 already wired it; this section cites what
exists (verified by reading the named sources, not by running anything):

1. **On every attempt record:** `FeasibilityAttemptRecord::
   unbalanced_deferral_note` (`crates/flexcore-grid-api/src/feasibility.rs`)
   is a REQUIRED field carrying `UNBALANCED_DEFERRAL_NOTE` — "The declared
   model scope does NOT model unbalanced three-phase asymmetry (…D3 OPEN…)
   … the unbalanced three-phase track is deferred to P4.34."
   `validate_complete()` REJECTS an attempt record whose note is empty —
   an attempt record cannot exist without the model-class statement.
2. **On every power-flow result:** the scope's `not_modelled` entry
   `unbalanced_three_phase` surfaces as a mandatory non-empty
   `coverage_limitations` list on every `PowerFlowResult`
   (`crates/flexcore-grid-api/src/port.rs`,
   `ResultCompletenessError::MissingCoverageLimitations` — a result
   without coverage limitations is incomplete and fail-closed).
3. **On every limit report:** the same scope limitation travels as a
   `ScopeLimitGap` (`crates/flexcore-grid-api/src/limits.rs::scope_gaps`)
   — "limits in this class cannot be verified by this evaluation and a
   violation here cannot be seen — never mistaken for a pass".
4. **On every verdict:** the coverage limitations and scope limit gaps are
   carried on `VerdictEvidence` (`crates/flexcore-grid-api/src/verdict.rs`)
   — a within-limits verdict over a network the scope cannot see into is
   explicitly QUALIFIED in the verdict reason.

Net effect (the ledger's risk-mitigation line): an unbalanced-network
verdict is never implied — every result states the balanced model class
and states what the model cannot see.

## 4. The trigger enumeration — what forces an unbalanced three-phase model

Each trigger names: the observable, whether it is checkable from data the
provider seam ALREADY carries, and who owns the re-open. The enumeration
is the governed contract: an unbalanced case that matches NO trigger is
**rejected, not approximated** (the scope gate refuses; see T0).

### T0 — an unbalanced phase-class REQUEST (executable today)

- **Observable:** the requested phase class is `PhaseScope::Unbalanced`.
- **Checkable today:** YES — `ScopeDeclaration::assert_supports`
  (`scope.rs`) refuses any request whose phase exceeds the declared
  scope, and the adapter runs this gate before any evaluation
  (`adapter.rs`). The adapter reports the refusal; it never approximates.
  This is the ONLY trigger the current port fully executes.
- **Owner:** the gate itself (P4.02 scope discipline). No re-open needed;
  this trigger is the refusal path, not a re-open path.

### T1 — a NAMED test case the balanced model cannot express or validate (executable today, process trigger)

- **Observable:** a named, recorded test case — e.g. an unbalanced feeder
  case from §19.3 (the IEEE 13-bus unbalanced case is the standing
  candidate; CONTEXT.md open question 5) — that cannot be expressed or
  validated with the balanced model.
- **Checkable today:** YES, as a process trigger — this is exactly the
  D-P4.01-3 provider-addition rule (P4.01, decision note
  `01_02_grid_provider.md`): a second provider (OpenDSS / GridLAB-D /
  PyPSA) is added only for a recorded named case; the case is recorded.
- **Owner on fire:** a new Phase 4 subtask owning the provider addition
  (per P4.01's addition rule), with the case recorded in this directory
  and D3 remaining OPEN.

### T2 — single/two-phase laterals or single/two-phase DER connections annotated in the model (NOT executable today — recorded gap)

- **Observable:** the model artefact declares connections that are not
  three-phase (a single- or two-phase lateral, a single-phase DER
  connection).
- **Checkable today:** NO. The port's model IR
  (`crates/flexcore-grid-api/src/port.rs`: `VersionedNetworkModel` with
  `BusSpec` / `BranchSpec` / `TransformerSpec` / `ExternalGridSpec`)
  carries **no phase annotation of any kind** — `BranchSpec` is
  explicitly positive-sequence; no field names a phase, a connection
  count, or a lateral class. A check for this trigger needs ADDITIVE
  model-IR fields (per-bus/per-branch phase annotation) carried through
  the versioned-model store (P4.03) — new infrastructure surface, not
  present.
- **Owner on fire:** a recorded decision extending the model IR (owner:
  power-systems + the grid-api crate owner), then a new subtask owning
  the unbalanced adapter per D-P4.01-3.

### T3 — unbalanced load annotations present in the model (NOT executable today — recorded gap)

- **Observable:** the model artefact declares per-phase load data or
  unbalanced load allocation.
- **Checkable today:** NO — same reason as T2: the IR carries no
  per-phase load fields (loads enter only through candidate injections
  at buses, `CandidateInjection`, aggregate MW/MVAr under the declared
  Q convention).
- **Owner on fire:** as T2.

### T4 — measurable phase imbalance above a configured threshold (NOT executable today — recorded gap)

- **Observable:** per-phase telemetry (phase currents or phase powers)
  whose imbalance ratio exceeds a configured threshold.
- **Checkable today:** NO. The only telemetry the gate seam carries is
  the ramp baseline `TelemeteredState` (`feasibility.rs`):
  ONE aggregate `active_power_mw` value with its quality flag — no
  per-phase values, and no phase identity. The power-flow result's
  `BusVoltage` / `BranchLoading` / `TransformerLoading` (`port.rs`) are
  likewise aggregate (positive-sequence) quantities. A check for this
  trigger needs per-phase telemetry values WITH their §10.4 quality
  flags at the seam — a new seam surface.
- **Owner on fire:** a recorded decision adding a per-phase telemetry
  seam (owner: the grid-api + telemetry crate owners), then the
  threshold becomes configuration per the P4.06 limit-configuration
  discipline (D-P4.06-1: configuration, source recorded, never
  hard-coded).

### T5 — negative- or zero-sequence indicators in a power-flow result (NOT executable today — recorded gap)

- **Observable:** the worker reports non-trivial negative- or
  zero-sequence content (sequence-domain residuals or per-phase result
  rows).
- **Checkable today:** NO. The backend seam's outcome vocabulary
  (`crates/flexcore-grid-api/src/backend.rs`: `BackendBusResult`,
  `BackendBranchResult`, `BackendTransformerResult`) carries one
  magnitude/loading per element — balanced quantities only; the worker
  contract has no sequence-domain fields, and the translation layer
  (`translation.rs`) defines no sequence conversion.
- **Owner on fire:** a recorded decision extending the worker contract
  (`GridModelBackend` payload vocabulary, the D-P4.02-1 pattern) plus the
  adapter scope change to `PhaseScope::Unbalanced`; then a provider that
  computes sequence quantities (per D-P4.01-3: OpenDSS/GridLAB-D class).

### Honest summary of the gap

**No automatic trigger check executes today except T0 and T1.** The port
carries no phase data in any direction (model IR, telemetry seam, or
result rows), so T2–T5 cannot be computed against the existing surface
without new recorded decisions and additive fields. What DOES hold today,
fail-closed: an unbalanced run cannot happen silently (T0 refuses it),
and every result, limit report, and verdict states the balanced model
class and the unbalanced coverage gap (§3). The residual risk the ledger
names — "an unbalanced feeder silently validated with a balanced model"
— is mitigated but NOT eliminated: a feeder that IS unbalanced in
reality but is ENTERED as a balanced model with no annotations produces
balanced-looking results. That residual is honest and recorded; closing
it is exactly what T2–T5's future surfaces are for, and until then the
mitigation is process (T1: named cases; the synthetic feeders are
declared balanced by construction).

## 5. The governed re-open procedure

1. **Trigger fires** (T1 process trigger, or T2–T5 once their surfaces
   exist). The firing is RECORDED in this directory: the named case, the
   trigger id, the evidence reference.
2. **Provider selection per the addition rule (D-P4.01-3):** the named
   case is matched against the primary provider's declared scope; if it
   cannot be expressed or validated, the provider addition (OpenDSS /
   GridLAB-D / PyPSA per §12.4) is proposed with the case recorded —
   never by familiarity.
3. **Adapter scope declaration:** the new adapter declares
   `PhaseScope::Unbalanced` (plus its topology and equipment scopes and
   its mandatory non-empty `not_modelled` list) — the same construction-
   error discipline as P4.02. A new solver-class statement is validated
   the same way (AC power-flow evaluation, NOT AC-OPF — the terms are
   never interchanged, and an unbalanced power flow is still not an
   AC-OPF).
4. **Model/telemetry/worker surfaces (T2–T5 only):** each additive IR
   field, seam, or payload extension is its own recorded decision (the
   D-P4.02-1 worker-contract pattern), with unit/sign translation owned
   and tested both directions (§10.2) — no silent convention change.
5. **D3 decision:** the network-class decision itself is made by the
   Phase 0 gate (owner power systems) with the accumulated named cases as
   input — this record's triggers are INPUTS to D3, never a substitute
   for it.
6. **Release placement:** unbalanced three-phase modelling enters the
   track from v0.4.0 per the release ladder (§2.1, §25.2) — a re-open
   does not move the capability into the v0.3.0 line.

## 6. Verification method and what was NOT done

- **Method (ledger P4.34):** deferral-note review against §12.4/§12.5;
  result-record inspection. The result-record side (§3) is verified by
  citation of the existing, reviewed code paths listed above; the actual
  build/test execution remains the human's (loop bans — nothing built,
  nothing run in authoring this record).
- **Not done here:** no code was added to `flexcore-grid-api` (no trigger
  predicate was implemented) because no T2–T5 predicate is computable
  against the existing port — the gap is recorded in §4, not papered
  over with a stub check that cannot see phase data.
- **D3 status after this record: OPEN** — unchanged, by construction
  (§0).
