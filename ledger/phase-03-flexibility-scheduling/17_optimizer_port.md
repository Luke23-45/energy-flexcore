# P3.17 — Decision note: the `Optimizer` port and the Tier 1 provider path

- **Date:** 2026-11-07.
- **Subtask:** P3.17 (ledger_phase_3.md §P3.17). Report trace: §9.1, §12.3, §7.4, §5.2, §7.2.
- **Deliverable crate:** `crates/flexcore-optimization-api` (core crate).

## 1. Recorded decision — D-P3.17-1: Tier 1 routes through the Python optimization worker (highspy), behind a transport-neutral Rust trait

**Decision.** The Tier 1 (LP / convex QP / MILP) solve path is the **Python
optimization worker** (`python/workers/optimization-worker`, per PLAN.md §3)
running **highspy**, with the Rust control plane calling it through the
`Optimizer` port (report §9.1). The Rust side defines an internal,
**gRPC-free, transport-neutral trait** `Tier1Backend` inside
`flexcore-optimization-api`; the worker adapter that implements this trait
(in-process bridge for tests, gRPC/Protobuf for deployment per §15.1 when the
Phase 7 protobuf contracts land) is built in the P3.19 batch. P3.17 ships the
port, the problem IR, the §12.6 result contract, the provider-owned solver
config, the Tier 1 provider logic (class gate → formulation assembly → outcome
mapping), and test doubles for the seam — everything complete except the real
worker adapter, which is P3.19's deliverable.

**Version pin (recorded).**

| Item | Value | Source |
|---|---|---|
| Tier 1 solver | HiGHS, via the Python binding **highspy 1.15.1** | `ledger/phase-03-flexibility-scheduling/RESEARCH.md` §1 (PyPI https://pypi.org/project/highspy/1.15.1/, release files read 2026-11-07) |
| Class | LP, convex QP, MIP. **Not SOCP.** | RESEARCH.md §1; highs.dev; report [R28] |
| License | **MIT** (HiGHS project repository https://github.com/ERGO-Code/HiGHS) | RESEARCH.md §1. Recorded here as D4-audit input; the formal D4 audit (gate, before v0.1.0) is still OPEN and is owned by legal + engineering — this note is input to it, not the audit. |

**Why this path and not the Rust binding.** RESEARCH.md §6 task 6 (iteration-02
review, 7 November 2026) records that **no Rust `highs` crate version was
researched**, and that "a wrong pin is worse than the recorded Python-worker
path". No WebSearch for a Rust binding version was performed for this decision;
instead the decision closes the binding question for v0.2.0 as **"Python worker
path chosen — Rust binding research not needed for Tier 1 in v0.2.0"**. If a
later batch wants a native Rust HiGHS binding, it must research + pin the crate
version from crates.io/docs.rs first and re-record this decision — the
`Tier1Backend` seam keeps the choice replaceable (CONTEXT.md §9 open question
5 anticipated exactly this).

**Boundary honouring.** Python is used for numerical solving only; it is never
the command authority (§7.2). The worker returns a solution record; the Rust
provider owns status mapping, result-contract construction, and determinism
metadata. Nothing in this decision touches the Phase 4 boundary (validation,
authorization, dispatch).

## 2. Problem-class discipline (AGENTS.md rule 4 — absolute)

- Every `OptimizationProblem` **declares** its `ProblemClass` per run:
  `lp | convex_qp | milp | socp | misocp | nonlinear`.
- The Tier 1 provider supports **only** `lp`, `convex_qp`, `milp`. A formulation
  declaring `socp`, `misocp`, or `nonlinear` is rejected with the §12.6 status
  `unsupported` **before any solver invocation** — tested in the crate
  (`provider_rejects_socp_misocp_nonlinear`), with a call counter proving the
  backend was never invoked.
- The problem IR cannot even represent a conic constraint to HiGHS: SOCP
  constraints live in a dedicated `SecondOrderCone` IR type that the Tier 1
  path never consumes. **HiGHS is never invoked on an SOCP formulation.**
- The port surface (port/problem/result/config/backend modules) contains no
  solver-specific type; a source-scan test enforces this (no `highs`/
  `highspy` identifier in those files). The provider and this decision note
  are the only places that name HiGHS.

## 3. Status mapping (§12.6 — closed six-status vocabulary)

The worker maps the pinned solver's native model status onto the neutral
`BackendStatus`; the Rust provider maps `BackendStatus` onto §12.6:

| BackendStatus | §12.6 status | Notes |
|---|---|---|
| Optimal | `optimal` | |
| Feasible | `feasible` | e.g. a MIP terminated within its gap |
| Infeasible | `infeasible` | always carries infeasibility evidence (Available, or NotAvailable with the recorded reason) — never a relaxed schedule (P3.19 step 3) |
| TimeLimitReached | `timeout` | |
| IterationLimitReached | `timeout` | a resource limit, not a numerical failure; the verbatim backend status text is recorded so the two limit kinds stay distinguishable |
| Unbounded | `numerical_failure` | the six-status vocabulary has no `unbounded` value and the vocabulary is closed, so an unbounded objective is reported as a failure with the verbatim backend status and an explicit warning; it is never mapped to `infeasible` (a feasible region exists) |
| UnboundedOrInfeasible | `numerical_failure` | same reasoning; ambiguity recorded verbatim |
| ModelError / SolveError / Unknown | `numerical_failure` | |

No other §12.6 status string exists; `SolveStatus` is a closed enum.

## 4. Infeasibility evidence (conflict / IIS)

The backend outcome carries an optional `ConflictEvidence` (conflicting
variable/row names + verbatim solver report). The worker adapter (P3.19) maps
the pinned solver's conflict-refinement / IIS output onto it.
**VERIFY-BUILD:** the exact IIS API surface of highspy 1.15.1 (entry point and
returned structures) must be confirmed against the installed binding at build
time before the adapter is written; until then the provider treats evidence as
optional-with-reason, which is contract-legal. A run reported `infeasible`
without exposed evidence records `NotAvailable { reason }`, never silence, and
never a relaxed schedule.

## 5. Determinism position (feeds P3.19)

- `SolverConfig` carries a **fixed thread count**; the recorded default is `1`
  (asserted in tests). Determinism = pinned solver version + fixed thread
  count + recorded config digest (FNV-1a 64 of the serialized config).
- RESEARCH.md §6 task 1 remains open: parallel-MIP determinism at 1.15.x is
  NOT assumed; P3.19 pins or disables parallel MIP in determinism tests.

## 6. VERIFY-BUILD list (established repo pattern; all offline-unverifiable)

1. highspy 1.15.1 status-enum values → `BackendStatus` mapping (worker side,
   P3.19): confirm the pinned binding's model-status names at build time.
2. highspy IIS/conflict API surface (see §4).
3. Objective hessian convention: the IR stores upper-triangle terms with
   contribution `c·xᵢ·xⱼ`; confirm the pinned binding's off-diagonal scaling
   convention (the C++ API documents the Hessian in a form whose off-diagonal
   handling must be checked) when the worker adapter is written.
4. Infinity representation: IR unbounded bounds map to `f64::INFINITY`; the
   worker maps them to the pinned solver's infinity constant (conventionally
   `1e30` — confirm at build time).
5. Option names for time limit, thread count, feasibility tolerances, MIP gap,
   and scaling strategy in the pinned binding — confirmed at build time by the
   P3.19 adapter; the IR-side defaults (1e-7 feasibility tolerances) are the
   solver's documented defaults and carry the same VERIFY-BUILD marker.
6. The deterministic-replay tests over the real worker (same inputs + pinned
   versions + same config → identical result records) land with P3.19/P3.26.
   The P3.17 tests are replay *skeletons* over the port with scripted backend
   outcomes and hand-computed reference fixtures — real tests, but they do not
   exercise the built solver.

## 7. What P3.17 does NOT do

- No worker process, no gRPC/protobuf (Phase 7), no transport code.
- No Tier 2 (P3.18), no determinism harness (P3.19), no Tier 3/4 guard (P3.20),
  no result persistence/event wiring (P3.21) — the port is built so those
  land without touching it.
- No claim that a solved schedule is feasible: it is a candidate (Phase 3
  entry-note discipline; feasibility is Phase 4).
