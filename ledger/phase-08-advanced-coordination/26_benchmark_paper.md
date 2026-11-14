# P8.26 — Benchmark paper/report: the Stage 8 deliverable (TEMPLATE — every measured value EMPTY; nothing measured, nothing fabricated)

Date: 2026-11-14. Subtask: P8.26 (`ledger_phase_8.md` §P8.26). Owner role:
research lead. Report trace: §23 Stage 8 (exit gate); §19.4; §27
(references). Inputs (the ledger's dependencies, each cited where its
content is used): P8.02 (`02_research_scope_freeze.md` — the frozen
question list and prohibitions); P8.06 (`05_06_harness_baselines.md` —
the frozen baseline registry); P8.09 (`09_admm_evaluation.md`); P8.12
(`12_gnn_evaluation.md`); P8.15 (`15_marl_evaluation.md`); P8.17
(`17_dynamic_evaluation.md`); P8.19 (`19_benchmark_cases.md` +
`benchmarks/research-set-v1.json` + `benchmarks/research-set-v1-not-tested.json`);
P8.20 (`20_reproducibility.md`); P8.21 (`21_topology_research.md`); P8.23
(`23_cross_module_comparison.md` — the comparability contract and the
comparison matrix); P8.05 (`05_06_harness_baselines.md` §2 — the
independent validation harness). ENTRY_NOTE.md boundary 4 governs every
results section of this paper.

## 1. Status of this document — WRITTEN-UNRUN, the paper TEMPLATE

This document is the **benchmark paper/report template** (the ledger's
Stage 8 deliverable). **Every measured value in §6–§11 is EMPTY and none
is fillable today**: each per-module evaluation record is itself a
TEMPLATE whose runs are human/lab gate inputs, the frozen P8.06 baseline
RESULTS artifact does not exist until the first run, the torch/PyG and
RL-framework DECISION-POINTs are OPEN, ANDES is never installed, no
reproducibility record exists, and no rerun has happened. **The paper
fills only when its input reports fill** (§13 — the recorded fill order);
a section that cannot fill records its input's honest status rather than
prose. Nothing was built, installed, or executed for this record
(ENTRY_NOTE loop bans).

What CAN be written now, and is written here: the methodology (§3–§5),
the honesty boundary (§12), and the limitations + not-tested list (§10 —
filled today from the frozen not-tested artifact, because "what was not
tested" is honest exactly when nothing has been run). The conclusions
(§11) are explicitly conditional on the filled matrix.

**The one-line honest summary of this paper as it stands:** no result
exists anywhere in Phase 8; this paper is the scaffold that makes the
future report honest — it fixes the questions, the benchmark-set
version, the comparability contract, the reproducibility standard, the
not-tested list, and the non-claims BEFORE any number exists.

## 2. Research questions (frozen; P8.02 §4 — restated, not re-decided)

The paper answers, per module, the question the P8.02 freeze registered.
The questions and metrics are quoted by reference; the numeric success
thresholds are NOT restated here because none is pre-registered yet (X-8
— they are pre-registered at P8.06 freeze time, before any
training/tuning run, and the verdict sections of this paper refuse to
fill without them).

| Module | Frozen question (P8.02 §4) | Metric set (§19.4 row) | Paper section |
|---|---|---|---|
| M1 — ADMM/DMPC | validated feasible schedule within the pre-registered gap of the centralized HiGHS baseline at a pre-registered coordination cost, under the P8.07 convexity assumptions | primal/dual residual, objective gap (where meaningful), iteration count, time, communication sensitivity | §6.1 |
| M2 — GNN warm-start | end-to-end feasible solve time on held-out topologies vs cold start and a non-GNN heuristic warm start, without a feasible-solver success-rate loss | feasible solver success, end-to-end time, speedup, generalization | §6.2 (a) |
| M2 — GNN screening | ranking of actually-active constraints/contingencies above inactive ones at a pre-registered miss rate and miss cost on held-out topologies | ranking quality vs true active sets, miss cost | §6.2 (b) |
| M3 — MARL | match or beat the deterministic and rule-based baselines on the pre-registered metrics under held-out regimes/outages/delays/uncertainty, every filter activation and infeasibility logged | constraint-violation frequency, return, robustness under distribution shift, filter statistics | §6.3 |
| M4 — dynamic | model-scoped change in frequency/transient behaviour vs the deterministic baseline controller on the recorded disturbance set | frequency nadir/settling, voltage behaviour, instability occurrences — each with its model/case reference | §8 |
| S-5 — transfer/scaling | transfer across topology families and scaling within the tested range, no claim beyond it (X-7) | the P8.21 §5 table metrics | §9 |

A question answered with an EMPTY cell is not answered; this paper does
not pre-state any answer (§11).

## 3. Methodology — the benchmark set (P8.19)

Every results row of this paper cites the frozen benchmark-set version
id **`flexcore-research-benchmark-set-v1`** (the P8.19 anti-silent-growth
rule: an evaluation row without the version id is invalid and must not
be entered; a case addition is benchmark-set v2, applied to ALL modules
or NONE). The set is `benchmarks/research-set-v1.json` — WRITTEN-UNRUN:
**nothing in the set has ever been run**.

The case list the paper's tables cover (recorded, with the honest IEEE
exclusion carried rather than silently dropped):

| Case (frozen v1 id) | §19.3 trace | Status |
|---|---|---|
| `lib-01-minimal-single-asset` | synthetic portfolios (BESS/PV/load/EVSE) | in the frozen set; NEVER RUN |
| `lib-02-small-portfolio` | synthetic portfolios | in the frozen set; NEVER RUN |
| `lib-03-medium-multi-site-meshed` | common substation constraint (balanced meshed fixture) | in the frozen set; NEVER RUN |
| `lib-06-stress-pv-ramp` | PV ramp + forecast error (incl. `v1-error-pv-ramp` fixture) | in the frozen set; NEVER RUN |
| `lib-07-stress-asset-dropout` | asset dropout (+ recorded comm-loss analogues) | in the frozen set; NEVER RUN |
| `lib-08-stress-reverse-power-flow` | reverse power flow (scripted-backend limit carried honestly) | in the frozen set; NEVER RUN |
| `lib-04-stress-worker-timeout` | solver/worker timeout analogue (worker_process leg, UNVERIFIED pandapower API) | in the frozen set; NEVER RUN |
| `lib-05-stress-nonconvergence` | non-convergence injection (worker_process leg, UNVERIFIED pandapower API) | in the frozen set; NEVER RUN |
| `phase3-scenario-artefacts` (a/b/c) | the P3.26 pinned artefacts the P3.27 harness measures | in the frozen set; NEVER RUN in a Phase 8 research evaluation |
| IEEE 13/34/123-bus, 39/118-bus | §19.3 bullets | **NOT INCLUDED** (per-case IEEE exclusion record; NT-2–NT-4) — never silently dropped from this list |

The seeds are the frozen derivation
`seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`
recorded verbatim in the artifact's `seeds` blocks; the solver pins
(HiGHS 1.15.1, Clarabel 0.11.1) are the frozen registry's (P8.06 §3).
The seed-derivation interpretation and the pins are IMPLEMENTED in the
P8.20 tooling from the artifact, never edited in it (P8.20 §10.4).

## 4. Methodology — the comparability contract (P8.23 §2, incorporated by reference)

The paper's every comparison stands on the P8.23 comparability
contract, recorded there BEFORE any number exists and incorporated here
by reference (one contract, no copy):

1. same cases (every row cites `flexcore-research-benchmark-set-v1`);
2. same seeds and metric definitions — a metric computed differently per
   module is `METRIC_NOT_COMPARABLY_DEFINED` and the comparison refuses
   rather than merges (the P8.23 §4.2 per-metric comparability table is
   the authority on which §19.4 metric is identically defined for which
   modules);
3. no missing cells without a reason (`NOT RUN (reason)` — never blank,
   never another module's number);
4. independent validation per cell (the P8.05 harness outcome travels
   with every reported solution; `VALIDATION_OUTCOME_MISSING` refuses
   the cell);
5. the dynamic track is a SEPARATE block, never merged into the
   quasi-static matrix (X-5 — §8 here is that block, structurally
   separate from §6–§7);
6. gating: the narrative fills only after the matrix fills; a failed
   spot-check BLOCKS the report (the P8.20 rerun discipline, §8(d)).

The refusal vocabulary (`NOT RUN (reason)`, `METRIC_NOT_COMPARABLY_DEFINED`,
`VALIDATION_OUTCOME_MISSING`, `THRESHOLD_NOT_PRE_REGISTERED`,
`BENCHMARK_SET_VERSION_MISSING`, `BASELINE_NOT_FROZEN`) is P8.23 §3's,
used verbatim in this paper's cells.

## 5. Methodology — baselines, harness, and reproducibility standard

**Baselines (P8.06, frozen BEFORE any research comparison):** the frozen
registry `baseline/highs-tier1-lp` (HiGHS LP/QP/MIP — never SOCP),
`baseline/clarabel-tier2-conic` (convex conic — never MISOCP), and
`baseline/rule-based-priority-list`. The registry freezes DEFINITIONS;
**the frozen baseline RESULTS artifact does not exist until the first
human/lab run** (P8.23 §6, recorded honestly) — the baseline columns of
this paper's tables are therefore `BASELINE_NOT_FROZEN` today, never
filled with an unfrozen number. The §13.3 PI baseline has no frozen id
in the registry (P8.23 §9.2) — a comparison against it is refused
(`BASELINE_UNKNOWN`) until P8.06 defines and freezes one; never invented
here.

**The independent validation harness (P8.05):** every reported solution
in this paper passed the P8.05 harness (the validator that does not
trust the researcher); each cell carries the validation outcome. The
harness is WRITTEN-UNCOMPILED; no validation has run.

**The reproducibility standard (P8.20, incorporated by reference):** every
number this paper would carry must have a schema-valid
`flexcore-research/reproducibility-record.v1` record (P8.20 §2/§3) —
code/component versions, the verbatim seed derivation, config and input
artifact digests, artifact-store references, the environment block, and
the pre-registered tolerance class (X-8). A result that failed its
rerun test carries the standing exclusion marking
`NOT_REPRODUCED_MARKED_FOR_EXCLUSION` — **this paper MUST NOT cite such
a result** (P8.20 §5 item 6); the marking is consumed at fill time in
§13. No reproducibility record exists and no rerun has happened; today
the paper cites nothing.

**Thresholds (X-8):** no verdict column exists anywhere in this paper
until the pre-registered tolerances are injected from the P8.06 freeze
record. The paper never states, suggests, or derives a threshold value.

## 6. Per-module results — ALL MEASURED VALUES EMPTY

Each subsection names its source record and its table; the cells are
EMPTY in the source records and remain EMPTY here — the paper's tables
are assembled from the source records' own outputs at fill time (§13),
never re-derived. A source record's status line is quoted as the honest
status of the corresponding paper section.

### 6.1 ADMM/DMPC (source: `09_admm_evaluation.md` §3)

Status of the source: TEMPLATE — all measured cells EMPTY; runs are
human/lab inputs. Paper status: **EMPTY**.

Structure (columns per the source table, assembled at fill time): case
id (citing `flexcore-research-benchmark-set-v1`), family, termination
status (P8.08 vocabulary), iterations, r vs ε_pri, s vs ε_dual, objective
gap vs Tier-1 / Tier-2 / priority-list (each gap filled only where the
comparison is declared meaningful — `OBJECTIVE_GAP_NOT_MEANINGFUL (reason)`
otherwise), fallback used, stale events, runtime (s, human-recorded),
P8.05 validation outcome, Win/Tie/Loss (pre-registered tolerance
REQUIRED — the column does not exist until the tolerance is injected).

| Row | All measured cells |
|---|---|
| EMPTY | EMPTY |

### 6.2 GNN warm-start and screening (source: `12_gnn_evaluation.md` §5)

Status of the source: TEMPLATE — all measured cells EMPTY, and not yet
fillable (torch/PyG DECISION-POINT OPEN, the P8.35 hold-out manifest
does not exist, the scope-extension follow-up unresolved). Paper status:
**EMPTY**.

(a) Warm-start (columns per the source §5.1): topology id (held-out per
the P8.35 manifest), strategy (cold / heuristic / gnn), inference/solver/
validation components, end-to-end feasible time (the HEADLINE — never
inference latency), feasible+validated outcome, paired speedups, P8.05
validation outcome, model artifact + manifest refs. Held-out rows ONLY
carry the generalization claim (X-7).

(b) Screening (columns per the source §5.2): topology id, true active
set ref (exact-solve ground truth), screen depth, precision@k, miss
rate, miss cost (recorded cost model), uncertainty carried, heuristic-
ranking baseline, refusals recorded. The warm-start and screening
results are NEVER merged into one headline (P8.02 §4 M2).

| Row | All measured cells |
|---|---|
| EMPTY | EMPTY |

### 6.3 MARL (source: `15_marl_evaluation.md` §4–§5)

Status of the source: TEMPLATE — all measured cells EMPTY, and not yet
fillable (RL-framework DECISION-POINT OPEN, the CBF derivation is
unsigned and training BLOCKED). Paper status: **EMPTY**.

Per-case metrics (source §4 columns): decisions N, constraint-violation
frequency (violated/N — refused where consequences are `not_simulated`,
never converted into zero), violation kinds (counted, NOT merged), mean
return (declared unit travelling on every record), per-metric deltas +
verdicts vs `baseline/highs-tier1-lp` and
`baseline/rule-based-priority-list`, robustness deltas (shifted −
reference). The two MARL metrics are never merged into one headline
(P8.13 §2.6). Wins, LOSSES, and ties are all reported.

Safety-filter statistics (source §5, from the closed
`safety_filter_outcome` vocabulary): activation rate, infeasibility
rate, refusal rate, and the outcome of EVERY infeasibility (per
occurrence: sequence, named outcome, affected assets, recorded flag,
fallback facts — `NOT_RECORDED_IN_LOG_SCHEMA` where the log schema
carries no fallback field). **All filter-statistics cells EMPTY.**

| Row | All measured cells (incl. filter statistics) |
|---|---|
| EMPTY | EMPTY |

### 6.4 Transfer and scaling (source: `21_topology_research.md` §5)

Status of the source: TEMPLATE — all measured cells EMPTY; the §3.2
scope-extension blocker is carried verbatim. Paper status: **EMPTY**.

Transfer table (held-out topology families ONLY for any generalization
claim) and scaling table (1×/2×/4× — research module AND centralized
baseline on the SAME case) per the source's columns; the limits note
(§5.3) fills with the runs. No claim beyond the tested range (X-7): the
tested range is fixture-scale synthetic feeders and in-memory ×2/×4
copies — feeder-scale results do not establish 10,000-bus capability
(§19.3 closing sentence; NT-1).

| Row (transfer) | All measured cells |
|---|---|
| EMPTY | EMPTY |

| Row (scaling) | All measured cells |
|---|---|
| EMPTY | EMPTY |

## 7. Cross-module comparison — EMPTY (source: `23_cross_module_comparison.md` §4–§5)

Status of the source: ONE comparison report template — the case×module
matrix, the per-metric comparability table, and the refusal vocabulary
are recorded; EVERY measured cell EMPTY; no winner claim possible or
pre-stated. Paper status: **EMPTY**.

The paper's comparison section is the filled P8.23 §4.1 case×module
matrix read through its §4.2 comparability rules; the narrative
(P8.23 §5 — frozen headings: "which module wins where", "where all lose
to the deterministic baseline", "what that implies for the next research
step") is written from the filled matrix only, reporting losses
alongside wins. Fixed limits (stated now, independent of results): no
cross-module winner claim before the matrix fills; no verdict without a
pre-registered tolerance (X-8); no merged headline across metrics or
blocks; solver-class discipline verbatim (HiGHS LP/QP/MIP, never SOCP;
Clarabel convex conic, never MISOCP; ADMM neither — heuristic on
nonconvex/MI instances; AC power flow is not AC-OPF).

| Comparison cell | All measured cells |
|---|---|
| EMPTY | EMPTY |

## 8. Dynamic results — SEPARATE BLOCK, ALL MEASURED VALUES EMPTY (source: `17_dynamic_evaluation.md` §3)

**Structural separation (X-5, restated as a layout rule):** this section
is the dynamic block. The ANDES disturbance metrics (frequency nadir,
ROCOF, damping, settling, voltage extremes) are model-scoped transient
evidence and never quasi-static scheduling metrics; quasi-static
feasibility results (§6–§7) are never transient-stability evidence. No
cell ever moves between this section and §6/§7.

Status of the source: TEMPLATE — all measured cells EMPTY; the
disturbance set D-1…D-4 is declared (P8.17 §2) but no study has run;
ANDES is never installed (UNVERIFIED API). Paper status: **EMPTY**.

Per-disturbance × controller-proposal rows, each carrying its model/case
reference (`RECORD-AT-RUN` until a run exists): a result is evidence
about the DECLARED ANDES model only, never about the grid (§19.1;
ADR-012). A failed study is recorded AS A FAILURE, a growing
oscillation as an instability observation, never as a damping ratio.

| Disturbance | Controller proposal | All measured cells | Model/case ref |
|---|---|---|---|
| D-1 | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-2 | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-3 | EMPTY | EMPTY | `RECORD-AT-RUN` |
| D-4 | EMPTY | EMPTY | `RECORD-AT-RUN` |

## 9. Reproducibility statement — EMPTY (source: `20_reproducibility.md`)

**No rerun has happened and no reproducibility record exists anywhere in
the repository** (P8.20 §1/§7). The paper's reproducibility statement
fills at fill time from the P8.20 §7.1 per-run records and §7.2
rerun-test records; it cites only results whose records are
schema-valid, and it carries every `NOT_REPRODUCED_MARKED_FOR_EXCLUSION`
result as an EXCLUSION, not a caveat (P8.20 §8(f) step 4). Until then
this section states exactly what the source states: no result in this
paper is evidenced as reproducible; the ledger's acceptance ("at least
one result per module passed the rerun test") is **UNRUN**.

| Result cited by this paper | Reproducibility record | Rerun outcome |
|---|---|---|
| (none — no result exists) | EMPTY | EMPTY |

## 10. Limitations and the not-tested list — FILLED NOW (source: `benchmarks/research-set-v1-not-tested.json`, frozen 2026-11-14)

This section is fillable TODAY and is filled here verbatim-in-substance
from the frozen not-tested artifact (list id
`flexcore-research-benchmark-set-v1-not-tested`, part of the frozen set
— it changes only with a NEW set version). Every item is a case the
phase must NOT be read as having covered.

| Item | Case NOT run | Status |
|---|---|---|
| NT-1 | a 10,000-bus utility-scale network (any transmission-scale network study) | NOT RUN — and a scale claim from the tested range is PROHIBITED |
| NT-2 | IEEE 39-bus transmission test system | NOT RUN |
| NT-3 | IEEE 118-bus transmission test system | NOT RUN |
| NT-4 | IEEE 13-bus, 34-bus, and 123-bus distribution test feeders | NOT RUN |
| NT-5 | unbalanced distribution networks (per-phase models, unbalanced power flow) | NOT RUN — out of the declared scope |
| NT-6 | runtime topology-change event (feeder switching / line trip mid-horizon in the quasi-static grid model) | NOT RUN — no such case exists |
| NT-7 | sustained communication loss (dedicated benchmark case) | NOT RUN as a dedicated case — only analogues exist |
| NT-8 | probabilistic forecast-error ensembles (distributional forecast scenarios) | NOT RUN — only a single deterministic error fixture exists |
| NT-9 | transmission/planning studies (the PyPSA track, §19.1) | NOT RUN — no integration exists |
| NT-10 | dynamic-stability cases (Kundur two-area, IEEE 14-bus with dynamic data) and any transient/frequency result | NOT RUN — out of this set's scope |
| NT-11 | hardware-in-the-loop commissioning | NOT RUN — out of phase scope |

Standing limitations that travel with every result this paper will ever
carry (fixed now, independent of results):

- **No safety guarantee from a finite test set** (X-4): zero observed
  constraint violations is a test outcome, not a universal guarantee;
  the limitation is stated in plain language in every evaluation note.
- **Model-scoped dynamic evidence** (X-5, ADR-012): the dynamic results
  are evidence about a declared ANDES model, never about the grid.
- **No generalization beyond the tested range** (X-7): fixture-scale
  synthetic feeders and in-memory ×2/×4 copies.
- **No HELICS-as-physics** (X-6): HELICS synchronizes federates; it is
  not the power-system model.
- **Simulation first** (AGENTS.md rule 5): the entire work is
  simulation-only; field write authority begins at v1.0.0 with operator
  approval, which this research phase does not approach.
- The scriptable backend limits (lib-08 reverse-power-flow) and the
  UNVERIFIED worker_process pandapower API legs (lib-04/lib-05) travel
  with those cases' rows.
- The comm-loss and topology-change stressors are PARTIAL mappings —
  analogues, not dedicated cases (NT-6/NT-7); the paper never silently
  promotes them to full coverage.

## 11. Conclusions — EXPLICITLY CONDITIONAL, content EMPTY

The conclusions section is written ONLY from the filled §6–§9, at
review time, by the human gate. Its headings are frozen now; every
content slot is EMPTY; nothing is hinted, extrapolated, or pre-stated:

- **What the experiments showed** — EMPTY. Not writable before any run
  exists.
- **Where the modules helped and where they did not** — EMPTY. Losses
  reported alongside wins; "where all lose to the deterministic
  baseline" is a named possible outcome (P8.23 §5).
- **What the not-tested list bounds** — fillable from §10 at fill time
  (the bound is already known: the §19.3-tested range, fixture-scale,
  balanced, no runtime switching, no ensembles, no HIL).
- **What the next research step is** — EMPTY. Derived from the filled
  matrix at review time (P8.23 §5; P8.30 consumes it).

Conditional framing (the ledger's risk row, answered in advance): this
paper must never read as a product claim. It is not "FlexCore now does
safe MARL dispatch"; it is a report of simulation-only research
proposals against frozen baselines on one frozen benchmark-set version,
with losses, not-tested items, and exclusions reported like any other
cell.

## 12. The honesty boundary (ledger step 3 — stated IN the report, in place now)

1. **Proposals only (ADR-010):** every research output is a candidate
   that enters the protected validation → feasibility → authorization
   path; no research module has any field authority, and per-feature
   qualification is granted NOWHERE in this phase (P8.29 records what
   field use would require).
2. **No safety guarantee from finite tests** (X-4): zero observed
   violations is a test outcome; the limitation is in §10 and travels
   with every result.
3. **No field authority:** the entire work is simulation-only (AGENTS.md
   rule 5); shadow/simulation is the ceiling of this phase (§2.1).
4. **No global optimality outside stated assumptions** (X-3): ADMM on
   nonconvex/mixed-integer instances is reported as heuristic behaviour
   with residuals and fallback outcomes; no convergence claim beyond the
   P8.07 convexity assumptions; AC power flow is not AC-OPF.
5. **Solver-class discipline, verbatim (AGENTS.md rule 4):** HiGHS is
   LP/QP/MIP, never SOCP; Clarabel is convex conic, never MISOCP; ADMM
   is neither; steady-state PF is not transient stability; HELICS is
   transport, not physics (X-6).
6. **No arbitrary universal targets** (X-8): thresholds are
   pre-registered per module before any run; none exists yet; this paper
   carries no threshold value.
7. **No result without its provenance:** every number carries the
   benchmark-set version id, the P8.05 validation outcome, and a
   schema-valid P8.20 record — a number without provenance is not
   enterable (the ledger's verification method makes unsourced numbers
   a review failure).

## 13. Fill order and exact filling commands (the human gate)

The paper fills ONLY when its input reports fill. The fill order is the
dependency chain, recorded (P8.23 §6, extended to this paper):

1. P8.06 baseline results per case (the freeze path) + tolerance
   pre-registration (X-8);
2. per-module evaluations (P8.09 §3, P8.12 §5, P8.15 §4–§5, P8.17 §3,
   P8.21 §5), each with its P8.20 run record (§7.1) and P8.05 validation
   outcome;
3. P8.20 §7.2 rerun records — at least one `REPRODUCED_*` result per
   module (the ledger acceptance); exclusions carried to §9;
4. P8.23 §4.1/§4.3 matrix assembled; verdict columns after tolerances;
   P8.23 §5 narrative;
5. §6–§9 of THIS paper assembled from the above (copy the source
   records' own outputs; refusals recorded AS the refusals);
6. §11 conclusions written by the human gate from the filled sections;
7. §14 independent review (step 4) recorded;
8. the claims gate below passes.

```sh
# (a) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the upstream gates FIRST (each input record owns its own commands;
#     a green upstream gate is a PRECONDITION, not an assumption):
#     - P8.06: freeze baseline results + pre-register tolerances;
#     - P8.09 §6 (a)-(d), P8.12 §8 (a)-(d), P8.15 §8 (a)-(d),
#       P8.17 §5 steps 0-2, P8.21 §7 (a)-(c): each module's evaluation
#       fills, each with its P8.20 run record and P8.05 outcome;
#     - P8.20 §8 (e)/(f): the run and rerun records fill;
#     - P8.23 §8 (a)-(e): the comparison matrix fills and its spot-checks
#       pass (a failed spot-check BLOCKS this paper until the cell is
#       corrected or re-run).

# (c) the paper fills (§6-§9): assemble each section FROM its source
#     record's own outputs only; carry the benchmark-set version id, the
#     P8.05 validation outcome, and the P8.20 record ref with every
#     filled cell; record every non-run/refusal AS the refusal; carry
#     NOT_REPRODUCED_MARKED_FOR_EXCLUSION results as EXCLUSIONS in §9,
#     never as caveats in the results text.

# (d) the ledger's verification method — every number traces to a
#     source: for each claimed number in §6-§9, open the source record
#     section (P8.09 §3 / P8.12 §5 / P8.15 §4-§5 / P8.17 §3 / P8.21 §5 /
#     P8.23 §4) and confirm the value appears there with its provenance
#     triple (benchmark-set version + validation outcome + P8.20 record).
#     Any number without a source FAILS the review (the ledger's
#     verification method, verbatim).

# (e) the claims gate (after §11 is written):
grep -n "win\|best\|outperform\|superior\|safe\|proves\|guarantee" \
  ledger/phase-08-advanced-coordination/26_benchmark_paper.md
#     every hit must sit inside §12's boundary statements, §10's
#     prohibitions, or a sentence scoped by the filled matrix and the
#     pre-registered tolerances — a product claim or an unsourced
#     winner claim is a finding against this paper.
```

## 14. Independent review record (ledger step 4) — UNRUN, the template

The report is reviewed by an independent reviewer (not a module author)
BEFORE it is called final; the review is recorded here. **No review has
happened** (nothing measurable exists to review); the template rows:

| Review item | Reviewer finding | Status |
|---|---|---|
| Every claimed number traces to its evaluation report (ledger verification method) | EMPTY | UNRUN — no filled number exists to check |
| Losses and not-tested items present (§10 filled; §6–§8 losses reported) | EMPTY | UNRUN |
| Honesty boundary present in the report (§12) | recorded PRESENT as a structural item — content review still UNRUN | template |
| No unaddressed major comment before "final" | EMPTY | UNRUN |
| Reviewer identity + date + verdict | EMPTY | UNRUN |

A "final" label without the recorded independent review and a green (a)–(e)
is a finding against this paper by definition.

## 15. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The frozen P8.06 baseline RESULTS artifact does not exist** (carried
   from P8.23 §9.1 and the per-module records): the baseline columns of
   §6–§7 cannot fill until the first human/lab run freezes them. No gap
   fix is invented here.
2. **The torch/PyG and RL-framework DECISION-POINTs are OPEN** (P8.10 §6,
   P8.13 §5; D4): the §6.2/§6.3 sections are structurally unfilled until
   they resolve. Recorded as dependencies, never fixed by inventing pins.
3. **The GNN scope-extension follow-up is unresolved** (carried verbatim
   from `12_gnn_evaluation.md` §7 gap 1 / `21_topology_research.md` §3.2):
   the transfer rows record `NOT RUN (scope-extension unresolved)`
   meanwhile.
4. **No object storage exists** (P8.20 §10.1): a result whose artifacts
   are not store-retrievable cannot be rerun-tested and cannot stand in
   this paper's claims — the store is the §6.5 owners' task, recorded as
   the dependency.
5. **No per-module tolerance is pre-registered** (P8.38 pending): no
   verdict column exists anywhere in this paper until the injected
   pre-registrations exist.
6. **The MARL shadow-log schema carries no fallback field** (carried
   from `15_marl_evaluation.md` §5): the filter-statistics rows record
   `NOT_RECORDED_IN_LOG_SCHEMA` for the fallback facts rather than
   fabricating a value; the additive log-schema extension is the
   recorded follow-up.

## 16. Non-claims (restated)

No result exists anywhere in Phase 8; every measured cell in §6–§9 is
EMPTY and (until the dependency chain of §13 resolves) not fillable. No
winner claim, ranking, safety claim, capability claim, or field-usage
claim exists in this paper, and none is pre-stated — the conclusions are
frozen headings over EMPTY content. The not-tested list (§10) bounds
every future claim to the tested range. Solver-class discipline verbatim
(HiGHS LP/QP/MIP, never SOCP; Clarabel convex conic, never MISOCP; ADMM
neither; AC power flow is not AC-OPF; steady-state PF is not transient
stability; HELICS is transport, not physics). The dynamic block's future
results will be evidence about a declared ANDES model only, never about
the grid (X-5, ADR-012). Proposals only (ADR-010) — nothing here is or
will be a dispatch decision, and no module is qualified for field use.
The entire paper governs simulation-only research artifacts (AGENTS.md
rule 5).
