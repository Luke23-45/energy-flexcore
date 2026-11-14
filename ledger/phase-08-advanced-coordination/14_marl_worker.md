# P8.14 — MARL simulation/shadow worker implementation (never live dispatch)

Date: 2026-11-13. Subtask: P8.14 (`ledger_phase_8.md` §P8.14).
Owner role: Research engineer (RL). Report trace: §13.3; §13.4;
§19.2(9); §5.7. Inputs: P8.13 design (`13_marl_design.md`); P8.04
contracts (`04_worker_contracts.md`;
`python/workers/worker_template/research.py`); P7.17 template; P8.03
license audit (§2 DECISION-POINT); the freshest worker patterns
(`apps/worker/python/andes_worker/andes_dynamic_worker.py`,
`apps/worker/python/gnn_worker/gnn_inference_worker.py`).

## 1. Status of this record — code WRITTEN, NEVER RUN; no framework exists

The RL framework (ray/rllib + gymnasium) is a recorded
**DECISION-POINT** (`03_research_license_audit.md` §2/§4; D4 OPEN;
RESEARCH.md §7 task 1 — no primary-source Ray version as of the
2026-11-07 cut-off): **no framework is selected, pinned, installed, or
verified in this repository's history**. The worker
(`apps/worker/python/marl_worker/`) was **NEVER executed** — not
against a real framework (there is none to run it against), not against
a stub. Every framework call follows the documented RLlib NEW API stack
/ Gymnasium 1.x API shape as of the 2026-11-07 research cut-off and is
**UNVERIFIED** until the human build/test pass pins the framework
(P8.13 §5 RECOMMENDATION) and executes it. **Every training/inference
result in this repository is an EMPTY cell** — no reward curve,
convergence statistic, win-rate, or violation frequency exists anywhere.
The tick on P8.14 means *implemented, awaiting human build/test* —
nothing more.

## 2. The boundary in code (ledger steps 2 and 4; the whole point)

The PROHIBITED-not-deferred boundary (P8.13 §1, stated verbatim there)
is enforced in code, not only in docs:

| # | Rule | Where implemented |
|---|---|---|
| 1 | The worker has NO path to the dispatch gateway | the ONLY output channel is ONE JSON document on stdout; the module's AST is pinned by test to import stdlib + the two sibling pure modules ONLY (`test_marl_worker.py::TestDispatchBoundary::test_module_ast_imports_stdlib_and_siblings_only`) — no transport, no dispatch client, no simulator import exists |
| 2 | No dispatch/issue/authorize/approve/command-shaped capability EXISTS | an introspection test asserts the module namespaces of `marl_worker`, `safety_filter`, and `shadow_log` contain NO callable whose name carries any of those tokens — there is no code path to name (`TestDispatchBoundary::test_no_dispatch_shaped_callable_*`) |
| 3 | No authorization token, no command capability in the response TYPE | `assert_proposal_only()` scans the whole result tree (shadow log included) against the closed `FORBIDDEN_RESULT_KEYS` and refuses emission at any depth; `authority: "none"` on every envelope |
| 4 | An action outside the permission ceiling is structurally impossible to route | `safety_filter.filter_action` refuses any `proposed_permission` other than `schedule.propose` with the named outcome `refused_permission_ceiling`, ZERO entries projected (no clamp turns a forbidden permission into an allowed one), and the envelope emits NO proposal |
| 5 | The mode is locked, not selectable | `MODE_LOCKED = "simulation-shadow"` is a module CONSTANT; a request field attempting to raise it is ignored (self-tested); no plugin or research module can select a mode (§21.4, §5.7) |
| 6 | The boundary test: "attempt to route a MARL action to dispatch and confirm there is no code path that allows it" | the closest expressible attempt — a dispatch-shaped permission — is refused in-band with no proposal (`test_dispatch_permission_refusal_emits_no_proposal`); combined with #1/#2/#3 the attempt fails by construction: there is no function to call, no key to smuggle, no transport to speak |

## 3. What was implemented (ledger steps 1–3 → code)

| Ledger step | Where it lives | Status |
|---|---|---|
| Step 1 (training offline + shadow inference, with a shadow action logger recording every proposed action, the safety-filter outcome, and the simulated consequence) | Training is the OFFLINE human/lab gate and is implemented as a GUARD, not code: `run_training()` is reachable only through the guarded framework imports (the NAMED DECISION-POINT ImportError dominates) — UNREACHABLE here, and no reward/hyperparameter/seed number exists anywhere (EMPTY cells; X-8 pre-registration rule). Shadow inference is `run_shadow_inference()` (guarded, UNREACHABLE — the policy path) plus `run_filter_probe()` (pure, runnable: a DECLARED candidate action probed through the filter — the P8.15 harness's deterministic filter oracle, never a policy output). The logger is `shadow_log.py`: closed entry schema, MANDATORY closed-vocabulary `safety_filter_outcome` per entry, sequence order strictly increasing from 1 (a gap would hide an action), consequence honestly `not_simulated` with the recorded reason (the environment path never ran); the P8.15 human/lab gate fills consequences | pure logger WRITTEN + hand-tested; training and policy inference UNRUN (no framework); consequences EMPTY |
| Step 2 (the boundary in code; output type is a proposal) | §2 of this record; the output payload is the CLOSED P8.04 MARL result-key set (`enforce_marl_result_schema`): `result_kind` (candidate_setpoint_vector), `policy_version`, `training_seed` (integer, bool-excluded, caller-declared — the worker never invents a seed), `environment_version`, `safety_filter_outcome` (closed vocabulary) | DONE (pure logic; hand-tested) |
| Step 3 (the safety filter incl. the infeasible-filter outcome; log every filter activation and every infeasibility) | `safety_filter.py`: the permission-ceiling gate (mirrors `plugins/profiles/sim-lab.profile.json`, pinned by a test that reads the JSON), the P/ramp feasible-interval projection, the closed five-value outcome vocabulary, the EMPTY-interval infeasibility with the fallback attempted and PROVABLY not viable (the theorem in the module header, item 4), activations recorded with before/after, infeasibility recorded on the log entry and counted in the log summary | DONE (pure logic; hand-computed tests with the arithmetic stated) |
| Step 4 (test the boundary) | The `TestDispatchBoundary` class (§2 row 6) + the fail-closed main tests (malformed stdin → exit 2, NOTHING on stdout) | WRITTEN, NEVER RUN (pytest never installed; the in-module self-tests mirror every case) |

Gate order on the policy path (mirrors the GNN worker): pure request
validation → (a caller-injected action in shadow mode is REFUSED — the
action must come FROM the policy) → guarded ray import → guarded
gymnasium import → policy-artifact check (missing file fails LOUDLY,
named `PolicyArtifactError`, exit 2, nothing on stdout) → UNVERIFIED
inference steps. With nothing installed, the named DECISION-POINT
requirement is the first honest refusal.

## 4. The safety_filter_outcome vocabulary (LOAD-BEARING)

The closed five-value set carried by the P8.04 result key and every
shadow log entry (`safety_filter.py::FILTER_OUTCOMES`):

| Outcome | Meaning | Proposal emitted? |
|---|---|---|
| `passed_within_limits` | every asset entry inside its feasible interval; no activation | yes |
| `filtered_clamped_to_limits` | at least one entry projected onto the interval boundary; activation recorded with before/after | yes (the FILTERED action, never the raw one) |
| `refused_permission_ceiling` | the action's permission is outside the closed ceiling — non-projectable | NO — in-band failed envelope |
| `refused_nonfinite_action` | a non-finite proposed setpoint (NaN/inf); no projection exists from a non-finite value | NO — in-band failed envelope |
| `infeasible_no_safe_projection` | the feasible interval is EMPTY; fallback attempted, provably not viable, RECORDED | NO — in-band failed envelope |

A result or log entry without a named outcome is UNREPRESENTABLE (the
schema refuses it) — the ledger's named failure mode ("a shadow log
that records actions but not filter outcomes, making safety
unevaluable") cannot occur through this code.

## 5. The named deployment requirements (verbatim, for the reviewer)

```
RAY_RLLIB_DEPLOYMENT_REQUIREMENT:
  flexcore MARL worker requires the research RL framework: ray/rllib
  is NOT INSTALLED and the RL framework choice is a recorded
  DECISION-POINT (P8.03 section 2: Ray/RLlib UNPINNED-REVERIFY — no
  version verified from a primary source as of the 2026-11-07 cut-off,
  RESEARCH.md section 7 task 1; D4 OPEN). At the human build/test pass,
  pin the framework per the P8.13 design record section 5 (RECOMMENDED:
  RLlib NEW API stack on the PyTorch leg) into an ISOLATED research
  environment (never a system environment, never the platform runtime)
  after the P8.36 at-pin-time advisory check, and update the pin status

GYMNASIUM_DEPLOYMENT_REQUIREMENT:
  flexcore MARL worker requires gymnasium (the environment API): NOT
  INSTALLED; Gymnasium 1.3.0 is RECORDED (RESEARCH.md section 3) but
  its license is UNVERIFIED until confirmed from the project's primary
  source at pin time. Pin only a verified-compatible version into the
  isolated research environment; never install into the platform runtime
```

## 6. Isolation/contract verification procedure (for the reviewer)

```sh
# (a) the ONLY ray/gymnasium imports in the repository, deferred+guarded:
grep -rn "import ray\|import gymnasium" apps python --include=*.py
#     expected: hits ONLY inside apps/worker/python/marl_worker/
#     marl_worker.py function bodies (no module-level import, no ray or
#     gymnasium anywhere else)

# (b) the worker directory imports no transport and no simulator:
grep -rn "grpc\|socket\|requests\|http\|pandapower\|helics\|import andes" \
  apps/worker/python/marl_worker --include=*.py
#     expected: no hits (the words may appear in comments only)

# (c) the pure core imports with NOTHING installed:
PYTHONPATH=apps/worker/python/marl_worker python -c \
  "import marl_worker, safety_filter, shadow_log; print('pure core OK')"

# (d) without the framework, no policy output can exist (the named
#     DECISION-POINT requirement):
PYTHONPATH=apps/worker/python/marl_worker python -c \
  "import marl_worker as w; w.run_shadow_inference(w._shadow_request())"
#     expected: ImportError naming RAY_RLLIB_DEPLOYMENT_REQUIREMENT and
#     the DECISION-POINT

# (e) the hand-computed pure self-tests:
python apps/worker/python/marl_worker/safety_filter.py
python apps/worker/python/marl_worker/shadow_log.py
python apps/worker/python/marl_worker/marl_worker.py

# (f) the pytest suite (incl. the P8.14 step-4 boundary tests):
PYTHONPATH=apps/worker/python/marl_worker python -m pytest \
  apps/worker/python/marl_worker/tests -v
```

Outputs of (a)–(f) are quoted into the human build/test pass record.
The ledger's verification method — "Code-path inspection from worker
output to dispatch (must be empty); attempt the routing in a test and
confirm failure" — is operationalized by (a)/(b) (inspection) and the
`TestDispatchBoundary` tests (the attempt), all exercisable WITHOUT any
framework at the human gate.

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Worker runs in simulation/shadow only | WRITTEN as an isolated Class C worker (one JSON doc in/out, no shared state, guarded imports, mode-locked constant); execution UNRUN (no framework exists to run; the pure filter_probe path is runnable but was never run per the loop bans) |
| Every action is logged with its filter outcome | DONE in code and hand-tested (the outcome is MANDATORY and closed-vocabulary on every entry — the unevaluable-log failure mode is unrepresentable); the actions themselves are EMPTY cells (no policy exists) |
| No code path from the worker to dispatch exists (verified by inspection + test) | DONE as WRITTEN tests (AST import audit, namespace introspection, permission refusal, proposal-only scan — §2); ALL TESTS NEVER RUN (pytest never installed); the human gate runs them first |
| Filter infeasibility is logged, never silent | DONE in code and hand-tested (named outcome, fallback attempted + provably not viable, `infeasibility.recorded: true` on the log entry, counted in the summary) |

## 8. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **gRPC binding seam (P8.04/P7.17):** the worker reuses the template's
   §9.2 provenance field names verbatim and the P8.04 closed MARL
   vocabularies, but binds no generated `flexcore.worker.v1` servicer
   (buf generation remains the activation step; the transport is the
   JSON-document discipline, mirroring the andes/gnn workers' recorded
   seam). Binding the servicer into a Class C research worker is the
   recorded activation task — a Phase 7/P8.04 seam, NOT a parallel
   contract.
2. **Closed result-key set too small for the filtered setpoints
   (P8.04):** `MODULE_RESULT_FIELDS["flexcore.research.marl_shadow.v1"]`
   = {`result_kind`, `policy_version`, `training_seed`,
   `environment_version`, `safety_filter_outcome`} has NO field to
   carry the filtered setpoint values themselves. The worker's JSON
   surface documents the needed shape (`outputs.proposal.setpoints`) as
   an additive extension PROPOSAL for the P8.04 owner; until that
   extension exists, the P8.04-schema check applies to the
   `result_payload` sub-object and the envelope carries the values
   alongside it (the P8.11 precedent). Recorded as a Phase 7 follow-up
   — never fixed by inventing a parallel contract.
3. **Plan.md location drift (recorded, not silently resolved):**
   `ledger/phase-08-advanced-coordination/PLAN.md` §3 names
   `python/workers/marl-research/` for the MARL worker; the implemented
   location is `apps/worker/python/marl_worker/`, following the
   wave-2b precedent (`apps/worker/python/gnn_worker/`,
   `apps/worker/python/andes_worker/`) and the wave-3b file-ownership
   assignment. The composition seam (a Class C research worker behind
   the Phase 7 template contracts) is unchanged; the location is a
   recorded deviation for the P8.30 retrospective to reconcile.
4. **Policy context / limits / observation scope declared IN-REQUEST
   (P8.20/P8.32/P8.35 seam):** the policy context, per-asset physical
   limits, and observation scope are declared in the REQUEST (validated
   on the pure-logic side) because the P8.20 artifact store, P8.32
   signing, and the P8.35 provenance record do not exist yet. An
   artifact-backed, signed manifest read by the worker itself is the
   activation step. The manifest-in-request is a documented stopgap,
   not a trust mechanism — a caller could declare false limits; the
   mitigation is that every filter decision is RECORDED with its
   declared inputs (before/after, allowed interval), so a false
   declaration is visible in the shadow log, and the P8.15 evaluation
   only counts runs whose inputs match the P8.35 record.
5. **Runtime enforcement items (VERIFY-BUILD, UNVERIFIED):** the
   artifact-store ref resolution (`object-store:` prefix), the
   with-framework inference steps (artifact load, policy forward,
   action extraction), and the training path's real entrypoint
   (dataset ref per P8.35, pre-registered hyperparameters per P8.02 §5,
   seeds per P8.38) are the human build/test pass's work — recorded
   honestly, never pretended here.

## 9. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, or scan was
performed for this implementation.
