# ENTRY NOTE — Phase 5: Operator Console and Simulator Integration

Date: 2026-11-11. Coordinator: implementation-coordinator. Loop rules in
force: nothing installed, nothing built (no cargo build/check, no npm/tsc/vite
build), no tests run — complete code + test files written only. Ledger ticks
are "implemented, awaiting human build/test", never "tested/verified/passed".

## Gate exception (same pattern as Phases 1–4)

Stage 4 exit gate (§23 Stage 4) is **NOT HELD**: P4.27's evidence pack is
UNRUN and its sign-off table is NOT SIGNED. The exception is recorded in
`ledger/phase-04-grid-feasibility-dispatch/32_handover_phase5.md` §4. Phase 5
starts on the recorded exception; its own Stage 5 gate (P5.30) requires the
human's build/test pass + demonstration.

## Hard boundaries Phase 5 must respect (from Phase 4 handover)

1. **Simulation-only**: every dispatch is to the simulator; no device write
   path exists; acknowledgement ≠ delivery. P5.13 enforces the console-side
   labelling; P5.27 enforces control-authority mode in the console.
2. **Console stack**: React + TypeScript + Vite (§7.5, provisional §25.1).
   Versions in `RESEARCH.md` are SECONDARY-SOURCE pins — package.json pins
   them as recorded, marked re-verify-at-first-install. **npm install is
   banned in this loop** — no node_modules, no lockfile generated here.
3. **No generic execute endpoint** (§15.2) — the console consumes the read
   APIs + gate-safe transitions from P4.30 (`routes_phase4.rs`).
4. **No safety/certification/protection-grade claims**; shadow agreement is
   not evidence for field dispatch (ShadowReport carries the statement).
5. **Alarm contract gap** — P5.02 records it, P5.31 resolves it (minimal
   model or formal escalation). Do not invent alarm semantics in P5.02.
6. HELICS 3.7.0 (RESEARCH.md) — pinned, NOT installed; integration written
   against the documented API, honest about unverified behaviour.
7. D1–D6 stay OPEN; pandapower 3.5.6 and HiGHS 1.15.1 remain recorded pins.

## Wave plan (max 2 subagents; disjoint file ownership per batch)

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | this file, HANDOVER.md plan | Gate exception + boundaries |
| 1a | P5.01, P5.25 | `console/**` (NEW scaffold: package.json pinned versions, tsconfig, vite config, app shell, routing, workspace stubs), SSE/WebSocket server-side in `apps/control-plane/src/` (additive `routes_live.rs`) + client hooks | Scaffold before all workspaces; live updates before ordering (P5.33) |
| 1b | P5.14, P5.15 | `crates/flexcore-simulator/**` (adapter integration), `crates/flexcore-grid-api` + worker (pandapower adapter wiring — additive), scenario fixtures | Rust side independent of console scaffold |
| 2a | P5.02, P5.03, P5.04 | console Overview + Portfolios/Sites/Assets + Grid workspaces | Depends on P5.01 |
| 2b | P5.05, P5.06, P5.09 | console Schedules + Dispatch + Audit workspaces | Depends on P5.01 |
| 3a | P5.07, P5.31 | console Market/Plugins/Admin workspaces; alarm model contract (`flexcore-contracts` additive) | P5.31 resolves P5.02's gap |
| 3b | P5.16, P5.17 | HELICS integration (worker + docs), scoping decision record | Depends on P5.14–15 |
| 4a | P5.18, P5.19, P5.20 | scenario definition/replay (`apps/sim-runner` additive), reproducibility, library | Depends on P5.14–16 |
| 4b | P5.21, P5.22, P5.23, P5.24 | failure injection (reuse P4.31 pattern), visibility, benchmark + load measurement TEMPLATES (EMPTY cells) | Depends on P5.14–16 |
| 5a | P5.10, P5.11, P5.12 | console design-principle pass across workspaces | Depends on waves 2–3 |
| 5b | P5.13, P5.26 | simulation-honesty layer, console security (RBAC/step-up) | Depends on P5.06/P5.09 |
| 6a | P5.27, P5.32, P5.33 | mode enforcement, tenant display/switch guard, live-update ordering/dedup | Depends on P5.13/26/25 |
| 6b | P5.28, P5.29 | CI quality gates for the console (additive ci.yml), operator guide | Depends on P5.01/25 |
| 7 (coordinator) | P5.30 | exit-gate demonstration record (UNRUN) + handover to Stage 6 | All P5.01–P5.33 |
