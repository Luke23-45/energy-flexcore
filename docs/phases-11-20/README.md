# Program v2 — Phases 11–20: Robustness and Enterprise Feature Expansion

Date: 2026-11-14. Status: **IMPLEMENTED (waves complete) — awaiting human
build/test** — 196/196 subtasks ticked across the ten ledgers as of 2026-11-17.
Everything is WRITTEN-UNRUN: the first human `cargo build --workspace` of the
0–10 base remains the program's real gate (hard status boundary 1, below).
Implementation followed the two-agent wave loop with per-batch decision
records; the program-level final audit is `02_PROGRAM_FINAL_AUDIT.md`. This directory is the planning baseline for the successor program to
the Phase 0–10 implementation (381 subtasks, 366 ticked, everything
WRITTEN-UNRUN — see `ledger/MASTER_LEDGER.md` and
`ledger/phase-10-v1-release-hardening/31_phase_exit_gate_record.md`).

## What this program is

A gap-driven roadmap that answers one question honestly: *what important
features is FlexCore missing for it to be a credible professional / enterprise
VPP platform?* The answer was produced by seven research tracks (one internal
repository inventory, six deep online research tracks, ~180 distinct web
searches, primary sources fetched where reachable), synthesized in
`00_GAP_ANALYSIS.md`, and converted into ten new phases:

| Phase | Theme | Subtasks |
|---|---|---|
| 11 | Operational Robustness and Observability | 22 |
| 12 | Survivability: Backup, DR and Resilience Engineering | 16 |
| 13 | Enterprise Identity, Administration and Integration Platform | 22 |
| 14 | Protocol and Device Connectivity Expansion | 24 |
| 15 | Forecasting and Asset Intelligence | 20 |
| 16 | Market Integration, Settlement and Revenue | 24 |
| 17 | EV Fleet and Charging Integration | 18 |
| 18 | Edge Fleet Lifecycle and OTA Updates | 16 |
| 19 | Security Certification and Compliance Readiness | 18 |
| 20 | Deployment Architecture and Scale Readiness | 16 |

Total: 196 subtasks. Ordering rationale and the dependency graph are in
`00_GAP_ANALYSIS.md` §9.

## How to read this directory

1. `README.md` (this file) — what the program is and its honest status.
2. `00_GAP_ANALYSIS.md` — the review: what exists, what is missing, why each
   missing capability matters (with source citations), and the phase mapping.
3. `01_PROGRAM_CONVENTIONS.md` — the rules every phase inherits (loop bans,
   tick discipline, decision register, honesty rules).
4. `phase-NN-<slug>/context.md` — why the phase exists, what exists today, the
   constraints it operates under.
5. `phase-NN-<slug>/plan.md` — scope, subtask outline, wave plan, risks,
   decision points (recorded OPEN with DEFAULT/RECOMMENDATION labels).
6. `phase-NN-<slug>/ledger_phase_NN.md` — the subtask register, all boxes
   unticked until subtasks are implemented under the standing discipline.

## Hard status boundaries (do not lose these)

1. **Nothing in the Phase 0–10 codebase has ever been compiled, tested, or
   run.** The first human `cargo build --workspace` is still the program's
   actual gate and remains open. Every phase 11–20 plan assumes that base
   compiles; that assumption is itself a recorded program-level risk.
2. **This directory is PLANNING ONLY.** No code, no crates, no manifests are
   created by this program's planning step. New crates named in plans are
   proposals; they become workspace members only during implementation, with
   the additive `check_dependency_rule.py` update that requires.
3. **No install, no build, no test run, no tool download** applies to the
   implementation of these phases exactly as it did for Phases 0–10.
   Implementation will follow the same two-agent wave loop, ledger ticks
   ("implemented, awaiting human build/test"), and per-phase close records.
4. **The release ladder does not move.** Simulation/shadow ceiling,
   operator-approved control by default at v1.0.0 with operator approval,
   bounded automation post-1.0 per deployment/site/asset class — unchanged.
   ADR-010 (research modules have no dispatch authority) is inherited
   verbatim and extended, not relaxed.
5. **Human gates stay human.** All 15 open P0/P1 criteria from Phases 0–10,
   D1–D6 signed closures, and every run/scan/sign-off/date in phases 11–20
   are human gate inputs. New decision points are registered as E-decisions
   (see conventions) and are never closed by agents.

## Research basis and honesty

All research was performed 2026-11-14. Sources are listed per claim in
`00_GAP_ANALYSIS.md` §11. Items that could not be verified from a primary
source are marked UNVERIFIED in the gap analysis and in the phase contexts
that inherit them. The `docs/plan` report's own research cut-off note
(7 November 2026) applies to that report; this directory is a separate,
later-reviewed planning baseline and says so here rather than editing the
report.
