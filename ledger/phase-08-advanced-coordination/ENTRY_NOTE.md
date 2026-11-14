# ENTRY NOTE — Phase 8: Advanced Coordination Research

Date: 2026-11-13. Coordinator: implementation-coordinator. Loop rules in
force: nothing installed, nothing built, no tests run, no CI run — complete
code + test files written only. Ticks are "implemented, awaiting human
build/test". Decision records per batch in this directory.

## Gate exception (same pattern as Phases 1–7)

Stage 7 exit gate is **NOT HELD** (witnessed swap/untrusted-plugin run UNRUN
— `ledger/phase-07-plugin-sdk/28_exit_gate_run_record.md`; the v0.5.0
position is recorded prepared in the P7.32 retrospective). Phase 8 starts on
the recorded exception; P8.01 records it honestly.

## Phase-8-specific boundaries

1. **Research only; proposals only (ADR-010)**: no research module has any
   field authority; a MARL worker issuing live device commands is
   PROHIBITED, not deferred. Every evaluation claim is per-feature-
   qualified NOWHERE — P8.29 records what field use would require and that
   none of it is granted here.
2. **No global-optimality or safety claims**: nonconvex AC-OPF and
   mixed-integer results carry no optimality claim without a theorem +
   experiment (§13.2); zero observed violations from a finite test set is a
   test outcome, not a guarantee (§13.3). Solver-class discipline holds
   (HiGHS LP/QP/MIP; Clarabel conic; ADMM is neither — its convexity
   assumptions stated explicitly; steady-state PF is not transient
   stability; HELICS is transport, not physics).
3. **No new plugin SDK contracts**: research workers REUSE the Phase 7
   gRPC worker template + contracts (`python/workers/contracts/
   worker_api.proto`); a contract gap is recorded as a Phase 7 follow-up,
   never fixed by inventing a parallel contract (P8.04).
4. **Every measurement is an EMPTY cell**: all evaluations (P8.09, P8.12,
   P8.15, P8.17, P8.19, P8.23) are written as TEMPLATES with EMPTY
   measured cells — the training/simulation runs are human/lab gate
   inputs. Never fabricate a figure, seed result, or benchmark number.
5. **D4 pins**: research-stack deps (ANDES, HELICS already pinned 3.7.0
   from Phase 5, torch/jax if any, grpcio UNPINNED-REVERIFY) recorded
   statically; P8.03 audits; P8.36 does the CVE check AT PIN TIME as a
   recorded procedure; nothing installed.
6. Component locations per PLAN.md §3 (read it): research providers/
   workers live where the ledger dictates — the Phase 7 seam table
   (`ledger/phase-07-plugin-sdk/31_phase8_handover.md`) is the composition
   input; new crates require additive workspace-member +
   check_dependency_rule.py updates.

## Wave plan (max 2 subagents; host caps concurrency at 2; disjoint file ownership)

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | this file, HANDOVER.md plan | Gate exception + boundaries |
| 1a | P8.01, P8.02, P8.03 | decision records | readiness / scope freeze / license audit |
| 1b | P8.04, P8.05, P8.06 | worker-contract reuse record + validation harness + baselines (new research crate/module per ledger) | |
| 2a | P8.07, P8.08 | ADMM/DMPC provider (formulation + residuals/termination/infeasibility/fallback) | |
| 2b | P8.10, P8.11 | GNN warm-start/screening design + isolated inference worker | |
| 3a | P8.09, P8.12 | ADMM + GNN evaluation TEMPLATES (EMPTY cells) | |
| 3b | P8.13, P8.14 | MARL design + simulation/shadow worker | |
| 4a | P8.15, P8.19 | MARL eval template + benchmark-case extensions | |
| 4b | P8.16, P8.17, P8.18 | ANDES scope/integration, dynamic eval template, HELICS co-sim | |
| 5a | P8.20, P8.21 | reproducibility + topology research record | |
| 5b | P8.22, P8.23 | portfolio analytics (reporting-only) + cross-module comparison | |
| 6a | P8.24, P8.25, P8.36 | drill, security review, CVE check procedure | |
| 6b | P8.26, P8.27, P8.28 | benchmark paper, console research surfacing, CI gates | |
| 7a | P8.29, P8.34 | qualification record + proposal validity windows | |
| 7b | P8.31, P8.32, P8.33 | tenant isolation, model-artifact signing, identity/mTLS/secrets | |
| 8a | P8.35, P8.37, P8.38 | training-data provenance, unit/property tests, GPU nondeterminism | |
| 8b | P8.39 | co-sim/dynamic cross-checks | |
| 9 (coordinator) | P8.30 | retrospective + exit-gate record (UNRUN) | All P8.01–P8.39 |
