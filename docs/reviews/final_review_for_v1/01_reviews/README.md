# 01_reviews — Review Pass Documents

**Iteration 1 status: COMPLETE.** Eight review areas, four waves of two
agents, static read only. Each document records its scope, method, the
cross-boundary checks that PASSED (diligence evidence), and its numbered
findings list (`FRV1-R<N>-NNN`, severity P0/P1/P2, `[phase-NN]` tag on every
finding).

**Program totals: 5 P0 · 35 P1 · 38 P2 = 78 findings**, all mapped into
`../02_ledgers/` (see `../02_ledgers/ledger_master.md`).

| Doc | Area | Status | P0 | P1 | P2 |
|---|---|---|---|---|---|
| review_R1_core_domain_application.md | Core domain, application services, events, storage | REVIEWED | 0 | 4 | 6 |
| review_R2_control_plane_lifecycle_authz_audit.md | Control plane, worker, authz, audit, lifecycle | REVIEWED | 0 | 4 | 7 |
| review_R3_operator_console.md | Operator console (TypeScript) | REVIEWED | 1 | 6 | 4 |
| review_R4_python_numerical_workers.md | Python numerical workers | REVIEWED | 2 | 4 | 6 |
| review_R5_grid_api_persistence_simulation.md | Grid API, persistence, NATS, simulator, optimization API | REVIEWED | 1 | 3 | 5 |
| review_R6_protocols_edge_plugin_kernel.md | Protocol adapters, edge agent, kernel, plugin host | REVIEWED | 0 | 3 | 4 |
| review_R7_market_settlement_ev_intelligence.md | Market/settlement, EV, forecasting, asset intelligence | REVIEWED | 1 | 5 | 4 |
| review_R8_security_identity_ota_deployment_release.md | Security, identity, OTA, deployment, release readiness | REVIEWED | 0 | 6 | 2 |

Review rules recap: static read only (no build/test/install); findings cite
`file:line`; cross-module signature checks are done by grepping definition and
call sites; no finding is ever marked "verified by running".
