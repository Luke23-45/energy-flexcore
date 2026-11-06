# Iteration 01 — Build pass (initial ledger creation)

- Date: 2026-11-07
- Angle: initial construction — all 11 phases drafted from the report (§2, §22, §23, §24, §25) and foundations conventions, with per-phase online research against primary sources.
- Status: applied (this iteration IS the build; findings below are the issues the four phase builders recorded during construction, not a post-hoc review).

## What was built

- 11 phase directories, 5 files each (55 files): `ledger_phase_N.md`, `PLAN.md`, `CONTEXT.md`, `RESEARCH.md`, `CHECKLIST.md`.
- Subtask totals at end of iteration 01 (counted by "Acceptance criteria" lines): P0 30, P1 32, P2 30, P3 30, P4 32, P5 30, P6 30, P7 32, P8 30, P9 30, P10 30 — **332 subtasks**.
- Every subtask carries ID, objective, report trace (§), inputs, steps, deliverable, testable acceptance criteria, owner role, dependencies, verification method, risks. Every phase has a CHECKLIST.md mirroring its §23 exit gate.

## Findings recorded during the build

| # | Severity | Item | File / subtask | Applied / deferred |
|---|---|---|---|---|
| 1-01 | High | D4 license audit gated "before v0.1.0" (§25.2) but still OPEN in `.agent/DECISIONS.md`; Phase 10 must treat D4 as closing for the v1.0.0 artifact set, not as already closed — if no earlier audit exists it blocks the go decision. | `phase-10-v1-release-hardening/ledger_phase_10.md` P10.19 | Applied in build (P10.19) |
| 1-02 | Medium | §2.1 v1.0.0 definition omits the control-authority qualification that §22.3 gate 11 requires; P10.15 closes the gap explicitly rather than assuming §2.1 exhaustive. | `phase-10-v1-release-hardening/ledger_phase_10.md` P10.15 | Applied in build (P10.15) |
| 1-03 | High | D1/D3/D5 still OPEN vs §22.3 requirement that Phase 0 gates be closed for deployment jurisdiction/network class/protocol mix before promotion beyond read-only — P9 subtasks can only proceed on recorded interim positions with change conditions, carried as open risks. | `phase-09-pilot-qualification/ledger_phase_9.md` P9.03, P9.04, P9.11, P9.27 | Applied in build |
| 1-04 | Medium | EPRI OpenDSS "What's new" page still headlines 10.0.0.1 while SourceForge news announces 11.0.0.1 — latter used, discrepancy noted. | `phase-04-grid-feasibility-dispatch/RESEARCH.md` | Applied in build (noted, not resolved) |
| 1-05 | Medium | IPOPT ChangeLog latest dated entry is 3.14.19 while package/source records show 3.14.20. | `phase-03-flexibility-scheduling/RESEARCH.md` | Applied in build (noted) |
| 1-06 | Medium | OpenDSS/GridLAB-D licenses not re-verified from distributions — labelled ASSUMPTION, open task for D4 audit. | `phase-04-grid-feasibility-dispatch/RESEARCH.md` | Applied in build (open task) |
| 1-07 | Medium | React 19.3.1 / TypeScript 7.0 / Vite 8.x NOT primary-verified — secondary round-ups only; open task to re-verify against react.dev / typescriptlang.org / vite.dev at scaffold time before pinning. TanStack versions and pyhelics 3.7.0 match likewise open. | `phase-05-operator-console-simulator/RESEARCH.md` | Applied in build (open tasks) |
| 1-08 | High | OpenEMS native auth is API key over WebSocket, NOT mTLS — FlexCore's outbound-mTLS edge channel is a different channel; report does not conflate them but an implementer could. | `phase-06-edge-protocol/ledger_phase_6.md` P6.07 | Applied in build (guard P6.07) |
| 1-09 | High | OpenEMS Getting Started defaults (`admin`/`admin`, `Metadata.Dummy`, `ws://`) are testing-only and unsafe as a deployment model. | `phase-06-edge-protocol/RESEARCH.md`, P6 subtasks | Applied in build |
| 1-10 | Medium | D5 must name the OCPP *edition* as well as the version (Edition 2 date conflicts Jun vs Dec 2026 across secondary sources). | `phase-06-edge-protocol/ledger_phase_6.md` / RESEARCH.md | Applied in build (open verification task) |
| 1-11 | High | Wasmtime <50.0.0 carries two CVSS 9.9 Cranelift sandbox escapes (CVE-2026-115889/115893, disclosed 19 Oct 2026), fixed only in ≥50.0.0 / 48.0.7 / 49.0.4 / 36.0.19 — ledger pins ≥50.0.0. New information, not a report error. | `phase-07-plugin-sdk/ledger_phase_7.md`, RESEARCH.md | Applied in build |
| 1-12 | High | WASI 0.3 released 11 Jun 2026 but `wasm32-wasip3` is nightly-only until Rust 1.100 (scheduled 12 Nov 2026); report's §5.4 WASI 0.2 baseline confirmed as the stable target. | `phase-07-plugin-sdk/RESEARCH.md` | Applied in build |
| 1-13 | High | ANDES 2.0.0 is GPLv3 — live D4 issue against Apache-2.0-recommended core; requires worker-boundary isolation + legal confirmation. | `phase-08-advanced-coordination/ledger_phase_8.md`, RESEARCH.md | Applied in build |
| 1-14 | Medium | PyTorch 2.15 (28 Oct 2026) vs PyG 2.8.0 supporting PyTorch only up to 2.12 — version skew flagged as pinning task, not assumed-compatible. | `phase-08-advanced-coordination/RESEARCH.md` | Applied in build (open task) |
| 1-15 | Medium | RLlib latest Ray version as of 7 Nov 2026 not verified from a primary source; new API stack is PyTorch-only and MADDPG/QMIX only in removed contrib collection — report's loose "RLlib" wording is a staleness risk. | `phase-08-advanced-coordination/RESEARCH.md` | Applied in build (open task) |
| 1-16 | Medium | OpenADR 3.1 (2025) certification status not verified — recorded as open, not a fact. | `phase-09-pilot-qualification/RESEARCH.md`, `phase-06-edge-protocol/RESEARCH.md` | Applied in build (open task) |
| 1-17 | Medium | SBOM/license tool versions (Syft ≥1.30, Grype ≥0.100, OSV-Scanner ≥2.2, Trivy v0.72.0, ORT) from mid-2026/Oct-2025-benchmark sources only — P10.19 must re-verify against each tool's own repo before recording. | `phase-10-v1-release-hardening/RESEARCH.md` | Applied in build (open task in P10.19) |
| 1-18 | Medium | SQLx 0.9.0 changelog vs crates.io date discrepancy noted; NATS server 2.15.2 CVE-2026-33222 detail secondary — verify against GitHub releases. PostgreSQL 18.6 minor date secondary — re-verify with `SELECT version()` at build time. | `phase-01-rust-foundation/RESEARCH.md` | Applied in build (open tasks) |
| 1-19 | Low | Report TOC lists §1–§28 but body also has "§0 Executive decision" (unnumbered in TOC). Cosmetic; § references still resolve. | — (report, NOT edited) | Deferred — report is out of scope for this ledger; noted here for a future report patch |
| 1-20 | Medium | §23 Stage 1 exit gate requires an end-to-end workflow through API/use case/database/event publication, but the report never says which process hosts the outbox publisher. Phase 1 records an explicit decision (worker process) rather than assuming it. | `phase-01-rust-foundation/ledger_phase_1.md` P1.19 | Applied in build |
| 1-21 | Low | §24 slice vs v0.1.0 scope: report itself resolves this (slice spans v0.1.0–v0.3.0, "not all promised in the first public release") — restated in Phase 0 P0.24 and PLANs so no phase claims the whole slice. | `phase-00-product-freeze/ledger_phase_0.md` P0.24 | Applied in build |
| 1-22 | Medium | Jepsen NATS 2.12.1 finding (write loss under file corruption; default fsync every 2 min, not before ack) — durability claims stay modest; outbox + idempotency keys are the guarantee, fsync policy is a deployment decision. | `phase-01-rust-foundation/RESEARCH.md` | Applied in build |
| 1-23 | Medium | PJM capacity-market date for FERC 2222 differs across secondary sources — recorded as open verification task, not a fact. | `phase-09-pilot-qualification/RESEARCH.md` | Applied in build (open task) |
| 1-24 | Low | GridLAB-D v5.2 exact release date not stated on its release page — not claimed. | `phase-04-grid-feasibility-dispatch/RESEARCH.md` | Applied in build (not claimed) |

## Counts

- Found: 24 items — Critical 0, High 7, Medium 13, Low 4.
- Applied in build: 23. Deferred: 1 (1-19, report TOC cosmetic — out of ledger scope).
- Remaining open Critical/High after iteration 01: 0 Critical; High items are all carried as guarded subtasks/open verification tasks inside the ledger (1-01, 1-03, 1-08, 1-09, 1-11, 1-12, 1-13), not resolved — they are decisions/verification work for implementation time, recorded as such.
