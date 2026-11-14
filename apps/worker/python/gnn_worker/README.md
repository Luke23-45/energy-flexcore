# P8.11 — The GNN warm-start/screening inference worker
#         (`apps/worker/python/gnn_worker/`)

Status: **WRITTEN, NEVER EXECUTED — UNVERIFIED-API, DECISION-POINT-GATED,
NO INFERENCE OUTPUT EXISTS.** The Phase 8 loop bans install/build/run:
the torch/PyG framework pair is a recorded **DECISION-POINT** (P8.03 §4;
D4 OPEN; R8-C3 version skew — PyG 2.8.0 supports PyTorch only up to
2.12 while PyTorch 2.15 is current), so **nothing is selected, pinned,
installed, or verified**; this worker was NEVER run, and **every
inference result in this repository is an EMPTY cell**. Every
torch/PyG call in `gnn_inference_worker.py` follows the documented PyG
message-passing API shape as of the 2026-11-07 research cut-off and is
**UNVERIFIED** until the human build/test pass pins a
verified-compatible pair and executes the worker. No accuracy, latency,
speedup, or ranking-quality figure exists anywhere in this directory.

## What this worker is — and what it is not

The worker implements the P8.10 design's two roles, never conflated
(§13.1; P8.02 M2: a warm-start win is NOT a screening win):

- **warm_start** (`result_kind: candidate_setpoint_vector`): returns a
  warm-start candidate as a STARTING POINT for the exact solver. The
  solver (HiGHS LP/QP/MIP for the declared class) still solves to its
  own termination criteria — **the GNN does not replace the solver, and
  a warm start affects SPEED of an exact solve, never its optimality.**
  HiGHS is LP/QP/MIP, never SOCP; solver classes are never conflated
  (AGENTS.md rule 4).
- **screening** (`result_kind: ranking`): returns a ranked list of
  constraint/contingency candidates with a score AND an uncertainty per
  entry (§13.1: surrogate estimates with uncertainty). A screening
  score is never a decision; a screened-out constraint may still be
  active, and no screening output removes a constraint from any solve.

It is:

- **DECISION-POINT-gated**: `import torch` / `import torch_geometric`
  are guarded INSIDE `import_torch()` / `import_torch_geometric()` with
  the NAMED ImportError constants (`TORCH_DEPLOYMENT_REQUIREMENT`,
  `PYG_DEPLOYMENT_REQUIREMENT`) stating the DECISION-POINT status. The
  pure-logic core is STDLIB-ONLY and imports/self-tests with NOTHING
  installed — which is also the honesty gate: with no framework, NO
  inference output can ever exist (an in-scope request raises the named
  requirement; an out-of-scope request is flagged in-band without
  needing the model at all).
- **Proposal-only** (ADR-010): the result type carries no authorization
  token and no device command; `assert_proposal_only()` scans every key
  of the result tree and refuses emission of any forbidden key. The
  worker has no field-write path — its only output channel is stdout.
- **Mode-locked to simulation/shadow** (§21.4): a constant, not a
  setting; no request field can change it.
- **Provenance-complete** (P8.11 step 3; §9.2 + P8.04): every output
  carries model version, `training_data_ref` (the P8.04 result key; the
  full training-data provenance record is **P8.35's** — this worker
  carries the reference, never the record), `input_snapshot_id`, and
  `topology_scope` (the P8.04 result key; semantics in the P8.10 design
  §4). The result payload is checked against the CLOSED P8.04 module
  key set for `flexcore.research.gnn_warm_start.v1`.
- **Scope-disciplined** (P8.11 step 4): a request whose `topology_id`
  is outside the model manifest's `trained_topology_ids` is FLAGGED
  in-band (`topology_scope_check.in_scope: false`) with NULL outputs —
  flagged, not silently answered, never answered confidently. A missing
  model artifact fails LOUDLY (named `ModelArtifactError`, exit 2,
  nothing on stdout). Timeout declarations are validated against the
  recorded maximum; runtime enforcement is a VERIFY-BUILD item.

## Files

| File | Content |
|---|---|
| `gnn_inference_worker.py` | The worker entrypoint: one `flexcore-gnn-worker/1` JSON request on stdin → one result envelope on stdout; guarded torch/PyG imports with the named DECISION-POINT requirements; pure-logic core (graph-shape checks, feature-shape/range checks, closed result schema, proposal-only scan, deterministic ranking); UNVERIFIED inference path; hand-computed in-module self-tests (`run_self_tests()`) |
| `requirements-gnn.txt` | The framework-pin procedure (DECISION-POINT status; NO pin line — an uncommented pin would be an unverified selection) |
| `tests/test_gnn_inference_worker.py` | The pytest suite mirroring the in-module self-tests plus request-validation, envelope-posture, and fail-closed-main tests (WRITTEN, NEVER RUN) |

## Transport contract

ONE JSON document on stdin (the `flexcore-gnn-worker/1` request:
`protocol`, `mode` ∈ {`warm_start`, `screening`}, `target_solver` (only
`highs_lp_qp_mip` is in the closed vocabulary), `model_manifest`
(model_version, model_artifact_ref, training_data_ref, topology_scope,
trained_topology_ids), `topology` (topology_id, node_count, edges),
`feature_declarations` (closed kinds `power`/`soc`/`voltage`/`status`
with canonical units; SOC a fraction in [0,1]; voltage pu > 0;
`features` rows matching the declarations), `screening.candidates` for
the screening role, optional `timeout_seconds` ≤ the recorded maximum,
`provenance.input_snapshot_id` + `request_id`), ONE JSON document on
stdout (the result envelope: model scope, topology_scope_check, outputs,
§9.2-shaped provenance, boundary statements, proposal-only posture). A
malformed request is answered with NOTHING on stdout + a reason on
stderr + exit 2. An out-of-scope topology is answered IN-BAND as a
flagged failure envelope (exit 0) — a flagged refusal is a RESULT, not
a crash, and no figure is ever fabricated.

## How the human build/test pass runs it (the recorded procedure)

1. **Resolve the framework DECISION-POINT first** (P8.03 §4): re-check
   PyG releases (RESEARCH.md §7 task 2), select and record a
   verified-compatible torch/PyG pair, confirm licenses from primary
   sources, run the P8.36 at-pin-time advisory check — then install
   into an ISOLATED research environment per
   `requirements-gnn.txt`. Nothing here grants the selection.
2. Run the pure self-tests (no framework needed):
   `python apps/worker/python/gnn_worker/gnn_inference_worker.py`
   — prints one PASS/FAIL line per hand-computed self-test.
3. Run the pytest suites (no framework needed for the pure tests; the
   torch-path tests skip loudly where the guard demands the real
   package):
   `PYTHONPATH=apps/worker/python/gnn_worker python -m pytest
   apps/worker/python/gnn_worker/tests -v`
4. Execute an inference end to end (the P8.12 human gate): train the
   model offline on the P8.10 training data (versioned artifacts per
   §6.5/P8.20; training-data provenance per P8.35), pipe the request
   document into `python apps/worker/python/gnn_worker/
   gnn_inference_worker.py`, and record the measured cells —
   **end-to-end feasible solve time, not inference latency** — in the
   P8.12 evaluation record's EMPTY cells via its recorded filling
   commands. An out-of-scope topology must produce the flag; a missing
   model file must fail loudly.
5. Update the framework pin status constant (`GNN_PIN_STATUS`) and this
   README's status line once the pair is verified — the record, not a
   silent edit.

## Honest limitations

- Nothing in this directory has ever executed; the torch/PyG API usage
  (artifact load, graph tensor assembly, inference-mode forward pass,
  per-candidate score/uncertainty extraction) is UNVERIFIED until the
  first run against the pinned pair — each step is marked UNVERIFIED in
  the worker and is a VERIFY-BUILD item.
- The model manifest is declared IN-REQUEST pending the P8.20 artifact
  store and P8.32 signing; an artifact-backed, signed manifest is the
  activation step (recorded as a follow-up in
  `ledger/phase-08-advanced-coordination/11_gnn_inference_worker.md`
  §8). The manifest-in-request is a documented stopgap, not a trust
  mechanism.
- The runtime inference-timeout enforcement mechanism is a VERIFY-BUILD
  item; the recorded maximum and the declaration validation exist.
- The gRPC binding of the generated `flexcore.worker.v1` servicer is
  the recorded activation seam (Phase 7 template pattern); until then
  the transport is the JSON-document discipline — the gap is recorded
  in `11_gnn_inference_worker.md` §8 as a Phase 7 follow-up, never
  fixed by inventing a parallel contract.
- The P8.04 closed result-key set for the GNN payload type carries
  `result_kind`/`training_data_ref`/`topology_scope`/`uncertainty` but
  no field for the warm-start vector or the ranking entries themselves;
  the additive extension is recorded as a Phase 7 follow-up (§8 of the
  record), not invented here.
- No claim of any accuracy, speedup, generalization, or safety result
  is made or implied: those are P8.12's measured cells, EMPTY until the
  human/lab gate, on held-out topologies only for any generalization
  statement.
