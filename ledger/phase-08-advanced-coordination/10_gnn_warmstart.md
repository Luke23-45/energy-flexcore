# P8.10 — GNN experiment design: warm-start and screening, with topology scope stated

Date: 2026-11-13. Subtask: P8.10 (`ledger_phase_8.md` §P8.10).
Owner role: Research engineer (ML). Report trace: §13.1; §19.3–§19.4;
§7.4 (PyTorch + PyG). Inputs: P8.02 plan (`02_research_scope_freeze.md`
— the frozen M2 question list); P8.03 license audit
(`03_research_license_audit.md` §4 — the torch/PyG DECISION-POINT);
P8.06 baselines (`05_06_harness_baselines.md` — per-module mapping);
Phase 8 `RESEARCH.md` §2 (PyTorch 2.15 / PyG 2.8.0 skew, R8-C3);
`python/workers/worker_template/research.py` (the P8.04 result-key
vocabulary: `training_data_ref` + `topology_scope`).

## 1. Status of this record — a DESIGN, nothing installed, nothing run

This is the P8.10 experiment DESIGN record. It defines the two GNN
roles, the topology scope semantics, the training-data and leakage
rules, the metrics, and the baselines — **before any model is built or
any training run is planned** (the P8.02 freeze rule). It records no
result and closes no decision: the torch/PyG framework pair remains a
recorded **DECISION-POINT** (`03_research_license_audit.md` §4; D4
OPEN) — **no framework is selected, pinned, installed, or verified in
this repository**, and no D-item or DECISION-POINT is closed here. The
tick on P8.10 means *implemented, awaiting human build/test* — the
human gates are the framework pin, the training runs, and every
measured cell (P8.12), all EMPTY in this loop (ENTRY_NOTE boundary 4).

Solver-class discipline (binding on everything below): **HiGHS is
LP/QP/MIP, never SOCP; Clarabel is convex conic, never MISOCP; AC power
flow is not AC-OPF** (AGENTS.md rule 4). A warm start affects the SPEED
of an exact solve, never its optimality — stated per role in §2 and
carried verbatim in the worker's boundary statements.

## 2. The two roles, separately (ledger step 1)

A warm-start win is NOT a screening win; the roles are never conflated
into one headline number (P8.02 M2; the ledger's named risk). Each role
has its own question, metric, and baseline — all frozen in P8.02 §4
(M2) and restated here with their design consequences.

| Item | Role (a) — candidate AC-OPF / scheduling warm-start generation | Role (b) — active-constraint prediction / contingency screening |
|---|---|---|
| What the GNN outputs | A warm-start candidate: starting VALUES for the exact solver's decision variables | A ranking of constraint/contingency candidates: a score AND an uncertainty per candidate (§13.1: surrogate estimates with uncertainty) |
| What it is FOR | Speeding up the EXACT solve (HiGHS LP/QP/MIP for the declared class); the solver still solves to its own termination criteria — the GNN does NOT replace the solver, and a warm start affects SPEED, never optimality | Focusing attention and pre-registering which constraints/contingencies matter; a screened-out candidate may still be active (the miss cost is a metric), and NO screening output removes a constraint from any solve |
| Frozen question (P8.02 M2a) | On topologies held out of training, does a GNN warm start reduce **end-to-end feasible solve time** (solver + validation, not inference latency) vs the cold-start solver and a non-GNN heuristic warm start, without reducing feasible-solver success rate? | On held-out topologies, does GNN screening rank constraints/contingencies that are actually active above inactive ones at a pre-registered miss rate and miss cost? |
| Metric (§19.4 GNN row) | feasible solver success rate; end-to-end feasible solve time; speedup vs baselines; generalization to unseen topologies — measured by P8.12 | ranking quality vs the TRUE active sets; cost of a missed constraint (a screened-out constraint that was actually active) — measured by P8.12 |
| Baseline (P8.06, named) | cold-start Tier 1 HiGHS solver + the rule-based record's warm-start heuristic (the non-GNN baseline) | the rule-based record's heuristic ranking (the non-GNN baseline) |
| Non-claims that apply | X-3 (no optimality claim — speed only); X-1 (the warm-started solve passes the same independent validation as any other solve); X-7 (generalization claimed only over the tested range) | X-1 (a screening output is a proposal, never a decision); X-8 (miss-rate/cost thresholds are pre-registered at P8.06 freeze time, never as universal targets) |
| Headline rule | Never merged with (b)'s numbers; end-to-end feasible solve time is the headline, never inference latency (P8.12) | Never merged with (a)'s numbers; misses and miss cost reported alongside wins |

**Warm-start honesty statement (carried verbatim in the worker,
`gnn_inference_worker.py` `BOUNDARY_STATEMENTS`):** a warm start changes
only the exact solver's starting point: the solver still solves the
problem to its own termination criteria — the GNN does not replace the
solver, and a warm start affects SPEED of an exact solve, never its
optimality.

## 3. Roles → the P8.11 worker contract (report §13.1/§13.4; P8.04)

The design's role separation is enforced structurally in the isolated
inference worker (`apps/worker/python/gnn_worker/gnn_inference_worker.py`,
the P8.11 deliverable):

- `mode` ∈ {`warm_start`, `screening`} — closed, per-request, and the
  two roles produce DIFFERENT result kinds (`candidate_setpoint_vector`
  vs `ranking`, both inside the closed §13.4 vocabulary); a request
  never mixes them.
- Every output carries the P8.04 provenance keys `training_data_ref`
  and `topology_scope` (closed `flexcore.research.gnn_warm_start.v1`
  result-key set), plus the §9.2 base (model version, input snapshot
  ID, artifact ref, termination status).
- Proposal-only in code: the authorization/command-shaped key scan
  refuses emission at any depth; the worker is mode-locked to
  simulation/shadow and has no field-write path (ADR-010).
- The inference path is UNREACHABLE in this repository (framework
  DECISION-POINT; named ImportErrors) — no inference output exists.

## 4. Topology scope semantics (ledger step 2; the D3 obligation)

The P8.04 result key `topology_scope` is a DECLARED LABEL naming the
topology class the model was trained on; the model manifest additionally
enumerates `trained_topology_ids`, the exact topology instances in
training. Semantics, binding on the worker and every evaluation:

| Term | Definition | Enforcement point |
|---|---|---|
| `topology_scope` | A non-empty scope label (e.g. a balanced-radial feeder class), declared in the model manifest and echoed on every output — never assumed | worker request validation (non-empty string, mandatory) |
| `trained_topology_ids` | The exact list of topology instance IDs the training data covers (from the P8.35 manifest); membership is EXACT string equality | the worker's scope check `evaluate_topology_scope` |
| In-scope | `request.topology.topology_id` ∈ `trained_topology_ids` (exact match; no fuzzy/prefix matching — a near-miss must flag, not pass) | worker returns the inference outputs; the P8.12 evaluation counts the run |
| Out-of-scope | `topology_id` NOT in the trained list | worker FLAGS in-band (`topology_scope_check.in_scope: false`, outputs NULL, termination `failed`) — flagged, never silently answered, never answered confidently |
| Transfer measurement | P8.12 evaluates on held-out topologies ONLY for any generalization statement (X-7), reporting where the GNN does not generalize and what would be needed | P8.12 template (EMPTY cells) |

**D3 obligation (network class; P8.02 §4):** the topology classes
covered are stated here — the 0.x default RECOMMENDATION is **balanced
radial distribution feeders** (matching the §19.3 scenario library's
distribution scope); unbalanced or meshed topologies are OUT of the
declared scope unless a later, explicitly scoped design decision adds
them (never assumed, never slipped in). This is a RECOMMENDATION label
on the default, not a D3 closure — D3 remains OPEN
(`.agent/DECISIONS.md`).

**Held-out sets (ledger step 2: "held-out topologies named").** The
concrete training instance list is the P8.35 training-data manifest's to
record (P8.35 owns the full provenance record — this design references
it and fixes only the RULE). The named hold-out assignment below is a
RECORDED RECOMMENDATION over the §19.3 base set (`scenarios/library/`,
lib-01…lib-08, via `apps/sim-runner`), to be confirmed against the P8.35
manifest BEFORE any training run; it is not a verified fact and not a
closed decision:

| Set | RECOMMENDED assignment (confirm at P8.35 manifest time) | Status |
|---|---|---|
| Training topologies | simulator-generated instances derived from the topology families of **lib-01…lib-05** (generator version + seeds recorded per instance; P8.35) | RECOMMENDATION — UNCONFIRMED until the manifest exists |
| Held-out topologies (unseen topologies, §13.1) | instances of the topology families of **lib-06, lib-07, lib-08**, PLUS at least one topology family absent from the library entirely (a structurally unseen feeder shape) | RECOMMENDATION — UNCONFIRMED; must be named in the P8.35 manifest and re-checked for overlap by the reviewer (ledger verification method) |
| Held-out operating regimes (§13.1) | the P8.19 research-case extensions (stressed scenarios, asset dropout, reverse power flow) that hold topology fixed while shifting regime | RECOMMENDATION — UNCONFIRMED until P8.19's artifact exists |

## 5. Training data and leakage controls (ledger step 3)

| Item | Design |
|---|---|
| Source | Simulator-generated cases only: `apps/sim-runner` over the §19.3 scenario library, with the generator version and seeds recorded PER INSTANCE (the P8.35 provenance record owns the inventory; this design references it, never duplicates it) |
| Size | Stated in the P8.35 manifest per dataset (never asserted here — no dataset exists in this loop) |
| Leakage control 1 — topology | NO held-out topology family or instance appears in training (the §4 assignment is checked by the reviewer against the manifest: "the held-out list against the training-data manifest for overlap" — the ledger's verification method) |
| Leakage control 2 — normalization | Normalization statistics (feature means/scales) are computed on the TRAINING split ONLY and frozen with the model artifact; test/held-out cases are normalized with the frozen training statistics — a test case never contributes to a statistic (no leakage through normalization) |
| Leakage control 3 — regime | Held-out operating regimes are not in the training scenarios; where a regime is defined by a seed, the seeds are disjoint and recorded |
| Ingestion validation | P8.35 step 3: schema, range, and duplication checks at ingestion; poisoned/malformed records quarantined WITH a record, never silently dropped or trained on |
| Versioning | The dataset version is linked to the model artifact (P8.35 step 4; P8.20 artifact store; P8.32 signing) so a rerun (P8.20/P8.38) trains on the same data |
| License | Every dataset's license consequence is recorded per P8.35 step 2 alongside the P8.03 code-dependency audit |

## 6. Stack pin (ledger step 4) — DECISION-POINT carried, RECOMMENDATION only

The ledger's step 4 ("pin the stack, verified PyTorch/PyG pair per
P8.03") is recorded here **as the DECISION-POINT it actually is**:
nothing is selected, installed, or verified in this loop, and the
verification the ledger asks for ("stack pins verified by install") is
by construction a HUMAN gate. What this design does is fix the
resolution rule and the RECOMMENDATION, with labels:

| Item | Recorded content | Label |
|---|---|---|
| Report default | PyTorch + PyG for GNN work (§7.4) | DEFAULT (report) — not a decision |
| Current blocker | R8-C3 skew: PyTorch 2.15 (28 Oct 2026) current; PyG 2.8.0 (5 Jun 2026) supports PyTorch 2.9–2.12 only; "latest + latest" on assumption is prohibited (a pair that installs but silently changes numerics is worse than one that fails loudly) | recorded finding (RESEARCH.md §2) |
| RECOMMENDED pin | **PyTorch 2.12 + PyG 2.8.0 IF that pair is verified at implementation time**; else re-check for a newer PyG release (RESEARCH.md §7 task 2) and record the verified result | RECOMMENDATION — UNVERIFIED, never installed; the human pass verifies by install and records the decision |
| License | PyTorch BSD-style family / PyG MIT family, both UNVERIFIED until confirmed from primary sources at pin time | UNVERIFIED |
| CVE | Whatever is pinned is checked AT PIN TIME by the P8.36 procedure (recorded per version with a date) | recorded procedure, not run |
| Where the pin lives | `apps/worker/python/gnn_worker/requirements-gnn.txt` (currently carries the procedure and NO pin line — an uncommented pin would be an unverified selection); `03_research_license_audit.md` §4 is updated at pin time | recorded |
| JAX alternative | If the design selects JAX instead, that selection is its own pinned, licensed, CVE-checked recorded decision (P8.03 §4) | open alternative — not selected |
| Status | **DECISION-POINT OPEN** — D-decisions are never closed by agents; the worker's `GNN_PIN_STATUS` constant carries this status verbatim | OPEN |

**Model architecture, hyperparameters, seeds (ledger step 4).** The
architecture family is a RECOMMENDATION (a message-passing GNN in the
PyG sense — node features per §7 below, edge structure per §4 — with a
per-role output head: value head for warm-start, score+uncertainty head
for screening); the exact architecture, hyperparameters, and seeds are
RECORDED IN THE TRAINING ARTIFACT BEFORE any tuning run, per the P8.02
§5 pre-registration rule (thresholds and hyperparameters are set after
model/service/benchmark/risk are defined, never as universal targets —
X-8). This loop records no hyperparameter number, because no model
exists; a number written here would be fabricated.

## 7. Data contracts (inputs/outputs as contracts — the P8.11 worker implements these)

**Request-side input contract** (enforced by the worker's pure-logic
core; all units §10.2 canonical):

| Field | Contract |
|---|---|
| `topology.node_count` / `edges` | positive integer count; edges as `[src, dst]` integer pairs in `[0, node_count)`; self-loops REFUSED; parallel edges allowed (real feeders) |
| `feature_declarations` | closed kinds {`power` (MW/MVAr/MVA/MWh), `soc` (fraction in [0,1], unit-free), `voltage` (pu, > 0), `status` (0/1)}; per-kind unit rules enforced |
| `features` | one row per node, one value per declaration, all finite; SOC ∈ [0,1] enforced at the API boundary; P > 0 = export at the point of connection (sign carried by the feature NAME, a caller-side declaration) |
| `model_manifest` | model_version, model_artifact_ref, training_data_ref, topology_scope, trained_topology_ids — all mandatory, all non-empty (the manifest-in-request is the recorded stopgap pending P8.20/P8.32) |
| `target_solver` | closed to `highs_lp_qp_mip` (HiGHS is LP/QP/MIP, never SOCP — AGENTS.md rule 4) |
| `provenance` | input_snapshot_id, request_id — mandatory (the snapshot-freshness discipline of the Phase 7 template) |

**Output-side contract** (the closed P8.04 `flexcore.research.gnn_warm_start.v1`
result-key set {`result_kind`, `training_data_ref`, `topology_scope`,
`uncertainty`} + the §9.2 provenance base; proposal-only, ADR-010):

| Role | Output |
|---|---|
| warm_start | `result_kind: candidate_setpoint_vector` + mandatory `training_data_ref` / `topology_scope` / `uncertainty`; the candidate values travel with the explicit statement that the solver still solves |
| screening | `result_kind: ranking` + the same mandatory keys; per-candidate score AND uncertainty; deterministic tie-break (candidate_id ascending) for replay (P7.42 determinism posture) |

The closed P8.04 key set carries no field for the candidate vector or
the ranking entries themselves — that gap is recorded as a Phase 7
follow-up (§8 below and `11_gnn_inference_worker.md` §8), never fixed by
inventing a parallel contract.

## 8. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Both roles have separate questions/metrics/baselines | DONE — §2 table, frozen in P8.02 M2; never merged into one headline |
| Held-out topologies named | NAMED AS RECOMMENDATIONS (§4) over the §19.3 library; UNCONFIRMED until the P8.35 manifest exists — the reviewer's overlap check is the evidence, not this record |
| Leakage controls written | DONE — §5 (topology, normalization, regime), enforced at P8.35 ingestion and the P8.12 evaluation |
| Stack pins are verified-compatible | **NOT VERIFIED — cannot be in this loop** (nothing installed; ledger verification is "stack pins verified by install", a human gate). DECISION-POINT OPEN with RECOMMENDATION recorded (§6) |

## 9. Verification method (for the human/reviewer)

1. Check the held-out list (§4) against the P8.35 training-data manifest
   for overlap — any overlap is a finding against the generalization
   claim (the ledger's verification method).
2. Verify the stack pins BY INSTALL in an isolated environment
   (`requirements-gnn.txt` gate order), then update the DECISION-POINT
   tables here and in `03_research_license_audit.md` §4 — a recorded
   decision, not a silent edit.
3. Run the P8.11 worker's pure self-tests (no framework needed):
   `python apps/worker/python/gnn_worker/gnn_inference_worker.py`, and
   the pytest suite:
   `PYTHONPATH=apps/worker/python/gnn_worker python -m pytest
   apps/worker/python/gnn_worker/tests -v`.
4. Confirm the out-of-scope flag and the missing-artifact loud failure
   with real requests (the ledger P8.11 verification method).
5. All P8.12 measured cells (end-to-end feasible solve time on held-out
   topologies, ranking quality, miss cost) stay EMPTY until the
   human/lab run fills them via the template's recorded commands.

## 10. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, or scan was
performed for this design.
