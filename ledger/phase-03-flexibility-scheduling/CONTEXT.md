# Context — Phase 3: Flexibility and Deterministic Schedule

## 1. Background

Phase 3 turns the Phase 2 asset and telemetry foundation into the platform's first
real product behaviour: estimating how much flexibility a portfolio can actually
deliver, and producing a rolling-horizon schedule that respects asset and
time-coupled constraints. It is the "estimate flexibility → schedule" half of the
report's core order (§28): model the assets → ingest trustworthy telemetry →
estimate flexibility → schedule → validate against the network → authorize →
dispatch through a controlled adapter → measure delivery → improve with research
plugins. This phase covers the middle two steps only. Validation, authorization,
and dispatch are Phase 4, and nothing in this phase may imply they have happened:
a schedule that solves is a *candidate*, not a feasible or approved dispatch.

The phase maps to release v0.2.0 (§2.1): "flexibility envelopes, rolling-horizon
scheduling", simulation only. Field write authority begins at v1.0.0 with operator
approval (§21.4); this phase creates no write path to any device.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §11.1–§11.6 | Capability models (generic, BESS, P-Q, load, EV) and portfolio aggregation — the phase's subject matter |
| §12.1–§12.3, §12.6 | Objective, PCC tracking, optimization tiers, result contract |
| §12.4–§12.5 | Read for the boundary: grid-model providers and the feasibility gate belong to Phase 4; do not build them here |
| §10.1–§10.4 | Entities, units/sign, time model, telemetry quality |
| §9.1 | `Optimizer` port and plugin result requirements |
| §14.1–§14.2 | Command lifecycle and SUPERSEDED — schedule revisions follow the same supersede discipline |
| §15.1–§15.5, §16.1–§16.2 | APIs, events, outbox, storage, tenancy |
| §19.2–§19.4 | Test layers, benchmark cases, evaluation metrics |
| §23 Stage 3 | Exit gate: schedules pass asset/time-coupled constraints on defined synthetic cases |
| §24 steps 3–4 | Vertical-slice landing points for this phase |
| §25.1–§25.2 | Provisional stack; deferred decisions D1–D6 |

## 3. Canonical units and sign

- Power/energy: MW, MVAr, MVA, MWh (§10.2). Do not mix kW/kWh into contracts.
- `P > 0` means export at the point of connection. Say "export" or "import" when the sign could be misread.
- Reactive-power sign is declared per grid model and device profile, never assumed (§10.2).
- Voltage is kV at the nominal base, per-unit in solver representation. Current is A or per-unit on an explicit base.
- SOC is a fraction in [0,1] at the API boundary.
- BESS convention (§11.2): `P_dis ≥ 0` discharge, `P_ch ≥ 0` charge, `P_net = P_dis − P_ch`.

## 4. Telemetry quality

Exactly one quality state per value (§10.4): GOOD, SUSPECT, BAD, MISSING,
ESTIMATED, STALE. Quality survives aggregation into the scheduling grid (P3.03)
and can invalidate an envelope (P3.13). A value with quality BAD or MISSING is
never silently treated as GOOD. See Phase 2 for how quality is assigned; this
phase consumes it.

## 5. Terminology

- **Capability model:** the time-indexed feasible operating set `X(i,t)` of an asset (§11.1). Not a single `max_mw` field.
- **Flexibility envelope:** the constrained portfolio aggregation (§11.6). Not the sum of nameplates.
- **Schedule:** a versioned, per-asset per-interval setpoint plan for a horizon, with a declared objective and input snapshot (§12.1, §12.6).
- **Optimization tier:** Tier 1 LP/QP/MILP; Tier 2 continuous SOCP; Tier 3 MISOCP/MINLP; Tier 4 nonlinear AC-OPF (§12.3). Tiers are problem classes, not quality rankings.
- **Candidate:** a schedule or optimization result that has not yet passed Phase 4 feasibility, authorization, or dispatch. Never call a candidate "feasible" in this phase.
- **SUPERSEDED:** a schedule revision invalidated by a newer approved revision, retained for audit (§14.2). Distinct from CANCELLED.
- Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).

## 6. Constraints (simulation-only 0.x)

- The entire 0.x line is simulation-only (AGENTS.md rule 5). No command path, no device writes, no operator-approved control in this phase.
- No safety or certification claims; no protection-grade claims (AGENTS.md rules 2–3). FlexCore does not perform primary frequency response, synthetic inertia, relay protection, or sub-cycle control.
- Solver classes must never be conflated (AGENTS.md rule 4): HiGHS is LP/QP/MIP, not SOCP; Clarabel is convex conic, not MISOCP; AC power flow is not AC-OPF; IPOPT is local, not global.
- Research cut-off for external claims is 7 November 2026 (AGENTS.md §7). Versions in RESEARCH.md are checked as of 2026-11-07.
- No hallucinated facts: any external claim needs a primary source or an explicit ASSUMPTION label.

## 7. Prior-phase outputs consumed

Phase 2 (Stage 2 — asset and telemetry foundation) is assumed to provide: tenant/
portfolio/site/asset registry; canonical telemetry model with quality flags;
simulator telemetry source; current-state and historical views; forecast inputs
(or synthetic forecasts with labelled provenance). If a Phase 2 output is missing
or differs from this description, record the gap in the handover note (P3.30)
instead of rebuilding it silently. This ledger does not contain Phases 1–2
ledgers; the report (§23 Stages 1–2) is the authority for what Phase 2 delivered.

## 8. D1–D6 relevance

| Gate | Status | Effect on Phase 3 |
|---|---|---|
| D1 target jurisdiction/market | OPEN | No market adapter; prices/commitments are synthetic and labelled (P3.14, P3.16) |
| D2 first portfolio/service | OPEN | Synthetic portfolio only; baseline method is a named synthetic rule (P3.10) |
| D3 network class/model fidelity | OPEN | Balanced radial default; unbalanced three-phase enters from v0.4.0; meshed transmission out of scope |
| D4 license/dependency audit | OPEN (before v0.1.0) | Solver pins (HiGHS MIT, Clarabel Apache-2.0, CVXPY Apache-2.0) feed the audit; record licenses in RESEARCH.md |
| D5 first protocol/asset mix | OPEN (before v0.4.0) | No protocol adapter work in this phase |
| D6 cyber/OT boundary review | OPEN (before any read-only pilot) | No pilot, no OT boundary crossed; P3.29 is a code-level review only |

Recommended defaults are planning aids, not decisions (`.agent/DECISIONS.md`).

## 9. Open questions

1. Interval length and horizon for the default schedule: proposed 15-minute intervals over 24 hours (P3.03) — confirm with product; it is configuration, not a report requirement.
2. Which Tier 1 formulation (LP vs MILP) is the default for the §24 vertical slice? Proposed: LP first, MILP where a discrete decision is needed (P3.17) — confirm at implementation.
3. Degradation cost: no verified model is on record; placeholder linear term with default zero (P3.06) — confirm that a zero default is acceptable for v0.2.0.
4. Correlation treatment in the envelope (P3.12): conservative derating vs explicit correlation model — to be decided with power-systems review; either way it must be documented, not assumed.
5. Solver language split: HiGHS via highspy in the Python worker vs a Rust binding — decide at P3.17 implementation time; the port makes the choice replaceable.
6. Objective-term weights for the synthetic default (P3.14) — product to confirm; weights are configuration either way.
