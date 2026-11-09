# P0.12 — Grid-model contract freeze

| Field | Value |
|---|---|
| Subtask | P0.12 — Freeze the grid-model contract |
| Report trace | §10.1 (GridModel entity), §12.4 (grid-model provider selection), §12.5 (network feasibility gate), §12.6 (result contract: grid model version on every record), §19.3 (benchmarks); §25.2 D3 |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.05 (D3 network class), P0.10 (asset/capability vocabulary) |
| Status | Draft for Stage 0 review |
| Decisions | D3 (network class and model fidelity) remains OPEN (§25.2); provider selection is Phase 4 work gated on D3 (ledger P0.12 step 6). This document selects nothing |

---

## 1. What a grid model is (§10.1 GridModel entity)

The GridModel is a canonical §10.1 entity. Its key fields, listed in full:

| Field group | §10.1 key fields |
|---|---|
| Identity | model/version |
| Topology | buses, branches, transformers, switches, phases |
| Electrical basis | base values |
| Constraints | limits |

The Site entity references its grid model (`grid model reference`, §10.1 Site row), and Asset carries a bus reference (§10.1 Asset row) — so a candidate injection can always be located on the versioned model it must be evaluated against. Per P0.10, this is vocabulary, not a database schema; schema design is Phase 1/2 work.

## 2. The versioned-model rule

Feasibility checks apply candidate injections and controls **to a versioned network model** (§12.5, step 3 of the validation pipeline). Every decision record carries the grid-model version: the §12.6 optimization-result contract requires the input snapshot and grid model version on every result, and the feasibility outcome reported to operators shows model version and timestamp (§20.2, "show model version and timestamp for grid-feasibility outcomes").

Consequences frozen here:

- A feasibility verdict is only meaningful against a named model version; an unversioned or unnamed model cannot support a FEASIBLE transition (§14.1 VALIDATED → FEASIBLE passes "within declared model scope and tolerances", §14.2).
- No command may reference a grid model so loosely that the evidence cannot be replayed against the same version (§14.3 invariant 12: grid model and telemetry snapshot references; §24 step 10: replay with the same versions and verify reproducibility).

## 3. AC power flow is not AC-OPF

An AC power-flow check is a **feasibility evaluation of specified injections/controls**. It is not itself an AC-OPF optimization (§12.5, explicit statement; §7.4 solver selection rule). The §12.3 validation row assigns AC power flow / three-phase unbalanced power flow the role "evaluate a candidate schedule/dispatch against the chosen network model" — evaluation of a given state, not optimisation of injections.

Solving AC power flow does not produce an optimal dispatch, and an AC-OPF result is not a power-flow verification of a specific candidate. The two problem classes stay separated in all Phase 0 artefacts and in every later document (AGENTS.md §3 rule 4; P0.13).

## 4. The SOCP-relaxation exactness rule

For a radial distribution feeder, a branch-flow SOCP relaxation may be useful. Its solution **may be inexact**. The system must validate the relaxation conditions and verify the candidate with an **independent AC power-flow engine** (§12.5). Exactness must not be assumed for (§12.5):

- reverse power flow;
- voltage upper bounds; or
- arbitrary network constraints.

A relaxation pass is therefore never sufficient evidence on its own; the independent AC power-flow verification is part of the frozen feasibility evidence.

## 5. The failure rule (stale/incomplete model is not a pass)

If the validator fails or the model is stale/incomplete, **the system must not infer feasibility**. It should reject or hold the command according to a documented policy (§12.5). This is restated as a §17.5 safety invariant — "a missing/uncertain/stale model is not interpreted as a passing feasibility check" (invariant 9) — which is enforced by protected platform code and cannot be bypassed by plugins. Silence, non-convergence, or model gaps are never evidence of a passing check; non-convergence and model coverage limitations must be reported explicitly (§12.5, step 6).

## 6. The feasibility pipeline (§12.5, all eight steps)

The validation pipeline must (§12.5):

1. validate candidate schema, units, timestamp, and validity window;
2. check resource capability and ramp constraints;
3. apply candidate injections and controls to a versioned network model;
4. run the appropriate AC power-flow or three-phase unbalanced power-flow model;
5. verify voltage, current/thermal, transformer, generator, and relevant equipment limits;
6. report non-convergence and model coverage limitations explicitly;
7. compute margin/diagnostics; and
8. reject, constrain, or return the candidate for re-optimization when it is not acceptable.

A candidate dispatch is not accepted merely because the optimizer reports success (§12.5, opening sentence).

## 7. Provider menu (§12.4 — a menu, not a selection)

No single simulator is appropriate for every grid analysis (§12.4, opening sentence). The §12.4 table is reproduced below with recommended roles and boundaries intact.

| Tool | Recommended role (§12.4) | Important boundary (§12.4) |
|---|---|---|
| pandapower | Python power-system analysis, balanced AC power flow, some three-phase/asymmetric calculations, OPF studies | Verify model and solver limitations for each case. [R20] |
| GridLAB-D | Detailed distribution simulation, unbalanced three-phase power flow and end-use/DER models | Use for distribution and time-series studies; not automatically a fast transient stability engine. [R21] |
| OpenDSS | Distribution-system analysis, unbalanced feeder studies, DER simulation | Use through a defined simulator adapter. [R22] |
| PyPSA | Energy-system and transmission/planning optimization | Not the sole distribution device-control validator. [R23] |
| ANDES | Dynamic power-system simulation, transient stability, eigenvalue analysis | Separate dynamic-stability validation track. [R24] |
| HELICS | Co-simulation orchestration across simulators, communication networks, market and end-use models | It coordinates federates; it does not replace their physics models. [R16] |

**No provider is selected in Phase 0.** The initial implementation should choose one primary deterministic path and add other providers only when a concrete test case requires them (§12.4, closing sentence) — but that choice is Phase 4 work, gated on decision D3 (network class and model fidelity), which remains OPEN (§25.2; ledger P0.05, P0.12). The recommended default recorded for 0.x simulation planning only (balanced radial feeders; pandapower as primary power-flow provider) is a RECOMMENDATION, not a decision, and must never be written up as decided (`.agent/DECISIONS.md` rule; `03_decision_records.md`).

## 8. Foundations §7.7 extras — Phase 4 carry-forwards

The report §12.5 verification list (§6 above, step 5) does not name every check the foundations specification §7.7 requires of a feasibility check. The following extras are recorded here as **cross-reference carry-forwards into Phase 4** — Phase 4 owns the checks; this step only prevents them from being silently dropped because §12.5 does not list them (ledger P0.12 step 7):

| # | Foundations §7.7 extra | Carry-forward |
|---|---|---|
| 1 | Tap/phase-shifter limits | Include in Phase 4 feasibility verification scope |
| 2 | Emergency vs normal ratings | Include in Phase 4 feasibility verification scope |
| 3 | Topology/model revision | Include in Phase 4 model-versioning checks (reinforces the §2 versioned-model rule) |
| 4 | Measurement freshness | Include in Phase 4 snapshot-freshness checks (§14.3 invariant 12) |
| 5 | Operating-state consistency | Include in Phase 4 operating-profile checks |

This is a cross-reference, not a second normative list; the foundations specification governs where it conflicts with the report (AGENTS.md §2).

## 9. What this contract does not do

- It does not select a provider or a network class (D3 open; Phase 4 gated).
- It does not implement any check; the entire 0.x line is simulation-only and Phase 0 produces no code (§0, §2.1).
- It makes no safety or certification claim: a passing feasibility check is simulation evidence within a declared model scope and tolerances (§14.2 FEASIBLE row), not a guarantee of field behaviour, and site-local safety and protection remain independent of FlexCore (§17.5, invariant 10).

## 10. Acceptance-criteria coverage (ledger P0.12)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| GridModel fields from §10.1 listed in full | §1 |
| Versioned-model rule stated | §2 |
| "Stale/incomplete model is not a pass" rule stated | §5 |
| AC power flow ≠ AC-OPF stated | §3 |
| SOCP-relaxation exactness warning stated | §4 |
| Foundations §7.7 extras recorded as Phase 4 carry-forwards | §8 |
| No provider selected as "the" grid provider in Phase 0 | §7 |

Verification method (ledger): comparison against §10.1, §12.4, §12.5 — performed in §1, §6, §7.
