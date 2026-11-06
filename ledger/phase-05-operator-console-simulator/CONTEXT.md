# Context — Phase 5: Operator Console and Simulator Integration

## 1. Background

Phase 5 makes the platform operable and checkable: an operator console that
shows every workspace the report defines, and a simulator integration that lets
an operator run a full simulated VPP scenario, inspect every scheduling and
dispatch decision, inject failures, and replay the scenario to verify
reproducibility. It completes the §24 vertical slice (steps 9–10) and closes
the 0.x simulation line's core loop: model → telemetry → flexibility →
schedule → validate → authorize → dispatch (to simulator) → measure →
inspect → replay.

The phase maps to §23 Stage 5. The console is the operator's view of
simulation/shadow work. It is not a field-control tool in this phase: control
authority stays Simulation or read-only/shadow (§21.4), and operator-approved
field control is a v1.0.0 matter.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §20.1–§20.2 | All 12 workspaces; design principles (the phase's UI contract) |
| §19.1–§19.4 | Simulation strategy, test layers, benchmark cases, evaluation metrics |
| §12.4 | Provider table incl. HELICS (coordinates federates; does not replace physics) |
| §24 steps 9–10 | Display the audit trail; replay reproducibility — the phase's exit demonstration |
| §23 Stage 5 | Exit gate: operator runs a full simulated VPP scenario and inspects every decision |
| §14.1–§14.2 | Command lifecycle states the Dispatch workspace must distinguish |
| §15.1, §21.3 | Live UI styles (SSE/WebSocket); observability correlation |
| §17.2–§17.4, §16.2 | Zones, required controls, threat model, tenancy/RLS |
| §21.4 | Build/distribution form; control-authority modes; browser matrix |
| §7.5 | Frontend stack (React + TypeScript, Vite, TanStack, generated clients) |
| §22.2–§22.3 | CI gates; release gates (incl. reproducible benchmarks, browser/OS matrix) |

## 3. Canonical units and sign

- MW, MVAr, MVA, MWh (§10.2). `P > 0` means export at the point of connection.
- Reactive-power sign is declared per grid model and device profile, never assumed.
- SOC is a fraction in [0,1] at the API boundary.

## 4. Telemetry quality

Exactly one quality state per value (§10.4): GOOD, SUSPECT, BAD, MISSING,
ESTIMATED, STALE. The console shows quality per value and marks STALE data as
stale; a live-update disconnect is shown as "live updates paused — data may be
stale" (P5.25), never silently gap-filled.

## 5. Terminology

- **Workspace:** one of the twelve §20.1 console areas. Workspaces are views;
  they never define authoritative permissions or issue device commands (§7.5).
- **Simulated value:** a value produced by a simulator or a schedule, labelled
  as simulated. Distinct from measured (telemetry), estimated, and stale (§20.2).
- **Acknowledgement:** application-layer acceptance (§14.1). In this phase the
  acknowledging party is the simulator. Never displayed as physical delivery.
- **Scenario:** a pinned, replayable record (inputs, versions, seeds, expected
  outcomes/tolerances). **Replay:** re-execution from the recorded inputs only.
- **Reproducibility:** the same scenario + versions + seeds produce the same
  results within a stated, pre-registered tolerance (§24 step 10).
- **Federation (HELICS):** a set of federates (simulators) coordinated in time
  by a HELICS broker. HELICS is the coordinator, not a physics model (§12.4).
- **Shadow mode:** proposed actions computed and compared against measurements
  without issuing commands (§21.4).
- Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).

## 6. Constraints (simulation-only 0.x)

- Simulation/shadow only (§21.4). No field writes; the simulator is the only dispatch target.
- No safety or certification claims; no protection-grade claims (AGENTS.md rules 2–3).
- Never present a proposed setpoint as executed, a simulated value as measured, or an acknowledgement as physical delivery (§20.2, §14.1).
- Never use a steady-state power-flow tool as evidence for transient frequency stability (§19.1).
- HELICS does not replace physics models (§19.1, §12.4).
- No hallucinated facts; external claims need primary sources or an ASSUMPTION label. Research cut-off 7 November 2026.
- Do not claim an IEEE feeder case was run unless its licensing and model availability were confirmed and the run is recorded (§19.3).

## 7. Prior-phase outputs consumed

- Phase 3: schedule/comparison APIs, envelope, §12.6 result records.
- Phase 4: command lifecycle + audit trail, feasibility evidence, rejection evidence pack, simulator dispatch adapter and its simulation-only boundary, invariant matrix.
- Phase 2: registry, telemetry with quality flags, asset simulator, forecasts.
- Phase 1 (assumed per report §23 Stage 1): Rust foundation, API/OpenAPI, PostgreSQL + tenant context, NATS + outbox, identity/RBAC, tracing. Record the actual state at P5.01/P5.26 if it differs.

## 8. D1–D6 relevance

| Gate | Status | Effect on Phase 5 |
|---|---|---|
| D1 target jurisdiction/market | OPEN | Market & Programs workspace shows synthetic commitments labelled as such; no market adapter claimed |
| D2 first portfolio/service | OPEN | Synthetic portfolio/scenarios only |
| D3 network class/model fidelity | OPEN | Balanced radial default; unbalanced three-phase from v0.4.0; meshed transmission out of scope |
| D4 license/dependency audit | OPEN (before v0.1.0) | Console + simulator dependency licenses (React/Vite/TanStack, HELICS BSD-3-Clause, pandapower BSD-3-Clause) recorded as audit input |
| D5 first protocol/asset mix | OPEN (before v0.4.0) | No protocol adapter; simulator only; edge enrollment shows lab state honestly |
| D6 cyber/OT boundary review | OPEN (before any read-only pilot) | No OT boundary crossed; console security is code-level (RBAC/tenancy/mode) only |

## 9. Open questions

1. Emergency actions (§20.2): the report requires emergency and normal scheduling to be visually separate, but no emergency action is defined for this phase — confirm whether an emergency action exists in scope or remains an open item (P5.06).
2. TanStack Router/Query vs equivalent: §7.5 allows an equivalent — confirm the choice at scaffold time and record it (P5.01).
3. React/TypeScript/Vite versions: verified as of 2026-11-07 in RESEARCH.md from secondary sources — re-verify against the official React/Vite release pages at scaffold time (RESEARCH.md open task).
4. `pyhelics` binding version matching HELICS 3.7.0: confirm at P5.16 implementation time.
5. Browser-matrix CI level: full matrix is a §22.3 release-gate activity — record which level runs in CI in this phase (P5.12, P5.28).
6. Demo data: is any demo data permitted in the console, and if so, which workspaces? Default position: labelled demo only, empty states preferred (P5.07, P5.12).
