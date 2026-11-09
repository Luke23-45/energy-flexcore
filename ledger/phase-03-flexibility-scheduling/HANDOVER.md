# HANDOVER — Phase 3: Flexibility and Deterministic Schedule

Status: **CLOSED (code written, nothing built or tested)** — 31/31 subtasks
implemented; CHECKLIST ticked with evidence locations; the Stage 3 gate
remains NOT HELD (human reviewers + scenario runs + benchmark record + scans
are the gate inputs). The Phase 4 handover pack is `30_handover_phase4.md`;
the security review is `29_security_review.md`; the build/test instructions
are in the per-crate VERIFY-BUILD comments and `25_26_tests_scenarios.md`.

## Blockers (carried to the human)

1. Stage 3 exit-gate review NOT HELD (reviewers + P3.26 runs + P3.27
   benchmark record + P3.29 scans required).
2. Everything unbuilt/unrun — first `cargo build`/`test --workspace` is the
   real proof; VERIFY-BUILD comments carry the flagged spots.
3. Tier 1 Python worker adapter (highspy side of `Tier1Backend`) lands with
   Phase 4 dispatch/shadow work; G5 real-solve determinism re-verified then.
4. Coverage gaps G1–G6 + the real coverage figure (`cargo llvm-cov`).
5. `pandapower==3.5.6` worker pin + CVXPY pin land in the Python worker
   lockfile (P4.02 handoff).

## Decisions still needed / Known risks

See `30_handover_phase4.md` §2 (limitations Phase 4 must not treat as
guarantees) and §4 (open items). D1–D6 remain SCHEDULED.

## Batch plan (written before delegation, per loop rules)

Max 2 subagents at a time; file ownership disjoint per batch. Loop bans hold
(nothing installed/built/run; tests written, never executed).

## Batch plan (written before delegation, per loop rules)

| Batch | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | `ENTRY_NOTE.md`, this plan | Gate exception + boundaries |
| 1a | P3.01–P3.04 | `crates/flexcore-contracts/**` (capability schema), `crates/flexcore-domain/**` (provenance, time grid, validators) | Contract freeze gates everything |
| 1b | P3.17 (port + Tier 1 provider core) | `crates/flexcore-optimization-api/**` (NEW crate — coordinator wires manifests), solver config, §12.6 status vocabulary + result-record types | §9.1 port; problem-class enforcement; HiGHS binding pinned (researched, not downloaded) |
| 2 | P3.05–P3.08 | `crates/flexcore-domain/**` capability models (BESS, P-Q) | Depends on batch 1a |
| 2 | P3.18 (Clarabel Tier 2) | `crates/flexcore-optimization-api/**` provider module | Depends on P3.17 port |
| 3 | P3.09–P3.11 | load + EV models, `flexcore-domain` | |
| 3 | P3.19 (solver config/determinism) + P3.21 (result contract completion) | optimization-api + storage | |
| 4 | P3.12–P3.13 | `crates/flexcore-application/**` envelope context | Depends on models + forecast fixture |
| 4 | P3.14–P3.16 | objective + PCC tracking, `flexcore-application` | |
| 5 | P3.22–P3.24 | schedule revisions/comparison/rolling horizon | |
| 5 | P3.31 | `scenarios/`, forecast fixture module + `ForecastPublished` | Depends on P3.01–P3.03 |
| 6 | P3.20, P3.25, P3.26 | Tier 3/4 guard, test suites, scenario tests | |
| 6 | P3.27, P3.28 | benchmarks + API/events/persistence | |
| 7 (coordinator) | P3.29, P3.30 | security review record, handover, ledger ticks, CHECKLIST, this file | |

## What was implemented

(filled at phase close)

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

(filled at phase close)
