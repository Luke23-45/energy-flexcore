# P5.17 — Additional simulator adapters: scoping decision (§19.1, §12.4)

Date: 2026-11-11. Status: **DECIDED (DEFAULTs recorded, D-gated items
marked pending)** — this is the decision record the ledger P5.17
deliverable requires. Dependencies P5.14–P5.16 are implemented
(awaiting the human build/test pass — see `14_15_adapters.md` and
`16_helics_cosim.md`); this scoping DECISION does not depend on their
execution: it scopes TRACKS, and an UNRUN implementation scopes the same
tracks an executed one would.

Report trace: §19.1 (the simulation-strategy table and its two boundary
sentences), §12.4 (provider table + "The initial implementation should
choose one primary deterministic path and add other providers only when a
concrete test case requires them"), §23 Stage 5, §19.2 layer 10 (HIL /
commissioning tests only after simulation, integration, and cyber reviews
pass).

The §19.1 track enumeration below is quoted from the report's §19.1
table (read from
`docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
lines 1708–1721, not from memory or a summary document).

---

## 1. The scoping decision, per §19.1 track

| # | §19.1 track (verbatim) | §19.1 tools | Decision | Where / when | Reason (traceable) | Trigger to re-open |
|---|---|---|---|---|---|---|
| 1 | Portfolio scheduling — "Does schedule respect SOC, ramp, duration, and portfolio commitments?" | Synthetic asset models + optimizer | **INTEGRATED — Phase 5** (P5.14; DEFAULT) | The scenario-run composition (`apps/sim-runner` `scenario_run.rs`) against the REAL Phase 2 simulator | §19.1's first track; §23 Stage 5 needs the simulated VPP scenario; the Phase 2 simulator + Phase 3/4 chain are the built providers | A new asset class beyond the Phase 2 profile family (BESS, PV, flexible load, EVSE) |
| 2 | Distribution quasi-static — "Are voltage/current/transformer constraints respected?" | pandapower, GridLAB-D, OpenDSS | **INTEGRATED — Phase 5, pandapower only** (P5.15; DEFAULT), under the D3 DEFAULT (balanced radial); unbalanced three-phase stays deferred | The Phase 4 pipeline (`run_feasibility_gate` + `PandapowerGridProvider`) over the selectable backend (scripted seam / worker process) | §12.4: ONE primary deterministic path; pandapower is the recorded provider pin (D-P4.01-1, 3.5.6); GridLAB-D/OpenDSS added only when a named case requires it | **PENDING D3** for the unbalanced three-phase extension: re-open per the T0–T5 trigger enumeration in `ledger/phase-04-grid-feasibility-dispatch/34_unbalanced_deferral.md` (a per-phase-model adapter is uncomputable against the current grid-api port — recorded gap). GridLAB-D/OpenDSS: re-open when a named test case requires a second provider (§12.4) |
| 3 | Transmission/planning — "How does portfolio behaviour affect higher-level system studies?" | PyPSA and selected benchmark cases | **DEFERRED — documented optional comparison only** (DEFAULT), pending D3 | Not built in Phase 5; no PyPSA dependency exists | §12.4: PyPSA is "Not the sole distribution device-control validator"; the D3 DEFAULT records meshed transmission as OUT OF SCOPE (`.agent/CONTEXT.md` §8); no named case in §19.3 needs it in v0.3.0 | **PENDING D3** (meshed/transmission scope): re-open when a D3 decision admits transmission studies AND a named §19.3 case requires it — then as a documented OPTIONAL COMPARISON only, never a distribution validator |
| 4 | Dynamic stability — "Does a proposed research controller affect frequency/transient behaviour in the declared model?" | ANDES / selected dynamic simulator | **DEFERRED — Stage 8** (DEFAULT) | Not built in Phase 5 | §12.4: ANDES is a "Separate dynamic-stability validation track" [R24]; research modules are §23 Stage 8; Phase 5 has no dynamic simulator dependency and no research controller to study | Stage 8 entry (research modules), with a named research controller and a declared dynamic model |
| 5 | Multi-domain co-simulation — "How do grid, communication, market, and end-use simulators interact across time?" | HELICS | **INTEGRATED — Phase 5, MINIMAL (three federates)** (P5.16; DEFAULT); the federation's market and communication legs are **PENDING D1 / deferred**, not decided | `apps/sim-runner/src/helics_orchestration.rs` + `apps/worker/python/helics_federates/` (grid + asset/end-use + FlexCore/control federates) | P5.16 step 1's three-federate minimum is implemented; §19.1 also names communication and market simulators — no market model exists (D1 OPEN: "Market & Programs shows synthetic commitments labelled as such", no market adapter), and no communication-network simulator is in any stage's scope | **PENDING D1** for a market federate (re-open when D1 fixes a target jurisdiction/market and a market model exists); a communication-network federate re-opens with the Stage 6 protocol/edge work, where a real network model would be needed |
| 6 | Hardware-in-the-loop — "Does command/control integration behave correctly with real controller hardware?" | Vendor/partner HIL platform | **DEFERRED — Stage 9** (DEFAULT; explicit) | Not built anywhere in the 0.x line | §19.2 layer 10: HIL/commissioning tests come "only after simulation, integration, and cyber reviews pass"; §23 Stage 9 is the qualification stage; the Phase 5 loop's own boundary: HIL "is never implied by a simulation result"; PLAN.md §9 lists HIL under "must NOT be built yet" | Stage 9 entry, after the §19.2 layers 1–9 pass AND the cyber review (D6) passes for the target site |

Every track has a recorded integrated/deferred decision with a reason;
the ANDES and HIL deferrals are explicit; no steady-state result is or
can be presented as stability evidence (§3 below).

## 2. The plugin-in-the-loop note (platform fact, not choice)

The §19.1 track list does not name plugins, but plugin-in-the-loop
simulation is the natural question this scoping decision must answer
honestly: **the plugin kernel does not exist.** The Phase 4 handover
(`32_handover_phase5.md` §5, invariant-7) records that the v0.5.0 plugin
kernel is unbuilt and the bypass-proof is structural (no plugin execution
path). Plugin-in-the-loop simulation is therefore **deferred to Stage 7
by platform fact, not choice**: there is no plugin host, no WASM runtime,
and no plugin boundary to put in a loop. When Stage 7 delivers the plugin
SDK/WASM host, its §19.2 contract-test layer (layer 3) and the P5.21
failure-injection pattern (which already lists "plugin crash" as a fault
type) are the integration points. Nothing in Phase 5 may imply a
plugin-in-the-loop capability.

## 3. The recorded constraint on the Simulation Lab's claims (§19.1, P5.17 step 3)

**A steady-state power-flow tool is not evidence for transient frequency
stability (§19.1).** Concretely, for every consumer of Phase 5 results
(P5.08 Simulation Lab, P5.23 benchmarks, P5.29 operator guide):

- pandapower `runpp` (the P5.15 leg) and the grid federate built on it
  produce STEADY-STATE, balanced, quasi-static results. They say nothing
  about frequency response, transient stability, inertial behaviour, or
  protection operation — the report excludes those functions from
  FlexCore entirely (no protection-grade claims, AGENTS.md rule 3).
- HELICS (the P5.16 leg) coordinates federates; a completed co-simulation
  is not a power-system validation by itself, and "HELICS itself [is not]
  the power-system physics model" (§19.1).
- Only the ANDES track (deferred, Stage 8) could produce
  frequency/transient statements, in a declared dynamic model, and only
  as research evidence — never as field-safety or certification evidence
  (AGENTS.md rule 2).

This constraint is a fixed display rule for the Simulation Lab (P5.08)
and a fixed limits statement in the operator guide (P5.29 step 2, which
already cites P5.17).

## 4. Filing (P5.17 step 4)

The scoping table is filed in PLAN.md (§11 of
`ledger/phase-05-operator-console-simulator/PLAN.md`, added additively by
this subtask) and in this decision record; PLAN.md's §2 decision row
"HELICS coordinates, never replaces physics" is unchanged by it.

## 5. What was NOT touched

No code was written or modified for P5.17: it is a decision record. The
D-gates (D1, D3) remain OPEN; nothing here resolves or narrows them. The
DEFAULT labels above are the recorded default positions (ledger P5.17
step 2, "to confirm at implementation") — the human build/test pass
confirms them against the actually-executed integrations.
