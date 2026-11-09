# P2.01 — Phase 2 entry note

Date: 2026-11-07. Owner: implementation coordinator (principal-engineering role).

## Stage 1 exit-gate outcome: NOT HELD — recorded exception (same as Phase 1's entry)

The §23 Stage 1 exit gate (P1.31) requires human reviewers plus a live or
freshly recorded P1.26 end-to-end demonstration. Neither exists: the gate
was not held, and the P1.26 demonstration was NOT run (see
`ledger/phase-01-rust-foundation/26_exit_gate_record.md` — runbook prepared,
all acceptance criteria open). Per the implementation coordinator's loop
rules, Phase 2 implementation proceeds **under a recorded exception**, not a
gate pass. No Phase 2 work may state the Stage 1 gate passed.

Conditions carried forward from Phase 1 (unchanged):

1. ADR-001…012 remain "Proposed"; the Stage 0 gate is also unheld.
2. No jurisdiction-specific code (D1); no license assertion (D4); no
   protocol selection (D5); network class stays the labelled 0.x
   recommendation (D3).
3. Simulation-only: the simulator telemetry source is a stand-in, not a
   site integration; no device writes; no control authority above Simulation.

## Handover inventory check (P1.29)

Artefacts verified present as written (workspace/toolchain pins, ten
crates, two apps, adapters, migrations, Compose stack, CI, security/fault
tests, observability/config/probes): see
`ledger/phase-01-rust-foundation/29_handover_pack.md`. The P1.26 re-run
required by this subtask cannot be executed under the loop bans; it is
recorded as NOT RUN — the Phase 2 exit gate (P2.26) must not be claimed
until both the Stage 1 and Stage 2 demonstrations run.

## Implemented-API-subset gap (P1.29 → P2.12)

Phase 1 implements ONLY: `POST/GET /api/v1/tenants`, `/portfolios`,
`/sites`, `/assets` (create/list/get). Phase 2 must add: full CRUD per the
P2.02 deletion rule, `GET /api/v1/telemetry/query`,
`GET /api/v1/assets/{id}/state`, and the quarantine read endpoint — all
inside the P0.15 16-resource list.

## D1–D6 status

All six remain SCHEDULED, not decided (`.agent/DECISIONS.md`). No Phase 2
artefact assumes a D1–D6 answer; `market_region` stays a label (P2.02),
and the baseline-method question remains OPEN pending D1/D2.
