# P8.11 — GNN warm-start/screening implementation (isolated inference worker)

Date: 2026-11-13. Subtask: P8.11 (`ledger_phase_8.md` §P8.11).
Owner role: Research engineer (ML). Report trace: §13.1; §13.4; §5.4
Class C; §7.4. Inputs: P8.10 design (`10_gnn_warmstart.md`); P8.04
contracts (`04_worker_contracts.md`;
`python/workers/worker_template/research.py`); P8.03 license audit
(§4 DECISION-POINT); the andes_worker pattern
(`apps/worker/python/andes_worker/`).

## 1. Status of this record — code WRITTEN, NEVER RUN; no framework exists

The torch/PyG framework pair is a recorded **DECISION-POINT**
(`03_research_license_audit.md` §4; D4 OPEN; R8-C3 version skew): **no
framework is selected, pinned, installed, or verified in this
repository's history**. The worker
(`apps/worker/python/gnn_worker/gnn_inference_worker.py`) was **NEVER
executed** — not against a real framework (there is none to run it
against), not against a stub. Every torch/PyG call follows the
documented PyG message-passing API shape as of the 2026-11-07 research
cut-off and is **UNVERIFIED** until the human build/test pass pins a
verified-compatible pair (P8.10 §6 RECOMMENDATION) and executes it.
**Every inference result in this repository is an EMPTY cell**; no
accuracy, latency, speedup, or ranking-quality figure exists anywhere.
The tick on P8.11 means *implemented, awaiting human build/test* —
nothing more.

## 2. What was implemented (ledger steps 1–4 → code)

Training and inference are implemented as SEPARATE steps with separate
version pins (ledger step 1): **training is an OFFLINE human/lab gate**
(versioned artifacts in object storage per §6.5, the P8.20 store /
P8.32 signing seam; training-data provenance per P8.35) and is NOT
implemented as code in this loop — no model was trained, and no
training script is claimed. What IS implemented is the **isolated
inference worker** with its pure-logic core and the gated inference
path:

| Ledger step | Where it lives | Status |
|---|---|---|
| Step 1 (training offline, versioned artifacts; CPU/GPU limits recorded) | The artifact/ref discipline is in the request contract (`model_manifest.model_artifact_ref`, `model_version`); the CPU-only default and the recorded timeout maximum are constants (`MAX_INFERENCE_TIMEOUT_S`; GPU use is a recorded deployment setting, never a worker choice); the training step itself is the human/lab gate | training UNRUN (no model exists); inference-worker code WRITTEN-UNRUN |
| Step 2 (warm-start candidate with the solver still solving / screening ranking with scores + uncertainty) | `run_inference` role branches: `warm_start` → `candidate_setpoint_vector` with the "solver still solves" statement attached to every output; `screening` → deterministic `rank_candidates` with per-entry score AND uncertainty (uncertainty >= 0 enforced; a ranking without uncertainty is refused) | pure logic WRITTEN + hand-tested; the forward pass itself UNVERIFIED/UNREACHABLE |
| Step 3 (§9.2 + P8.04 provenance on every output) | Every envelope carries model version, `training_data_ref`, `input_snapshot_id`, `topology_scope`, artifact ref, termination status; the result payload is checked against the CLOSED P8.04 key set (`enforce_gnn_result_schema`; `training_data_ref` and `topology_scope` mandatory and non-empty — stricter than the P8.04 base because the ledger makes them mandatory here) | DONE (pure logic; hand-tested) |
| Step 4 (failure modes: model file missing / out-of-scope topology flagged / inference timeout) | missing model → NAMED `ModelArtifactError`, exit 2, nothing on stdout; out-of-scope → in-band FLAG (`topology_scope_check.in_scope: false`, outputs NULL, termination `failed`, exit 0) computed BEFORE any framework import; timeout → declaration validated against `MAX_INFERENCE_TIMEOUT_S`, runtime enforcement a VERIFY-BUILD item | pure parts WRITTEN + hand-tested; runtime enforcement UNVERIFIED |

## 3. The framework gate and the honesty gate (P8.03 §4 carried)

The guarded imports are the module's honesty gate, mirroring the
andes_worker pattern with the DECISION-POINT status in the named
requirements:

| # | Rule | Where implemented |
|---|---|---|
| 1 | torch/PyG live ONLY inside this isolated Class C worker process | the ONLY `import torch` / `import torch_geometric` in the repository are deferred INSIDE `import_torch()` / `import_torch_geometric()`; a missing framework raises the NAMED `TORCH_DEPLOYMENT_REQUIREMENT` / `PYG_DEPLOYMENT_REQUIREMENT`, which state the DECISION-POINT status verbatim |
| 2 | No core crate, SDK crate, or app depends on torch | no Rust change was made in this step; `requirements-gnn.txt` records that torch/PyG must never enter the cargo tree |
| 3 | FlexCore consumes GNN outputs AS DATA over the JSON boundary | the envelope carries vectors/rankings as JSON; no torch type crosses the boundary; the pure core sees only validated Python structures |
| 4 | Any framework selection goes through the recorded decision path | P8.10 §6: RECOMMENDATION only; the human pass verifies by install, confirms licenses, runs the P8.36 advisory check, and records the decision — agents never close it |
| 5 | The gate order makes the DECISION-POINT the FIRST honest refusal | `run_inference` gate order: pure validation → out-of-scope FLAG (no framework needed) → guarded torch import → guarded PyG import → artifact check → UNVERIFIED inference steps. With nothing installed, an in-scope request raises the named torch requirement; an out-of-scope request is flagged without needing the model at all |
| 6 | Proposal-only (ADR-010) enforced in code, not prose | `assert_proposal_only()` scans the whole result tree for authorization/command-shaped keys and refuses emission; `authority: "none"`; mode-locked to simulation/shadow by a constant; no field-write path — stdout only |

Because no inference output can exist without the framework, the loop's
honesty holds structurally: **without torch, no inference result; and
an out-of-scope input is refused even if a model existed** (the scope
gate precedes the model).

## 4. The two roles in code (never conflated)

| | `warm_start` | `screening` |
|---|---|---|
| `mode` value | `warm_start` | `screening` |
| `result_kind` | `candidate_setpoint_vector` | `ranking` |
| Output | warm-start candidate values for the declared exact solver (`target_solver` closed to `highs_lp_qp_mip`), carrying the statement: the solver still solves; a warm start affects SPEED of an exact solve, never its optimality | deterministic descending ranking with per-candidate score and uncertainty; tie-break by candidate_id ascending (replayable, P7.42 determinism posture) |
| Never | a schedule, a device command, a replacement for the solver, an optimality claim | a decision; a constraint removal; a silently screened-out active constraint (miss cost is P8.12's metric) |

The role/kind mapping is pinned by tests
(`tests/test_gnn_inference_worker.py::TestClosedResultSchema::test_result_kind_matches_mode_map`).

## 5. Failure modes (ledger step 4) — honest status

| Failure mode | Behaviour | Status |
|---|---|---|
| Model file missing | NAMED `ModelArtifactError` ("a missing model file fails loudly ... no default model and no degraded-mode inference"), exit 2, NOTHING on stdout | pure check WRITTEN + hand-tested; the with-torch end-to-end path is the human gate's test |
| Out-of-scope topology | FLAGGED in-band: `topology_scope_check.in_scope: false` with a reason, outputs NULL, termination `failed`, exit 0 — flagged, not silently answered, and computed BEFORE any framework import (testable today, with nothing installed) | WRITTEN + hand-tested (self-test 18 and the pytest mirror run the actual path on this machine) |
| Inference timeout | Request declarations validated against the recorded `MAX_INFERENCE_TIMEOUT_S` (over-max and non-positive refused); the runtime enforcement mechanism is a VERIFY-BUILD item (UNVERIFIED) — recorded honestly, not pretended | declaration validation WRITTEN + hand-tested; enforcement UNVERIFIED |
| Malformed request | fail-closed: nothing on stdout, reason on stderr, exit 2 | WRITTEN + hand-tested (pytest fail-closed-main class) |

## 6. The named deployment requirements (verbatim, for the reviewer)

```
TORCH_DEPLOYMENT_REQUIREMENT:
  flexcore GNN inference worker requires the research ML framework:
  torch is NOT INSTALLED and the torch/PyG pair is a recorded
  DECISION-POINT (P8.03 section 4, D4 OPEN) — no version is selected,
  pinned, or verified in this repository. At the human build/test pass,
  pin the verified-compatible pair recorded in
  ledger/phase-08-advanced-coordination/10_gnn_warmstart.md section 6
  into an ISOLATED research environment (never a system environment,
  never the platform runtime) after the P8.36 at-pin-time advisory
  check, and update the pin status

PYG_DEPLOYMENT_REQUIREMENT:
  flexcore GNN inference worker requires torch_geometric (PyG): NOT
  INSTALLED and the torch/PyG pair is a recorded DECISION-POINT (P8.03
  section 4, D4 OPEN; R8-C3 version skew — a PyG release supporting the
  selected torch version must be re-checked at pin time per RESEARCH.md
  section 7 task 2). Pin only a verified-compatible pair into the
  isolated research environment; never install into the platform runtime
```

## 7. Isolation/contract verification procedure (for the reviewer)

```sh
# (a) the ONLY torch/PyG imports in the repository, deferred+guarded:
grep -rn "import torch" apps python crates --include=*.py --include=*.rs
#     expected: hits ONLY inside apps/worker/python/gnn_worker/
#     gnn_inference_worker.py function bodies (no module-level import,
#     no torch anywhere in crates/ or python/workers/)

# (b) no framework equivalent in the Rust tree:
grep -rni "torch" apps --include=Cargo.toml crates --include=Cargo.toml
#     expected: no hits

# (c) the worker's pure core imports with NOTHING installed:
PYTHONPATH=apps/worker/python/gnn_worker python -c \
  "import gnn_inference_worker; print('pure core OK')"

# (d) without torch, no inference output can exist (the named
#     DECISION-POINT requirement):
PYTHONPATH=apps/worker/python/gnn_worker python -c \
  "import gnn_inference_worker as w; w.run_inference(w._valid_request())"
#     expected: ImportError naming TORCH_DEPLOYMENT_REQUIREMENT and the
#     DECISION-POINT

# (e) the hand-computed pure self-tests:
python apps/worker/python/gnn_worker/gnn_inference_worker.py

# (f) the pytest suite:
PYTHONPATH=apps/worker/python/gnn_worker python -m pytest \
  apps/worker/python/gnn_worker/tests -v
```

Outputs of (a)–(f) are quoted into the human build/test pass record.
The ledger's verification ("run inference on an out-of-scope topology
and confirm the flag; kill the worker and confirm the control plane
survives") is **UNRUN** — the flag part is exercisable today per (e)/(f)
(self-test 18), the worker-kill part belongs to the human pass with the
composed control plane.

## 8. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **gRPC binding seam (P8.04/P7.17):** the worker reuses the template's
   §9.2 provenance field names verbatim and the P8.04 closed
   vocabularies, but binds no generated `flexcore.worker.v1` servicer
   (buf generation remains the activation step; the transport is the
   JSON-document discipline, mirroring the andes_worker's recorded
   seam). Binding the servicer into a Class C research worker is the
   recorded activation task — a Phase 7/P8.04 seam, NOT a parallel
   contract.
2. **Closed result-key set too small for the role payloads (P8.04):**
   `MODULE_RESULT_FIELDS["flexcore.research.gnn_warm_start.v1"]` = {
   `result_kind`, `training_data_ref`, `topology_scope`, `uncertainty` }
   has NO field to carry the warm-start candidate vector or the
   screening ranking entries themselves. The worker's JSON surface
   documents the needed shapes (`outputs.warm_start_vector`,
   `outputs.ranking`) as an additive extension PROPOSAL for the P8.04
   owner; until that extension exists, the P8.04-schema check applies to
   the `result_payload` sub-object and the envelope carries the values
   alongside it. Recorded here as a Phase 7 follow-up — never fixed by
   inventing a parallel contract.
3. **No screening payload type (P8.04):** P8.04 defines exactly one GNN
   payload type; the screening role rides
   `flexcore.research.gnn_warm_start.v1` with
   `result_kind: "ranking"`. If the P8.04 owner prefers a distinct
   `flexcore.research.gnn_screening.v1` type, that is an ADDITIVE
   contract change — the follow-up, not a fork made here.
4. **Model manifest in-request (P8.20/P8.32 seam):** the trained
   topology list, training-data ref, and artifact ref are declared in
   the REQUEST (validated pure-logic side) because the P8.20 artifact
   store and P8.32 signing do not exist yet; an artifact-backed, signed
   manifest read by the worker itself is the activation step. The
   manifest-in-request is a documented stopgap, not a trust mechanism —
   a caller could declare a false scope; the mitigation is that the
   scope check still forces the FLAG on anything not in the declared
   list, and the P8.12 evaluation only counts runs whose manifest
   matches the P8.35 training-data record.
5. **Runtime timeout enforcement:** the recorded maximum exists and the
   declaration is validated; the mechanism that actually enforces the
   timeout during a forward pass is a VERIFY-BUILD item against the
   pinned framework (UNVERIFIED), recorded as part of the human
   build/test pass rather than pretended here.

## 9. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Worker runs isolated | WRITTEN as an isolated Class C worker (one JSON doc in/out, no shared state, guarded imports); execution UNRUN (no framework exists to run) |
| Every output carries provenance and uncertainty where applicable | DONE in code and tests (§9.2 names + P8.04 keys mandatory; uncertainty required per ranking entry) — the outputs themselves are EMPTY cells (no inference ever ran) |
| Out-of-scope topology is flagged | DONE in code and hand-tested WITHOUT any framework (the scope gate precedes the model gate) |
| A missing model file fails loudly | DONE in code and hand-tested (named `ModelArtifactError`, exit 2, nothing on stdout); the with-torch end-to-end path is the human gate's test |
| Training implemented as a separate step with separate version pins | Training is the OFFLINE human/lab gate (no training code or run claimed in this loop); the artifact/ref version discipline is in the request contract; pins recorded per P8.10 §6 (DECISION-POINT OPEN) |

## 10. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, or scan was
performed for this implementation.
