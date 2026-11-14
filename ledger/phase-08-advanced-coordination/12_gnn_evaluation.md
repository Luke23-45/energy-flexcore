# P8.12 — GNN evaluation: end-to-end feasible solve time vs baselines, on unseen topologies (TEMPLATE — every measured cell EMPTY)

Date: 2026-11-13. Subtask: P8.12 (`ledger_phase_8.md` §P8.12). Owner role:
research engineer (ML). Report trace: §13.1; §19.3–§19.4; §23 Stage 8.
Inputs: P8.10 (`10_gnn_warmstart.md` — the design and the hold-out
RECOMMENDATION) and P8.11 (`11_gnn_inference_worker.md` — the worker this
template evaluates); P8.05 (`05_06_harness_baselines.md` §2); P8.06
(`05_06_harness_baselines.md` §3 — the frozen baselines; the GNN module
mapping); P8.02 (`02_research_scope_freeze.md` — question M2, prohibitions
X-1/X-7/X-8); ENTRY_NOTE.md boundary 4.

## 1. Status of this record — a TEMPLATE, nothing measured, and (today) nothing measurable

**Every measured cell in this document is EMPTY**, and there is a stronger
honest statement: **no measured cell is fillable today at all** — the
torch/PyG framework pair is a recorded **DECISION-POINT OPEN**
(`03_research_license_audit.md` §4; D4 OPEN; R8-C3 skew): no framework is
selected, pinned, installed, or verified in this repository, therefore no
inference output exists (`11_gnn_inference_worker.md` §1 — the worker's
guarded imports are the honesty gate), therefore no end-to-end run, no
held-out solve, and no screening ranking exists. Additionally, the
hold-out assignment is **UNCONFIRMED until the P8.35 training-data
manifest exists** (P8.10 §4), and the worker as implemented REFUSES
held-out topologies (the recorded scope-semantics follow-up, §7 below).
The runs are **human/lab gate inputs**, twice over.

What exists and is verifiable to the extent the loop allows:

- the **evaluation protocol** (`apps/worker/python/gnn_worker/
  gnn_evaluation.py`, `flexcore-gnn-evaluation/0.1.0`) — pure, stdlib-only
  metric definitions computed on INJECTED data, each pinned by
  HAND-COMPUTED self-tests; it reuses the worker's OWN pure functions
  (`rank_candidates`, `evaluate_topology_scope`) — one definition of the
  ranking order and the scope semantics, never a parallel one. Written,
  **never executed**;
- the **pytest mirror** (`tests/test_gnn_evaluation.py`) — additive,
  written, never run;
- the **filling procedure** (§8).

**The test-arithmetic / experiment-result distinction (stated explicitly,
as the subtask requires):** the self-tests assert hand-derived values over
toy inputs (e.g. solver 2.0 s + validation 0.5 s = 2.5 s end-to-end; a GNN
run with inference 0.1 s gives 0.1 + 2.0 + 0.5 = 2.6 s; five candidates
with true active {c1, c3} screened at depth 2 give precision@2 = 0.5,
miss rate = 0.5, miss cost = 7.0 from c3's recorded cost). That is
**test-grade arithmetic pinning the metric definitions** — NOT a benchmark
measurement, NOT a seed result, NOT evidence about any topology.

## 2. Role (a) — the warm-start experiment: end-to-end feasible solve time ONLY

**The headline rule, VERBATIM (§13.1; carried in the worker's boundary
statements and pinned by self-tests):** a warm start changes only the
exact solver's starting point: the solver still solves the problem to its
own termination criteria — the GNN does not replace the solver, and a
warm start affects **SPEED of an exact solve, never its optimality**.
Nothing in this evaluation measures, asserts, or scores optimality; the
speedup metrics compare TIMES of successful validated solves only.

The metric definitions (protocol functions, over injected per-run
records):

| Metric | Function | Rule enforced in code |
|---|---|---|
| End-to-end feasible solve time (THE HEADLINE) | `end_to_end_feasible_time` | inference + solver + validation up to a FEASIBLE, INDEPENDENTLY VALIDATED solution; **never inference latency** — a run without a recorded validation component is refused (`VALIDATION_COMPONENT_MISSING`); a run that did not reach feasible+validated has NO time (`NOT_A_SUCCESS`) — if the solver fixed an infeasible warm start, that is the SOLVER's success, not the GNN's (the ledger's named risk, refused in code) |
| Feasible-solver success rate | `feasible_success_rate` | fraction reaching feasible+validated; a warm start must not reduce this (P8.02 M2a); failures are failures, never averaged in |
| Paired speedup vs baselines | `speedup` / `paired_speedups` | pairs by EXACT topology_id; ratio of END-TO-END times of two successful validated solves; non-positive/non-finite times refused; a treatment FAILURE is recorded as a failure (never in the speedup table); an unpaired success is LISTED, never silently dropped |
| Generalization | per-topology rows on held-out topologies ONLY (§4) | any generalization claim cites held-out runs only (X-7) |

**The baselines (P8.06 module mapping `GnnWarmStart`):** the cold-start
Tier 1 HiGHS solver (`baseline/highs-tier1-lp` — LP/QP/MIP, never SOCP) and
the rule-based record's non-GNN heuristic warm start
(`baseline/rule-based-priority-list`). Both run on the SAME topologies,
seeds, and tolerances as the GNN runs; their objectives/timings come from
the frozen P8.06 results artifacts, which do not exist until the first
human/lab run.

## 3. Role (b) — the screening experiment: ranking quality and miss cost

A warm-start win is NOT a screening win — the two roles' numbers are
never merged into one headline (P8.02 M2; pinned by tests). The
falsifiable question (frozen, P8.02 M2b): on held-out topologies, does
GNN screening rank constraints/contingencies that are actually active
above inactive ones at a pre-registered miss rate and miss cost?

| Metric | Function | Rule |
|---|---|---|
| Ranking order | the worker's own `rank_candidates` | descending score, candidate_id-ascending tie-break (the replayable P7.42 order) — imported, never duplicated |
| Precision@screen-depth | `screening_metrics` | hits / screen_depth over the top-k screened-in set |
| Miss rate | `screening_metrics` | \|misses\| / \|true_active\| against the recorded ground truth from the exact solves; empty true-active refused (`EMPTY_TRUE_ACTIVE`) |
| Cost of a missed constraint | `screening_metrics` | Σ of the RECORDED per-candidate miss costs; a miss without a recorded cost is refused (`MISS_COST_UNRECORDED`) — the cost model is pre-registered, never a fabricated zero |
| Screening uncertainty | carried from the worker | every ranking entry carries score AND uncertainty (§13.1: surrogate estimates with uncertainty); a screening output is a proposal, never a decision, and no screening output removes a constraint from any solve |

No threshold is applied anywhere (X-8): the metrics are raw; the
falsification verdicts are the human evaluator's against the
pre-registered thresholds (P8.06 freeze time at the earliest; the
protocol's `PRE_REGISTRATION_STATUS` constant carries this verbatim).

## 4. The held-out topology protocol (P8.10 §4, carried UNCONFIRMED)

The RECOMMENDATION, verbatim status (pinned by tests,
`gnn_evaluation.HOLDOUT_STATUS`): **RECOMMENDATION — UNCONFIRMED until
the P8.35 training-data manifest exists** — training families
lib-01…lib-05; held-out families lib-06/lib-07/lib-08 PLUS at least one
structurally unseen feeder family; held-out operating regimes = the P8.19
research-case extensions. Named hold-out assignments must be confirmed
against the P8.35 manifest BEFORE any training run, and the reviewer's
overlap check is the evidence.

| Protocol element | Where it lives | Status |
|---|---|---|
| Overlap check (the ledger's verification method, in code) | `check_heldout_overlap(trained, heldout)` — exact string equality; a non-empty overlap is a recorded LEAKAGE finding, invalidating the generalization claim for the overlapping topologies | WRITTEN-UNRUN |
| Leakage control 1 — topology | the overlap check above + `verify_leakage_controls` field `heldout_topology_overlap_checked` | WRITTEN-UNRUN |
| Leakage control 2 — normalization | normalization statistics computed on the TRAINING split only, frozen with the artifact; `verify_leakage_controls` field `normalization_split == "training_only"` | WRITTEN-UNRUN; enforcement at P8.35 ingestion + the training pipeline |
| Leakage control 3 — regime | held-out regime seeds disjoint from and recorded against training seeds; `verify_leakage_controls` field `regime_seeds_disjoint_from_holdout` | WRITTEN-UNRUN |
| Dataset versioning | `dataset_version` linked to the model artifact (P8.35 step 4; P8.20 store; P8.32 signing) | WRITTEN-UNRUN |
| Scope flag (contract-correct behaviour) | `evaluate_topology_scope` — a held-out id is by definition outside the trained list, so the worker as implemented FLAGS it and returns NULL outputs (self-test pins this) | the recorded follow-up, §7 gap 1 |

## 5. Result tables — ALL MEASURED CELLS EMPTY

### 5.1 Warm-start table (held-out topologies ONLY for the generalization claim)

| Topology id (P8.35-confirmed; RECORD-AT-FIRST-RUN) | Held-out? (per manifest) | Strategy (cold / heuristic / gnn) | Inference (s) | Solver (s) | Validation (s) | End-to-end feasible time (s, HEADLINE) | Feasible+validated? | Paired speedup vs cold | Paired speedup vs heuristic | P8.05 validation outcome | Model artifact ref + manifest ref |
|---|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

### 5.2 Screening table (same held-out discipline)

| Topology id | True active set ref (exact-solve ground truth) | Screen depth | Precision@k | Miss rate | Miss cost (recorded cost model) | Uncertainty carried? | Baseline: heuristic ranking, same metrics | Refusals recorded (`EMPTY_TRUE_ACTIVE` / `MISS_COST_UNRECORDED` / out-of-scope flag) |
|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Filling rules (recorded before any number): cells come from the protocol
functions' outputs and the worker's result envelopes, never from memory;
a refusal is recorded AS THE REFUSAL; every counted "success" passed the
P8.05 independent validation (the ledger acceptance — the `NOT_A_SUCCESS`
refusal enforces it); the warm-start and screening tables are never
merged; the generalization claim cites held-out rows only (X-7); misses
and non-generalization are reported, never absorbed.

## 6. The evaluation note skeleton (ledger step 4 — written BEFORE the runs)

- **Where the GNN does not generalize** — EMPTY (this is a REQUIRED
  section; the ledger acceptance: "misses and non-generalization are
  reported").
- **What would be needed for it to** — EMPTY (more/better training
  topologies, regime coverage, architecture changes — measured from the
  per-topology rows, not asserted).
- Fixed limits stated now: generalization claimed only over the tested
  range (X-7); feeder-scale results are not 10,000-bus capability
  (§19.3); the GNN warms the solver and screens — it never replaces the
  solver, never removes a constraint, never dispatches (ADR-010; §13.4);
  a finite test set with zero misses is a test outcome, not a guarantee
  (§13.3; X-4); solver-class discipline VERBATIM (HiGHS is LP/QP/MIP,
  never SOCP; Clarabel is convex conic, never MISOCP; AC power flow is
  not AC-OPF — AGENTS.md rule 4).

## 7. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The scope-semantics gap (NEW, recorded by this evaluation):** the
   P8.11 worker FLAGS any topology outside the manifest's
   `trained_topology_ids` and returns NULL outputs — correct for
   production, but held-out topologies are BY DEFINITION outside the
   trained list, so the transfer experiment as designed cannot produce
   outputs from the worker as implemented (pinned by self-test 9 and the
   pytest mirror). What is needed: a recorded scope-extension mechanism —
   an explicit evaluation-scope manifest version, or an equivalent
   recorded P8.04 contract extension (e.g. an `evaluation_scope` field
   the flag logic honours), decided by the P8.04/P8.35 owners. NOT fixed
   by inventing a parallel contract or by bypassing the flag here.
2. **The closed result-key set carries no payload fields** (P8.04):
   carried from `11_gnn_inference_worker.md` §8 item 2 — the ranking
   entries and warm-start vectors travel beside the `result_payload`;
   the ADDITIVE extension proposal belongs to the P8.04 owner.
3. **No distinct screening payload type** (P8.04): carried (§8 item 3).
4. **Model manifest in-request** (P8.20/P8.32 seam): carried (§8 item 4)
   — the P8.12 evaluation only counts runs whose manifest matches the
   P8.35 training-data record.
5. **Runtime timeout enforcement** (VERIFY-BUILD): carried (§8 item 5).
6. **The torch/PyG DECISION-POINT is OPEN** (P8.03 §4; D4 OPEN): carried
   verbatim in `GNN_PIN_STATUS` and pinned by self-tests; the pin, the
   P8.36 at-pin-time check, and the license verification are the human
   gates (P8.10 §6).
7. **The frozen P8.06 baseline timings/objectives do not exist** (same
   gap as `09_admm_evaluation.md` §8.1): the speedup columns cannot be
   filled until the first human/lab run freezes them.

## 8. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the protocol's hand-computed self-tests (no framework, no pytest):
python apps/worker/python/gnn_worker/gnn_evaluation.py
#    expected: every hand-computed self-test PASSes (e2e 2.5/2.6 s;
#    speedup 2.5 / 0.4; success rate 0.5; paired {"t1": 2.5} with t2
#    failed + t3 unpaired; screening precision@2 0.5, miss rate 0.5,
#    miss cost 7.0; overlap finds t2; 4 findings for an empty record;
#    the held-out flag pinned; the verbatim statements carried).

# (b) the pytest mirror (plus the existing worker suite):
PYTHONPATH=apps/worker/python/gnn_worker python -m pytest \
  apps/worker/python/gnn_worker/tests -v

# (c) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (d) FIRST RUN — the fills of §4/§5 (human/lab inputs, in the recorded
#     ISOLATED research environment, never the platform runtime):
#     1. resolve the DECISION-POINT: pin the verified-compatible
#        torch/PyG pair per 10_gnn_warmstart.md §6 after the P8.36
#        advisory check; record the decision (P8.03 §4 update);
#     2. confirm the hold-out assignment against the P8.35 manifest;
#        run check_heldout_overlap and record the result (any overlap is
#        a finding against the generalization claim);
#     3. resolve the §7 gap 1 scope-extension follow-up (a recorded
#        decision by the P8.04/P8.35 owners) — until then, held-out runs
#        produce the contract-correct out-of-scope flag, NOT numbers;
#     4. train per the P8.10 design (offline, versioned artifacts) and
#        record model artifact + manifest refs;
#     5. per held-out topology: run cold-start, heuristic-warm-start,
#        and GNN-warm-start solves with pinned seeds/versions; record
#        inference/solver/validation components, feasible+validated
#        outcome, and the P8.05 harness outcome per run;
#     6. run the screening experiments against the exact-solve ground
#        truth with the pre-registered cost model recorded per candidate;
#     7. pre-register the numeric thresholds (P8.06 freeze record) BEFORE
#        any falsification verdict;
#     8. assemble §5 from the protocol functions' outputs; independent
#        re-run of two held-out topologies from the recorded artifacts
#        and verify the headline metric RECOMPUTES from the raw timings
#        (the ledger's verification method).
```

A green (a)/(b)/(c) earns only a one-line "human build/test gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger tick
reverts to `- [ ]`. The measured cells of §5 stay EMPTY until the (d)
runs fill them.

## 9. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Evaluation covers held-out topologies only for the generalization claim | Rule recorded (§4/§5); UNCONFIRMED until the P8.35 manifest; the overlap check is in code (`check_heldout_overlap`) |
| End-to-end feasible solve time (not inference latency) is the headline metric | ENFORCED IN CODE (`end_to_end_feasible_time` refuses a missing validation component; failures have no time); the headline rule carried verbatim |
| Every counted "success" passed independent validation | ENFORCED IN CODE (`NOT_A_SUCCESS` — feasible+validated or failure) |
| Misses and non-generalization are reported | Rule recorded (§5 rules; §6 REQUIRED section); no misses exist yet to report |

## 10. Non-claims (restated)

A warm start affects the speed of an exact solve, never its optimality;
the GNN never replaces the solver and never removes a constraint; screening
outputs are proposals with uncertainty, never decisions; no generalization
claim beyond the tested range; no safety claim and no field authority
(proposals only, ADR-010); no accuracy, latency, speedup, success-rate,
ranking-quality, or miss-cost figure from a real run exists anywhere in
this record, its protocol module, or its tests — every measured cell is
EMPTY, and (until the DECISION-POINT, the P8.35 manifest, and the scope
follow-up resolve) not yet fillable. The tests' toy arithmetic pins the
metric definitions and is not a measurement.
