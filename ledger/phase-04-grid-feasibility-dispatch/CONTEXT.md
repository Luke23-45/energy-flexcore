# Context — Phase 4: Grid Feasibility and Dispatch Assurance

## 1. Background

Phase 4 is where FlexCore stops trusting the optimizer. A schedule that solves
(Phase 3) is only a candidate: this phase checks it against the physical network
with an independent AC power-flow evaluation, then moves it through the protected
command lifecycle — validation, feasibility, human authorization, issue to a
simulator, acknowledgement, tracking, completion — with every rejection,
expiry, and fallback recorded. It is the "validate against the network →
authorize → dispatch through a controlled adapter → measure delivery" part of
the report's core order (§28).

The phase maps to release v0.3.0 (§2.1): "validation, feasibility, command
lifecycle, approvals, audit", simulation and shadow (§21.4). The dispatch
adapter's only target is the simulator. There is no device write path in this
phase, and operator-approved field control remains a v1.0.0 matter.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §12.4–§12.5 | Provider selection; the eight-step feasibility gate; fail-closed rule; SOCP relaxation caveat |
| §12.6 | Optimization result contract — feasibility results follow the same evidence discipline |
| §14.1–§14.5 | State machine, state definitions, invariants, fallback, timing honesty |
| §17.1–§17.5 | Security model, zones, required controls, threat model, safety invariants |
| §11.1–§11.6 | Capability models the feasibility gate checks against |
| §10.1–§10.4 | Entities, units/sign, time model, telemetry quality |
| §15.1–§15.5, §16.1–§16.3 | APIs, events, outbox, storage, tenancy, retention |
| §19.1–§19.4 | Simulation strategy, test layers (esp. fault injection, security, shadow), benchmarks, metrics |
| §20.1 items 5, 7, 11 | Grid, Dispatch, Audit workspaces — display contracts only; UI is Phase 5 |
| §23 Stage 4 | Exit gate: six rejection categories in deterministic tests |
| §24 steps 5–8 | Vertical-slice landing points for this phase |

## 3. Canonical units and sign

- MW, MVAr, MVA, MWh (§10.2). `P > 0` means export at the point of connection.
- Reactive-power sign is declared per grid model and device profile, never assumed.
- Voltage is kV at the nominal base, per-unit in solver representation. Current is A or per-unit on an explicit base.
- SOC is a fraction in [0,1] at the API boundary.

## 4. Telemetry quality

Exactly one quality state per value (§10.4): GOOD, SUSPECT, BAD, MISSING,
ESTIMATED, STALE. Feasibility checks that need telemetry fail closed on
BAD/MISSING/STALE inputs (P4.08). A missing/uncertain/stale model is never
interpreted as a passing feasibility check (§17.5 invariant 9).

## 5. Terminology

- **Feasibility gate:** the §12.5 eight-step pipeline. A pass means the candidate
  satisfied the declared network model and limits within stated tolerances — not
  that it is safe in the field, certified, or approved.
- **AC power flow:** evaluation of a network state for specified injections and
  controls. **AC-OPF** is a different problem (optimization). Never use the terms
  interchangeably.
- **Validation / feasibility / authorization:** three distinct gates (§14.1).
  Validation is schema/freshness/asset checks; feasibility is the network check;
  authorization is human/policy approval.
- **Acknowledgement:** application-layer acceptance of a command. Not physical
  delivery (§14.1).
- **SUPERSEDED vs CANCELLED:** superseded means replaced by a newer approved
  revision (retained for audit); cancelled means explicitly cancelled by an
  operator or policy (§14.2). Do not merge them.
- **FALLBACK:** the state entered when an asset/site fallback policy activates
  after FAILED or EXPIRED (§14.2, §14.4). There is no universal fallback.
- **Shadow mode:** proposed actions are computed and compared against measured
  behaviour without issuing commands (§21.4). Shadow agreement is not a safety
  claim.
- Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).

## 6. Constraints (simulation-only 0.x)

- Simulation and shadow only (§21.4). No field writes; the simulator is the sole dispatch target, guarded by test.
- No safety or certification claims; no protection-grade claims (AGENTS.md rules 2–3).
- Solver classes never conflated (AGENTS.md rule 4): AC power flow is not AC-OPF; HiGHS LP/QP/MIP not SOCP; Clarabel convex conic not MISOCP; IPOPT local not global.
- SOCP relaxation exactness is conditional and must be checked, not assumed (§12.5).
- No hallucinated facts; external claims need primary sources or an ASSUMPTION label.
- Research cut-off 7 November 2026 (AGENTS.md §7).

## 7. Prior-phase outputs consumed

- Phase 3: schedule ID + revision (P3.22), envelope version (P3.13), §12.6 optimization result records (P3.21), comparison records (P3.23), and the Tier 2 SOCP provenance marking (P3.18).
- Phase 2: asset registry, telemetry with quality flags, site/feeder grid-model definitions, simulator telemetry source.
- Phase 1 (assumed per report §23 Stage 1): Rust foundation, API, PostgreSQL + tenant context, NATS + outbox, identity/RBAC contracts. If the identity/RBAC implementation is staged differently, record the actual state in P4.13 rather than assuming it.

## 8. D1–D6 relevance

| Gate | Status | Effect on Phase 4 |
|---|---|---|
| D1 target jurisdiction/market | OPEN | No market adapter; commitments remain synthetic; no market qualification claimed |
| D2 first portfolio/service | OPEN | Synthetic portfolio and synthetic feeder cases only |
| D3 network class/model fidelity | OPEN | Balanced radial default; pandapower primary; unbalanced three-phase enters from v0.4.0; meshed transmission out of scope |
| D4 license/dependency audit | OPEN (before v0.1.0) | pandapower (BSD-3-Clause), OpenDSS/GridLAB-D/PyPSA licenses recorded in RESEARCH.md as audit input |
| D5 first protocol/asset mix | OPEN (before v0.4.0) | No protocol adapter; simulator adapter only |
| D6 cyber/OT boundary review | OPEN (before any read-only pilot) | No OT boundary crossed; no pilot; P4.28–P4.29 are code-level controls only |

## 9. Open questions

1. Limit configuration source: are voltage bands and equipment ratings taken from the synthetic feeder fixtures, or from a named reference feeder (IEEE 13/34/123-bus, §19.3)? Decide at P4.06; record the source per limit.
2. Constrain-vs-return policy (§12.5 step 8): when is a violating candidate constrained and returned to the scheduler vs rejected outright? Decide with power-systems review at P4.07.
3. Hold-vs-reject split in the fail-closed policy (P4.08): which triggers hold (operator-releasable) and which reject? Decide at implementation; the table is the decision record.
4. Approval scope/limits defaults for the simulation workflow (P4.13): what scope does a simulated approval carry? Record explicitly — a simulated approval must not be mistaken for a field approval later.
5. Second provider (OpenDSS for unbalanced work): needed in this phase only if a named test case requires it (P4.01 rule). The IEEE 13-bus unbalanced case (§19.3) may be that case — decide at P4.01, not by default.
6. Clock-skew tolerance for command timestamps (P4.21): pick a configured value and record it; it interacts with §17.4 (clock drift threat).
