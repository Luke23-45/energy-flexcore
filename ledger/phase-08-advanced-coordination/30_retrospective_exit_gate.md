# P8.30 — Phase 8 Retrospective, Open Items, and Exit-Gate Record

Phase: 8 — Advanced Coordination Research. Date: 2026-11-14.
Coordinator: implementation-coordinator (close performed after the
read-only consistency audit; all P8.01–P8.39 records are inputs).

## 1. The plain-language statement (ledger step 4)

**This phase is not a product feature, not a safety case, and not
field-qualified. It produces research proposals only.** No research module
in this phase has dispatch authority, field-write capability, or any form
of operational clearance. A MARL worker issuing live device commands is
PROHIBITED, not deferred (ADR-010; ENTRY_NOTE boundary 1). The ceiling of
this phase is simulation / shadow. Per-feature qualification is a separate
later decision (§2.1; Stage 9), and `29_qualification_record.md` records,
per module, what field use would require — every row NOT GRANTED /
NOT EVIDENCED today.

## 2. What was implemented (P8.01–P8.39, all 39 subtasks)

- **Governance (P8.01–P8.03):** readiness check (all seven stage-gate
  inputs recorded NOT HELD/UNRUN — the phase ran on the recorded Stage 7
  exception), research scope freeze (S/X registers, falsifiable questions,
  no numeric thresholds — X-8), research license audit (ANDES 2.0.0 GPLv3
  six-point isolation; HELICS 3.7.0 RE-VERIFY; torch/PyG DECISION-POINT).
- **Contracts, harness, baselines (P8.04–P8.06):** `research.py` reusing
  `worker_api.proto` unchanged (no parallel contract);
  `crates/flexcore-research` (20th workspace member, serde/serde_json
  only): the independent validation harness (the validator that does not
  trust the researcher; sealed `ValidatedProposal`; injected feasibility
  gate) and the frozen baseline registry (HiGHS LP/QP/MIP — not SOCP;
  Clarabel conic — not MISOCP; deterministic priority-list heuristic;
  frozen-version discipline).
- **ADMM/DMPC (P8.07–P8.09):** exact two-block ADMM with closed-form
  updates, five named convexity-assumption constants, exact feasibility
  certificates, infeasibility-before-iteration, bounded staleness via the
  injected local-worker port, centralized fallback to the frozen P8.06
  registry, DMPC rolling-horizon driver, proposals into the P8.05 harness;
  evaluation template with EMPTY cells.
- **GNN (P8.10–P8.12):** warm-start/screening design (speed, never
  optimality), isolated inference worker (`apps/worker/python/gnn_worker/`)
  with guarded torch/PyG imports (DECISION-POINT OPEN — nothing installed),
  stdlib-runnable pure-logic core, evaluation template with EMPTY cells.
- **MARL (P8.13–P8.15):** CTDE design with reward SHAPE only, safety
  filter with closed outcome vocabulary and a provable infeasible-filter
  fallback failure, shadow logger, worker with training UNREACHABLE,
  evaluation template with EMPTY cells and the §13.3 finite-test-set note.
- **Dynamic/co-simulation (P8.16–P8.18, P8.39):** ANDES worker (guarded
  imports; G-4 legal gate named; ANDES 2.0 call table UNVERIFIED),
  hand-computed dynamic metrics harness, EMPTY dynamic evaluation
  template, HELICS research-controller federate (transport not physics;
  no field-write path), co-sim/dynamic cross-check design (CC-1..CC-4,
  X-5 structural separation enforced in code, EMPTY results).
- **Benchmarks and comparison (P8.19–P8.23, P8.26):** versioned benchmark
  set `benchmarks/research-set-v1.json` + 11-item not-tested list (NT-1
  the 10,000-bus network first); reproducibility standard + seed
  derivation + rerun-check tooling (`apps/worker/python/reproducibility/`);
  topology transfer/scaling protocol; portfolio analytics (reporting-only,
  dispatch-shaped callables unrepresentable); cross-module comparison
  template (same-cases/same-metrics contract, refusal rather than merge);
  benchmark paper template (the not-tested section is the one fillable
  today — filled).
- **Hardening (P8.24, P8.25, P8.28, P8.31–P8.38):** failure-mode drill
  plan (8 fault groups × 6 modules, expected behaviors cited from code,
  run UNRUN), security review by reading (HIGH finding F-1 recorded —
  see §4), additive CI gates (fail-closed on the pin gaps), tenant
  isolation (`tenant_isolation.rs`: host-stamped scopes, echo-mismatch
  refusal, model-serving register), model-artifact signing
  (`artifact_signing.rs`: provenance-v1 mirror, NoArtifactVerifier
  fail-closed default, revocation register, load gate composing the P8.31
  register), worker identity/mTLS/secrets (`worker_identity.rs`:
  references-only secrets, value-never-echoed scan), proposal validity
  windows (`proposal_windows.rs`: exactly-at-expiry REJECTED,
  restart-surviving supersession checkpoints), training-data provenance +
  the held-out-topology manifest (un-blocks the P8.10/P8.12/P8.21 hold-out
  at record level, with two recorded fixture-fact findings), hand-written
  property suites (stdlib generators, fixed seeds), GPU-nondeterminism
  disclosure standard (tolerance classes; values pre-registered before
  first rerun).
- **Console and qualification (P8.27, P8.29):** read-only Research
  workspace + control-plane GET-only route (every run fact an explicit
  gap, never a plausible value); the per-feature qualification record.

**Volume:** `crates/flexcore-research` = 10 source files, 14,710 lines
+ 3 integration-test files; `apps/worker/python/` grew six new research
directories (andes, gnn, marl, portfolio, reproducibility, drill) ≈ 19,000
lines incl. tests; benchmarks artifacts; 37 decision records + this one;
all suites WRITTEN-UNRUN.

## 3. What passed, what did not (ledger step 1 — honesty)

**Nothing has been executed.** Nothing installed, nothing built, no test
run, no CI run, no drill run, no advisory query — the loop bans held for
all 39 subtasks. Consequently:

- Every evaluation/training/benchmark cell in P8.09, P8.12, P8.15, P8.17,
  P8.21, P8.23, P8.26, P8.39 is **EMPTY by design**. There are no
  inconclusive evaluations in this phase because there are no evaluations;
  presenting the templates as results would be the failure mode the ledger
  names for P8.30, and this record exists to prevent it.
- The Stage 8 exit gate is **NOT HELD** (§5 below): "compared to strong
  baselines" and "independently validated" require RUNS. What exists is
  the complete machinery for those runs plus the frozen baselines and
  templates they will fill.
- The read-only consistency audit (37-record inventory, tick-wording
  sweep, empty-cell spot-check, cross-reference check, TODO sweep,
  honesty grep: UNRUN×187, EMPTY×792, WRITTEN-UN×75, NOT_RUN×47,
  NOT HELD×18; zero fabricated figures found; three permitted tools OK)
  was run before this close; its one substantive finding (8 tick lines
  lacking canonical wording) was fixed in the coordinator pass; P8.39's
  record landed after the audit snapshot and is included here.

## 4. Findings carried (owners named)

| ID | Item | Owner |
|---|---|---|
| O-1 | P8.06 frozen baseline **RESULTS** artifact does not exist — objective-gap/speedup/baseline-comparison cells unfillable until the first human/lab run freezes it | Human/lab gate (research lead) |
| O-2 | §19.4 numeric thresholds not pre-registered (X-8) — must be registered BEFORE the first run, dated amendment procedure per record | Research lead |
| O-3 | GNN scope-extension mechanism: `trained_topology_ids` is EMPTY until training instances exist, so every inference flags out-of-scope (fail-closed, correct — but the transfer experiment needs the P8.04/P8.35 extension recorded in `12_gnn_evaluation.md` §7) | P8.04/P8.35 owners |
| O-4 | The §13.3 rule-based/PI baseline has **no frozen id** in the P8.06 registry — MARL comparisons refuse (`BASELINE_UNKNOWN`) until one is defined/run/frozen | P8.06 owner |
| O-5 | Security F-1 (HIGH): unguarded `torch.load(...)` at `gnn_inference_worker.py:924–927` — unreachable today (guarded import) but BLOCKS the with-framework activation until a non-executing format is pinned; F-2/F-3 analogues in MARL/ANDES load sites | P8.10/P8.11 + P8.14/P8.16 owners at activation |
| O-6 | P8.24 drill RUN is UNRUN — independent witness re-runs two cells (human gate input) | Human gate |
| O-7 | P8.36 advisory lookups NOT-EXECUTED — every pin cell UNRUN; re-check triggers on any version move | Human gate at pin time |
| O-8 | P8.28 CI jobs written, never run; Python gates fail-closed while pins are UNPINNED-REVERIFY/DECISION-POINT | Human gate |
| O-9 | `research.status.read` authz action does not exist (interim `telemetry.read` deny-by-default gate, single swap point) | flexcore-authz owner |
| O-10 | No `research_controller` role in the P5.16 HELICS orchestrator — the research federate cannot run under it until the additive role variant lands | sim-runner owner |
| O-11 | Proto extensions needed (Phase 7 contract owner, buf governance): validity-window fields (WF-1), tenant-scope field (TF-1), research payload types (dynamic-study, warm-start vector/ranking entries, MARL filtered setpoints) | Phase 7 contract owner |
| O-12 | torch/PyG and RL-framework DECISION-POINTs OPEN (recommendations recorded; D-decisions are never closed by agents) | Product (dated decision not made) |
| O-13 | ANDES 2.0.0 GPLv3 legal confirmation gate G-4 OPEN — six-point isolation designed and coded, legal sign-off pending | Human/legal gate |
| O-14 | Training-data inventory EMPTY (no dataset exists); the held-out manifest pins FAMILIES, instances pending | Research lead |
| O-15 | PLAN.md §3 location drift (marl-worker location) recorded | P9 planning |

## 5. Exit-gate position (§23 Stage 8)

**NOT HELD — prepared, not demonstrated.** The gate's substance ("every
research module compared to strong deterministic baselines and
independently validated") requires executed evaluations against frozen
baseline results; every such cell is EMPTY. What this phase delivered is
the gate's *machinery*: frozen baseline registry, independent validation
harness, versioned benchmark set, comparability contract, reproducibility
standard, and per-module templates whose measured cells await exactly the
runs the gate demands. `CHECKLIST.md` records each item's evidence state;
the run-gated items are annotated NOT MET — prepared. The "research only,
not field-qualified" limitation is stated in plain language in §1 above
and in `29_qualification_record.md`.

## 6. Deferred work with owners (ledger step 2)

- **WASI 0.3-dependent research workers:** none this phase; WASI 0.3
  remains tracked-not-blocking (Phase 7 handover carries the item).
- **HELICS federation extensions:** the `research_controller` orchestrator
  role (O-10) and any multi-federate research federation (owner: sim-runner
  + research lead, post-first-run).
- **Additional test systems:** IEEE 39/118-bus transmission cases (NT-2/3,
  licensing + models required), an unseen-family feeder fixture (O-3
  family), benchmark-set v2 items — all gated on real need, never silent
  growth of v1 (anti-silent-growth rule).
- **Inconclusive evaluations:** none exist to carry — all cells EMPTY by
  design (§3).

## 7. Open research questions and the next experiment each needs (ledger step 3)

Frozen in `02_research_scope_freeze.md` §4; per-question protocols and
EMPTY tables live in the per-module evaluation records. Summary:

1. *Does ADMM coordination beat the frozen Tier-1 baseline within its
   convexity assumptions?* → fill `09_admm_evaluation.md` after O-1/O-2.
2. *Does GNN warm-starting improve end-to-end feasible solve time without
   costing feasibility?* → fill `12_gnn_evaluation.md` after O-3, the
   framework DECISION-POINT (O-12), and O-1/O-2.
3. *Does the MARL policy beat deterministic/rule-based baselines under
   distribution shift with a safety filter engaged?* → fill
   `15_marl_evaluation.md` after the RL DECISION-POINT (O-12), O-4, and
   O-1/O-2; training remains BLOCKED until the safety-filter derivation
   is reviewed (`13_marl_design.md`).
4. *Do dynamic responses stay within the declared limits under the
   recorded disturbance set?* → fill `17_dynamic_evaluation.md` after G-4
   (O-13) and the ANDES install gate.
5. *Do co-sim/dynamic cross-checks agree on their overlapping domain?* →
   fill `39_cosim_crosschecks.md` after 4 and the shared case-exchange
   manifest (P8.39 follow-up).
6. *Do results transfer across topology families and scale?* → fill
   `21_topology_research.md` after 2 and the P8.35 instance manifest.

## 8. Reviewer verification

Read `CHECKLIST.md` (every item carries its evidence state), the
per-subtask records `01`–`39`, and this file's §3–§5; trace any claim to a
deliverable. Build/test commands are in `HANDOVER.md`. Everything in this
phase is WRITTEN-UNRUN; the human build/test gate owns all execution
evidence.
