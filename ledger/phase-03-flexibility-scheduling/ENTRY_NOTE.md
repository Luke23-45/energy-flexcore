# Phase 3 — Entry note (flexibility + deterministic schedule)

Date: 2026-11-07.

## Entry conditions

- **Stage 2 exit gate NOT held** (`../phase-02-asset-telemetry/26_exit_gate_record.md`
  = NOT RUN; `29_exit_gate_review.md` = NOT HELD). Same loop-rules exception as
  Phase 2: implementation proceeds; the gate must be recorded before any release
  claim. Gate debt: Stage 0, 1, 2.
- Phase 2 outputs are consumed, not rebuilt: asset registry, canonical telemetry
  + quality, events/outbox, simulator + scenario-v2, corrections/as-of.
- **Forecast boundary** (`../phase-02-asset-telemetry/28_handover_pack.md` §3):
  no Phase 2 forecast exists; Phase 3 owns the synthetic forecast fixture
  (P3.31) and nothing consumes an unlabelled/unversioned forecast.
- **D1–D3 remain OPEN** (`.agent/DECISIONS.md`): all market/PCC data in this
  phase is SYNTHETIC and labelled; no baseline method is decided; defaults are
  recorded as recommendations, never decisions.

## Phase-specific discipline

- Solver-class discipline (P0.13 / AGENTS.md §3.4) is absolute: HiGHS =
  LP/QP/MIP, never SOCP; Clarabel = convex conic incl. SOCP, no integer
  variables, never MISOCP; integer input to Tier 2 returns `unsupported`.
- A solved schedule is a CANDIDATE — never "feasible" (feasibility is Phase 4).
- Schedules are immutable; revisions supersede (§14.2).
- Tier 3/4 are NOT built (P3.20); requests return `unsupported`.
- Units MW/MVAr/MWh, `P > 0` = export, SOC [0,1]; kW/kWh rejected at the
  contract boundary.
