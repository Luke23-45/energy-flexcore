# Final Review for v1 — Initial Public Release Gate

**Program:** Final static review of every implemented feature before the first
public release ("initial public release").
**Created:** 2026-11-17.
**Status:** **COMPLETE (written) — iteration 1 review pass + four fix waves:
78 findings (5 P0 / 35 P1 / 38 P2); 77 of 78 fixed (written, awaiting human
build/test); the single OPEN item is FRV1-R8-003 (LICENSE / decision D4), a
recorded HUMAN GATE.**

---

## 1. What this program is and is not

This is a **manual, senior-principal-engineer review** of everything the
phases 00–20 program actually wrote. Its goal is a **strong initial release**:
the release does not need every feature, but whatever ships must be coherent,
honest, and free of incomplete implementations, missing small pieces, and
unpolished surfaces.

**Hard boundary (unchanged from the main program):**

| Rule | Meaning |
|---|---|
| No build | No `cargo build/check/run`, no `tsc`, no `vite`, no `buf` |
| No test execution | No `cargo test/clippy/fmt`, no `pytest`, no `vitest` |
| No installation | No `cargo add/install`, `pip install`, `npm install`, docker pulls |
| Review is static read | Findings cite `file:line`; nothing is "verified by running" |
| Everything stays WRITTEN-UNRUN | The first human `cargo build --workspace` remains the real gate |

Cross-module signature and type mismatches are found by **static
cross-checking** (grepping the defining site and every call site), which is the
review equivalent of a compile check — it is recorded as a static finding, never
as "compiles".

## 2. Directory layout

```text
docs/reviews/final_review_for_v1/
  README.md            This file: methodology, iteration protocol, status.
  01_reviews/          The review pass. One document per review area, each
                       written like a senior principal engineer's file-by-file
                       pass, ending in a numbered findings list.
  02_ledgers/          The fix program. ledger_master.md tracks progress per
                       phase; ledger_phase_NN.md (00–20) carries the per-phase
                       fix subtasks derived from the findings.
```

## 3. Review areas (01_reviews/)

Eight review areas cover the whole implementation. Each review document is
produced by one agent per wave, two agents running concurrently, disjoint
ownership.

| Area | Document | Scope | Phase coverage |
|---|---|---|---|
| R1 | `review_R1_core_domain_application.md` | flexcore-contracts, flexcore-domain, flexcore-ports, flexcore-application (core services), flexcore-events, flexcore-storage | 00, 01, 02, 03 |
| R2 | `review_R2_control_plane_lifecycle_authz_audit.md` | apps/control-plane, apps/worker (Rust), flexcore-authz, flexcore-audit, command lifecycle wiring | 04, 09, 10, 11 |
| R3 | `review_R3_operator_console.md` | console/ (TypeScript/React/Vite): API clients, auth, live SSE, components, security | 05, 13 |
| R4 | `review_R4_python_numerical_workers.md` | apps/worker/python (forecast, gnn, marl, andes, portfolio_analytics, drill, reproducibility, helics) | 03, 08, 15 |
| R5 | `review_R5_grid_api_persistence_simulation.md` | flexcore-grid-api, adapters/postgres (incl. migrations), adapters/nats, flexcore-simulator, flexcore-optimization-api, apps/sim-runner | 01, 02, 03, 04, 12 |
| R6 | `review_R6_protocols_edge_plugin_kernel.md` | 8 protocol adapters, flexcore-protocol-ports, edge-agent, flexcore-kernel, apps/plugin-host, apps/worker edge pieces | 06, 07, 14, 18 |
| R7 | `review_R7_market_settlement_ev_intelligence.md` | market/settlement application modules, EV fleet charging, forecasting integration, BESS degradation/SoH/warranty, fleet health | 15, 16, 17 |
| R8 | `review_R8_security_identity_ota_deployment_release.md` | security/, deploy/, OTA/update stack, identity lifecycle, backup/DR, docs accuracy, release readiness cross-cutting | 10, 12, 13, 18, 19, 20 |

## 4. Findings vocabulary

**Issue ID:** `FRV1-R<N>-NNN` (FRV1 = this review program, R<N> = area, NNN
sequential within the area, zero-padded to 3).

**Severity:**

| Class | Meaning | Release rule |
|---|---|---|
| P0 | Release blocker: would-not-compile class, broken frozen invariant, honesty violation, security defect, data-loss path | Must be fixed (written) before release |
| P1 | Important: missing implementation, unwired seam, API/client mismatch, missing error path, stale reference | Must be fixed or explicitly accepted with a recorded reason |
| P2 | Polish: docs, naming, consistency, dead code, comment drift | Fix if cheap; may ship with an accepted note |

**Finding status:** `OPEN` → `FIXED (written, awaiting human build/test)` →
(release time) human disposition. No finding is ever closed as "tested" or
"works" — builds and tests are the human gate.

**Phase tag:** every finding carries a `[phase-NN]` tag so it maps to exactly
one per-phase ledger in 02_ledgers/.

## 5. Ledger vocabulary (02_ledgers/)

- `ledger_master.md` — one row per phase (00–20): findings count by severity,
  fix subtasks ticked, status. This is the single progress tracker.
- `ledger_phase_NN.md` — per-phase fix subtasks (`FRV1-P<NN>.<xx>`), ticked
  only as **"fixed, awaiting human build/test"** with a citation to the
  finding that spawned it. No `todo!()`/`unimplemented!()`/TODO/FIXME
  placeholders in any fix.
- Fix iterations are waves of two agents with disjoint file ownership, exactly
  like the main program. A fix touching a shared composition root follows the
  marked-block protocol.

## 6. Iteration protocol

1. **Iteration 1 (review pass):** eight review areas, four waves of two
   agents. Output: findings lists in 01_reviews/, findings mapped into
   per-phase ledgers in 02_ledgers/.
2. **Iteration 2+ (fix passes):** work the ledgers P0 → P1 → P2, two agents
   per wave, ticking subtasks as "fixed, awaiting human build/test".
3. **Re-review between iterations:** each fix wave's completion is
   cross-checked statically (the same three permitted verification tools plus
   targeted grep sweeps) before its ticks are applied.
4. **Release gate (human):** the first human `cargo build --workspace`, then
   the human test gates from every phase CHECKLIST — none of which this
   program may execute.

## 7. Standing honesty rules (carried from the main program)

- Every E-decision (P11-E-1 … P20-E-3) stays OPEN; agents never close them.
- Frozen invariants stay byte-identical: 14-state/27-edge command lifecycle,
  never-granted permission set, 26-event vocabulary, `deny.toml`, kernel
  plugin-rollout policy, telemetry quality states, canonical units.
- ADR-010 extension holds: forecasts/health scores/degradation prices are
  inputs, never commands.
- No new dependencies; no solver-class creep (HiGHS LP/QP/MIP, Clarabel conic).
- Production crypto stays a deployment requirement; test-grade primitives stay
  labeled test-grade.
