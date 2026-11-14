# P8.23 — Cross-module comparison: one report, all modules, same cases, same metrics (TEMPLATE — every measured cell EMPTY, no comparison exists)

Date: 2026-11-14. Subtask: P8.23 (`ledger_phase_8.md` §P8.23). Owner role:
research lead. Report trace: §23 Stage 8 (exit gate); §19.4. Inputs:
P8.09 (`09_admm_evaluation.md` — the ADMM per-case §19.4 statistics) and
P8.12 (`12_gnn_evaluation.md` — the GNN warm-start/screening metrics);
P8.15 (`15_marl_evaluation.md` — the MARL per-case metrics and filter
statistics); P8.17 (`17_dynamic_evaluation.md` — the dynamic
per-disturbance metrics); P8.06 (`05_06_harness_baselines.md` §3 — the
frozen baseline registry: `baseline/highs-tier1-lp`,
`baseline/clarabel-tier2-conic`, `baseline/rule-based-priority-list`);
P8.19 (`19_benchmark_cases.md` + `benchmarks/research-set-v1.json` — the
frozen case set, version id **`flexcore-research-benchmark-set-v1`**,
cited on every row); P8.05 (`05_06_harness_baselines.md` §2 — the
independent validation harness whose outcome accompanies every cell);
P8.20 (`20_reproducibility.md` — the per-run records and rerun-test
outcomes the comparison's spot-checks consume); P8.02
(`02_research_scope_freeze.md` — prohibitions X-5/X-8 and the frozen
question list); ENTRY_NOTE.md boundary 4.

## 1. Status of this record — WRITTEN-UNRUN, ONE report template

This is the **single cross-module comparison report template** (the
ledger's "one report, all modules, same cases, same metrics"): one
report covers ALL modules — ADMM/DMPC (P8.09), GNN warm-start/screening
(P8.12), MARL (P8.15), the dynamic track (P8.17) — plus the frozen
P8.06 baselines, on the SAME cases with the SAME metrics, so no module
is judged on a private benchmark. **Every measured cell in §4 is EMPTY,
and none is fillable today**: §6 records the dependency table with its
current EMPTY status honestly — every per-module evaluation report is
itself a template whose runs are human/lab gate inputs, the frozen P8.06
baseline RESULTS artifact does not exist until the first run, the
torch/PyG and RL-framework DECISION-POINTs are OPEN, and ANDES is never
installed. Nothing was built, installed, or executed for this record
(ENTRY_NOTE loop bans). **No cross-module winner claim is possible until
the report is filled, and none is pre-stated anywhere in it.**

## 2. The comparability contract (the rule the whole report stands on)

Stated as a CONTRACT, recorded before any number exists:

1. **Same cases.** Every column of §4 is filled on the identical case
   ids from the frozen benchmark set — every row cites
   **`flexcore-research-benchmark-set-v1`** (the P8.19 rule: an
   evaluation row without the version id is invalid and must not be
   entered; the same holds per COLUMN here). A case one module ran and
   another did not is a "not run (reason)" cell in that module's column —
   never a different case substituted quietly, and never a case added
   for one module (a case addition is a benchmark-set v2 under the P8.19
   anti-silent-growth rule, applied to ALL columns or NONE).
2. **Same seeds and metric definitions.** Every module's runs on a case
   use the same recorded seeds (the frozen derivation, P8.19) and the
   same metric definitions per §19.4. **A metric computed differently
   per module makes the comparison invalid — the comparison REFUSES
   rather than merges.** Concretely: a runtime measured by one module
   inside the solver and by another outside it, an objective in
   different unit conventions, or a "violation" judged by different
   feasibility rules are not comparable numbers and must not appear in
   the same comparison table. The refusal vocabulary is in §3; the
   per-metric comparability table (§4.2) names, for each §19.4 metric,
   which modules it is identically defined for.
3. **No missing cells without a reason.** A cell that could not be run
   is marked `NOT RUN (reason)` with the reason named — never left
   blank, never filled with another module's number (ledger step 2,
   verbatim discipline). A blank cell in a filled report is itself a
   finding.
4. **Independent validation per cell (P8.05).** Every reported solution
   passed the P8.05 independent validation harness; each cell carries
   the validation outcome, and a cell whose proposals did not pass is
   recorded with that outcome — the ledger step 3 requirement.
5. **The dynamic track is a SEPARATE block, never merged** (§4.3): the
   ANDES disturbance metrics (frequency nadir, ROCOF, damping —
   model-scoped transient evidence) are not quasi-static scheduling
   metrics, and quasi-static feasibility results are not
   transient-stability evidence (X-5). One report, two structurally
   separate blocks; no cell ever moves between them.
6. **Gating.** The report fills ONLY after the per-module evaluation
   reports (§6) are filled; the narrative (§5) is written from the
   filled matrix, and a failed spot-check (§8, the ledger's verification
   method) BLOCKS the comparison report until the cell is corrected or
   re-run (the ledger's acceptance criterion).

## 3. Refusal vocabulary (recorded before any number)

| Refusal | Meaning |
|---|---|
| `NOT RUN (reason)` | the module did not run the case; the reason is named (framework unresolved, baseline results frozen missing, case not in the module's declared scope, validation not passed) — a reason, never a blank |
| `METRIC_NOT_COMPARABLY_DEFINED` | the metric is defined differently for this module than for the comparison — the cell is refused rather than merged (contract item 2) |
| `VALIDATION_OUTCOME_MISSING` | the P8.05 validation outcome was not recorded for the cell — refused at row assembly (the P8.09 discipline, carried) |
| `THRESHOLD_NOT_PRE_REGISTERED` | no pre-registered tolerance exists (X-8) — verdict cells are refused until P8.06 freeze registers them |
| `BENCHMARK_SET_VERSION_MISSING` | the row/column does not cite `flexcore-research-benchmark-set-v1` — invalid, must not be entered |
| `BASELINE_NOT_FROZEN` | the P8.06 frozen baseline RESULTS artifact for the case does not exist (§6) — the comparison column is `NOT RUN (BASELINE_NOT_FROZEN)`, never filled with an unfrozen number |

## 4. The comparison tables — ALL MEASURED CELLS EMPTY

### 4.1 The case×module matrix (quasi-static block)

Rows are the frozen benchmark-set cases; columns are the frozen P8.06
baselines and the research modules; cells are the §19.4 metrics for that
module (objective, feasible interval count, runtime, violations) plus
the P8.05 validation outcome. One row per case; **every measured cell is
EMPTY**, filled only via §8's commands. The IEEE rows are recorded as
their honest status — NOT INCLUDED in the set (the per-case IEEE
exclusion record) — and are never silently dropped from the list.

| Case (frozen v1 id) | §19.3 trace | Tier-1 `baseline/highs-tier1-lp` | Tier-2 `baseline/clarabel-tier2-conic` | `baseline/rule-based-priority-list` | ADMM/DMPC (P8.09) | GNN warm-start (P8.12) | GNN screening (P8.12) | MARL policy (P8.15) | Benchmark-set version |
|---|---|---|---|---|---|---|---|---|---|
| `lib-01-minimal-single-asset` | synthetic portfolios (BESS/PV/load/EVSE) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-02-small-portfolio` | synthetic portfolios | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-03-medium-multi-site-meshed` | common substation constraint (balanced meshed fixture) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-06-stress-pv-ramp` | PV ramp + forecast error (incl. `v1-error-pv-ramp` fixture) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-07-stress-asset-dropout` | asset dropout (+ the recorded comm-loss analogues) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-08-stress-reverse-power-flow` | reverse power flow (scripted-backend limit carried honestly) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-04-stress-worker-timeout` | solver/worker timeout analogue (worker_process leg, UNVERIFIED pandapower API) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `lib-05-stress-nonconvergence` | non-convergence injection (worker_process leg, UNVERIFIED pandapower API) | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| `phase3-scenario-artefacts` (a/b/c) | the P3.26 pinned artefacts the P3.27 harness measures | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| IEEE 13/34/123-bus, 39/118-bus | NOT INCLUDED in the set (exclusion record; no model exists) | NOT RUN (no model — exclusion record) | NOT RUN (no model) | NOT RUN (no model) | NOT RUN (no model) | NOT RUN (no model) | NOT RUN (no model) | NOT RUN (no model) | n/a — not in any version |

Filling rules (recorded now, before any number): cells come from the
per-module evaluation records' own outputs (P8.09 §3, P8.12 §5, P8.15 §4)
and the frozen P8.06 baseline results artifact — never from memory,
never estimated, never from another module's column; a refusal from §3
is recorded AS THE REFUSAL; every filled cell carries the benchmark-set
version id and the P8.05 validation outcome; the same seeds (the frozen
derivation) and the same metric definitions apply to every column of a
row; the units are the canonical ones (§10.2: MW/MVAr/MVA/MWh; `P > 0` =
export at the point of connection; SOC a fraction in [0, 1] at the API
boundary) for EVERY column alike — a unit divergence between columns is
itself a `METRIC_NOT_COMPARABLY_DEFINED` refusal; wins, LOSSES, and ties
are all reported once the verdict columns exist — and a verdict column
does not exist until the pre-registered tolerances are injected from the
P8.06 freeze record (`THRESHOLD_NOT_PRE_REGISTERED` otherwise).

### 4.2 Per-metric comparability table (contract item 2, made concrete)

For each §19.4 metric: which modules it is IDENTICALLY defined for (a
comparable cell), and which report it module-specifically (reported, but
never a cross-module comparison cell).

| §19.4 metric | Identically defined for | Module-specific (reported, never compared) | Comparability notes |
|---|---|---|---|
| Objective value | Tier-1, Tier-2, rule-based, ADMM, MARL (same accounting per §13.3) | GNN (the GNN warms the solver — the SOLVED objective is the solver's, recorded under the solver column, never as a GNN metric) | objective gap vs a baseline only where MEANINGFUL (the P8.09 rule — never against a nonconvex/mixed-integer comparison); the same cost units declared per case |
| Feasible interval count | Tier-1, Tier-2, rule-based, ADMM, MARL | GNN (via the same solved schedule) | the same feasibility definition (the platform's §12.5 gate) for every column |
| Runtime | all columns, measured the SAME way: human-recorded wall-clock, hardware/container block recorded (P3.27 rule), developer-laptop figure never a deployment figure | — | the P8.09 separation: wall-clock is a human-run measurement injected outside the determinism-controlled modules — for EVERY column alike |
| Violations | all columns, judged by the SAME gate (the platform feasibility pipeline behind the P8.05 harness) | — | zero observed violations is a test outcome, never a guarantee (X-4); the gate, not the module, decides |
| Primal/dual residual, iteration count | — | ADMM only (P8.09) | ADMM convergence evidence under the P8.07 convexity assumptions; no solver-class conflation (ADMM is neither LP/QP/MIP nor convex conic) |
| End-to-end feasible solve time, feasible-success rate, paired speedup | — | GNN only (P8.12) | a warm start affects the speed of an exact solve, never its optimality; the headline metric is end-to-end (inference + solver + validation), never inference latency |
| Return, constraint-violation frequency, robustness deltas, filter statistics | — | MARL only (P8.15) | the two MARL metrics are never merged into one headline (P8.13 §2.6); filter statistics ride with every MARL cell |
| Frequency nadir, ROCOF, damping, settling, voltage extremes | — | dynamic track only (§4.3) | model-scoped evidence about the DECLARED ANDES model only (P8.17); never quasi-static feasibility evidence (X-5) |

A metric that appears in two modules' records but is defined differently
between them is a `METRIC_NOT_COMPARABLY_DEFINED` refusal in the
comparison — the report refuses rather than merges.

### 4.3 The dynamic block (separate; never merged into the quasi-static matrix)

The dynamic track's rows are the P8.17 disturbance set (D-1…D-4,
declared before any run there), columns are the controller proposals and
the deterministic baseline controller on the SAME disturbance, same
case, same settings — the P8.17 table is THE dynamic block; it is
referenced here, not duplicated. Its cells are EMPTY (P8.17 §3), its
model/case references are `RECORD-AT-RUN`, and no dynamic number ever
enters §4.1 or vice versa (X-5). The dynamic columns satisfy the same
contract items 1–4 within their own block: same disturbances, same
metrics, validation outcomes carried, no missing cells without a reason.

## 5. The comparison narrative (ledger step 4) — headings frozen NOW, content EMPTY until the matrix fills

- **Which module wins where** — EMPTY. Not pre-stated, not hinted, not
  extrapolated from the designs. Written only from the filled §4.1
  matrix, reporting losses alongside wins.
- **Where all lose to the deterministic baseline** — EMPTY. The ledger's
  step 4 names this outcome explicitly as possible; the strong
  deterministic baselines exist precisely so a loss to them is visible
  (the P8.06 assumptions records state each baseline's strength, so a
  "win" against a weak baseline is visible as such).
- **What that implies for the next research step** — EMPTY. Derived from
  the filled matrix at review time (P8.26/P8.30 consume it); written by
  the human gate, never asserted by this template.

Fixed limits stated now, independent of results: no cross-module winner
claim before the matrix fills; no verdict without a pre-registered
tolerance (X-8); no merged headline across metrics or across blocks;
solver-class discipline VERBATIM (HiGHS LP/QP/MIP, never SOCP; Clarabel
convex conic, never MISOCP; ADMM neither — heuristic on nonconvex/MI
instances; AC power flow is not AC-OPF); a finite test set with zero
observed violations is a test outcome, not a guarantee (X-4); proposals
only — nothing here is a dispatch decision (ADR-010); the entire
comparison is simulation-only (AGENTS.md rule 5).

## 6. The dependency table — the per-module reports' current EMPTY status, recorded honestly

The report's filling is gated on these being filled first. Their status
below is the honest status on 2026-11-14 — NOT an assumption of
completion.

| Dependency | What this report needs from it | Current status (recorded honestly) |
|---|---|---|
| P8.19 (`benchmarks/research-set-v1.json`) | the frozen case list and version id every row cites | **FROZEN** — `flexcore-research-benchmark-set-v1` (WRITTEN-UNRUN; nothing in the set has ever been run) |
| P8.06 baseline RESULTS | the frozen baseline objectives/timings per case — the comparison's reference columns | **DOES NOT EXIST** — the registry freezes DEFINITIONS; the results artifact comes from the first human/lab run (`05_06_harness_baselines.md` §3) |
| P8.09 (`09_admm_evaluation.md`) | the ADMM per-case §19.4 statistics | **TEMPLATE — all measured cells EMPTY**; runs are human/lab inputs |
| P8.12 (`12_gnn_evaluation.md`) | the GNN warm-start/screening metrics | **TEMPLATE — all measured cells EMPTY**, and not yet fillable: torch/PyG DECISION-POINT OPEN, the P8.35 hold-out manifest does not exist, the scope-extension follow-up is unresolved |
| P8.15 (`15_marl_evaluation.md`) | the MARL per-case metrics and filter statistics | **TEMPLATE — all measured cells EMPTY**, and not yet fillable: RL-framework DECISION-POINT OPEN, the CBF derivation is unsigned and training BLOCKED |
| P8.17 (`17_dynamic_evaluation.md`) | the dynamic per-disturbance metrics (§4.3 block) | **TEMPLATE — all measured cells EMPTY**; ANDES never installed (UNVERIFIED API) |
| P8.05 harness results | the validation outcome per cell | harness WRITTEN-UNCOMPILED; no validation has run |
| P8.20 reproducibility records | the per-run records and the rerun-test outcomes the spot-checks verify against | **NO RECORD EXISTS; NO RERUN HAS HAPPENED** (`20_reproducibility.md` §7); the P8.38 tolerances are not pre-registered |
| P8.06 freeze record (tolerances) | the pre-registered tie tolerances/verdict thresholds (X-8) | **NOT PRE-REGISTERED** — verdict columns do not exist |

Fill order (recorded): P8.06 baseline results per case → per-module
evaluations (each with its P8.20 run record and P8.05 validation
outcome) → §4.1/§4.3 assembled → tolerances pre-registered → verdict
columns → §5 narrative → the §8 spot-checks pass → the report counts as
filled. A failed spot-check blocks the report until the cell is
corrected or re-run.

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Same cases/seeds/metrics for all modules | THE CONTRACT (§2) and the per-metric comparability table (§4.2) are recorded; no cell exists yet to satisfy it — the matrix is EMPTY |
| No missing cells without a reason | RULE RECORDED (§3 — `NOT RUN (reason)`, never blank, never another module's number); the matrix is EMPTY |
| Validation outcomes present | RULE RECORDED (§2 item 4; `VALIDATION_OUTCOME_MISSING` refusal); no outcome exists yet |
| Narrative reports losses as well as wins | HEADINGS FROZEN (§5) with the losses requirement stated before any number; content EMPTY — no winner claim is possible or pre-stated |
| A failed spot-check blocks the report | RULE RECORDED (§2 item 6; §8 procedure) — the gate is the P8.20 rerun discipline over the filled cells |

## 8. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the upstream gates FIRST (their own records own these commands):
#     - P8.06: freeze the baseline results artifact per case (the
#       registry's freeze path) and pre-register the tolerances;
#     - P8.09 §6 (a)-(d), P8.12 §8 (a)-(d), P8.15 §8 (a)-(d),
#       P8.17 §5 steps 0-2: each module's evaluation fills, each with
#       its P8.20 run record and P8.05 validation outcome;
#     - P8.20 §8 (e): the per-run reproducibility records validated.
#     A green upstream gate is a PRECONDITION, not an assumption.

# (c) the matrix fills (§4.1/§4.3) — from the per-module records' own
#     outputs ONLY:
#     1. per case row: verify every column cites
#        flexcore-research-benchmark-set-v1 (a column without it is not
#        entered);
#     2. per cell: copy the module record's own §19.4 metric values and
#        the P8.05 validation outcome; record every non-run as
#        NOT RUN (reason) with the reason named;
#     3. check each metric's definition is IDENTICAL across the columns
#        it is compared in (§4.2) — any divergence is
#        METRIC_NOT_COMPARABLY_DEFINED, and the comparison refuses
#        rather than merges;
#     4. pre-register the tolerances (P8.06 freeze record) BEFORE any
#        verdict column is filled (X-8); fill the verdict columns —
#        wins, LOSSES, and ties alike.

# (d) the ledger's verification method — spot-check THREE cells:
#     re-run each of the three selected cells from its recorded inputs
#     (the P8.20 rerun discipline: a second engineer, from the record
#     alone, clean environment, pre-registered tolerance); a failed
#     spot-check BLOCKS the comparison report until the cell is
#     corrected or re-run (the ledger acceptance).

# (e) the claims gate (after the narrative is written):
grep -n "win\|best\|outperform\|superior" \
  ledger/phase-08-advanced-coordination/23_cross_module_comparison.md
#     every hit must sit inside a sentence scoped by the filled matrix
#     and §5's fixed limits — a winner claim without the filled matrix
#     behind it is a finding against this record.
```

A green (a) plus upstream (b) earns only a one-line "human build/test
gate: PASSED (date)" appended here; failures are recorded verbatim and
the ledger tick reverts to `- [ ]`. The cells of §4 stay EMPTY until the
(c) fills happen, and the narrative of §5 stays EMPTY until §4 is filled.

## 9. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The frozen P8.06 baseline RESULTS artifact does not exist** (carried
   from `09_admm_evaluation.md` §8.1 / `12_gnn_evaluation.md` §7 gap 7 /
   `15_marl_evaluation.md` §9.4): the baseline columns cannot fill until
   the first human/lab run freezes them. No gap fix is invented here.
2. **The §13.3 PI baseline has no frozen id in the P8.06 registry**
   (carried from `15_marl_evaluation.md` §9.1): a delay/loss/uncertainty
   comparison against a PI baseline is refused (`BASELINE_UNKNOWN` in
   the P8.15 harness; the same refusal applies here) until P8.06
   defines, runs, and freezes one. Never invented here.
3. **The torch/PyG and RL-framework DECISION-POINTs are OPEN** (P8.10 §6,
   P8.13 §5; D4): the GNN and MARL columns are structurally unfilled
   until they resolve. Recorded as dependencies (§6), never fixed by
   inventing pins.
4. **The GNN scope-extension follow-up is unresolved** (carried verbatim
   from `12_gnn_evaluation.md` §7 gap 1): held-out topologies produce
   the contract-correct out-of-scope flag, not numbers, until the
   P8.04/P8.35 owners decide. The comparison column records
   `NOT RUN (scope-extension unresolved)` meanwhile.
5. **The comm-loss and topology-change stressors are PARTIAL mappings**
   (P8.19 §3, honestly recorded): the comparison does not silently
   promote them to full coverage; the matrix rows carry the honest case
   ids and the not-tested list (NT-6/NT-7) carries the limits.
6. **No object storage exists** (P8.20 §10.1): a cell whose run artifacts
   are not store-retrievable cannot be spot-checked (§8 step d) and
   therefore cannot stand in a filled report — the store is the §6.5
   owners' task, recorded as the dependency.

## 10. Non-claims (restated)

No cross-module comparison exists; every measured cell in §4 is EMPTY
and (until §6's dependencies resolve) not fillable; no winner claim, no
ranking, no "which module is better" statement exists anywhere in this
record, and none is pre-stated — the narrative sections are frozen
headings over EMPTY content. No verdict exists without a pre-registered
threshold (X-8). The dynamic block's results will be evidence about a
declared ANDES model only, never about the grid, and never
quasi-static feasibility evidence (X-5). No safety claim and no field
authority (proposals only, ADR-010). Solver-class discipline verbatim
(HiGHS LP/QP/MIP, never SOCP; Clarabel convex conic, never MISOCP; ADMM
neither; AC power flow is not AC-OPF). The §19.4 numeric thresholds are
still missing by design (X-8).
