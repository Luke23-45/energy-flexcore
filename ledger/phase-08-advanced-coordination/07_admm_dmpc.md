# P8.07 — ADMM/DMPC provider: formulation with explicit convexity assumptions

Date: 2026-11-13. Subtask: P8.07 (`ledger_phase_8.md` §P8.07). Owner role:
research engineer (optimization); the reviewer signature below is the
power-systems reviewer's gate. Report trace: §13.2; §12.3; §7.4. Inputs:
P8.02 (`02_research_scope_freeze.md`, question M1 and prohibitions X-3/X-8),
P8.04 (`04_worker_contracts.md`), P8.06 (`05_06_harness_baselines.md`),
report §13.2, §12.3, §7.4, AGENTS.md rule 4, ENTRY_NOTE.md boundaries 1–6.

## 1. Status of this record — WRITTEN-UNCOMPILED

The provider is implemented in `crates/flexcore-research/src/admm.rs`
(the formulation, ~1000 lines with its in-module test suite) and operated
by `crates/flexcore-research/src/dmpc.rs` (P8.08, record
`08_admm_operations.md`). **WRITTEN-UNCOMPILED**: no `cargo build` /
`check` / `test` / `clippy` / `fmt` command was executed in this loop (the
subtask's loop bans; ENTRY_NOTE.md). Every type reference was written
against source that was read (validation.rs, baselines.rs, lib.rs,
Cargo.toml), and every test expectation is hand-computed arithmetic stated
in the test comments — the toys' optima and ADMM fixed points are derived
on paper IN the comments. NOTHING here is evidence that the crate compiles
or that any test passes; the human build/test gate (AGENTS.md rule 7) owns
that evidence.

**Numeric results: every measured cell is EMPTY** (ENTRY_NOTE boundary 4).
No benchmark figure, seed result, or runtime number exists in this record
or in the code. The known-answer arithmetic inside tests (e.g. the exact
fixed point x = (2, 2), z = (−2, −2), u = (−2, −2) of the 2-device toy) is
derived mathematics, not a measured result.

## 2. The formulation (ledger step 1) — as code

`admm.rs` implements the §13.2 form verbatim: minimize Σ_k f_k(x_k)
subject to Σ_k a_k x_k ≤ b and x_k ∈ X_k, where

| Element | Content | Units / discipline |
|---|---|---|
| x_k | participant k's dispatched power for ONE coordination interval | MW; `P > 0` = export at the point of connection (§10.2) |
| f_k | `0.5 · q_k · (x − c_k)²`, q_k ≥ 0 | convex quadratic; q_k = 0 is a flat cost; NEGATIVE q_k is a NAMED validation error (the convexity gate) |
| X_k | `[p_min_k, p_max_k]` | closed box = the capability envelope (§11 discipline); MW |
| coupling | exactly ONE `Σ_k a_k x_k ≤ b`, a_k finite non-zero | the multi-feeder substation constraint of §19.3 (a_k = +1) or the aggregate flexibility request (a_k = −1, b = −d) |

**The algorithm** is exact two-block ADMM (Boyd, Parikh, Chu, Peleato,
Eckstein 2011, "Distributed Optimization and Statistical Learning via the
Alternating Direction Method of Multipliers", §3.2 — the algorithmic
primary source for the convergence claim) on the consensus-on-copies
reformulation: per-participant copies z_k of the coupling contribution
(z_k = a_k x_k), g(z) = I(Σ z_k ≤ b), scaled duals u. Both blocks solve
EXACTLY in closed form — no solver crate, no iteration inside a block:

- **x-update** (`local_update_closed_form`): x_k = clamp((q_k c_k +
  ρ a_k (z_k + u_k)) / (q_k + ρ a_k²), p_min_k, p_max_k) — the box clip is
  exactly the Euclidean projection onto X_k.
- **z-update** (`copy_update`): if Σ v ≤ b then z = v (v_k = a_k x_k −
  u_k), else z_k = v_k − ν with ν = (Σ v − b)/n > 0 — the exact KKT
  solution (identical quadratic weight per copy ⇒ one uniform shift;
  μ = ρν > 0 satisfies complementarity).
- **u-update** (`dual_update`): u ← u + z − a∘x (scaled form).

**The §13.2 anti-risk is enforced in the formulation, not in prose**: the
copies z_k are per-participant CONTRIBUTION copies — no step of the
algorithm forces unrelated sites to identical power values; only the dual
price is shared (`ASSUMPTION_NO_IDENTICAL_SETPOINTS`, pinned by tests).
The ledger's named risk "consensus variables that force unrelated sites to
identical setpoints" cannot occur in this formulation by construction.

**Determinism**: participants live in a `BTreeMap` (sorted-id iteration is
the contract, pinned by tests); fixed update order; no wall clock; no
randomness (`seed_supported = false` is the honest P7.42 admission on every
proposal). Same input → same result, pinned by a byte-equality test.

## 3. The convexity assumptions and the closed class vocabulary (ledger step 2)

Each assumption is a named, test-pinned constant in `admm.rs` — the
assumptions ARE part of the deliverable, not commentary:

| Constant | Content |
|---|---|
| `ASSUMPTION_LOCAL_COSTS_CONVEX` | f_k convex (q_k ≥ 0 enforced at construction — the convexity gate refuses negative curvature with `NEGATIVE_COST_CURVATURE`, never "with a warning") |
| `ASSUMPTION_LOCAL_SETS_CONVEX` | X_k a closed convex box; the clip = projection |
| `ASSUMPTION_COUPLING_LINEAR_SCALAR` | exactly one scalar linear coupling; a_k finite, non-zero |
| `ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS` | THE convergence claim, scoped exactly: the two-block ADMM theorem (closed, proper, convex blocks — both blocks here solve exactly) gives residual convergence to the configured tolerances. NO iteration-count guarantee (X-8: no universal "converges in K iterations" target), no uniqueness claim without strong convexity, NO global-optimality claim outside the supported class |
| `ASSUMPTION_NO_IDENTICAL_SETPOINTS` | copies are contribution copies, never a consensus setpoint (§13.2 risk, enforced by construction) |

**Supported class** (`SUPPORTED_PROBLEM_CLASS`, the only class the provider
declares per §7.4): convex quadratic local costs over closed boxes coupled
by exactly one linear constraint, solved by exact two-block ADMM.

**Unsupported classes** (`UNSUPPORTED_PROBLEM_CLASSES`, closed 4-item
vocabulary, pinned by tests): nonconvex AC-OPF (AC power flow evaluates a
state and is not AC-OPF, §12.5); mixed-integer local problems (ADMM on them
is a heuristic, §13.2 — such a run would be reported as heuristic behaviour
with residuals and fallback outcomes, never as convergent); multiple or
nonlinear couplings; sub-cycle/protection-grade control (AGENTS.md rule 3).

**Solver-class discipline VERBATIM (AGENTS.md rule 4)**: ADMM is NEITHER
LP/QP/MIP (the HiGHS class) NOR convex conic (the Clarabel class). Its
convergence claim rests on the stated convexity assumptions, not on a
solver's certificate. The statement constant carries both negations.

**D3 obligation (P8.02 M1)**: the formulation is device-level coordination
and is network-agnostic — the coupling constraint REPRESENTS the network
limit (e.g. the substation headroom) but the formulation contains no power
flow. The network class of every evaluation case is the P8.09 evaluation
record's obligation (balanced radial is the standing default
RECOMMENDATION; unbalanced/meshed only if explicitly scoped). This record
claims no network fidelity beyond "the coupling encodes an aggregate
limit".

## 4. Dependency decision (recorded — no new dependency, none needed)

**No solver crate was added and none is needed for the supported class**:
every supported-class update is a closed-form expression (one-variable
local QP; exact uniform-shift copy block), so the formulation requires no
external solver. The CENTRALIZED comparison/fallback path is the frozen
P8.06 baseline registry (`crate::baselines`) consumed through ports. The
crate's dependency set stays serde + serde_json exactly as before (see
`crates/flexcore-research/Cargo.toml` — unchanged by this wave). Had the
ledger demanded a non-closed-form local problem class, this would have been
a STOP-and-record decision point; it does not.

## 5. Composition seams (recorded honestly)

- **`Optimizer` port (ledger step 3)**: mirrored by citation
  (`OPTIMIZER_PORT_ALIGNMENT_SEAM`, the same discipline as
  `baselines.rs`); `AdmmProvider` declares its problem class per §7.4
  through `OptimizerProviderSeam`.
- **Phase 7 Class C gRPC worker shell**: PLAN.md §3 places the provider in
  `python/workers/` behind the `Optimizer` contract. That shell is **NOT
  written in this loop** — `python/workers/**` is another wave's file
  ownership and the loop bans forbid the toolchain (buf, grpcio). Recorded
  as `P7_GRPC_WORKER_SEAM` in the module: the Rust formulation +
  operational contract are the algorithm core that worker executes; the
  composition wiring (worker shell reusing the Phase 7 worker contracts
  per ENTRY_NOTE boundary 3 — no new contracts, X-10) is a recorded
  follow-up, never silently assumed.
- **Rolling-horizon seam (P8.08)**: `ROLLING_HORIZON_SEAM` cites the
  platform's EXISTING P3.24 surface
  (`crates/flexcore-application/src/rolling_horizon.rs`:
  `plan_rolling_horizon_roll` / `project_rolling_schedule` /
  `rolling_horizon_schedule_proposed_publication`) BY CITATION — this
  crate does not depend on flexcore-application; the composition root
  re-invokes the DMPC driver each scheduling cycle with refreshed SOC
  state.

## 6. How outputs flow (the P8.05 / P8.06 composition, both records' obligation)

1. `dmpc.rs::plan_horizon` produces a `DmpcPlan`; its FIRST STEP is the
   receding-horizon committed schedule (`FinalSchedule`: the converged
   distributed solution, or the frozen fallback's schedule, or NOTHING —
   a recorded failure adopts nothing).
2. `dmpc.rs::proposal_for_first_step` packages the first step as a
   `validation::ResearchProposal` (§13.4 `proposed_schedule` kind) with
   honest §9.2 provenance: termination status = the named run status
   (never the bare "ok"), `seed_supported = false` (no randomness — the
   P7.42 admission), `model_artifact_ref` = THIS record (the signed
   formulation is the artifact governing the provider), diagnostics =
   staleness/fallback records + the ADR-010 boundary statement.
3. The proposal enters the P8.05 INDEPENDENT validation harness
   (`validation.rs::ValidationHarness`) exactly like any other candidate —
   envelope shape, provenance completeness, validity window (injected
   clock), snapshot freshness, then PHYSICAL re-validation through the
   injected platform feasibility gate. Pinned by tests: the proposal
   passes a full harness pass; an expired window is REFUSED by the harness
   (no self-trust — the harness gates this crate's own outputs too); a
   proposal with an incomplete provenance block is refused
   (`PROVENANCE_INCOMPLETE`).
4. Comparison against the frozen baselines (P8.06) happens at EVALUATION
   time (P8.09), against the frozen registry records
   (`baseline/highs-tier1-lp`, `baseline/clarabel-tier2-conic`,
   `baseline/rule-based-priority-list`) — the objective gap where
   meaningful, per §19.4. Nothing in the provider computes or claims a gap.

## 7. Status of the deliverable — EMPTY cells and pending gates

| Ledger step | Status |
|---|---|
| 1 formulation as code | WRITTEN (admm.rs, with its in-module test suite) — UNCOMPILED |
| 2 convexity assumptions + class vocabulary | WRITTEN as named constants — UNCOMPILED |
| 3 provider behind the Optimizer contract | WRITTEN as the mirrored port + `AdmmProvider` declaration; the Phase 7 gRPC worker shell is a recorded follow-up (§5) — NOT WRITTEN here |
| 4 design note reviewer-signed | **EMPTY — the signature block below is unfilled; the note is not "signed" until a power-systems reviewer records their name/date here** |

**Reviewer signature (power-systems reviewer) — PENDING:**

> Reviewed by: ______________ Date: ______
> Checked the assumptions note against the implementation; every
> occurrence of "converges"/"global optimum" in the provider docs is
> scoped to the stated assumptions (grep discipline below).

**Grep discipline (the ledger's verification method, unrun):**

```sh
grep -n "converge\|global optimum\|optimality" crates/flexcore-research/src/admm.rs \
  crates/flexcore-research/src/dmpc.rs
# Every hit must sit inside a sentence scoped by ASSUMPTION_* or the
# statement constants; any unscoped hit is a finding against this record.
```

## 8. Exact filling commands (the human gate)

```sh
cargo build -p flexcore-research
cargo test -p flexcore-research            # in-module suites (admm, dmpc, validation, baselines)
cargo test -p flexcore-research --test admm_dmpc_proposal_flow
cargo clippy -p flexcore-research -- -D warnings
cargo fmt --check
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green run fills nothing in this record except the reviewer signature and
a one-line "human build/test gate: PASSED (date)" note appended here;
failures are recorded verbatim and the tick reverts to `- [ ]`.

## 9. Non-claims (restated; X-3/X-8)

No global-convergence or global-optimality claim beyond
`ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS`; no iteration-count guarantee;
no safety claim; no field authority (proposals only, ADR-010); no network
fidelity claim beyond "the coupling encodes an aggregate limit"; no
benchmark or runtime figure exists anywhere in this record.
