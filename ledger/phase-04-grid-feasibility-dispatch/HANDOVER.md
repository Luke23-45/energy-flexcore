# HANDOVER — Phase 4: Grid Feasibility and Dispatch Assurance

Status: **in progress** (batch plan below; completed at phase close).

Max 2 subagents at a time; file ownership disjoint per batch. Loop bans hold
(nothing installed/built/run; tests written, never executed).

## Batch plan (written before delegation, per loop rules)

Dependency graph: P4.01→P4.02→P4.03→P4.04→{P4.05→P4.06→P4.07}→P4.08→P4.09;
P4.10 (needs P3.22 only) is independent of the whole feasibility chain;
P4.11→P4.12→P4.13→P4.14→P4.15; P4.16 (needs P4.13–15)→P4.17→P4.18, P4.19;
rejection suites P4.20–P4.26 (need P4.04/06/11/12/14/15); P4.27–P4.31, P4.33–37
close; P4.28–P4.30, P4.32 are review/wiring records.

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 1a | P4.01, P4.02 | `crates/flexcore-grid-api/**` (NEW core crate — coordinator wires manifests), decision doc | Provider port + pandapower decision (D3 stays OPEN); Python-worker boundary |
| 1b | P4.10 | `crates/flexcore-domain/**` (command state machine impl), `crates/flexcore-storage/**` | P0.11 frozen table = oracle; needs only P3.22 |
| 2a | P4.03–P4.05 | grid-api model-health + gate steps 1–4 | Feasibility chain start |
| 2b | P4.11–P4.13 | lifecycle: PROPOSED/VALIDATED gate, feasibility gate (P4.12), approval gate | Distinct-evidence rule |
| 3a | P4.06–P4.09 | gate steps 5–8 + fail-closed + SOCP boundary | Completes feasibility chain |
| 3b | P4.14–P4.16 | invariants, expiry/cancel/supersede, simulation dispatch adapter | Lifecycle completion |
| 4a | P4.17–P4.19 | fallback policies, bounded hold, timing honesty | |
| 4b | P4.20–P4.23 | rejection suites: invalid/stale/infeasible/unauthorized | Pre-registered expectations |
| 5a | P4.24–P4.26 | rejection suites: duplicate/expired/adversarial | |
| 5b | P4.27, P4.33 | evidence pack + exhaustive 14×14 transition matrix test (P0.11 oracle) | |
| 6a | P4.28–P4.30 | security controls, safety invariants, audit/events/persistence/observability | |
| 6b | P4.31, P4.34–P4.37 | fault/shadow/integration, unbalanced deferral, runtime measurement, audit-read API, constrain/return contract | |
| close (coordinator) | P4.32 | documentation + handover to Phase 5, ledger ticks, CHECKLIST, this file | |

## What was implemented

(filled at phase close)

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

(filled at phase close)
