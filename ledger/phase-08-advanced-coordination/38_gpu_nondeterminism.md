# P8.38 — Training-run reproducibility and GPU-nondeterminism disclosure (the disclosure standard — every measured and pre-registration VALUE cell EMPTY, no run exists)

Date: 2026-11-14. Subtask: P8.38 (`ledger_phase_8.md` §P8.38). Owner role:
Research lead. Report trace: §19.2(5); §9.2; §6.5. Inputs: **P8.20**
(`20_reproducibility.md` — the reproducibility standard this record
completes: the record schema's `tolerance_class`/`tolerance_ref`/
`nondeterminism_sources` fields, the rerun-test definition, and the
comparator's `DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38` refusal);
P8.06 (`05_06_harness_baselines.md` — the frozen baseline registry, its
seed-policy column, and the HiGHS `thread_count` 1 pin); P8.10
(`10_gnn_warmstart.md` §6) and P8.13 (`13_marl_design.md` §5) — the
framework DECISION-POINTs this record carries OPEN; P8.35
(`35_training_data_provenance.md` — the seed/dataset provenance the
mitigation procedure pins); P8.02 (`02_research_scope_freeze.md` X-8 —
no arbitrary universal targets); ENTRY_NOTE boundary 4.

## 1. Status of this record — a DISCLOSURE STANDARD; no run exists, no tolerance VALUE exists

This record is what P8.20 deferred to P8.38: the per-module tolerance-
CLASS rule (§3), the nondeterminism-source register (§4), the
mitigation procedure (§5), the disclosure that travels on every
training result (§6), and the rerun-set definition (§7). **No training
run, no inference rerun, and no measured cell exists anywhere** — every
execution and every tolerance VALUE below is EMPTY and is filled only
by the human/lab gates via §9's procedure. Nothing was installed that
could verify any framework behaviour, so every nondeterminism source in
§4 is a recorded RISK from documented framework-family behaviour, never
a verified observation (the framework DECISION-POINT is OPEN and is
carried, not closed — §4.1).

The class/value distinction (stated once, binding everywhere): this
record DOES fix, per module, which tolerance CLASS applies to a rerun
(`exact` / `within_tolerance` / `distribution` — a structural fact
about the module's recorded nature, declarable now); it does NOT fix
any tolerance VALUE. A value is registered per module BEFORE that
module's first rerun (X-8: after the model, service, benchmark, and
operational risk exist — all of which are EMPTY today), with its basis
recorded, never fitted after the rerun. P8.20's "PENDING — no tolerance
is pre-registered anywhere yet" remains true for VALUES after this
record; what changed is that the CLASSES and the registration
procedure now exist.

## 2. Why this record exists (the ledger's named risks, answered structurally)

| Ledger P8.38 risk | Answer (where) |
|---|---|
| "GPU nondeterminism unrecorded and presented as exact reproduction" | the class rule (§3): every ML module is `within_tolerance` (inference) or `distribution` (training) BY DECLARATION — no ML module may claim `exact`; the record schema REFUSES a within-tolerance/distribution record with empty `nondeterminism_sources` (`NONDETERMINISM_SOURCES_REQUIRED`), so an undisclosed GPU-nondeterminism record is structurally unenterable (§6) |
| "inference reruns conflated with training reruns" | the class rule (§3) + P8.20's comparator: a distribution-class primary is `BLOCKED_DISTRIBUTION` in the local comparator, and an inference tolerance applied to a training rerun is the exact class confusion the schema and comparator make unrepresentable |
| "tolerances fitted after the rerun" | the registration procedure (§3.2): the value enters the pre-registration record BEFORE the rerun, the comparator refuses without it (`TOLERANCE_NOT_PRE_REGISTERED`), and the record carries the `tolerance_ref` (`TOLERANCE_REF_REQUIRED`) |
| "a result that does not reproduce silently dropped" | P8.20 §5 item 6: `NOT_REPRODUCED_MARKED_FOR_EXCLUSION` with the standing exclusion statement — P8.26 must not cite it (§8 here restates the consumption rule) |

## 3. The tolerance-class rule (ledger step 1) — declared per module, VALUES EMPTY

### 3.1 The class assignment (declared NOW — a fact about each module's recorded nature)

| Module (the closed P8.20 vocabulary) | Rerun kind | Class | Basis (recorded) |
|---|---|---|---|
| `baseline/highs-tier1-lp` | inference-equivalent (a solve) | `exact` | HiGHS at `thread_count` 1 with a fixed caller seed is deterministic by its recorded operating posture (P8.06/P3.26/P8.20 §5); any deviation means the run was not the recorded run |
| `baseline/clarabel-tier2-conic` | inference-equivalent | `exact` | the algorithm has no seed input and is deterministic by its recorded nature (P8.06 §3 seed-policy column) |
| `baseline/rule-based-priority-list` | inference-equivalent | `exact` | no randomness anywhere in the heuristic (P8.06 §3) |
| `admm-dmpc` | inference-equivalent | `exact` | `seed_supported = false` — no randomness; the byte-equality discipline (P8.08); the provider is in-crate with no external solver (P8.07) |
| `gnn-warm-start` | INFERENCE rerun | `within_tolerance` | a GPU/framework-executed model — kernel reduction order and library algorithm selection are disclosed nondeterminism sources (§4); the VALUE is registered before the first rerun |
| `gnn-screening` | INFERENCE rerun | `within_tolerance` | same basis as `gnn-warm-start` |
| `marl-policy` | INFERENCE rerun | `within_tolerance` | same basis, for the RL framework's policy forward (framework DECISION-POINT OPEN — P8.13 §5) |
| `gnn-warm-start` / `gnn-screening` / `marl-policy` | TRAINING rerun | `distribution` | a training rerun reproduces a DISTRIBUTION of outcomes (initialization, data order, kernel nondeterminism), not a number; the local comparator refuses it (`BLOCKED_DISTRIBUTION`) and the P8.38-owned distribution-comparison procedure (§7.2) applies |
| `dynamic-controller` | simulation rerun | `within_tolerance` — **RECOMMENDATION, UNVERIFIED** | ANDES 2.0.0 is pinned-but-never-installed (P8.03 §2); the class is declared as a recommendation and CONFIRMED at the dynamic track's first pre-registration, when the installed solver's determinism behaviour is a recorded fact. No dynamic rerun cell exists to commit it today |

### 3.2 The value-registration procedure (BEFORE the first rerun — X-8)

For each module, BEFORE its first rerun, the human/lab gate records one
pre-registration entry: `{module_id, rerun_kind, tolerance_class,
tolerance_value | distribution_procedure_ref, basis, registered_utc,
registered_by}`. Rules:

1. The VALUE (or the distribution procedure reference) is recorded
   BEFORE the rerun and never edited after — a change is a NEW
   pre-registration version, and the affected results are re-judged
   against the class they were run under (the P8.19 anti-silent-growth
   rule, mirrored).
2. The basis states WHY the number (e.g. the metric's decision
   relevance, the run-to-run spread of a warm-up series on the pinned
   hardware) — never a universal target (X-8); "zero deviation" is the
   `exact` class, not a tolerance value of 0.
3. The entry is INJECTED into the P8.20 comparator as
   `{class, value?, pre_registration_ref}`; the comparator refuses any
   comparison without it and never derives, defaults, or suggests a
   value (P8.20 §5 item 4).
4. **Every pre-registration VALUE cell in this record is EMPTY** — the
   pre-registration table itself is created at the first
   pre-registration (§9); writing numbers here, with no model, no
   benchmark, and no risk defined, would fabricate the X-8
   precondition.

## 4. The nondeterminism-source register (ledger step 3) — recorded RISKS, nothing verified

These are the sources any training/inference rerun must DISCLOSE. They
are named GENERically, from the documented behaviour of the candidate
framework families (PyTorch/PyG for the GNN line; Ray/RLlib + Gymnasium
for the MARL line — both DECISION-POINTs OPEN). **Nothing is installed
in this repository, so no source below is a verified observation**;
each becomes a verified fact (or is scoped out) at the human pin, per
§4.1.

| # | Source (generic) | What it affects | Recorded risk statement |
|---|---|---|---|
| N-1 | GPU kernel reduction order (parallel floating-point reductions are order-dependent) | any GPU-executed model forward/training step | bit-exact reproducibility of a GPU model run is NOT assumed — hence no ML module carries the `exact` class (§3.1) |
| N-2 | Atomic-operation accumulation order | training-step gradients/accumulations on GPU | run-to-run divergence of TRAINING outcomes is expected — hence the `distribution` class for training reruns |
| N-3 | Nondeterministic library algorithm selection (autotuner/benchmark-selected kernels, e.g. the cuDNN-family algorithm search) | conv/rnn-family forward and backward passes | a library may PICK different algorithms between runs unless its deterministic mode is enabled — a mitigation (§5, M-2), not an assumption |
| N-4 | Parallel data-loading order (multi-worker shuffling/prefetch) | training batches; any worker that parallelizes input | the data ORDER is part of the seed discipline — the loader's worker seeding/rng handling is verified at pin time and recorded per run |
| N-5 | Framework multi-threaded CPU reduction order | CPU execution paths of the same frameworks | nondeterminism is not GPU-exclusive; a CPU fallback (§5, M-3) is a mitigation, not a proof of exactness |
| N-6 | Framework-version sensitivity (numerics change between releases) | every ML record | the rerun's dependency versions must EQUAL the record's (`ENVIRONMENT_VERSION_DIFFERS` is a named identity finding, P8.20 §5 item 2); a version difference can never hide inside a "reproduction" |
| N-7 | Solver-internal threading (HiGHS/Clarabel multi-thread reductions) | the `exact`-class solver modules | already mitigated by the recorded pins: HiGHS at `thread_count` 1 (P8.06); Clarabel's deterministic algorithm; the byte-equality discipline (P8.08) — the `exact` classes of §3.1 stand only WITH these pins recorded on the run |

### 4.1 DECISION-POINT carried (not closed)

The torch/PyG pair (P8.10 §6; P8.03 §4) and the RL framework (P8.13 §5;
Ray/RLlib UNPINNED-REVERIFY) remain OPEN; D4 remains OPEN
(`.agent/DECISIONS.md`). Which of N-1…N-6 actually apply, and which
framework flags exist to mitigate them, are verified ONLY by install at
the human pin — this register is the checklist that verification fills,
never a substitute for it. No framework behaviour is claimed from this
record.

## 5. The mitigation procedure (recorded; applied at the human pin/run gates)

| # | Mitigation | Applies to | Status |
|---|---|---|---|
| M-1 | **Seed pinning**: every stochastic surface carries a seed derived by the recorded derivation — `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))` — with the master seed recorded per dataset (P8.35 §2) and per run (the P8.20 record's `determinism.seeds` block); the derivation string is verbatim-pinned (`SEED_DERIVATION_MISMATCH` otherwise) | training runs; seeded inference; data loaders | PROCEDURE RECORDED; no seed exists yet |
| M-2 | **Deterministic-mode flags**: where the pinned framework offers a deterministic-algorithm mode or an exclusion of nondeterministic kernels, it is ENABLED for any run whose module could otherwise claim tighter reproducibility, and the flag state is recorded per run (a flag that does not exist in the pinned version is recorded as unavailable — never assumed) | N-3, N-1 mitigation | PROCEDURE RECORDED; the flag surface is a pin-time verification item (nothing installed) |
| M-3 | **CPU fallback**: where a result must be re-proven tightly (an audit rerun, a disputed figure) and the GPU path cannot assure it, the rerun executes on CPU with single-threaded reductions where the framework allows, and the hardware delta is recorded in the rerun record's environment block — the class does not change (still `within_tolerance`), the disclosure gets stronger | N-1/N-5 audit path | PROCEDURE RECORDED; never executed |
| M-4 | **Thread pinning**: solver modules record `thread_count = 1` (HiGHS, P8.06) and any framework thread-count that affects reduction order; a deviation is an identity finding | N-7; the `exact` classes | PIN RECORDED at P8.06; carried |
| M-5 | **Version pinning**: the P8.20 record's `component_versions` must equal at rerun (`ENVIRONMENT_VERSION_DIFFERS` otherwise) — the framework-version sensitivity (N-6) is disclosed so it cannot masquerade as reproduction | N-6; every rerun | ENFORCED IN `rerun_check.identity_findings` (P8.20) |
| M-6 | **Parallel-loader ordering**: where data loading parallelizes, the loader seeding is pinned and recorded with the run's seeds (N-4); an unseeded loader is a finding against the run's record, never a silent acceptance | N-4 | PROCEDURE RECORDED; no loader exists |

## 6. The disclosure that travels on every training result (ledger step 3)

Every reported training or ML-inference result carries, on its P8.20
reproducibility record, a NON-EMPTY disclosure block — structurally
enforced, not prose:

| Field | Content | Enforcement |
|---|---|---|
| `tolerance_class` | `exact` / `within_tolerance` / `distribution` (§3.1) | `record_schema.TOLERANCE_CLASS_INVALID` otherwise |
| `tolerance_ref` | the pre-registration reference (§3.2) for every non-exact record | `TOLERANCE_REF_REQUIRED` otherwise |
| `determinism.nondeterminism_sources` | the applicable sources from §4, named per run (e.g. `gpu_kernel_reduction_order`, `parallel_data_loading_order`, `framework_version_sensitivity`) | `NONDETERMINISM_SOURCES_REQUIRED` for every `within_tolerance`/`distribution` record — the empty disclosure is unenterable |
| `determinism.seed_supported` + `seeds` + derivation | the seed honesty block (M-1) | `SEED_POLICY_CONFLICT` / `SEED_DERIVATION_MISMATCH` otherwise |
| `component_versions` | the pinned framework versions the result's class depends on (the DECISION-POINT keys carry `UNPINNED-REVERIFY` honestly until resolved) | per-module required keys (`COMPONENT_VERSION_MISSING` otherwise) |
| the claim-class statement | the record states WHICH claim it supports: an `exact` record claims byte-level reproduction; a `within_tolerance` record claims reproduction within the pre-registered value; a `distribution` record claims only distributional agreement under the §7.2 procedure — never more | the comparator's outcome vocabulary (§8) |

**Every measured cell of this disclosure is EMPTY today**: no run
exists, so no disclosure instance exists — the schema and the refusal
names above are the enforced shape the first run fills.

## 7. The rerun set (ledger step 2) — defined, UNRUN

### 7.1 The required minimum (each row EMPTY until executed)

| Rerun | Requirement | Class | Status |
|---|---|---|---|
| One INFERENCE rerun per module | `baseline/highs-tier1-lp`, `baseline/clarabel-tier2-conic`, `baseline/rule-based-priority-list`, `admm-dmpc`, `gnn-warm-start`, `gnn-screening`, `marl-policy`, `dynamic-controller` | per §3.1 | **UNRUN — no rerun exists** |
| One TRAINING rerun per module where training is claimed reproducible | the ML modules only, and ONLY IF training reproducibility is claimed for them; a module that does not claim it records that honestly instead of running a vacuous rerun | `distribution` | **UNRUN; no training-reproducibility claim exists yet** |
| The GNN held-out-topology inference rerun from artifacts | re-proves the P8.12 headline metric specifically (P8.20 §5 item 7): rerun from the artifact-store references alone, on a held-out topology per the P8.35 manifest | `within_tolerance` | **UNRUN — blocked on the framework pin, the trained model, and the P8.35 instance generation** |
| The second-engineer rule | every rerun is executed by an engineer who is NOT the result's author, from the record ALONE, on a clean environment (P8.20 §5) | — | procedure recorded |

### 7.2 The distribution-comparison procedure (owned HERE, defined now, executed never-yet)

A `distribution`-class rerun is judged by a procedure registered BEFORE
the rerun (§3.2), which must state: the statistic(s) compared (e.g.
final-metric distribution over N seeds), the reference distribution's
construction (the primary run's per-seed outcomes), the agreement
criterion, and the N. The local comparator refuses to guess one
(`DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38`) — that refusal is the
seam this record now owns. No procedure instance exists (no training
rerun); the FIRST one is registered at the first training rerun's
pre-registration, and its N is a recorded decision, never a default.

## 8. Outcome handling (ledger step 4) — the marking with teeth

- The comparator's outcome vocabulary is exactly `REPRODUCED_EXACT`,
  `REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE`,
  `NOT_REPRODUCED_MARKED_FOR_EXCLUSION`, `INVALID_RERUN_IDENTITY`,
  `BLOCKED_*` (P8.20 §7.2).
- A result that does not reproduce within its pre-registered tolerance
  (or distribution procedure) carries
  `NOT_REPRODUCED_MARKED_FOR_EXCLUSION` with the standing statement:
  **EXCLUDED from the benchmark report's claims — P8.26 must not cite
  this result** (P8.20 §5 item 6). The marking is standing, not a
  footnote; the P8.23 comparison and the P8.26 report consume it.
- A FAILED non-reproduction is recorded AS THE OUTCOME — never as a
  blank, never silently retried into significance.
- **No outcome exists**: every row of the P8.20 §7.1/§7.2 tables is
  EMPTY, and this record adds no row.

## 9. Reviewer-verification procedure (the human gate)

```sh
# (a) the structural enforcement is already testable without any run:
#     the P8.20 tooling's self-tests + the P8.37 property suite pin
#     NONDETERMINISM_SOURCES_REQUIRED, TOLERANCE_REF_REQUIRED, the
#     inclusive boundary, and the DISTRIBUTION refusal:
python apps/worker/python/reproducibility/seed_derivation.py
python apps/worker/python/reproducibility/record_schema.py
python apps/worker/python/reproducibility/rerun_check.py
PYTHONPATH=apps/worker/python/reproducibility python -m pytest apps/worker/python/reproducibility/tests -v

# (b) FIRST PRE-REGISTRATIONS (before any rerun, X-8): create the
#     per-module pre-registration entries of §3.2 — class from §3.1,
#     VALUE + basis from the run's own risk definition; a distribution
#     procedure per §7.2 where the class is `distribution`.

# (c) FIRST RERUNS (the ledger step 2 minimum, §7.1): from the records
#     alone, second engineer, clean environment; outcomes per §8; fill
#     the P8.20 §7 tables from the outputs only.

# (d) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green (a) + (d) earns a one-line human-gate note; failures are
recorded verbatim and the ledger tick reverts to `- [ ]`.

## 10. Contract gaps → follow-ups (recorded, not invented)

1. **Phase 7 follow-up (the worker-template seam):** the Phase 7
   worker-template result envelope (`python/workers/worker_template/`)
   carries the §9.2 provenance base and `DeterminismMetadata`, but has
   NO slot for the run-level tolerance class / nondeterminism
   disclosure of §6 — that disclosure lives on the P8.20 RUN record,
   not on the worker result. Extending the template's result contract
   to carry a disclosure block (or a reference to the run record) is a
   **Phase 7 contract follow-up for the template owners** — never fixed
   by inventing a parallel contract here (X-10 posture).
2. **No object storage exists** (P8.20 §10.1): the artifact store
   behind the `flexcore-artifact://` references is the §6.5 owners'
   task; the rerun set (§7.1) is structurally blocked on it for the
   "from artifacts" reruns. Carried, not re-owned.
3. **The framework DECISION-POINTs are OPEN** (P8.10 §6; P8.13 §5; D4):
   no N-register row is verified, no M-2 flag surface is known. The
   first pin updates the register with verified facts — a recorded
   decision, not a silent edit.
4. **The dynamic-controller class is a RECOMMENDATION** (§3.1): ANDES
   is pinned-but-never-installed; the class is confirmed at the dynamic
   track's first pre-registration. Recorded, not decided.
5. **The pre-registration table does not exist yet** (§3.2 rule 4): it
   is created at the first pre-registration with VALUES, and this
   record deliberately contains none — the empty state is the honest
   one (X-8).

## 11. Non-claims (restated)

No training run, inference rerun, or rerun outcome exists; no tolerance
value is pre-registered anywhere; no framework behaviour in §4 is
verified (nothing installed); no result is claimed reproducible —
reproduction is proven only by recorded rerun outcomes, of which there
are none. The `exact` classes of §3.1 are declarations about recorded
module nature under recorded pins (M-4/M-5), not evidence. Everything
here governs simulation-only research artifacts (AGENTS.md rule 5);
nothing grants field authority, and no disclosure block is a safety or
certification claim (ADR-010).
