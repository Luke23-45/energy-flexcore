# P4.27 — Exit-gate evidence pack for the deterministic rejection suites
# (§23 Stage 4; P4.20–P4.26 + the P4.33 transition-matrix appendix)

- **Date:** 2026-11-10 (pack assembled; NO run of any kind has happened).
- **Subtask:** P4.27 (`ledger_phase_4.md`). Report trace: §23 Stage 4 exit
  gate, §22.2, §19.2.
- **Deliverable:** this pack. It cites the six decision records; the
  per-fixture expectation tables live there and are NOT duplicated here.

## 0. Status — read first

**Status: UNRUN. Every result cell in this pack is EMPTY, and the §23
Stage 4 gate is NOT HELD.** Nothing in Phase 4's rejection work was ever
compiled or executed (the loop bans — no installs, no builds, no test
execution — were held for every batch; every decision record states this
in its build/test status block). This pack therefore contains:

1. the **pre-registered** per-fixture expectations (cited, never re-stated),
2. the exact **two-clean-runs procedure** the human executes (§2),
3. **empty run-identifier cells** (run 1 / run 2 per suite) and **empty
   sign-off rows** (§3) that only the human's build/test pass and the named
   reviewers may fill.

No outcome, pass rate, duration, or run identifier appears anywhere in this
pack, and none may be written from memory or inference — every cell is
filled from an actual run's output only (rule 1 of `27_benchmark_report.md`
§0, the P3.27 precedent, applies here verbatim). Nothing in this file is a
test result. The ledger P4.27 checkbox is ticked as "implemented, awaiting
human build/test" only; the acceptance criterion ("all six rejection
categories pass deterministically in two clean runs; sign-offs recorded per
suite") is **NOT MET** until §1's run cells and §3's sign-off rows are
filled from real runs and real reviews.

## 1. Suite index

Test counts are counted from the actual test files (`#[test]` attributes,
counted 2026-11-10). 61 rejection-suite tests (P4.20–P4.26) plus the
P4.33 appendix (9) = **70 tests** in the pack.

| Suite (ledger) | Test file | `#[test]` count (composition) | Pre-registered expectation table (REFERENCE — do not duplicate) | Run 1 identifier (EMPTY — fill from the actual run) | Run 2 identifier (EMPTY) |
|---|---|---|---|---|---|
| P4.20 invalid | `crates/flexcore-application/tests/rejection_invalid.rs` | 9 (F-IV-1, F-IV-1b, F-IV-2, F-IV-3, F-IV-4, F-IV-4b, F-IV-5, F-IV-6 + the create-and-validate control) | `20_22_rejection_suites_a.md` §1, table "P4.20 — invalid commands" | | |
| P4.21 stale | `crates/flexcore-application/tests/rejection_stale.rs` | 8 (F-ST-1, F-ST-1b expiry-refusal half, F-ST-2, F-ST-3, F-ST-4, F-ST-5, F-ST-6 + the control) | `20_22_rejection_suites_a.md` §1, table "P4.21 — stale commands" | | |
| P4.22 infeasible | `crates/flexcore-grid-api/tests/rejection_infeasible.rs` | 8 (the routing-decision pin, F-IF-1..F-IF-4 single-violation fixtures, F-IF-5 non-convergence, F-IF-6 stale model + the control) | `20_22_rejection_suites_a.md` §1, table "P4.22 — infeasible commands" | | |
| P4.23 unauthorized | `crates/flexcore-application/tests/rejection_unauthorized.rs` | 9 (least-privileged-role denial, tenant-kind-approval denial, the correct-role control, proposer self-approval, cross-tenant block, expired scope, issue-without-APPROVED, the aggregate no-ISSUED exit check, the two-run determinism test) | `23_25_rejection_suites_b.md` §2, table "P4.23 — unauthorized" | | |
| P4.24 duplicate | `crates/flexcore-application/tests/rejection_duplicate.rs` | 7 (derivation-property tests ×2, fixtures (a)–(d), the two-run determinism test) | `23_25_rejection_suites_b.md` §2, table "P4.24 — duplicate" | | |
| P4.25 expired | `crates/flexcore-application/tests/rejection_expired.rs` | 6 (four stage fixtures APPROVED/ISSUED/ACKNOWLEDGED/TRACKING, the never-early/end-exclusive guard, the fallback-only-where-required test) | `23_25_rejection_suites_b.md` §2, table "P4.25 — expired" | | |
| P4.26 combined/adversarial | `crates/flexcore-application/tests/rejection_combined.rs` | 14 (F-CM-1 ×2, F-CM-2 ×2, F-CM-3 ×3 incl. the inactive-model control, F-CM-4 ×2, F-CM-5 ×3, the aggregate no-ISSUED exit check, the two-run determinism test) | `26_33_combined_matrix.md` §2, fixture table F-CM-1..F-CM-5 | | |
| Appendix — P4.33 transition matrix (§22.3 input, not a rejection suite) | `crates/flexcore-domain/tests/transition_matrix.rs` | 9 (full-matrix cell test, legal-edge walk, illegal-pair rejections, terminal-row cells, named shortcuts, totality check, freeze-§2.1 outgoing-count diff, bogus-edge mutation demonstration, 14-state vocabulary pin) | `26_33_combined_matrix.md` §3 | | |

### 1.1 Pinned versions (environment of record)

| Item | Pin | Where recorded | Exercised by these suites? |
|---|---|---|---|
| Grid-model provider | pandapower **3.5.6** (BSD-3-Clause; D4 audit OPEN) | P4.01 decision note `01_02_grid_provider.md` §2; coordinator note in `crates/flexcore-grid-api/Cargo.toml` | **NO.** P4.22 drives the grid-model PORT over the scripted backend double (suite header; D-P4.22-3) — no pandapower execution, no worker process, no I/O. The Python worker requirements/lockfile carrying `pandapower==3.5.6` does not exist yet (the pin is RECORDED; the lockfile file is the coordinator's P4.01 §2 step, pending) |
| Scheduler Tier 1 solver | HiGHS **1.15.1** via the Python binding **highspy 1.15.1** (LP/QP/MIP — never SOCP) | P3.17 decision note `17_optimizer_port.md` §1 | **NO.** The feasibility gate runs no optimization (`35_gate_benchmark_report.md` §2); the scheduler is not in this pack's path |
| Tier 2 conic solver | Clarabel.rs **0.11.1** (continuous convex conic — not MISOCP) | P3.18 decision note `18_clarabel_tier2.md` §2 | **NO** (same reason) |
| Rust toolchain | `rust-toolchain.toml` channel — record the resolved version in each run identifier | repo root | YES (compiles the suites) |

Solver-class discipline (AGENTS.md rule 4) travels with the pins wherever
they are cited: HiGHS is LP/QP/MIP, Clarabel is convex conic, the feasibility
gate is an AC power-flow evaluation (never AC-OPF). No figure about any
solver is claimed in this pack: no solver ran.

### 1.2 Seed / clock configuration (identical for every suite)

- **Clock:** controlled everywhere. Each test constructs its own injected
  fixed `LifecycleClock` (e.g. `FixedClock` at one pinned millisecond
  instant); clock advances happen only where a fixture explicitly needs them
  (P4.21 F-ST-5, P4.25 all four stages, P4.26 F-CM-3 — each to a named,
  end-exclusive §10.3 boundary instant). **No wall-clock read exists in any
  suite; the suites are independent of the time they run at.**
- **Randomness / seeds:** none. No RNG, no generated inputs, no seeds — the
  ledger's "fixed fixture set, pinned inputs, no randomness" is literal.
  (Property-style suites with seeds exist in Phase 3; none is part of this
  pack.)
- **Network/transport:** none. No NATS, no PostgreSQL, no worker process —
  every seam (store, outbox, authz evaluator, transport, grid backend) is an
  in-test fake that fails closed by construction (decision-record §1 of
  `23_25_rejection_suites_b.md` and `26_33_combined_matrix.md`).

## 2. The two-clean-runs procedure (the human's gate pass)

### 2.1 Clean state — what it means here, exactly

Every test builds its full world at test start: fresh in-memory harness, a
fresh empty store seam, fresh empty journal, fresh empty `FakeEventOutbox`,
fresh ID generator, and its own fixed clock. There is no shared mutable
state between tests, no file, no database, no environment variable, and no
cached process state — so a "clean state" for a run is **the run itself**:
each test is its own clean room. Nothing needs to be reset between runs;
"from a clean state" is satisfied by running the commands below in a fresh
shell with no prior state, and it is verified structurally by the fact that
no suite writes outside the process. The build cache (`target/`) does not
carry test state and needs no cleaning for determinism purposes.

### 2.2 Commands — run the FULL pack, twice, in order

```sh
# Run 1 — the full pack (7 suites + the appendix), in one shell, in order:
cargo test -p flexcore-application --test rejection_invalid
cargo test -p flexcore-application --test rejection_stale
cargo test -p flexcore-grid-api       --test rejection_infeasible
cargo test -p flexcore-application --test rejection_unauthorized
cargo test -p flexcore-application --test rejection_duplicate
cargo test -p flexcore-application --test rejection_expired
cargo test -p flexcore-application --test rejection_combined
cargo test -p flexcore-domain      --test transition_matrix   # appendix

# Run 2 — the same commands again, unchanged, from a fresh shell.
```

The equivalent single command (`cargo test --workspace`, the CI
`unit-tests` job) is acceptable as a run identifier ONLY if both runs use
it — the two runs must be identical in form. Do not mix a per-suite pass
with a workspace pass and call them two runs of the same pack.

**Full-pack re-run rule (ledger P4.27 risk note, binding):** if ANY suite
fails, the phase is not complete. Record the failing fixture and the fix,
then re-run the pack IN FULL — never only the failing suite. A partial
re-run is not a run of the pack.

### 2.3 Identical-outcomes assertion — the two layers

1. **In-suite (already written, runs as part of each execution):** every
   fixture closure runs twice with identical inputs inside the test and
   asserts the outcomes equal (`assert_eq!` on the error/verdict/outcome
   values) — D-P4.20-2, D-P4.26-3, and the aggregate determinism tests in
   P4.23/P4.24. This is the deterministic-behaviour PROOF; it holds within
   one process at one instant.
2. **External two-run comparison (what fills §1's run cells):** run the
   §2.2 commands twice. Compare the two outputs' per-test result lines:
   the assertion is **the same set of tests, with the same pass/fail
   outcome, in both runs** (61 rejection-suite tests + 9 appendix tests).
   Any difference — a pass flipping to fail, a test appearing in one run
   and not the other, a different failure message — is a determinism
   FAILURE of the pack: investigate, fix, and re-run the full pack per
   §2.2. Record for each run: date (UTC), git commit hash, toolchain
   channel, command form, and the pass/fail count line per suite.

A green pack = 70/70 passing, identically, in both runs. That — and only
that — plus the §3 sign-offs holds the §23 Stage 4 gate.

## 3. Reviewer sign-off table

Sign-offs are HUMAN gate inputs (ledger P4.27 step 3). Every cell below is
**NOT SIGNED (gate input)** — the pack was assembled by an agent loop that
cannot sign, and no review has happened. A reviewer fills a cell only after
reading the suite's decision record and the run output; "NOT HELD" stays in
any cell not so filled, and an unsigned gate is an unmet gate.

Reviewer scopes per the ledger: **power-systems** (the infeasible/stale
fixtures — magnitudes, single-violation design, P4.08 policy split),
**security** (unauthorized/duplicate/forged coverage against §17.3–§17.4),
**QA** (determinism form, CI wiring, pre-registration discipline).

| Suite | Power-systems | Security | QA |
|---|---|---|---|
| P4.20 invalid | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.21 stale | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.22 infeasible | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.23 unauthorized | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.24 duplicate | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.25 expired | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| P4.26 combined/adversarial | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |
| Appendix P4.33 transition matrix | NOT SIGNED (gate input) | NOT SIGNED (gate input) | NOT SIGNED (gate input) |

Review-input pointers per row (what each reviewer reads): the suite's
decision record (§1 references) + that record's "Review status" section +
the §2 run outputs. P4.20–P4.22 also carry named review asks in
`20_22_rejection_suites_a.md` §5 (fixture magnitudes 0.88/1.12 pu, 105 %
loadings; determinism form); P4.26/P4.33 carry theirs in
`26_33_combined_matrix.md` §8 (F-CM-2/F-CM-4 vs §17.4; matrix
exhaustiveness argument).

## 4. Determinism statement and honest limitations

**Determinism statement.** All 70 tests use pinned inputs, an injected
controlled clock, no randomness, no network, no persistence outside the
process, and no timing-dependent assertion. Their outcomes are functions of
the inputs alone; the in-suite double-run assertions (§2.3 layer 1) plus
the two external runs (layer 2) are the recorded evidence form. The
simulation signing keyring produces a deterministic NON-cryptographic
signature reference (D-P4.16-1) — traceability only, no security claim.

**Limitations carried into the gate, named, none silently filled:**

1. **First-failure-only gate/journal surfaces (D-P4.26-2).** The validation
   gate, the authorization gate, and the adapter refusal order each record
   ONE failing check per rejection; only the pure §14.3 verdict layer lists
   all failing checks (F-CM-5 asserts both names there). Whether a
   multi-reason journal record is wanted is an open enforcement-code
   question for the human review — it would touch `GateResult`/
   `CommandTransition` shapes and every suite.
2. **Zero clock-skew tolerance gap (D-P4.21-3).** The implemented future
   tolerance is ZERO (the P2 time-model ingest ordering rule refuses any
   strictly-future timestamp); a non-zero configurable future tolerance on
   the command path is NOT implemented and is an open review item. F-ST-6
   pins the boundary that exists; nothing asserts enforcement that is not
   there.
3. **P4.28 re-verification open item.** The unauthorized/duplicate/forged
   suites exercise existing enforcement points through in-test fakes
   (mirrored grant matrix D-P4.23-2; store-seam duplicate/sequence contract
   D-P4.24-1). When P4.28 lands, its re-verification must confirm: (a) the
   real `flexcore-authz` policy set wired into the gate evaluator seam; (b)
   cross-tenant/out-of-scope denials appended to the audit trail via the
   `AuditSink`; (d) real signature verification at the adapter (tampered
   payload fails, missing signature rejected) with F-CM-4's expectations
   re-verified and extended by decision-record revision — today signatures
   are non-cryptographic simulation references (D-P4.26-4).
4. **P4.30 re-verification open item.** When P4.30 lands, it must confirm:
   (c) the production store implements the duplicate/sequence seam contract
   of D-P4.24-1; (e) the production idempotency-key derivation satisfies
   the full-payload+sequence coverage property pinned in
   `rejection_duplicate.rs`; (f) rejected-attempt audit rows for illegal
   transition attempts, where the audit policy requires one (D-P4.33-3 —
   the domain refusal is a returned named error; no persistence exists at
   the domain layer).
5. **P4.22 exercises the grid layer only.** The command-layer half of the
   VALIDATED → feasibility path lives behind the `CommandFeasibilityRunner`
   seam; the verdict-to-rejection mapping is pinned by the existing
   `crates/flexcore-application/tests/command_gates.rs` suite and cited,
   not duplicated. The grid fixtures run against scripted backend doubles —
   an honest consequence is that the pack proves ROUTING and limit-matching
   logic, not power-flow numerics; the real pandapower worker leg is the
   P4.31 integration layer.
6. **Cross-tenant denial of record.** The cross-tenant approval attempt is
   refused `NotFound` (existence never leaks, P0.16 rule 5); its
   caller-visible refusal is the record of record today, and the AuditSink
   append for it is the P4.28/P4.30 wiring (D-P4.23-3).
7. **The P4.33 mutation check is pending the human pass.** The ledger's
   verification method ("add a bogus transition, confirm failure, revert")
   requires a build+run; the suite's mechanical demonstration
   (`a_bogus_added_transition_would_fail_this_matrix`) is written, not
   executed.
8. **Nothing in this pack has ever been executed.** Every expectation in
   the cited tables is PRE-REGISTERED and UNRUN. When the suites are first
   run, a mismatch against a pre-registered table is a FINDING to be
   investigated — never a reason to edit the table to fit (the P3.26
   anti-fitting rule). Compile fixes on first build are expected to be
   mechanical and are part of the human pass.
9. **Research cut-off:** no external sources were consulted in assembling
   this pack; the 7 November 2026 cut-off is unchanged.

## 5. Filing note — where this pack is cited

- **Phase 5 (P5.04, feasibility evidence display):** the pack is the Stage 4
  evidence base P5.04's display consumes — every feasibility verdict the
  console shows must carry the model version + hash, outcome, margins, and
  evidence references this pack's suites pin (P4.12/P4.07), and must repeat
  the acceptance-is-not-delivery and no-timing-guarantee wording
  (`17_19_fallback_timing.md` D-P4.19-3). The Phase 4 handover note (P4.32)
  carries the pack location to Phase 5.
- **§22.3 release gate:** the gate's clause "all command-state transitions
  are tested" is evidenced by the P4.33 appendix row (the exhaustive
  14×14 matrix, decision record `26_33_combined_matrix.md` §3); the gate's
  Stage 4 clause ("invalid, stale, infeasible, unauthorized, duplicate, and
  expired commands rejected in deterministic tests") is evidenced by this
  pack's six suite rows once §1's run cells and §3's sign-offs are filled.
  P4.32 step 4 cites this pack for the exit-gate sign-off; the CI scenario
  regression suite (§22.2) runs the same suites per D-P4.20-3 (the existing
  `cargo test --workspace` job — no CI file was modified).
