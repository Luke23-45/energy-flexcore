# P8.13/P8.14 — The MARL simulation/shadow worker
#         (`apps/worker/python/marl_worker/`)

Status: **WRITTEN, NEVER EXECUTED — UNVERIFIED-API, DECISION-POINT-GATED,
NO POLICY OUTPUT EXISTS, ALL TRAINING/INFERENCE CELLS EMPTY.** The Phase
8 loop bans install/build/run: the RL framework is a recorded
**DECISION-POINT** (P8.03 §2 — Ray/RLlib UNPINNED-REVERIFY, no
primary-source version as of the 2026-11-07 cut-off; Gymnasium 1.3.0
recorded, license UNVERIFIED), so **nothing is selected, pinned,
installed, or verified**; this worker was NEVER run, and **every
training/inference result in this repository is an EMPTY cell** — no
reward curve, convergence statistic, win-rate, or violation frequency
exists anywhere in this directory, and none may be fabricated
(ENTRY_NOTE boundary 4). The CBF safety-layer derivation is RECORDED AS
NOT YET DERIVED (`13_marl_design.md` §3) and **training is BLOCKED until
a power-systems reviewer signs it**.

## What this worker is — and what it is not

The worker implements the P8.13 design's simulation/shadow research
surface (never live dispatch — **PROHIBITED, not deferred**:

> "No live device commands from MARL — a research MARL worker has no
> code path to the dispatch gateway; this is prohibited, not deferred"
> (`02_research_scope_freeze.md` X-2; `ENTRY_NOTE.md` boundary 1).

- **filter_probe** (`filter_probe` mode): the request DECLARES a
  candidate action (replayed from a recording or produced by a rule
  baseline); the worker runs ONLY the pure safety filter + shadow
  logging + envelope — stdlib-only, runnable today. This is the P8.15
  evaluation harness's deterministic filter oracle, never a policy
  output.
- **shadow_inference** (the policy path): the action comes FROM the
  policy — guarded `ray`/`gymnasium` imports raise the NAMED
  DECISION-POINT requirement first, so **this path is UNREACHABLE in
  this repository**. A caller-injected action in a shadow_inference
  request is REFUSED (it would bypass the policy).

Every output is a **PROPOSAL** (`result_kind: candidate_setpoint_vector`
inside the CLOSED P8.04 MARL payload `flexcore.research.marl_shadow.v1`:
`result_kind`, `policy_version`, `training_seed`, `environment_version`,
`safety_filter_outcome`) into the P8.05 validation-harness vocabulary —
it enters at lifecycle state PROPOSED through the same
validation → feasibility → authorization gates as any other candidate
and gains no authority from having been proposed (ADR-010; §13.4).

It is:

- **Proposal-only in code** (ADR-010): `assert_proposal_only()` scans
  the whole result tree (shadow log included) and refuses emission of
  any authorization/command-shaped key at any depth; `authority: "none"`;
  **no function whose name is dispatch/issue/authorize/approve/command-
  shaped exists in any module here** (pinned by an introspection test);
  the module's AST imports stdlib + the two sibling pure modules ONLY
  (pinned by an AST test) — there is no transport, no dispatch client,
  no simulator import. The only output channel is ONE JSON document on
  stdout.
- **Mode-locked to simulation/shadow** (§21.4, §5.7): a constant, not a
  setting; no request field can change it.
- **Permission-ceiling-gated**: the safety filter's ceiling
  (`telemetry.read`, `asset.read`, `schedule.propose`) mirrors
  `plugins/profiles/sim-lab.profile.json` verbatim (pinned by a test
  that reads the JSON). An action whose permission is outside the
  ceiling is refused with the named outcome
  `refused_permission_ceiling` — no clamping turns a forbidden
  permission into an allowed one.
- **Filter-outcome-complete**: the `safety_filter_outcome` key is
  MANDATORY, closed-vocabulary, on every result payload AND every shadow
  log entry — the ledger's "a shadow log that records actions but not
  filter outcomes" failure mode is structurally unrepresentable.
- **Infeasibility-honest** (P8.13 §4): when the feasible interval is
  EMPTY the filter is INFEASIBLE; the hold-current fallback is attempted,
  PROVABLY infeasible (derivation in `safety_filter.py` header item 4),
  and RECORDED — never an unfiltered action passed through silently.
- **Provenance-complete** (§9.2 + P8.04): every envelope carries worker
  version, policy version, training seed (caller-declared; the worker
  never invents one), environment version, input snapshot ID, request ID,
  termination status, diagnostics.

## The safety filter — what it covers, and what it does NOT

`safety_filter.py` is the STATIC limit/permission projection defined by
the P8.13 design (§3/§4 of `13_marl_design.md`):

- covers: the permission ceiling; per-asset P bounds (MW; P > 0 = export,
  a caller-side sign declaration) and ramp limits (MW per decision
  interval) via the feasible interval
  `[max(p_min, p_current - ramp), min(p_max, p_current + ramp)]`;
- does NOT cover: voltage, branch-current, or SOC-dynamic constraints —
  those are exactly the constraints the design warns are NOT simple
  dynamic state constraints (voltage and branch current are functions of
  the algebraic power-flow solution; their relative degree w.r.t.
  inverter setpoint inputs is generally NOT 1, and a naive clamp would be
  unsound). They belong to the CBF layer — **NOT YET DERIVED; training
  blocked** until the derivation is reviewed (P8.13 §3; the ledger's
  verification method).

The closed outcome vocabulary (the value set of the P8.04
`safety_filter_outcome` key): `passed_within_limits`,
`filtered_clamped_to_limits`, `refused_permission_ceiling`,
`refused_nonfinite_action`, `infeasible_no_safe_projection`.

## The shadow action logger

`shadow_log.py` (ledger P8.14 step 1): every proposed action is recorded
with its filter outcome and its simulated consequence. The consequence is
honestly EMPTY in this loop (`consequence_status: "not_simulated"`,
`simulated_consequence: null` with a recorded reason — the environment
path was never executed); the P8.15 human/lab gate fills consequences,
and the schema refuses a fabricated consequence riding on a
not-simulated entry. The log travels INSIDE the result envelope (the
documented stopgap; the artifact-backed store reference is the P8.20
seam). The summary (`log_summary`) computes the counts the P8.15 filter
statistics consume — counts only, never invented rates.

## Files

| File | Content |
|---|---|
| `marl_worker.py` | The worker entrypoint: one `flexcore-marl-worker/1` JSON request on stdin → one result envelope on stdout; guarded ray/gymnasium imports with the named DECISION-POINT requirements; the UNREACHABLE policy path and the UNREACHABLE training path (`run_training` — guarded behind the framework import; EMPTY cells); pure request validation, closed P8.04 result schema, proposal-only scan, envelope; hand-computed in-module self-tests (`run_self_tests()`) |
| `safety_filter.py` | The pure safety filter: closed permission ceiling (mirrors `sim-lab.profile.json`, pinned by test), the feasible-interval projection, the closed outcome vocabulary, the infeasible-filter outcome with the provably-dead fallback; hand-computed self-tests with the arithmetic stated |
| `shadow_log.py` | The pure shadow action logger: closed entry schema, mandatory closed-vocabulary filter outcome per entry, consequence honesty rules, strictly-increasing sequence order, hand-computed summary counts |
| `requirements-marl.txt` | The framework-pin procedure (DECISION-POINT status; NO pin line — an uncommented pin would be an unverified selection) |
| `tests/test_safety_filter.py` | Pytest mirror: projection arithmetic, infeasibility, permission ceiling incl. the profile-JSON pin, closed vocabularies (WRITTEN, NEVER RUN) |
| `tests/test_shadow_log.py` | Pytest mirror: entry schema, sequence order, outcome mandatory, consequence honesty, summary arithmetic (WRITTEN, NEVER RUN) |
| `tests/test_marl_worker.py` | Pytest mirror: the dispatch-boundary tests (introspection + AST), closed P8.04 schema, envelope posture, framework honesty gate, fail-closed main (WRITTEN, NEVER RUN) |

## Transport contract

ONE JSON document on stdin (the `flexcore-marl-worker/1` request:
`protocol`, `mode` ∈ {`filter_probe`, `shadow_inference`},
`policy_context` (policy_version, training_seed integer,
environment_version), `action` (filter_probe ONLY: action_kind ∈
{`setpoint_delta`, `hold_current_setpoint`}, `proposed_permission` (only
`schedule.propose` accepted), `assets` with per-asset limits), or
`policy_manifest` (shadow_inference ONLY), `provenance`).
ONE JSON document on stdout (the result envelope: policy scope, filter
outcome, filter report, proposal, closed P8.04 result payload, shadow
log + summary, §9.2-shaped provenance, boundary statements verbatim,
proposal-only posture). A malformed request is answered with NOTHING on
stdout + a reason on stderr + exit 2 (fail-closed). A refused action
(permission outside the ceiling, non-finite proposal, infeasible filter)
is answered IN-BAND as a failed envelope (exit 0) with the named
outcome — a refusal is a RESULT, never a crash, never a silent
pass-through, and no figure is ever fabricated.

## How the human build/test pass runs it (the recorded procedure)

1. **Resolve the framework DECISION-POINT first** (P8.03 §2): verify the
   current Ray/RLlib version from a primary source (RESEARCH.md §7
   task 1), record the API-stack choice explicitly (P8.13 §5: new stack
   RECOMMENDED), resolve the forced PyTorch leg per
   `requirements-marl.txt`, confirm licenses from primary sources, run
   the P8.36 at-pin-time advisory check — then install into an ISOLATED
   research environment. Nothing here grants the selection.
2. **Reviewer gate**: the CBF safety-layer derivation (P8.13 §3) must be
   signed by a power-systems reviewer BEFORE training — it is recorded
   as NOT YET DERIVED, and training is blocked until it is.
3. Run the pure self-tests (no framework needed):
   `python apps/worker/python/marl_worker/safety_filter.py`,
   `python apps/worker/python/marl_worker/shadow_log.py`,
   `python apps/worker/python/marl_worker/marl_worker.py`
   — each prints one PASS/FAIL line per hand-computed self-test.
4. Run the pytest suites (no framework needed for the pure tests; the
   guarded-path tests skip loudly where the guard demands the real
   package):
   `PYTHONPATH=apps/worker/python/marl_worker python -m pytest
   apps/worker/python/marl_worker/tests -v`
5. Execute a shadow inference end to end (the P8.15 human gate): train
   the policy offline (pre-registered hyperparameters/seeds per
   P8.02 §5/X-8; training-data provenance per P8.35; GPU
   nondeterminism procedure per P8.38), pipe the request document into
   `python apps/worker/python/marl_worker/marl_worker.py`, and fill the
   P8.15 evaluation record's EMPTY measured cells (constraint-violation
   frequency, return, robustness, filter activation/infeasibility
   statistics) via its recorded filling commands. Zero observed
   violations is a test outcome, never a safety guarantee (X-4).
6. Update the framework pin status constant (`MARL_PIN_STATUS`) and this
   README's status line once the framework is verified — the record, not
   a silent edit.

## Honest limitations

- Nothing in this directory has ever executed; every framework call
  (artifact load, policy forward, action extraction) is UNVERIFIED and a
  VERIFY-BUILD item — each step is marked in the worker, and the
  guarded-path tests skip loudly if a framework is present.
- NO training code exists beyond the guarded, unreachable
  `run_training` gate: no reward is defined numerically anywhere (the
  reward SHAPE is recorded in the P8.13 design; numbers are
  pre-registered at the P8.06 freeze before training, X-8), no
  hyperparameter, no seed result, no convergence statistic, no win-rate.
- The policy context / limits / observation scope are declared
  IN-REQUEST pending the P8.20 artifact store, P8.32 signing, and the
  P8.35 training-data provenance record — documented stopgaps, not trust
  mechanisms (recorded as follow-ups in `14_marl_worker.md` §8).
- The simulated-consequence field is EMPTY (`not_simulated`) on every
  entry this loop can produce; the P8.15 harness fills it.
- The gRPC binding of the generated `flexcore.worker.v1` servicer is the
  recorded activation seam (Phase 7 template pattern); until then the
  transport is the JSON-document discipline — the gap is recorded in
  `14_marl_worker.md` §8 as a Phase 7 follow-up, never fixed by
  inventing a parallel contract.
- The P8.04 closed MARL result-key set carries no field for the filtered
  setpoints themselves; they travel in `outputs.proposal` alongside the
  schema-checked `result_payload` — the additive extension is recorded
  as a Phase 7 follow-up (§8 of the record), not invented here.
- No claim of any performance, robustness, generalization, or safety
  result is made or implied: those are P8.15's measured cells, EMPTY
  until the human/lab gate.
