# P3.19 + P3.20 — Decision note: solver determinism/failure handling and the Tier 3/4 boundary

- **Date:** 2026-11-07.
- **Subtasks:** P3.19 (ledger_phase_3.md §P3.19) and P3.20 (§P3.20). Report
  trace: §12.3, §12.6, §7.4, §13, §19.2 layer 5.
- **Deliverable crate:** `crates/flexcore-optimization-api` (additive modules
  `determinism.rs`, `infeasibility.rs`, `tier_guard.rs`, an additive
  `config.rs` extension, and the integration tests `tests/determinism.rs`,
  `tests/infeasibility.rs`, `tests/tier_guard.rs`).

## 1. Recorded decision — D-P3.19-1: determinism = pinned versions + fixed thread count + verifiable config record + replay-identity check

**Configuration (ledger P3.19 step 1, additive to the frozen P3.17
`SolverConfig` contract — no existing field, default, or digest value
changed):**

- The per-provider option applicability is now an explicit, tested table:
  `config::config_contract(ProviderTier)` (`ConfigContract`). Tier 1
  consumes time limit, MIP relative gap, both tolerances, scaling, and the
  fixed thread count; Tier 2 consumes everything except the MIP gap (a
  continuous conic solve has none — a configured gap is recorded as
  ignored, mirroring 18_clarabel_tier2.md §5). The provider modules are
  UNCHANGED; the contract table documents and tests what they do.
- **Thread-count recording (the ledger's risk mitigation).** The recorded
  default is `config::DETERMINISM_THREAD_COUNT_DEFAULT = 1` (unchanged from
  P3.17; now a named constant). The value is part of every result's
  `DeterminismRecord::thread_count` (recorded by the providers since
  P3.17/P3.18 — asserted per provider in the tests, and now also asserted
  as part of the replay identity).
- **Verifiable config record, not a blob.** `SolverConfig::canonical_json`
  serializes the whole configuration canonically (derived serde, stable
  field order); `SolverConfig::config_digest` is the FNV-1a 64 digest of
  exactly those bytes, so a recorded digest and a recorded canonical JSON
  cross-check each other (`SolverConfig::matches_digest`). Every configured
  value — time limit, MIP gap, both tolerances, scaling, thread count — is
  therefore verifiable in the record. P3.21 embeds the canonical JSON (or
  the config itself) alongside the digest when it completes the §12.6
  snapshot plumbing.
- No random seed exists in the configuration: both pinned provider paths
  are deterministic given fixed options and a fixed thread count; the
  config digest stands in for "seed/config"
  (`determinism::DETERMINISM_POLICY`, stated verbatim in code).

**Status mapping audit (ledger P3.19 step 2 — outcome: no gaps found).**
Both recorded mapping tables (17_optimizer_port.md §3, 18_clarabel_tier2.md
§4) already cover every native status of their pinned versions, including
the timeout path (Tier 1: `TimeLimitReached` and `IterationLimitReached` →
`timeout`; Tier 2: `MaxTime` and `MaxIterations` → `timeout`), and both are
exhaustive-match-guarded in tests. Additions made in this batch are tests,
not table changes:

- The timeout status construction WITH an incumbent payload (the pinned
  Tier 1 solver reports its time-limit model status and may carry a
  feasible incumbent): the recorded table maps the outcome to `timeout`,
  the incumbent payload is recorded, and no optimality claim is made
  (`tests/infeasibility.rs::timeout_with_incumbent_maps_to_timeout...`).
- A Tier 2 time-limited real solve exercising the `MaxTime` → `timeout`
  construction (`tests/determinism.rs::verify_build_time_limited...`).
- An explicit all-six-statuses-reachable acceptance test through the Tier 1
  port path (`tests/infeasibility.rs::all_six_statuses_are_reachable...`).

**VERIFY-BUILD:** the exact Tier 1 binding signal for the time-limit-hit
model status (and its incumbent payload convention) is confirmed at worker
adapter build time (decision note 17 §6 item 1); the Tier 2 signal is the
native `MaxTime` solver status (docs.rs 0.11.1; confirmed at build time by
the verify-build test above).

**Infeasibility policy (ledger P3.19 step 3).** Implemented as stated
policy + structured types in `infeasibility.rs`:

- `INFEASIBILITY_POLICY` (verbatim): an infeasible run returns CONFLICT
  EVIDENCE — which constraints conflict, per the solver's diagnostics where
  available — and NEVER a silently relaxed schedule; where the solver
  cannot provide evidence, the record carries an explicit not-available
  reason.
- `InfeasibilityEvidence { conflicting_constraints, solver_excerpt,
  not_available_reason }` with an exactly-one-of-two validation rule
  (silence and both-present are both illegal). The providers' existing
  evidence channels are unchanged: Tier 1 via the backend `ConflictEvidence`
  (IIS — VERIFY-BUILD item 2 of 17_optimizer_port.md), Tier 2 records the
  explicit not-available reason (the pinned conic solver exposes no IIS).
  `evidence_from_result` bridges the §12.6 result diagnostic onto the
  policy type and rejects silence as defence in depth.
- **Relaxation-decision types (recorded decision).** `RelaxationDecision {
  decision_name, review_reference, relaxed_constraint_refs, rationale }` —
  all fields mandatory (a decision must be NAMED and REVIEWED) — and
  `RelaxedScheduleRecord { base_candidate_id, decision, applied_to }`,
  constructible ONLY through `RelaxedScheduleRecord::new`, which validates
  the decision and enforces that every applied relaxation is within the
  reviewed scope. There is no code path that applies a relaxation without
  one; P3.21/P3.22 embed this type in the schedule record. The policy names
  the declared-hard EV departure-energy requirement (`flexcore-domain`
  `DepartureEnergyPolicy::Hard` — the ONLY variant of that policy) as the
  canonical example; no dependency on flexcore-domain is added (doc
  reference only).

**Determinism / replay identity (ledger P3.19 step 4).** Implemented in
`determinism.rs`:

- `ReplayIdentity` — the canonical serialization of (problem IR digest,
  config digest + verifiable canonical config JSON, pinned solver version,
  provider version, thread count), with a stable `identity_digest` (same
  FNV-1a 64 convention as the config digest).
- `compare_replay_records` — the replay-identity check over two solve
  records: byte-identical at tolerance 0.0, or tolerance-identical with the
  tolerance STATED and recorded on the comparison
  (`DEFAULT_REPLAY_TOLERANCE = 1e-9` for real-solver replays; the pinned
  solvers' own tolerances are at 1e-7..1e-8, so the stated tolerance
  absorbs only representation noise). Measured fields (wall-clock runtime,
  solver-log excerpt) are excluded, and the exclusion is stated on every
  comparison. Non-identity is a recorded finding with the differing field
  named — never smoothed over.
- `ReplayHarness::verify_replay(optimizer, problem, config)` — solves the
  SAME problem twice through the port and verifies identity. **P3.26's
  scenario-regression calls this unchanged over real solves**; this batch's
  tests run it over the deterministic Tier 1 double (byte-identical) and
  the offline identity of the P3.18 conic fixtures (assembled conic model +
  config digest + declared versions).

**VERIFY-BUILD (P3.19):**

1. The real-solver replay runs — the Tier 1 worker adapter with the pinned
   binding, and the full Tier 2 replay suite over both conic fixtures —
   are P3.26's job (17_optimizer_port.md §6 item 6, 18_clarabel_tier2.md §7
   item 8). `verify_build_real_socp_solve_replays_identically` already
   covers the Tier 2 inverter fixture once the root-manifest `clarabel`
   line lands.
2. The Tier 1 time-limit-hit signal and incumbent convention (see above).
3. Parallel-MIP determinism at the pinned Tier 1 version remains NOT
   assumed (RESEARCH.md §6 task 1): the fixed thread count is pinned in the
   config and recorded in the identity; if parallel MIP is ever enabled,
   the harness must be run with it on and off and the outcome recorded
   here.

## 2. Recorded decision — D-P3.20-1: Tier 3 (MISOCP/MINLP) and Tier 4 (nonlinear AC-OPF) are NOT built in v0.2.0

**The boundary (report §12.3, §13; ledger P3.20 steps 1–2):**

- Tier 3 (MISOCP/MINLP) is built only when discrete decisions materially
  require it AND a suitable solver exists (a solver that actually supports
  the class — never a Tier 1/2 solver forced past its class).
- Tier 4 (nonlinear AC-OPF) is advanced research / validated use cases
  only.
- NEITHER is in v0.2.0. Tiers are problem classes, not quality rankings.
- **Reformulation rule:** a formulation needing Tier 3 is reformulated as a
  Tier 1 MILP (linearising the conic terms — the P3.18 inverter fixture
  pair shows exactly this translation and its 1 MW approximation gap) or a
  Tier 2 continuous SOCP (integrality removed by an explicit, reviewed
  modelling decision), or the gap is recorded as an open Stage-8 item —
  never forced through an unsuitable solver (`REFORMULATION_RULE`).

**PLAN.md §1 non-goals note — deferred (coordinator-owned file).** The
ledger step 1 asks for the boundary to be recorded in PLAN.md. PLAN.md is
coordinator-owned and this batch makes no edit there; this decision note is
the recording of record. **Follow-up for the coordinator:** add ONE bullet
to PLAN.md §1's non-goals (additive, no reformatting): "Tier 3
(MISOCP/MINLP) and Tier 4 (nonlinear AC-OPF) solvers are not built in
v0.2.0 — see ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md."

**The guard (ledger P3.20 step 3).** `tier_guard.rs` exposes the public
surface the scheduling path (P3.24) and the port surface call BEFORE
provider selection:

- `check_tier_boundary(ProblemClass)` — rejects Tier 3/4 classes with
  outcome `unsupported`, the decision reference
  (`TIER_BOUNDARY_DECISION_REFERENCE`, naming this file), and per-class
  reformulation guidance; Tier 1/2 classes pass.
- `guard_problem(&OptimizationProblem)` — the formulation-level form.
- `check_capabilities_within_boundary(&SolverCapabilities)` — a provider
  capability declaration must never declare a Tier 3/4 class (this is the
  "no Tier 3/4 code path" guarantee made checkable against both providers).
- Defence in depth is unchanged: the Tier 1 and Tier 2 providers
  additionally return in-band `unsupported` for every class outside their
  supported sets (P3.17/P3.18 tests, untouched).

**Tests** (`tests/tier_guard.rs`): MISOCP and nonlinear requests rejected
with `unsupported` + the decision reference; per-class reformulation
guidance; no provider declares a Tier 3/4 class; a structural source scan
asserting the crate exposes no Tier 3/4 entry point (fails the build if a
Tier 3/4 path is added without updating this decision — the ledger's risk
mitigation). No CI change is needed: the scan is a plain `cargo test` test
inside the crate's normal suite (.github/workflows/ci.yml was not touched
by this batch).

**IPOPT honesty note (ledger P3.20 step 4).** `IPOPT_LOCALITY_NOTE`,
verbatim: IPOPT performs LOCAL nonlinear optimization only and carries NO
global-optimality guarantee (report §7.4). It is NOT wired as a Phase 3
scheduler solver; any future use is justified only by a specific Tier 4
research case and lands in Stage 8, not Phase 3. Its **Eclipse Public
License (EPL)** is recorded for the D4 license/dependency audit (still
OPEN; RESEARCH.md §4) — unlike the MIT/Apache-2.0 solvers actually wired in
this phase (HiGHS MIT, Clarabel Apache-2.0).

## 3. What P3.19/P3.20 do NOT do

- No real worker adapter, no transport code, no manifest change (the
  coordinator's P3.18 root-manifest `clarabel` line is still pending; the
  verify-build tests run once it lands).
- No change to the providers, the port, the IR, or the result contract —
  every existing API and recorded decision is unchanged; the additions are
  the three new modules and the additive `config.rs` surface.
- No Tier 3/4 implementation, no AC-OPF, no global-optimality claim
  anywhere; a solved schedule remains a candidate (feasibility is Phase 4).
- No edit to PLAN.md (coordinator-owned; follow-up above), CI, or any other
  crate.
