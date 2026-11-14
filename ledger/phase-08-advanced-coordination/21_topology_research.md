# P8.21 — Topology-aware optimization research (transfer and scaling): protocol and evaluation record (TEMPLATE — every measured cell EMPTY)

Date: 2026-11-14. Subtask: P8.21 (`ledger_phase_8.md` §P8.21). Owner role:
research engineer (ML/optimization). Report trace: §13.1 (topology
embedding and transfer learning); §2.1 (v2.x topology-aware optimization
research); §19.3–§19.4. Inputs: P8.12 (`12_gnn_evaluation.md` — the GNN
evaluation this transfer protocol builds on) and P8.09
(`09_admm_evaluation.md` — the ADMM per-case evaluation the scaling
protocol builds on) — both TEMPLATES with EMPTY cells; P8.10
(`10_gnn_warmstart.md` §4 — the hold-out RECOMMENDATION, UNCONFIRMED
until the P8.35 manifest); P8.19 (`19_benchmark_cases.md` +
`benchmarks/research-set-v1.json` — the versioned benchmark set this
record cites); P8.02 (`02_research_scope_freeze.md` — the frozen M2
questions and prohibitions X-3/X-7/X-8); `benchmarks/README.md` (the
P3.27 no-targets and raw-scaling-numbers rules); P8.06
(`05_06_harness_baselines.md` §3 — the frozen baseline registry).

## 1. Status of this record — WRITTEN-UNRUN, a protocol + EMPTY template

This record is the transfer/scaling PROTOCOL (ledger steps 1–2) and the
EMPTY evaluation template (steps 3–4 reserved for the measured fills).
**Every measured cell in §5 is EMPTY, and none is fillable today**: the
dependencies are recorded in §6 — P8.12 (the GNN held-out numbers) and
P8.09 (the ADMM per-case numbers) are themselves templates whose runs
are human/lab gate inputs; the torch/PyG framework DECISION-POINT is
OPEN (`03_research_license_audit.md` §4 — nothing installed, no
inference output exists); the hold-out assignment is UNCONFIRMED until
the P8.35 training-data manifest exists; the frozen P8.06 baseline
RESULTS artifact does not exist. Nothing was built, installed, or
executed for this record (ENTRY_NOTE loop bans). No transfer, runtime,
objective-gap, or scaling figure exists anywhere in it.

## 2. The falsifiable questions (from the P8.02 freeze; restated, not re-decided)

| # | Question (frozen form) | Metric (§19.4 row) | Baseline | Falsified if |
|---|---|---|---|---|
| Q-T1 (GNN transfer) | On topology families held out of GNN training — varying in size, meshing, and DER mix (ledger step 1) — does the warm-start/screening behaviour measured by P8.12 hold, per topology family, without degrading beyond the pre-registered thresholds? | feasible solver success; end-to-end feasible solve time; speedup; generalization to unseen topologies (per topology) | cold-start Tier 1 HiGHS + the non-GNN heuristic warm start (P8.06) | a family shows no end-to-end speedup, or a feasible-success loss, or screening miss-rate/miss-cost beyond the pre-registered threshold — with per-family numbers reported, wins and losses alike |
| Q-T2 (ADMM transfer) | Does the ADMM/DMPC provider's validated coordination behaviour (P8.09's per-case §19.4 statistics) hold across topology families — radial fixture vs balanced meshed fixture, and across coupling strength — within the pre-registered thresholds? | primal/dual residual, iteration count, time, communication sensitivity, objective gap WHERE MEANINGFUL | centralized HiGHS (Tier 1/Tier 2) + rule-based priority list (P8.06) | a topology family or coupling regime where residuals do not terminate within budget or the objective gap exceeds the threshold without a compensating coordination benefit |
| Q-S (scaling) | How do runtime and solution quality (objective value, feasible interval count) change with problem size for the research modules AND the centralized baseline on the SAME cases (ledger step 2) — and at which measured point does each approach stop scaling acceptably? | runtime; objective value; feasible interval count (§19.4 scheduling row) — RAW NUMBERS ONLY | the centralized baseline (P8.06 registry) on the identical cases | a degradation vs the baseline beyond the pre-registered threshold at a named measured point — reported, never extrapolated away |

The numeric thresholds in the "pre-registered" clauses do not exist yet
— the §19.4 preconditions are still missing (X-8; P8.02 §5): they are
pre-registered in writing after the model, service, benchmark, and
operational risk are defined, at P8.06 freeze time at the earliest,
never as universal targets, never after research numbers are known.

## 3. The transfer protocol (ledger step 1)

### 3.1 Train/evaluate across topology families; the hold-out discipline

- **Training families / held-out families**: exactly the P8.10 §4
  RECOMMENDATION, carried UNCONFIRMED — training families
  lib-01…lib-05; held-out families lib-06/lib-07/lib-08 PLUS at least
  one structurally unseen feeder family; held-out operating regimes =
  the P8.19 research-case extensions (now versioned as
  `flexcore-research-benchmark-set-v1`). **UNCONFIRMED until the P8.35
  training-data manifest exists** and the reviewer's overlap check
  passes (the ledger P8.10 verification method). No training run may
  start before that confirmation.
- **The overlap check** is the P8.12 protocol's `check_heldout_overlap`
  (exact string equality over `trained_topology_ids`); any overlap is a
  recorded LEAKAGE finding invalidating the generalization claim for
  the overlapping topologies.
- **Per-topology transfer metric** (ledger step 1: "record the transfer
  metric per topology"): the P8.12 warm-start and screening metrics,
  computed per held-out topology/family — end-to-end feasible solve
  time, feasible-success rate, paired speedups, ranking quality, miss
  rate, miss cost — in §5.1's rows. No aggregate hides a family: a
  per-family table is the deliverable, and the "where it does not
  generalize" section (§5.3) is REQUIRED, not optional.
- **The size/meshing/DER-mix axes** (ledger step 1) are recorded per
  row from the benchmark-set artifact's parameters — node/asset counts,
  radial vs meshed (lib-03 is the only meshed fixture), and the DER
  composition of the underlying portfolio. They are descriptive axes of
  the tested range; they are NOT a size claim (§4).

### 3.2 The known blocker, carried verbatim from P8.12 §7 gap 1

The P8.11 worker FLAGS any topology outside `trained_topology_ids` and
returns NULL outputs — correct for production, but held-out topologies
are BY DEFINITION outside the trained list, so the transfer experiment
as designed cannot produce worker outputs until the recorded
scope-extension follow-up (an evaluation-scope manifest version, or an
equivalent P8.04 contract extension such as an `evaluation_scope` field
the flag logic honours) is decided by the P8.04/P8.35 owners. NOT fixed
by inventing a parallel contract or bypassing the flag. Until it is
resolved, held-out runs produce the contract-correct out-of-scope flag,
NOT numbers (§8, filling-command step 3).

### 3.3 Transfer non-claims (fixed now)

- A warm start affects the SPEED of an exact solve, never its
  optimality (X-3; the P8.10 headline rule, verbatim) — a transfer
  result is a speed/success result, never an optimality improvement.
- Generalization is claimed ONLY over the tested range (X-7); feeder-
  scale transfer results are not 10,000-bus capability (§19.3).
- ADMM convergence evidence is residual convergence under the P8.07
  convexity assumptions; no global-convergence/optimality claim outside
  them; nonconvex/MI instances are heuristic behaviour (X-3).
- Solver-class discipline VERBATIM (AGENTS.md rule 4; §7.4): HiGHS is
  LP/QP/MIP, never SOCP; Clarabel is convex conic, never MISOCP; AC
  power flow is not AC-OPF.
- A finite test set with zero observed failures is a test outcome, not
  a guarantee (§13.3; X-4).

## 4. The scaling protocol (ledger step 2) — explicit points and what they do NOT claim

### 4.1 The explicit scaling points

The only scaling mechanism that exists in this repository is the P3.27
in-memory portfolio-copy probe (`apps/sim-runner` `benchmark --scaling
2,4`: a deterministic deep copy of every portfolio site, keys suffixed
`--x<factor>-<k>`, recorded in the record's `scaling` block — never
committed scenario files). The declared scaling points of this protocol
are therefore:

| Point | Size | Source | Status |
|---|---|---|---|
| 1× | the lib-02 / P3.26 base portfolio (2 sites, 10 assets) | `scenario-a-normal-day.json` (and the case-specific artefacts) | NOT RUN |
| 2× | 4 sites, 20 assets (in-memory deterministic copy) | `benchmark --scaling 2` | NOT RUN — raw numbers only |
| 4× | 8 sites, 40 assets (in-memory deterministic copy) | `benchmark --scaling 4` | NOT RUN — raw numbers only |

Rules, fixed before any number:

1. **Raw numbers only** — no ratio, exponent, trend, or "scaling law"
   may be derived from these points in any document (the benchmarks
   README rule, quoting ledger P3.27 verbatim: "do not claim a scaling
   law from two points — report the raw numbers"). Three points make
   this no more true.
2. **Same cases for every column** (ledger step 2): the research module
   and the centralized baseline run on the IDENTICAL case inputs,
   seeds, and versions; the baseline objectives/timings come from the
   frozen P8.06 results artifact, which does not exist until the first
   human/lab run.
3. **A scaling-point change is a benchmark-set version change**: any
   additional scaling point (a larger feeder fixture, a structurally
   unseen family, any transmission-scale case) enters ONLY as
   `flexcore-research-benchmark-set-v2` per the P8.19 anti-silent-growth
   rule — never by growing the set silently between evaluations.
4. **Runtime honesty**: wall-clock runtimes are human-run measurements
   recorded outside the determinism-controlled modules (the P8.09
   separation, verbatim); a developer-laptop or CI-runner figure is
   never a deployment figure (the benchmarks README rule 4).

### 4.2 What the scaling protocol does NOT claim (fixed now)

- **No 10,000-bus claim** (ledger step 4, verbatim): no capability for
  a 10,000-bus utility network may be stated, implied, or extrapolated
  from feeder-scale tests (§19.3; P8.02 X-7; not-tested NT-1). The
  tested range ends at the 4× in-memory portfolio copy on synthetic
  fixtures.
- **No scaling law** from two or three points (rule 1 above).
- **No linear extrapolation**: the named risk of this subtask is
  "scaling extrapolated linearly from two points" — prohibited; limits
  are stated only at MEASURED points, with the measured numbers.
- **No solver-class drift**: the baseline comparison is against the
  declared classes (HiGHS LP/QP/MIP; Clarabel conic) — an objective-gap
  column is filled only where the comparison is meaningful (the P8.09
  rule).

## 5. Result tables — ALL MEASURED CELLS EMPTY

### 5.1 Transfer table (per held-out topology family; generalization claims cite these rows ONLY)

| Topology family (P8.35-confirmed; RECORD-AT-FIRST-RUN) | Held-out? | Size class (nodes/assets) | Meshing | DER mix | GNN role (warm-start / screening) | End-to-end feasible time (s) | Feasible-success rate | Paired speedup vs cold | Paired speedup vs heuristic | Screening precision@k / miss rate / miss cost | P8.05 validation outcome | Benchmark-set version | Model artifact + manifest refs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 | EMPTY |

### 5.2 Scaling table (per scaling point; research module AND centralized baseline on the SAME case)

| Scaling point | Case (recorded artefact) | Sites/assets | Module (ADMM / GNN-warm-start / baseline tier) | Runtime (s, human-recorded) | Objective value (declared cost units) | Feasible interval count | Violations | P8.05 validation outcome | Benchmark-set version |
|---|---|---|---|---|---|---|---|---|---|
| 1× | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| 2× | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |
| 4× | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | flexcore-research-benchmark-set-v1 |

Filling rules (recorded before any number): cells come from the P8.12
protocol functions, the P8.09 harness outputs, and the human-recorded
wall-clock figures — never from memory, never estimated; a refusal
(`VALIDATION_OUTCOME_MISSING`, `THRESHOLD_NOT_PRE_REGISTERED`,
`OBJECTIVE_GAP_NOT_MEANINGFUL`, out-of-scope flag) is recorded AS THE
REFUSAL, never as a blank or a number; every counted "success" passed
the P8.05 independent validation; every row carries the benchmark-set
version id; the Win/Tie/Loss vocabulary requires the pre-registered
tolerance injected from the P8.06 freeze record (the harness refuses
otherwise).

### 5.3 The limits note skeleton (ledger step 3 — headings frozen BEFORE the runs)

- **Where transfer stops** — EMPTY: the topology size, meshing class,
  and DER mix beyond which each approach degrades, WITH THE MEASURED
  NUMBERS at measured points only (ledger step 3). This section is
  REQUIRED even when the answer is "no degradation observed within the
  tested range" — which is then stated as exactly that, not as
  capability.
- **Where scaling stops** — EMPTY: the measured point beyond which
  runtime/quality degrades beyond the pre-registered threshold vs the
  centralized baseline, with the raw numbers; no extrapolation beyond
  the last measured point.
- Fixed limits, independent of results: §3.3 and §4.2 in full (no
  optimality claim from a warm start; no convergence claim outside the
  P8.07 assumptions; no scale claim beyond the tested range; no scaling
  law from the probe points; feeder-scale results are not 10,000-bus
  capability; proposals only, ADR-010).

## 6. Dependencies and ordering (recorded, per the ledger's inputs line)

| Dependency | What this record needs from it | Its status |
|---|---|---|
| P8.12 (`12_gnn_evaluation.md`) | the per-topology GNN numbers the transfer table regroups | TEMPLATE — EMPTY cells; torch/PyG DECISION-POINT OPEN; runs are human/lab inputs |
| P8.09 (`09_admm_evaluation.md`) | the per-case ADMM §19.4 statistics and the frozen baseline objectives the transfer (Q-T2) and scaling (Q-S) comparisons consume | TEMPLATE — EMPTY cells; the P8.06 frozen RESULTS artifact does not exist until the first run |
| P8.35 training-data manifest | the CONFIRMED hold-out assignment (P8.10 §4) — no training run before it | NOT STARTED |
| P8.19 (`benchmarks/research-set-v1.json`) | the versioned case list and parameters every row cites | FROZEN — `flexcore-research-benchmark-set-v1` (WRITTEN-UNRUN) |
| P8.06 freeze record | the pre-registered thresholds/tie tolerances (X-8) without which no verdict column exists | thresholds NOT pre-registered |
| P8.04/P8.35 owners | the scope-extension follow-up (§3.2) without which held-out GNN runs produce the flag, not numbers | OPEN |

Ordering: nothing in §5 fills before P8.12/P8.09 fill; the transfer
rows additionally need the P8.35 confirmation and the §3.2 follow-up;
the scaling rows additionally need the frozen P8.06 baseline results.

## 7. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the upstream gates FIRST (their own records own these commands):
#     - P8.12 §8 (a)-(d): the GNN protocol self-tests, the pytest mirror,
#       the DECISION-POINT resolution, the hold-out confirmation against
#       the P8.35 manifest, check_heldout_overlap recorded;
#     - P8.09 §6 (a)-(d): the flexcore-research build/tests, the grep
#       discipline, the baseline freeze per case.
#     A green upstream gate is a PRECONDITION, not an assumption.

# (c) the transfer fills (ledger step 1; §5.1) — after (b):
#     1. confirm the hold-out families against the P8.35 manifest and
#        record the confirmation IN THE MANIFEST (this record never
#        upgrades the RECOMMENDATION by assertion);
#     2. resolve the §3.2 scope-extension follow-up with the P8.04/P8.35
#        owners (a recorded decision) — until then held-out runs produce
#        the out-of-scope flag, NOT numbers;
#     3. per held-out topology: run cold-start, heuristic-warm-start,
#        and GNN-warm-start solves with pinned seeds/versions (P8.12 §8
#        step 5) and the screening experiments against exact-solve
#        ground truth (P8.12 §8 step 6);
#     4. assemble §5.1 rows from the P8.12 protocol functions' outputs;
#        cite flexcore-research-benchmark-set-v1 on every row.

# (d) the scaling fills (ledger step 2; §5.2) — after the P8.06 freeze:
#     1. run the centralized baseline per scaling point from the frozen
#        registry (the P8.06 composition-root path), pinned versions/
#        seeds/tolerances;
#     2. run the research module per scaling point on the IDENTICAL
#        case inputs; wall-clock recorded OUTSIDE the modules and
#        injected (the P8.09 separation);
#     3. every proposal through the P8.05 harness; record the outcome;
#     4. pre-register the thresholds/tie tolerances (P8.06 freeze
#        record) BEFORE any verdict; assemble §5.2 rows — RAW NUMBERS
#        ONLY, no ratio, exponent, or trend anywhere.

# (e) the ledger's verification method:
#     independent re-run of ONE transfer case and ONE scaling point
#     from the recorded inputs; the transfer metric must recompute from
#     the raw timings; the scaling row must reproduce within the
#     pre-recorded tolerance (P8.20/P8.38 discipline — declared per
#     module BEFORE the rerun, never fitted after).

# (f) the claims gate:
#     grep -n "scale\|10,000\|optimality\|converge" ledger/phase-08-advanced-coordination/21_topology_research.md
#     every hit must sit inside a sentence scoped by §3.3/§4.2 — a bare
#     capability/scale/optimality claim is a finding against this record.
```

A green (a) plus upstream (b) earns only a one-line "human build/test
gate: PASSED (date)" appended here; failures are recorded verbatim and
the ledger tick reverts to `- [ ]`. The measured cells of §5 stay EMPTY
until the (c)/(d) runs fill them.

## 8. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Transfer measured on unseen topologies with per-topology results | PROTOCOL RECORDED (§3); MEASUREMENT UNRUN — §5.1 EMPTY; the hold-out assignment is UNCONFIRMED until the P8.35 manifest; the scope-extension follow-up (§3.2) must resolve before held-out outputs exist |
| Scaling measured against the centralized baseline | PROTOCOL RECORDED (§4 — explicit points 1×/2×/4×, same cases for every column); MEASUREMENT UNRUN — §5.2 EMPTY; the frozen P8.06 baseline results do not exist |
| Limits stated with numbers | REQUIRED SECTION FROZEN (§5.3) — content EMPTY until the runs; limits will be stated ONLY at measured points, with the measured numbers |
| No scale claim beyond the tested range | §4.2 fixed now; the (f) grep gate enforces it on the record itself; X-7 and not-tested NT-1 carry it repo-wide |

## 9. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The scope-semantics gap** (carried VERBATIM from
   `12_gnn_evaluation.md` §7 gap 1): the held-out-by-definition flag
   blocks the transfer experiment's outputs; needs a recorded
   scope-extension decision by the P8.04/P8.35 owners — never a
   parallel contract, never a bypass.
2. **The closed P8.04 result-key set carries no payload fields** and
   **no distinct screening payload type** (carried from
   `11_gnn_inference_worker.md` §8 items 2–3): the ADDITIVE extension
   proposal belongs to the P8.04 owner.
3. **The frozen P8.06 baseline RESULTS artifact does not exist**
   (carried from `09_admm_evaluation.md` §8.1): no speedup/objective-gap
   cell can fill until the first human/lab run freezes it.
4. **No structurally-unseen topology-family fixture or generator
   exists**: P8.10's "at least one topology family absent from the
   library entirely" has no concrete artefact — creating one is a
   benchmark-set v2 candidate under the P8.19 anti-silent-growth rule
   (recorded candidate, not silently promoted).
5. **No scaling mechanism beyond the ×2/×4 in-memory portfolio copies**
   exists in `apps/sim-runner`: larger fixed fixtures (or a real
   multi-feeder size ladder) would be new sim-runner capability —
   recorded as a follow-up for its owners, not invented here, and
   gated on the benchmark-set version rule (§4.1 rule 3).
6. **Wall-clock runtime lives outside the determinism-controlled
   modules by design** (the P8.09 separation, carried): the runtime
   column is a human-run measurement injected into the harness — a
   recorded separation, not a defect.

## 10. Non-claims (restated)

No transfer, runtime, objective, success-rate, ranking-quality, miss-
cost, or scaling figure from a real run exists anywhere in this record
— every measured cell is EMPTY and (until §6's dependencies resolve)
not yet fillable. No optimality claim from a warm start; no ADMM
convergence claim outside the P8.07 assumptions; no scaling law from
the probe points; no capability claim beyond the tested range — and
never a 10,000-bus claim from feeder-scale tests (§19.3; X-7; NT-1).
No safety claim and no field authority (proposals only, ADR-010).
Solver-class discipline verbatim (HiGHS LP/QP/MIP, never SOCP; Clarabel
convex conic, never MISOCP; AC power flow is not AC-OPF). The §19.4
numeric thresholds are still missing by design (X-8).
