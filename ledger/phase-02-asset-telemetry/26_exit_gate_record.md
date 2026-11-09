# P2.26 — §23 Stage 2 exit-gate demonstration record

Date: 2026-11-07. Outcome: **NOT RUN — BLOCKER RECORDED** (same class as the
Phase 1 record, `ledger/phase-01-rust-foundation/26_exit_gate_record.md`).

## Why not run

The implementation loop bans running services, building, or executing tests.
The Stage 2 gate requires a live demonstration on a clean Compose stack
(register → simulate 24 h → ingest → query → replay). Everything the
demonstration needs is WRITTEN and ready; the run itself is the human's.

## What is ready (all implemented, unbuilt, unrun)

| Gate element | Artefact | Location |
|---|---|---|
| Exit portfolio (≥2 sites × BESS/PV/load/EVSE/meter) | scenario-v2 with expected-quality plan | `scenarios/synthetic-portfolio/scenario-v2.json` |
| Registration via API (never SQL) | runbook command sequence | `ledger/phase-02-asset-telemetry/25_exit_gate_runbook.md` |
| Worker simulator-source composition | `compose_ingestion` wiring | `apps/worker/src/main.rs` |
| Ingestion pipeline (validate → normalize → store → publish) | 7-stage pipeline | `apps/worker/src/ingestion.rs` |
| Current-state query | `GET /api/v1/assets/{id}/state` | `apps/control-plane/src/routes_phase2.rs` |
| Historical query (raw + bucketed) | `GET /api/v1/telemetry/history` | same file; D-H1…D-H6 |
| Coverage + fault accounting | expected-gap vs unexplained-gap tests | `apps/control-plane/tests/exit_portfolio.rs` |
| Deterministic replay | `sim-runner run` / `replay`, verdict report | `apps/sim-runner/` |
| Corrections + as-of (foundations §6.4) | versioned rows, `as-of` subcommand, `as_of` API param | migration 0012, `corrections.rs`, `as_of.rs` |
| Fault variant (P2.15 plan) | 8-case fault suite | `apps/worker/tests/fault_suite.rs` |

## Demonstration sequence (when a human runs it)

Execute `ledger/phase-02-asset-telemetry/25_exit_gate_runbook.md` on a clean
`docker compose -f deploy/compose/docker-compose.yml up --build` stack,
then record HERE: commands, outputs, timings, counts, versions (toolchain,
PostgreSQL, NATS, scenario version + seed, code version), and the observed
ingestion rate + query latencies. Numbers are OBSERVATIONS, not targets —
no performance target may be claimed (§19.4). Re-run by a second person is
the gate's verification method.

## Blocking status for later phases

Per the loop-rules exception recorded in `ENTRY_NOTE.md`, Phase 3
implementation proceeds; the Stage 2 gate must be recorded (this file, with
a real run) before any release claim. Carried with the Phase 0/1 gate debt.
