# P8.37 — Unit and property tests for research workers (WRITTEN-NEVER-RUN — the suites exist; no run has happened)

Date: 2026-11-14. Subtask: P8.37 (`ledger_phase_8.md` §P8.37). Owner role:
Research engineer (per module). Report trace: §19.2(1) (unit tests);
§19.2(2) (property tests); §10.2 (units/signs). Inputs: P8.07 (the ADMM
provider — Rust-side, see the ownership note in §2); P8.11 (the GNN
inference worker); P8.14 (the MARL worker family); P8.28 (the CI gates
the suites run in); the pure-logic worker modules under
`apps/worker/python/` (read in full while writing the tests — every
tested constant and refusal below was verified against the module
source, not assumed); the existing per-worker test suites (the unit
layer this subtask extends, never modified).

## 1. Status of this record — WRITTEN, NEVER RUN

This record is the P8.37 deliverable: the test-coverage map (§3), the
property-oracle register (§4), and the findings the test-writing
surfaced (§6). **No test in this record has ever been executed**: the
loop bans forbid running tests, pytest was never installed, and the CI
run (P8.28's job) has not happened. Every expected value in the suites
is a HAND-DERIVED oracle stated in the test docstring (never an
observed output). The tick on P8.37 means *implemented, awaiting human
build/test* — the human gates are the first pytest run, the fresh-seed
property rerun (the ledger's verification method), and the CI run with
per-worker coverage recorded (§5 records the coverage map the run
fills).

The two-layer structure, per the ledger:

- **Unit layer** (§19.2(1)): the per-worker pytest suites from the
  earlier waves — `tests/test_*.py` beside each worker — mirror each
  module's hand-computed self-tests (units and signs §10.2, closed
  vocabularies, boundary values SOC 0/1, zero-length/empty-argument
  refusals, error shapes). This subtask did not duplicate them; §3 maps
  them.
- **Property layer** (§19.2(2)): the NEW suites this subtask adds —
  hand-written, stdlib-only, fixed-seed generators (no property-testing
  library exists in this repository and none was installed), each
  property carrying its ORACLE and its determinism contract in the
  docstring.

## 2. Ownership note (the ADMM row, honestly scoped)

The ledger's P8.37 dependencies name P8.07 (ADMM/DMPC provider). The
ADMM provider is Rust (`crates/flexcore-research`) — **another agent's
concurrent file ownership this wave**; a property suite for it belongs
to that owner's surface, not this one (touching `crates/**` is outside
this subtask's file ownership). The Python research workers (the P8.11
GNN worker, the P8.14 MARL worker family, the P8.17 dynamic metrics,
the P8.20 reproducibility tooling, and the P8.22 analytics) are fully
covered here. The gap is recorded, not hidden: **the ADMM provider has
no property suite from this subtask** — a Phase-8 follow-up for the
crates-side owner (§6, G-4).

## 3. Test-coverage map (module → test file → what it pins)

All paths relative to `apps/worker/python/`. "NEW" = added by this
subtask (additive only; no existing file was modified). Every suite is
run by the same recorded command shape:
`PYTHONPATH=<module dir> python -m pytest <module>/tests -v`.

| Module (under test) | Test file | Layer | What it pins |
|---|---|---|---|
| `marl_worker/safety_filter.py` | `marl_worker/tests/test_safety_filter.py` (existing) | unit | permission-ceiling gate, P/ramp projection arithmetic (hand-computed MW figures), the infeasible-filter outcome with the provably dead fallback, non-finite refusals, malformed-input error class, closed vocabularies, the sim-lab profile pin |
| `marl_worker/marl_worker.py` | `marl_worker/tests/test_marl_worker.py` (existing) | unit | request refusals, result-envelope posture, closed P8.04 payload schema, AST import-set pin (no transport), the no-dispatch-callable introspection, mode lock |
| `marl_worker/shadow_log.py` | `marl_worker/tests/test_shadow_log.py` (existing) | unit | entry schema (closed field set), sequence order, the mandatory closed filter-outcome key, the fabricated-consequence refusal, summary counts |
| `marl_worker/evaluation_harness.py` | `marl_worker/tests/test_evaluation_harness.py` (existing) | unit | filter statistics with stated fractions, case metrics, per-metric verdicts at stated tolerances, every named refusal |
| marl family (all four modules) | `marl_worker/tests/property_test_marl_worker.py` (NEW) | property | P-VOCAB (outcome always in the closed vocabulary for arbitrary P/ramp), P-ENVELOPE (no setpoint outside the feasible interval, recomputed independently; refusing outcomes emit no proposal), P-SCAN (an injected dispatch key is caught at ANY nesting depth; case/whitespace variants too), P-REFUSE (the evaluation harness refuses every corrupted shape — a number is never produced from malformed input), P-STATS (statistics internally consistent for arbitrary valid logs; reordered logs refused), replay determinism (identical input → identical report) |
| `gnn_worker/gnn_inference_worker.py` | `gnn_worker/tests/test_gnn_inference_worker.py` (existing) | unit | request/graph/feature/manifest validation, the scope check, the proposal-only scan, the closed result schema, the missing-artifact loud failure, the guarded-import named requirements |
| `gnn_worker/gnn_evaluation.py` | `gnn_worker/tests/test_gnn_evaluation.py` (existing) | unit | end-to-end feasible time (inference included, NOT_A_SUCCESS refusals), speedup, success rate, screening metrics with miss costs, the overlap and leakage-control checks |
| gnn family (both modules) | `gnn_worker/tests/property_test_gnn_scope.py` (NEW) | property | P-SCOPE (scope enforcement is EXACT string equality for arbitrary scopes/trained lists; near-misses — prefix, suffix, case, whitespace, unicode look-alike — never pass; an empty trained list flags EVERYTHING, the P8.35 manifest's current fail-closed state), P-GRAPH (every malformed graph refused; valid graphs preserved verbatim incl. parallel edges), P-FEATURE (every range/shape violation refused; conforming rows accepted for arbitrary matrices), P-OVERLAP (leakage iff the trained and held-out sets intersect; the P8.35 manifest's family ids are disjoint) |
| `reproducibility/seed_derivation.py`, `record_schema.py`, `rerun_check.py` | `reproducibility/tests/test_reproducibility.py` (existing) | unit | published FNV-1a vectors, splitmix64 structure, the schema refusals, the comparator boundary arithmetic (8.25−8.0 = 0.25 ≤ 0.25; 8.3−8.0 ≈ 0.3 > 0.25) |
| reproducibility (all three modules) | `reproducibility/tests/property_test_reproducibility.py` (NEW) | property | P-SEED (derivation deterministic, range [0,2^64), the XOR-structure identity; sampled-key distinctness labelled SAMPLE evidence, not a theorem; invalid inputs refused), P-SCHEMA (EVERY required-field removal is named — a property over the whole tuple, not hand-picked fields; every placeholder token refused; the within-tolerance class requires the P8.38 pre-registration ref + nondeterminism disclosure), P-RERUN (the tolerance boundary is INCLUSIVE; the exact class fails on any nonzero deviation; distribution/missing-tolerance/key-mismatch are refusals, never verdicts; every identity-field mismatch is named) |
| `portfolio_analytics/analytics.py` | `portfolio_analytics/tests/test_portfolio_analytics.py` (existing) | unit | the reporting-only scan, the energy arithmetic (stated figures), the calibration coverage, the baseline-accounting comparison and its refusals |
| `portfolio_analytics/analytics.py` | `portfolio_analytics/tests/property_test_portfolio_analytics.py` (NEW) | property | P-REPORT (an injected dispatch-shaped key caught at ANY depth; case variants caught; clean trees pass), P-ENERGY (per-event formula recomputed independently; shortfall and over-delivery never both positive; aggregates ADDITIVE and recomputable from per_event; the zero-denominator refusal), P-CALIB (coverage ∈ [0,1] and equals the recomputed fraction; closed-band boundary actuals covered; inverted/non-finite/mixed-unit refused), P-REFUSE (every value corruption of an event refused; the comparison refuses a missing/invalid tolerance and non-aggregate totals) |
| `andes_worker/evaluation_harness.py` | `andes_worker/tests/test_evaluation_harness.py` (existing) | unit | the hand-computed metric figures (nadir, ROCOF two-segment artifact, damping closed form, settling re-excitation) |
| `andes_worker/evaluation_harness.py` | `andes_worker/tests/property_test_andes_harness.py` (NEW) | property | P-REFUSE (every series corruption refused by every metric), P-EXTREME (nadir/zenith equal the recomputed window min/max with FIRST-achieving times), P-ROCOF (exact interior slopes on piecewise-linear signals; the two-sample refusal), P-SETTLE (the verdict matches the recorded definition recomputed — including never-settled, never extrapolated), P-DAMP (the closed form when peaks are sampled exactly; a growing envelope refused) |

Not covered here (recorded, not hidden): the Rust-side ADMM provider
(§2, G-4); the ANDES worker's transport/AND-facing code paths (they
require the ANDES 2.0.0 pin — the human build/test pass; the pure
metric module is covered); the grid feasibility worker's pandapower
mapping (`grid_worker.py` is a Phase 5 worker with UNVERIFIED-API
mappings — its first run IS the test, per its honesty header; it is not
a Phase 8 research worker).

## 4. The property-oracle register (the ledger step-2 requirements, as stated oracles)

| # | Property (ledger P8.37 step 2 examples mapped) | Suite | Oracle | Determinism |
|---|---|---|---|---|
| 1 | "a worker never returns a setpoint outside the declared capability envelope" | marl P-ENVELOPE | the interval arithmetic `[max(p_min, p_current−ramp), min(p_max, p_current+ramp)]` recomputed INDEPENDENTLY in the test from the raw limits | `random.Random(20261114)`, fixed in-file; 400 cases; boundary-biased |
| 2 | safety-filter outcome always in the closed vocabulary | marl P-VOCAB | membership in the closed `FILTER_OUTCOMES` / `ENTRY_OUTCOMES` frozensets | same seed; 300 arbitrary actions |
| 3 | proposal-only scan catches an injected dispatch key at ANY nesting depth | marl P-SCAN; portfolio P-REPORT | the closed `FORBIDDEN_RESULT_KEYS` / `DISPATCH_SHAPED_KEYS` sets (case/whitespace-normalized) | same seed; random trees depth 1–12, injected key at a random position; negative control without injection |
| 4 | the validation harness refuses any malformed input shape | marl P-REFUSE; andes P-REFUSE; portfolio P-REFUSE | the named refusal classes (`EvaluationHarnessError`/`ShadowLogError`, `MetricInputError`, `AnalyticsInputError`) — a NUMBER is never produced from malformed input | same seed; a corruption catalogue applied to valid constructed inputs |
| 5 | scope enforcement denies every out-of-scope read for arbitrary scopes | gnn P-SCOPE | exact string equality — `in_scope` iff the id is an element; near-misses never pass; empty list flags everything | same seed; arbitrary id/managed-list triples + mutation battery. NOTE (recorded honestly): the research workers carry no TENANT concept; the platform's tenant isolation is P8.31's crates-side surface (another owner's files). The scope-enforcement surface this directory owns is the topology scope, and that is what is property-tested here — recorded as the mapping, not silently renamed |
| 6 | invariant preservation across generated asset states / constraints | reproducibility P-SEED/P-RERUN; portfolio P-ENERGY; gnn P-GRAPH/P-FEATURE | the derivation identity, the comparison rules, the stated formulas, the graph/feature contracts — each recomputed independently in the test | same seed everywhere |

Generator discipline (binding for future suites): stdlib `random.Random`
only, one module-level seed constant, every generator derived from it,
each property's docstring names oracle + seed + case count, and the
property must hold for ANY seed (a fresh seed is the ledger's
verification method — §5). A generator that only produces the
happy-path distribution is the ledger's named risk; the catalogues
above include malformed, boundary, and hostile inputs by construction.

## 5. CI and the verification method (both human gates, UNRUN)

- The suites run under the P8.28 CI jobs (`research-python-gates`,
  `research-worker-conformance`) once the human gate runs CI at all —
  no CI run has happened (P8.28's record is honest about the same
  status). Coverage per worker is RECORDED FROM THE RUN, not asserted
  here: the table in §3 is the map; the per-worker coverage figures are
  empty until the first run fills them.
- The ledger's verification method: "run one property suite with a
  fresh seed and confirm it passes". Procedure: change the
  `PROPERTY_SEED` constant at the top of any `property_test_*.py`, run
  the suite, confirm pass; the properties are seed-invariant by
  construction (each docstring says so). A failure under a fresh seed
  is a finding against the property (or the generator), recorded and
  never absorbed.

## 6. Findings and contract gaps surfaced while writing the tests (recorded, never fixed here)

Rule: this subtask adds TESTS only. A suspected defect in another
owner's logic is a FINDING with file + line, never an edit.

| # | Finding / gap | Location | Disposition |
|---|---|---|---|
| F-1 | NOT A DEFECT — a verified strength worth recording: the shadow-log validation refuses ANY sequence reordering (`sequence == index + 1` exactly), so the P-STATS permutation property can never silently pass; the same strictness in `filter_statistics` means a gapped log is a named refusal, not a partial statistic | `apps/worker/python/marl_worker/shadow_log.py:256-264` | recorded (no action) |
| F-2 | The corruption catalogue in the MARL suite deliberately treats a MISSING required field as a caller SIGNATURE error (a `TypeError`, not a harness refusal) for the keyword-only `build_case_consequence`, while the slice-level test covers the missing-record shape through the log schema. The asymmetry is the module's real contract (keyword-only arguments vs validated values) — recorded so a future reader does not "fix" either side blindly | `apps/worker/python/marl_worker/evaluation_harness.py:245-252` (signature) vs the refusal body | recorded (no action) |
| F-3 | `_is_finite_number` in the MARL harness accepts an int/float only AFTER the caller's own bool exclusion; the property suite therefore corrupts `constraint_violation` with non-bool types only, because a bool IS the valid type there. Recorded as a test-design note (the validator's bool exclusion is load-bearing and pinned) | `apps/worker/python/marl_worker/evaluation_harness.py:618-626` | recorded (no action) |
| F-4 | OWNERSHIP GAP: the ADMM provider (P8.07, Rust) has no property suite from this wave — the crates-side files are concurrently owned. Follow-up: the crates owner adds a Rust-side property suite (residual-decrease and termination-bound invariants are the natural properties) — never invented in another owner's crate this wave | `crates/flexcore-research` (not touched) | recorded Phase-8 follow-up |
| F-5 | COVERAGE-RUN GAP: per-worker coverage FIGURES are a run product; no run has happened. The §3 map is the static coverage evidence; the figures land in the P8.28 CI record at the first run | this record §3; `28_ci_gates.md` | recorded; fills at the human gate |
| F-6 | The GNN scope check compares against `trained_topology_ids` carried IN THE REQUEST (the manifest-in-request stopgap, P8.10 §7). A property suite can only test the check's semantics, not the trustworthiness of a caller-supplied list — the trust question resolves with the P8.20 store + P8.32 signing (recorded there; repeated here so a test reader does not mistake the property for an end-to-end guarantee) | `apps/worker/python/gnn_worker/gnn_inference_worker.py:611-625` | recorded dependency, owned by P8.20/P8.32 |

## 7. Reviewer-verification procedure (the human gate)

```sh
# (a) one command per module (the recorded procedure; from the repo root):
PYTHONPATH=apps/worker/python/marl_worker python -m pytest apps/worker/python/marl_worker/tests -v
PYTHONPATH=apps/worker/python/gnn_worker python -m pytest apps/worker/python/gnn_worker/tests -v
PYTHONPATH=apps/worker/python/reproducibility python -m pytest apps/worker/python/reproducibility/tests -v
PYTHONPATH=apps/worker/python/portfolio_analytics python -m pytest apps/worker/python/portfolio_analytics/tests -v
PYTHONPATH=apps/worker/python/andes_worker python -m pytest apps/worker/python/andes_worker/tests -v

# (b) the ledger's fresh-seed verification method: change PROPERTY_SEED
#     in ONE property_test_*.py, re-run that suite, confirm pass.

# (c) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green (a)–(c) earns a one-line human-gate note appended here; any
failure is recorded verbatim in this record's findings and the ledger
tick reverts to `- [ ]`.

## 8. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Every research worker has a unit suite and a property suite | UNIT: yes for every Python research worker (existing suites, mapped §3). PROPERTY: yes for the MARL, GNN, reproducibility, portfolio, and dynamic-metric surfaces (NEW suites). The Rust ADMM provider is the recorded exception (F-4, ownership) |
| Both run in CI | **UNRUN** — no CI run has happened (P8.28's status); the suites are CI-shaped and the map is recorded |
| Coverage per worker is recorded | THE MAP is recorded (§3); the per-worker coverage FIGURES are run products, empty until the first run (F-5) |
