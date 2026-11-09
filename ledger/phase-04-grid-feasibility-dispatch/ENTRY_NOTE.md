# Phase 4 — Entry note (grid feasibility + dispatch assurance)

Date: 2026-11-07.

## Entry conditions

- **Stage 3 exit gate NOT held** (Phase 3 close-out records; demonstration =
  the human's, loop bans). Same loop-rules exception as Phases 2–3:
  implementation proceeds; the gate must be recorded before any release
  claim. Gate debt: Stage 0, 1, 2, 3.
- **Phase 3 code complete** (P3.01–P3.28 implemented; P3.27 + close-out
  finishing). The P3.30 handover pack is the coordinator's final Phase 3
  deliverable; Phase 4 subtasks that name it as a prerequisite (P4.01)
  consume the contracts directly — the handover pack is an index of
  already-frozen artefacts, and its pending status is recorded HERE as the
  exception, per the established pattern.
- Consumed from Phase 3: schedule identity/revision + `is_revision_superseded`
  (P3.22), envelope version + staleness gates (P3.12/13), §12.6 result
  records (P3.21), comparison records (P3.23), solver/provider pins,
  Tier guard, `RelaxationDecision`.
- D3 (network class) remains OPEN — pandapower is the RECOMMENDED DEFAULT
  primary provider (P4.01), never a D3 decision.

## Phase-specific discipline

- A solved schedule is a CANDIDATE until THIS phase's feasibility gate says
  otherwise; validation, feasibility, and authorization are DISTINCT gates
  (§14.1) — never conflated, separate evidence references.
- AC power flow ≠ AC-OPF; pandapower = balanced AC power flow with some
  three-phase/asymmetric calculations, NOT a universal validator (§12.4).
- Simulation + shadow only (v0.3.0). The dispatch adapter's ONLY target is
  the simulator; acknowledgement ≠ physical delivery; NO device write path;
  no hard-real-time claim (§14.5).
- Fail-closed everywhere: validator failure, stale model, missing telemetry
  → reject/quarantine, never silently proceed (§17.5 inv. 9).
- The command state machine (P0.11 frozen 14 states, 27 edges) is the ORACLE:
  P4.33's exhaustive matrix test uses the Phase 0 table as the test oracle.
