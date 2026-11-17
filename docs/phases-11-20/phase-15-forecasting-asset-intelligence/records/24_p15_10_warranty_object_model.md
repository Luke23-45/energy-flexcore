# P15.10 — Warranty object model (domain)

| Field | Value |
|---|---|
| Subtask | P15.10 — capacity-retention schedule, cycle/throughput limits, RTE/availability commitments, augmentation events; warranty-headroom computation (remaining allowance vs consumed counters from P15.07) — Megapack CMA and Fluence Dispatchable Energy Guarantee as reference STRUCTURES, no vendor conformance claims [S-150][S-151][S-152][S-153] |
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11) |
| Status | WRITTEN-UNRUN: the domain module and its tests are written, never compiled, never executed. No warranty was loaded from a contract, no headroom was computed against a real fleet, no augmentation occurred. |
| Sources | [S-150] Tesla Megapack; [S-151] Fluence 25-year guarantees (Dispatchable Energy Guarantee class); [S-152] Avantis BESS warranty guide (throughput/cycles/years); [S-153] Sunlith C&I BESS warranty guide. **All four are cited as REFERENCE STRUCTURES for the FIELD SET only** — the texts were not consulted; every semantic below is UNVERIFIED against the primary sources; no vendor conformance is claimed and no warranty value is invented (every term is declared input with a citation). |
| Files touched | `crates/flexcore-domain/src/bess_warranty.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). |

## 1. What landed (deliverable class: domain object model + pure computation)

| Piece | Where | Honest status |
|---|---|---|
| `WarrantyTerms` — warranty id, validity window (strictly ordered), capacity-retention schedule (years 1-based, strictly ascending, retention fractions in [0,1]), optional `max_full_equivalent_cycles` and `max_cumulative_throughput_mwh` (positive, finite), optional `RteCommitment` and `AvailabilityCommitment` (declared fractions with REQUIRED declared measurement bases), REQUIRED citation, provenance | `bess_warranty.rs` | Implemented; UNRUN |
| `resolve_retention(elapsed_years)` — the schedule holds flat between points (the reference structure's reading, UNVERIFIED [S-150]); beyond the last declared year the LAST point applies and `beyond_schedule_coverage` is flagged — the schedule is NEVER extrapolated with an invented stricter value | same | Implemented; UNRUN |
| `AugmentationEvent` — timestamp, strictly positive added usable energy (MWh), audit-safe note, provenance — a RECORD of what was declared to have happened, never a physical action this platform took | same | Implemented; UNRUN |
| `compute_headroom(terms, ledger, now, soh_config)` — the pure headroom computation: window status (`Within`/`Expired`/`NotYetEffective`), capacity-retention headroom (scheduled minimum vs the latest CREDITABLE SoH observation via the P15.09 gate), cycle headroom (limit vs the P15.07 FEC counter), throughput headroom (limit vs the P15.07 cumulative-throughput counter); `chemistry_in_scope` (P15-E-3) recorded on every report; RTE and availability carried as DECLARED-ONLY records (no verdict computed — recorded limitation) | same | Implemented; UNRUN |
| `HeadroomStatus` — `NotConfigured` / `NotCreditable { reason }` / `Within { remaining, unit }` / `Exceeded { overage, unit }` / `Evaluated(CapacityRetentionHeadroom)`; a negative margin (observed SoH below the scheduled minimum) and an exceeded allowance are RECORDS for the operator — never a penalty, a claim, or a dispatch input | same | Implemented; UNRUN |
| Creditability enforcement: headroom computed against UNKNOWN-provenance counters or a STALE SoH report is `NotCreditable` with the recorded reason (P3.02 / P15.09 — unknown narrows, stale is never silently used) | same | Implemented; UNRUN |
| `WARRANTY_REFERENCE_STRUCTURE_STATEMENT`, `WARRANTY_HEADROOM_STATEMENT`, `WARRANTY_YEAR_MILLIS` (the Julian-year convention, declared, not physics) — carried in code constants, not comments | same | Implemented; UNRUN |
| Tests: schedule resolution (between points, at points, beyond coverage flag), term validation refusals, headroom arithmetic (within/exceeded, exact margins), non-creditability paths (no SoH, unknown provenance, stale SoH), window status, declared-only RTE/availability, augmentation validation | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. Reference structures — the honesty line

The Megapack CMA [S-150] and the Fluence Dispatchable Energy Guarantee
[S-151] informed the FIELD SET (retention schedule over years; cycle and
throughput limits [S-152][S-153]; augmentation as a recorded event). The
statement `WARRANTY_REFERENCE_STRUCTURE_STATEMENT` travels with the module
and the P15.11 pack so no reader can mistake the object model for a
conformance claim: texts not consulted, semantics UNVERIFIED, no conformance,
no invented values.

## 3. What this module is NOT

1. **Not a claim engine.** Headroom SUPPORTS human warranty review; it
   never asserts a claim (the P15.11 pack export carries the same boundary).
2. **Not a verdict computer for RTE/availability** — the commitments are
   DECLARED DATA; the P15.07 ledger accumulates no efficiency or
   availability measurement (recorded limitation; availability KPIs are a
   wave-2 P15.12 concern).
3. **Not a dispatch authority** (ADR-010): a headroom record never
   authorizes or produces a dispatch command.
4. **Not a schedule extrapolator**: beyond-coverage evaluation is flagged,
   never extended.

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The "schedule holds flat between points" reading of capacity-retention schedules [S-150] — UNVERIFIED | `resolve_retention` |
| 2 | The Julian-year (365.25 d) warranty-year convention — a declared convention, not a contractual calendar | `WARRANTY_YEAR_MILLIS` |
| 3 | Whether real warranty terms measure throughput as (charge + discharge) MWh or discharge-only — UNVERIFIED; the P15.07 cumulative-throughput counter (charge + discharge) is the declared comparator | `compute_headroom` throughput arm |
| 4 | The augmentation event's exact contractual role [S-151] — UNVERIFIED; recorded as declared data only | `AugmentationEvent` |

## 5. Verification (permitted tools)

Outputs quoted verbatim in `26_phase_15_wave1_agent_b_close_out.md`. No
build, no test run (conventions §1.3).
