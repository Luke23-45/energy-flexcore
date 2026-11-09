# P4.06 + P4.07 + P4.08 + P4.09 — Decision note: feasibility-gate completion
# (limits, margins, routing, fail-closed policy, SOCP boundary)

- **Date:** 2026-11-07.
- **Subtasks:** P4.06, P4.07, P4.08, P4.09 (`ledger_phase_4.md`
  §§P4.06–P4.09). Report trace: §12.5 steps 5–8 and the branch-flow SOCP
  paragraph, §12.3, §12.6, §17.5 invariant 9, §10.4, §15.3, §19.4.
- **Deliverable crate:** `crates/flexcore-grid-api` (new modules
  `limits.rs`, `verdict.rs`, `fail_closed.rs`, `socp_boundary.rs`; tests
  `tests/limits.rs`, `tests/verdict.rs`, `tests/fail_closed.rs`,
  `tests/socp_boundary.rs`; `lib.rs` additive declarations only).
- **P4.05 relationship:** this note consumes the `FeasibilityAttemptRecord`
  (D-P4.05-4, evidence only) and completes the pipeline the earlier note
  pre-wired (non-convergence, injection quality, hash hooks).

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P4.06-1 | **Limit ownership / configuration source (CONTEXT open question 1):** the limit set is caller-supplied CONFIGURATION per network model — voltage bands and loading fractions via the port's `NetworkLimits`, generator/inverter apparent-power capability via the new `GeneratorLimitSet` (§11 capability-derived, per bus, source note REQUIRED). For the 0.x synthetic feeders the values derive from the fixture definitions. The source is recorded PER LIMIT on every check record; no limit is hard-coded in the check logic. The per-unit base is the bus nominal voltage (model data; the conversion is the translation layer's tested contract). | `limits.rs` (`LIMIT_SOURCE_STATEMENT`, `GeneratorLimit(Set)`, `check_limits`) |
| D-P4.06-2 | **Margins are evidence:** every check records the measured value, the limit, the margin in the limit's own units, AND the margin as a fraction of the limit (`margin / limit`; `None` only where a zero limit would make it meaningless — the validated configuration refuses zero limits, so it is always `Some` in practice). The §12.5 step 7 diagnostics (worst margin per category, binding constraint, per-element loading percentages) are computed from the records; P4.35 consumes them. The binding constraint is the smallest margin FRACTION across categories — fractions are dimensionless, the one comparable currency across per-unit voltages and fractions of rating. | `limits.rs` (`LimitCheckRecord`, `MarginDiagnostics`, `BindingConstraint`, `MARGIN_STATEMENT`) |
| D-P4.06-3 | **Boundary convention, decided once:** `value == limit` is WITHIN limits (boundary-inclusive; margin 0). A violation requires the measured value STRICTLY beyond the limit. Tested on both sides of every category. Matches the crate's existing conventions (`age == limit` not stale, P2.08/P3.13; the port-level `evaluate_limits` strict comparisons). | `limits.rs` (`BOUNDARY_CONVENTION`, pass predicates) |
| D-P4.06-4 | **Multi-violation reporting:** the verifier names EVERY violated constraint (`<category code>:<element id>`, frozen format). A single-violation fixture yields exactly one name; the type supports any number. | `limits.rs` (`LimitCheckRecord::constraint_name`, `LimitCheckReport::violations`) |
| D-P4.07-1 | **The routing rule, decided once (CONTEXT open question 2), implemented as the pure function `route_candidate`, in evaluation order:** (1) fail-closed trigger (P4.08 outcome, fail-closed SOC boundary, fail-closed limit report) → REJECT or HOLD per the P4.08 table, never a pass; (2) non-convergence / provider error / inconclusive limit report → INCONCLUSIVE (first-class category, retry NONE — never a pass, never a rejection-by-limit); (3) no violated constraint → WITHIN-LIMITS, evidence only, coverage qualifications attached; (4) any HARD violation (loading strictly above the model-declared physical rating, fraction > 1.0 — the rating base is model data, so "hard" is a definition, not a configured limit) → REJECTED, all constraints named; (5) a SINGLE operational thermal violation (branch/transformer, ≤ physical rating) → CONSTRAINED with a deterministic proposal recorded; (6) otherwise (single voltage violation, single capability/S_max violation, or ≥ 2 violations) → RETURNED-FOR-RE-OPTIMIZATION with every violated constraint as a named input. Rationale: the gate never optimizes (§12.5) — a voltage fix or a multi-constraint adjustment is a re-dispatch decision (the scheduler's); a hard rating violation admits no gate-time adjustment; a single thermal overload has a well-defined first-order scaling. | `verdict.rs` (`ROUTING_DECISION_REF`, `ROUTING_RULE_STATEMENT`, `route_candidate`, `route_pipeline_outcome`, `route_fail_closed`) |
| D-P4.07-2 | **The constraining proposal is EVIDENCE, not an applied mutation:** a uniform proportional scaling of every candidate injection by `limit / measured` (first-order; brings the violated element to its configured operational limit), recorded with the proposed values, the rule, the predicted value, and the evidence-not-applied statement. The constrained candidate MUST re-enter the gate from step 1; **P4.37 defines the consumption contract**. Never silently adjusted and passed. | `verdict.rs` (`CONSTRAIN_PROPOSAL_STATEMENT`, `ConstrainProposal`) |
| D-P4.07-3 | **Diagnostics + echo + event:** the verdict evidence finalises the P4.06 margins (violations named, worst margin fraction, binding constraint), the input-snapshot echo (model id/version/hash, provider id/version, injection derivation refs — §12.6), the coverage limitations and scope limit gaps (stated, never hidden), and the caveats/findings from the P4.09 boundary. A `FeasibilityCheckCompleted` (§15.3) payload is produced for EVERY verdict (outcome, model version, evidence references); the outbox emission is the composition root's (grid-api cannot name the event infrastructure). The decision reference `D-P4.07-1` is on every verdict. | `verdict.rs` (`VerdictEvidence`, `InputSnapshotEcho`, `FeasibilityCheckCompleted`, `FEASIBILITY_CHECK_COMPLETED_DECISION`) |
| D-P4.08-1 | **The fail-closed policy table (CONTEXT open question 3), decided once** — every trigger maps to REJECT or HOLD; zero triggers yield a pass; a HOLD is releasable ONLY by an operator with a recorded reason, and release re-enters the gate (never converts to a pass): ValidatorFailed → HOLD; ModelStaleOrIncomplete → HOLD; ModelHashMismatch → HOLD; TelemetryQualityBlocked (BAD/MISSING/STALE, D-P4.04-3) → REJECT; TelemetryUnavailable → HOLD; SnapshotRefUnresolvable → REJECT; ScopeUndeclared → REJECT; CapabilityDataMissing → REJECT; CandidateMalformed → REJECT; PowerFlowNonConvergence → HOLD (rendered INCONCLUSIVE at the verdict layer — the same fail-closed shape, retry NONE, never a rejection-by-limit); ProviderError → HOLD; RelaxationRecordMissing → REJECT; UnexpectedError → HOLD (the default posture). Rationale per row in `fail_closed.rs` docs. | `fail_closed.rs` (`FailClosedTrigger`, `FailClosedAction`, `fail_closed_action`, `fail_closed_outcome`, `HOLD_RELEASE_POLICY`) |
| D-P4.08-2 | **The default posture is structural:** the pipeline's error classes are enumerated in `PipelineError` and classified by a match WITHOUT a wildcard arm — a new error variant without a fail-closed mapping is a COMPILE failure, and the tests assert every variant classifies to REJECT/HOLD with a non-empty reason. No function in the module returns anything but a fail-closed outcome. | `fail_closed.rs` (`PipelineError`, `classify_pipeline_error`, `EXHAUSTIVE_MATCH_STATEMENT`) |
| D-P4.09-1 | **The SOCP boundary:** a Tier 2 candidate is accepted into the AC verification only WITH its P3.18 relaxation-residual record attached (per-branch DistFlow residuals + the tolerance); a missing OR malformed/failed record is FAIL-CLOSED (`RELAXATION_RECORD_MISSING`, REJECT-class). The AC verification runs REGARDLESS of the relaxation result — the relaxation never substitutes for the power-flow check. A residual outside ±tolerance (cone violated below −tolerance, loose above +tolerance) is a RECORDED FINDING that travels to the verdict evidence — it neither fails nor passes the gate by itself; the AC power flow is the authority. | `socp_boundary.rs` (`gate_socp_boundary`, `RelaxationResidualReport::validate`, `assess_residuals`) |
| D-P4.09-2 | **The caveat travels:** `SOCP_EXACTNESS_CAVEAT` (mirrored verbatim from flexcore-optimization-api — grid-api cannot name that crate) is attached to the verdict evidence of EVERY Tier 2 candidate. The verdict is a CANDIDATE verdict for Phase 5/operations — never a blanket relaxation-exactness claim. | `socp_boundary.rs` (`SOCP_EXACTNESS_CAVEAT`, `TIER2_VERDICT_STATEMENT`), `verdict.rs` (`tier2_caveats_and_findings`) |
| D-P4.09-3 | **The condition checklist:** radiality (from the DECLARED scope — a scope that also covers meshed networks cannot confirm radiality), phase balance, and the three §12.5 no-assumption conditions (reverse power flow, voltage upper bounds, arbitrary network constraints) are recorded per case with explicit outcomes; the three are NEVER "satisfied" — they are recorded `NotAssumedExact` on every case. Not-satisfied conditions are findings on the gate result, never silently skipped. | `socp_boundary.rs` (`check_relaxation_conditions`, `ConditionOutcome`) |
| D-P4.09-4 | **The cross-check hook:** the gate's limit-check results feed the P3.18 Tier 1-vs-Tier 2 comparison record as a new FEASIBILITY-VERDICT dimension (tier, gate outcome code, violated constraints, worst margin fraction, caveats) — the objective and binding-set dimensions already exist in flexcore-optimization-api. The dimension is a mirrored, self-contained record the composition root attaches (grid-api cannot name flexcore-optimization-api; the D-P4.04-2 seam pattern). | `socp_boundary.rs` (`FeasibilityVerdictDimension`, `feasibility_verdict_dimension`) |

## 2. Gaps and limitations (recorded, none silently filled)

- **Event emission:** the `FeasibilityCheckCompleted` payload is
  produced here; the §15.3 outbox emission and the exact flexcore-events
  schema conversion are the composition root's (VERIFY-BUILD item below).
- **Mirrored types:** the residual record, the caveat, the residual
  formula, and the default tolerance mirror
  `flexcore-optimization-api::clarabel_provider` because flexcore-grid-api
  depends only on flexcore-contracts + serde/serde_json/time (the
  dependency-direction test pins the set). The composition root converts
  the provider's own record; the residual VALUES are computed upstream by
  `distflow_branch_residual`, never recomputed here.
- **Cross-check attachment point:** whether the feasibility-verdict
  dimension lands as a dedicated field on `Tier1Tier2ComparisonRecord` or
  as a rendered `notes` entry is finalized when flexcore-optimization-api
  consumes the dimension (VERIFY-BUILD item below).
- **Provenance source:** `TierProvenance` is derived by the composition
  root from the Phase 3 result record's tier marking (P3.18); grid-api
  records and enforces the boundary, it does not re-derive the tier.
- **Review status:** power-systems + security review of the D-P4.08-1
  policy table and the D-P4.07-1 routing rule is PENDING (the standing
  status for this phase), required before the ledger checkboxes are
  ticked.

## 3. Verification

- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for
  these files. The tests are written against the recorded APIs
  (grid-api P4.02–P4.05 surface, flexcore-contracts frozen types); the
  first `cargo test -p flexcore-grid-api` run must confirm, and any fix
  must keep the §1 decisions unchanged.
- **Acceptance mapping:** single-violation fixtures per limit category
  with hand-computed margins (`tests/limits.rs`); boundary-inclusive
  handling on both sides (D-P4.06-3); multi-violation naming; every
  routing outcome reachable on designed fixtures; inconclusive never
  routes to reject; decision reference on every verdict; every P4.08
  trigger fails closed with the structural exhaustive-match guarantee;
  Tier 2 with/without residual record, caveat travel, tolerance-breach
  findings (`tests/socp_boundary.rs`).
- **No feasibility-guarantee language:** a within-limits verdict is an
  evidence-backed outcome against the DECLARED model and limits within
  stated tolerances — never a field-safety, certification, or approval
  claim (§14.1: authorization is a distinct gate).
