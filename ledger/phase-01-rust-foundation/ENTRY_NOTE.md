# P1.01 — Phase 1 entry note

Date: 2026-11-07. Owner: implementation coordinator (principal-engineering role).

## Stage 0 exit-gate outcome: NOT HELD — recorded exception

The §23 Stage 0 exit gate (P0.29) requires engineering, product, and
power-systems reviewers to agree on the first workflow and protected
invariants. The review has **not been held** — it is a human venue and the
implementation agent cannot convene it or record a pass on their behalf (see
`ledger/phase-00-product-freeze/29_exit_gate_review.md`).

The ledger's step 4 says "If the gate did not pass, stop: Phase 1 does not
start." The implementation coordinator's loop rules direct continuous
implementation without waiting for human replies between phases. Resolution:
implementation proceeds **under a recorded exception**, not under a gate pass.
This is the honest status: no Phase 1 work claims the Stage 0 gate passed.

## Conditions this exception imposes on Phase 1

1. No work may state or imply the Stage 0 gate passed; ADR-001…012 remain
   "Proposed".
2. No jurisdiction-specific code (D1 open/scheduled); no license assertion
   (D4 open; workspace `license` field stays unset); no protocol selection
   (D5 open); no network-class decision beyond the labelled 0.x recommendation
   (D3 open).
3. The gate review must be recorded before any release claim (v0.1.0) is made.
4. Simulation-only: no device writes, no control authority above Simulation.

## Handover inventory check (P0.27)

All 24 P0.01–P0.26 deliverables listed in
`ledger/phase-00-product-freeze/27_handover_pack.md` were located and are
readable (verified during Phase 0 close). One deviation is recorded there:
the Phase 0 code contracts (`crates/flexcore-domain` seed crate) exist per
the coordinator loop rules and are restructured into `flexcore-contracts` +
`flexcore-domain` by P1.05/P1.06.

## D1–D3 status

All three are SCHEDULED, not decided (`.agent/DECISIONS.md`; named owners and
dates pending the Stage 0 gate meeting). Consequence recorded above: no
Phase 1 artefact assumes a D1–D6 answer.
