# P4.20 + P4.21 + P4.22 — Decision note: the deterministic rejection suites
# for invalid, stale, and infeasible commands

- **Date:** 2026-11-10.
- **Subtasks:** P4.20, P4.21, P4.22 (`ledger_phase_4.md`). Report trace:
  §23 Stage 4 exit gate, §12.5, §14.3, §14.1, §10.4, §17.5 invariant 5,
  §17.4, §22.2.
- **Deliverable files:** `crates/flexcore-application/tests/rejection_invalid.rs`
  (P4.20), `crates/flexcore-application/tests/rejection_stale.rs` (P4.21),
  `crates/flexcore-grid-api/tests/rejection_infeasible.rs` (P4.22), this
  note, and the ledger checkbox ticks.
- **Build/test status (honest):** NOTHING was built and NO test was run.
  The loop bans (no installs, no builds, no test execution) were held for
  the whole batch; every suite is WRITTEN, never executed. All three
  subtasks are "implemented, awaiting human build/test" — never
  "tested"/"verified". In particular: **the determinism requirement is
  recorded as in-suite assertions (each fixture calls its path twice and
  asserts identical outcomes), not as two actual suite runs** — no run of
  any kind happened.
- **Reused, never restated:** the P4.11–P4.13 command gates and their seam
  surfaces (`command_gates.rs`, decision record `11_13_command_gates.md`);
  the P4.14 §14.3 record validators and rejection rules
  (`command_record.rs`); the P4.15 sweep/expiry service
  (`command_expiry.rs`); the P3.13 staleness evaluation
  (`envelope_staleness.rs`); the P2 time-model ingest rule
  (`flexcore-contracts::time_model`); the P4.04–P4.08 grid-api feasibility
  gate, limit check, verdict routing, and fail-closed policy
  (`flexcore-grid-api`; decision records `01_02_grid_provider.md`,
  `06_09_gate_completion.md`); the P4.02 contract-test doubles
  (`tests/common`). Pre-registered expectations are derived from the
  FROZEN decisions of those subtasks (the D-P4.07-1 routing rule, the
  D-P4.08-1 policy table, the P0.11 frozen transition table), not fitted
  to any run.

## 1. Pre-registered expectations (written BEFORE the assertions; UNRUN)

Every row's expectation is stated in the suite file as a comment directly
above its assertions, and each test is preceded by that comment. The
label on every decision below is **DEFAULT** (a recorded recommendation
pending the human build/test pass and the P4.27 review), never a decided
product default.

### P4.20 — invalid commands (`tests/rejection_invalid.rs`)

| Fixture | Input (pinned) | Pre-registered outcome | Failing check / evidence |
|---|---|---|---|
| F-IV-1 | §14.3 intent `reason` blank at creation | Creation refused; no record, no event | `Schema(EmptyField { field: "reason" })` (P1.06 validators, group 5) |
| F-IV-1b | Flat §14.3 record with blank `idempotency_key` | REJECTED by the P4.14 creation validator | `MissingField { phase: Creation, field: "idempotency_key" }` (group 11; §14.3-order first-miss rule) |
| F-IV-2 | kW-scaled numeric (1500) supplied where MW is required | REJECTED with the §14.3 `outside_capability` rule; NEVER rescaled | `outside_capability` — "active-power setpoint 1500 MW is outside the declared window [-5, 5] MW"; the flat record renders canonical `MW + MVAr` units only (kW unrepresentable, §10.2) |
| F-IV-3 | `valid_from > valid_until` | Refused at the contracts type (`ValidityWindow::new` errors — the gate cannot even receive it) and REJECTED at the persisted-record boundary (defence in depth) | `InvalidField { phase: Creation, field: "valid_from_millis" }`, detail names the inversion |
| F-IV-4 | Asset unknown / foreign / of an unknown site (indistinguishable by design, P0.16 rule 5) | PROPOSED → REJECTED at the validation gate; `DispatchProposed` then `DispatchRejected` | `asset_unknown_in_tenant_scope`; evidence reference `validation:command/<id>@<at>:check/asset_unknown_in_tenant_scope`; journal reason = the check |
| F-IV-4b | Schedule revision unknown in the caller's tenant scope | Creation refused; no record, no event | `CreationRejected { check: SourceScheduleRevisionUnknown }` — unknown is never usable |
| F-IV-5 | P = 6.0 MW vs [-5, 5]; Q = 3.0 MVAr vs [-2, 2]; Q with no declared reactive range; ramp 3.0 MW/s vs 2.0 declared | REJECTED with the §14.3 `outside_capability` rule, each detail naming the measured value and the declared window | `outside_capability` (P4.14 `outside_capability_rejection` / `ramp_outside_capability_rejection`) |
| F-IV-6 | Duration: negative is UNREPRESENTABLE (`std::time::Duration` is unsigned — recorded, not worked around); the expressible boundary violation, ZERO duration | Creation refused; flat record REJECTED | `Schema(InvalidDuration)`; `InvalidField { phase: Creation, field: "duration_millis" }` |
| control | The unmodified fixture command | CREATES and VALIDATES (harness sanity — every rejection above is designed, not a harness artifact) | — |

### P4.21 — stale commands (`tests/rejection_stale.rs`)

| Fixture | Input (pinned) | Pre-registered outcome | Failing check / evidence | REJECTED vs EXPIRED, and why |
|---|---|---|---|---|
| F-ST-1 | `valid-until` passed before validation; command still at PROPOSED | REJECTED via the §14.3 STALE rule | `stale` (code of `CommandRejectionRule::Stale`), detail records the §10.3 end-exclusive convention; boundary pinned: `now == valid_until` fires, `now == valid_until − 1` does not | **REJECTED, not EXPIRED**: PROPOSED has no frozen EXPIRED edge — only APPROVED/ISSUED/ACKNOWLEDGED/TRACKING expire (P4.15); asserted via the pure sweep (no decision for a PROPOSED candidate) and the service refusal (`WrongState` on `apply_expiry`) |
| F-ST-2 | Command built on a SUPERSEDED schedule revision | REJECTED AT CREATION (not merely at issue); no record, no event | `CreationRejected { check: SourceScheduleRevisionSuperseded }` (P4.11 step 1) | REJECTED (no command record exists to expire) |
| F-ST-3 | Source envelope invalidated by a `TelemetryQualityChanged` event carrying quality **STALE** on a scoped asset (§10.4; §17.5 invariant 5) | PROPOSED → REJECTED at the validation gate; `DispatchRejected` after `DispatchProposed` | `envelope_stale:TelemetryQualityChanged { .. }` — the verdict is produced by the REAL P3.13 `evaluate_envelope_trigger`; the gate seam carries it (the composition-root shape); journal reason = the check | REJECTED (the freshness gate rejects before any milestone) |
| F-ST-4 | Telemetry/envelope snapshot older than the CONFIGURED max age (`CONFIGURED_MAX_AGE_MILLIS = 60_000` in the fixture; **DEFAULT** — the recorded one-hour default `DEFAULT_MAX_AGE_MILLIS` is pinned separately) | REJECTED at the validation gate | `envelope_stale:MaxAgeExceeded { age_millis, max_age_millis }`; the P2.08 convention pinned at the boundary: age == max_age is NOT stale, max_age + 1 is (the at-bound case is the control and PASSES) | REJECTED |
| F-ST-5 | Command already APPROVED when validity ended | APPROVED → EXPIRED (the frozen edge), milestone `BeforeIssue` recorded, expiry time = the controlled clock instant, `DispatchExpired` emitted | journal reason = the milestone not reached ("validity ended before issue …") | **EXPIRED, not REJECTED**: the command was already past the gates when validity ended — the frozen APPROVED → EXPIRED edge is the designed outcome (P4.15) |
| F-ST-6 | Clock skew: command timestamp 1 h in the future (§17.4) | REJECTED at the time-model boundary before any command can be recorded | `TimeModelError::IngestBeforeEvent` (the ingest ordering rule `ingest_time ≥ event_time`); boundary pinned: ingest == event is accepted, ingest + 1 ms skew is refused | REJECTED |
| control | Fresh envelope, open window | CREATES and VALIDATES | — | — |

### P4.22 — infeasible commands (`crates/flexcore-grid-api/tests/rejection_infeasible.rs`)

The full path per fixture: `run_feasibility_gate` (§12.5 steps 1–4 through
the PORT, over the scripted backend double) → `check_limits` (step 5) →
`route_pipeline_outcome` (steps 6–8). The expectations are the FROZEN
routing rule D-P4.07-1 and the D-P4.08-1 policy table — the command-layer
consequence of every non-pass verdict (VALIDATED → REJECTED with
`network_limit:<constraint>`, or the fail-closed reason) is the P4.12 gate
mapping, already pinned in `flexcore-application/tests/command_gates.rs`
(a transformer-limit violation rejects with the constraint named there).

| Fixture | Designed violation (EXACTLY ONE limit; asserted `violations.len() == 1`) | Pre-registered verdict | Named constraint |
|---|---|---|---|
| F-IF-1 | bus_3 vm_pu 0.88 < 0.9 lower band | RETURNED for re-optimization (D-P4.07-1 rule 6 — a voltage-band violation cannot be gate-constrained); never WITHIN-LIMITS | `bus_voltage_lower:bus_3` |
| F-IF-2 | bus_2 vm_pu 1.12 > 1.1 upper band | RETURNED for re-optimization (rule 6); never WITHIN-LIMITS | `bus_voltage_upper:bus_2` |
| F-IF-3 | line_2_3 loading 1.05 — HARD, above the physical rating | REJECTED (rule 4 — a hard violation is never gate-constrained); no constrain proposal | `branch_thermal:line_2_3` |
| F-IF-4 | trafo_1_2 loading 1.05 — HARD | REJECTED (rule 4); no constrain proposal | `transformer_loading:trafo_1_2` |
| F-IF-5 | The power flow does not converge | INCONCLUSIVE (first-class; retry policy NONE; never a pass and never a limit-rejection, D-P4.05-2) — the P4.08 non-convergence posture | none (no violations are named; nothing converged) |
| F-IF-6 | Stale model (a source change beyond the health bound) | Step-3 ABORT **before any power flow** (asserted: the backend was never called), routed **HELD** per the D-P4.08-1 policy table (`ModelStaleOrIncomplete` → HOLD, operator-releasable only per `HOLD_RELEASE_POLICY`) | the fail-closed outcome names trigger `MODEL_STALE_OR_INCOMPLETE` |
| control | None | WITHIN-LIMITS (harness sanity) | — |

The REJECTED-vs-CONSTRAINED-vs-RETURNED split is the recorded P4.07
design, not an evasion: at the grid layer a single OPERATIONAL (sub-rating)
thermal violation routes CONSTRAINED and a voltage violation routes
RETURNED (both name the constraint, and the constrained proposal is
evidence that must re-enter the gate, P4.37); the ledger's "REJECTED with
the named constraint" fixtures are the HARD (above physical rating)
violations F-IF-3/F-IF-4. Every fixture in the table asserts
`never WithinLimits` and that the named constraint equals the DESIGNED
violation.

## 2. Recorded decisions (all labels DEFAULT)

| ID | Decision | Where |
|---|---|---|
| D-P4.20-1 | **Fixture set and pre-registration form.** The six ledger fixture classes are each implemented against the EXISTING public gates/validators (no new enforcement code, no src changes); each fixture's expectation is written as a comment directly above its assertions, and the table above is the normative copy. All labels DEFAULT. | `tests/rejection_invalid.rs` header + per-test comments |
| D-P4.20-2 | **Determinism is an in-suite assertion.** Every fixture closure runs twice with identical inputs and the outcomes are asserted equal (`assert_eq!` on the error/verdict values). This is the test-time form of the ledger's "run the suite twice"; no external second run was performed (nothing was run at all). | per-fixture closures in both application suites |
| D-P4.20-3 | **CI wiring (recorded, not edited).** The suites are CI-runnable by construction: the `ci` workflow's `unit-tests` job runs `cargo test --workspace`, which includes `cargo test -p flexcore-application` and `cargo test -p flexcore-grid-api`; the §22.2 `scenario-regression` job is the regression-gate job the ledger names. No CI file was modified (file ownership). | `.github/workflows/ci.yml` (read only); suite headers |
| D-P4.21-1 | **REJECTED-vs-EXPIRED split.** A command still at PROPOSED when its window ends is REJECTED (§14.3 STALE rule); a command already at APPROVED/ISSUED/ACKNOWLEDGED/TRACKING moves EXPIRED via the frozen edges with the milestone recorded (P4.15). The suite demonstrates both and asserts WHY (no PROPOSED sweep decision; the `apply_expiry` refusal). | `tests/rejection_stale.rs` F-ST-1/F-ST-5 |
| D-P4.21-2 | **Staleness is decided by the REAL P3.13 evaluation.** The STALE-telemetry and max-age fixtures call `evaluate_envelope_trigger` / `evaluate_max_age` and carry the produced verdict through the gate seam (the composition-root shape) — the gate rejection is not a scripted string. | F-ST-3/F-ST-4 |
| D-P4.21-3 | **Clock-skew tolerance: the implemented value is ZERO** (the P2 time-model ingest ordering rule refuses any strictly-future timestamp with `IngestBeforeEvent`; the boundary is inclusive at zero skew). A NON-zero configurable future tolerance is NOT implemented in the current Phase 4 surfaces — recorded as a gap, not silently assumed (answers CONTEXT.md open question 6 for this suite: DEFAULT zero, pending review). | F-ST-6; §3 limitations |
| D-P4.22-1 | **Single-violation design and constraint-name matching.** Each limit fixture differs from the all-within-limits base in EXACTLY ONE hand-computed value, and each test asserts `report.violations.len() == 1` and the exact constraint name — the ledger's ambiguity risk (a fixture violating two limits) is excluded by construction. | F-IF-1..F-IF-4 |
| D-P4.22-2 | **P4.08 cases keep their policy outcomes.** Non-convergence renders INCONCLUSIVE (never a pass, never a limit-rejection, retry NONE) and a stale model aborts before the power flow and routes HELD (`ModelStaleOrIncomplete` → HOLD) — both per the frozen D-P4.08-1 table, asserted against the named triggers. | F-IF-5/F-IF-6 |
| D-P4.22-3 | **Backend doubles carry consistent currents.** The scripted worker outcomes keep currents consistent with the reported loading percentages (the adapter cross-checks the branch loading within 1e-4 relative), so a fixture's designed violation cannot be an artifact of inconsistent doubles. | fixture builder, `tests/rejection_infeasible.rs` |

## 3. Gaps and limitations (recorded, none silently filled)

- **Nothing was built or run.** No `cargo build/check/test/clippy/fmt`, no
  installs, no test execution, no Python beyond the two AGENTS.md §6 doc
  checkers. The suites are written against the actual public APIs (read
  end-to-end before writing) but are unexecuted — the human build/test
  pass is the gate. The determinism "two runs" are in-suite assertions,
  not actual runs.
- **Clock-skew future tolerance (D-P4.21-3):** the zero tolerance is the
  ingest ordering rule at the contracts layer; the grid-api step-1
  timestamp check covers only the PAST side of the §12.5 step-1 rule (a
  future-dated `created_at` yields a negative age and passes there), and
  no command-path field carries a creation timestamp (§14.3 has none). A
  nonzero configurable future tolerance on the command path is an open
  item for the P4.27 review; this suite pins the boundary that exists and
  records the gap instead of asserting enforcement that is not there.
- **F-ST-1's enforcement surface is the pure §14.3 STALE rule plus the
  issue-time/sweep surfaces**: the validation gate's own check list does
  not re-check window lapse (recorded after reading
  `validation_checks`); the composition applies the STALE verdict when
  processing a command whose window has ended. Recorded here so the human
  review can confirm the wiring point.
- **The unknown-site case (P4.20 F-IV-4) has no separate site-existence
  check** in the application layer: asset, site, and tenant existence
  surface as the one tenant-scope asset check (foreign and missing are
  indistinguishable, P0.16 rule 5) plus the tenant-scoped schedule-
  revision check (F-IV-4b). Persistence-layer RLS scoping (§16.2) is the
  deeper layer and is out of this suite's crate.
- **P4.22 exercises the grid layer only.** The command-layer half of the
  VALIDATED → feasibility path lives behind the `CommandFeasibilityRunner`
  seam (flexcore-application cannot depend on flexcore-grid-api); the
  mapping of grid verdicts to command rejections is pinned by the existing
  `tests/command_gates.rs` suite and cited, not duplicated.
- **Research cut-off:** no external sources were consulted; the
  7 November 2026 cut-off is unchanged.

## 4. File/test index

| File | Content |
|---|---|
| `crates/flexcore-application/tests/rejection_invalid.rs` | P4.20 (8 tests + control): the six fixture classes with pre-registered expectations, per-fixture run-twice determinism, the create-and-validate control |
| `crates/flexcore-application/tests/rejection_stale.rs` | P4.21 (7 tests + control): window-ended-before-validation (stale-not-expired, both halves), superseded-revision creation rejection, STALE-telemetry invalidation via the real P3.13 evaluation, configured max age with the exclusive boundary, APPROVED-when-lapsed → EXPIRED with milestone, the zero-tolerance clock-skew rejection, the fresh control |
| `crates/flexcore-grid-api/tests/rejection_infeasible.rs` | P4.22 (7 tests): the routing-decision pin, four single-violation fixtures (low/high voltage RETURNED, hard branch/transformer REJECTED) each asserting `violations.len() == 1` and the designed constraint name, non-convergence → INCONCLUSIVE, stale model → step-3 abort + HELD per P4.08, the within-limits control; per-fixture run-twice determinism |
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.20/P4.21/P4.22 objective boxes ticked as "implemented, awaiting human build/test" with this record cited |
| `ledger/phase-04-grid-feasibility-dispatch/HANDOVER.md` | Wave 4b status updated |

## 5. Review status and verification

- **Build/test sign-off: PENDING (human).** Nothing in this batch was
  compiled or executed (loop bans held); the ledger checkboxes are ticked
  as "implemented, awaiting human build/test" only.
- **Power-systems review sign-off: PENDING** — should confirm the P4.22
  fixture magnitudes (0.88/1.12 pu, 105 % loadings against the 0.9/1.1
  bands and the 1.0 physical rating) and the P4.08 outcome split.
- **QA review sign-off: PENDING** — should confirm the determinism form
  (in-suite double runs) and the CI wiring statement.
- **AGENTS.md §6 tools:** run at wave close over the documentation
  changes (the ledger record, this note, HANDOVER); they scope the report
  and the agent harness, not the crates.
