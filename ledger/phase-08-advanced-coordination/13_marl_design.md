# P8.13 — Safe MARL design: CTDE baseline, safety filter, and the infeasible-filter outcome

Date: 2026-11-13. Subtask: P8.13 (`ledger_phase_8.md` §P8.13).
Owner role: Research engineer (RL). Report trace: §13.3; §19.1; §7.4
(RLlib / Gymnasium). Inputs: P8.02 plan (`02_research_scope_freeze.md`
— the frozen M3 question list and prohibitions X-1/X-2/X-4/X-8);
P8.06 baselines (`05_06_harness_baselines.md` — per-module mapping);
RESEARCH.md §3 (RLlib/Ray, Gymnasium 1.3.0, ANDES `andes.rl`) and §6
(R8-C4); P8.03 license audit (`03_research_license_audit.md` §2/§4 —
Ray/RLlib UNPINNED-REVERIFY, the torch-leg constraint);
`python/workers/worker_template/research.py` (the P8.04 MARL result-key
vocabulary: `policy_version` + `training_seed` +
`environment_version` + `safety_filter_outcome`); the P8.10 design
(`10_gnn_warmstart.md` — the held-out-set rule and DECISION-POINT
record pattern this record follows).

## 1. Status of this record — a DESIGN, nothing installed, nothing run

This is the P8.13 design record. It defines the MARL research setup,
the environment's relationship to the platform simulator, the agent/
action/state/observation spaces as data contracts, the safety layer
and its honest derivation status, the infeasible-filter outcome, and
the framework choice space — **before any model is built or any
training run is planned** (the P8.02 freeze rule). It records no
result and closes no decision: the RL framework remains a recorded
**DECISION-POINT** (`03_research_license_audit.md` §2/§4; D4 OPEN) —
**no framework is selected, pinned, installed, or verified in this
repository**, and no D-item or DECISION-POINT is closed here. The tick
on P8.13 means *implemented, awaiting human build/test* — the human
gates are the framework pin, the CBF derivation sign-off, the training
runs, and every measured cell (P8.15), all EMPTY in this loop
(ENTRY_NOTE boundary 4).

**The boundary this design serves (stated verbatim, binding):**

> "No live device commands from MARL — a research MARL worker has no
> code path to the dispatch gateway; this is prohibited, not deferred"
> (`02_research_scope_freeze.md` X-2).

> "no research module has any field authority; a MARL worker issuing
> live device commands is PROHIBITED, not deferred" (`ENTRY_NOTE.md`
> boundary 1).

MARL proposes coordination actions through the same gates as
everything else: every MARL output is a PROPOSAL (ADR-010) that enters
at lifecycle state PROPOSED through the protected
validation → feasibility → authorization path (§13.4) and gains no
authority from having been proposed. The enforcement is in code
(`14_marl_worker.md` §2), not only in this prose.

## 2. The MARL setup (ledger step 1)

### 2.1 CTDE baseline (named)

The research baseline is **centralized training with decentralized
execution (CTDE)**, per §13.3. The form is RECOMMENDED, not decided:

| Item | Recorded content | Label |
|---|---|---|
| Training | Centralized: a centralized critic (and/or centralized coordination signal) over the site/global observation trains all agents jointly — the classic CTDE structure | RECOMMENDATION |
| Execution | Decentralized: each agent acts on its LOCAL observation at execution time; no centralized critic, no global state access, no inter-agent channel is assumed at execution | RECOMMENDATION |
| Why CTDE is the baseline | §13.3 names it; it matches the deployment shape (per-asset controllers, site-level coordination) without claiming the deployment (X-9: no productizing) | frozen scope |
| Baselines against it (P8.06) | deterministic optimization AND the rule-based/PI baseline (§13.3; `05_06_harness_baselines.md` per-module mapping); centralized-training baselines where applicable | frozen in P8.02 M3 |
| Frozen question (P8.02 M3) | In simulation/shadow only, under held-out topologies, load/weather regimes, device outages, delayed/lost messages, and uncertain state estimates, does a CTDE MARL policy with a derived safety-projection layer match or beat the deterministic and rule-based/PI baselines on the pre-registered §19.4 MARL metrics — with every safety-filter activation and infeasibility logged? | frozen |
| Non-claims that apply | X-2 (never live dispatch), X-4 (zero observed violations from a finite test set is a test outcome, not a guarantee — stated in plain language in every evaluation note), X-8 (no universal targets), X-9 (no productizing) | binding |

### 2.2 Agents' scope (record the choice)

| Choice | Content | Label |
|---|---|---|
| Agent granularity | **Per-asset agents**: one agent per controllable DER asset (storage unit, controllable feeder resource) within a site; coordinated through the centralized training structure of §2.1 | RECOMMENDED — RECORDED AS THE CHOICE for the design; re-litigable only through a new recorded decision before training |
| Alternative recorded | Per-site single agent (one agent owning all site assets) | recorded alternative — not selected |
| Rationale | Per-asset action spaces stay small and map 1:1 onto the filter's per-asset entries (`safety_filter.py` projects per asset), keeping the safety layer's model per-asset and checkable | recorded |

### 2.3 The environment and its relationship to the platform simulator

| Item | Recorded content |
|---|---|
| Environment | A Gymnasium-API environment (Gymnasium 1.3.0 recorded, license UNVERIFIED until pin time; RESEARCH.md §3) wrapping the platform's simulation capability in simulation/shadow only |
| Relationship to `crates/flexcore-simulator` | **BY CITATION, NO DEPENDENCY**: the design references the simulator's documented behaviour as the environment's dynamics source; the worker imports no crate, no Rust code, no FFI, and no simulator — the environment consumes simulator OUTPUTS as scenario artifacts over the worker boundary (§8.2: research workers communicate via Protobuf/gRPC or scenario artifacts, never internal tables). The environment's model scope is recorded per experiment (model version + generator version + seeds, per P8.35) |
| `andes.rl` / `AndesEnv` (RESEARCH.md §3/§4) | A CANDIDATE for the dynamic extension of the environment (P8.16–P8.18 track), NOT the 0.x baseline environment: ANDES is GPLv3, isolated per the P8.03 six-point design, and gate G-4 (legal confirmation) is OPEN. Recording it here selects nothing |
| Transport | The P8.14 worker's JSON-document discipline (one request in, one result envelope out); gRPC binding of `flexcore.worker.v1` is the recorded activation seam (Phase 7 follow-up) |
| D3 obligation (network class; P8.02 §4) | The environment's declared network class is **balanced radial distribution feeders** — the 0.x default RECOMMENDATION, matching the §19.3 scenario library's distribution scope; unbalanced or meshed topologies are OUT of the declared scope unless a later, explicitly scoped design decision adds them. A RECOMMENDATION label on the default, not a D3 closure — D3 remains OPEN (`.agent/DECISIONS.md`) |

### 2.4 Observation space (a data contract, not prose)

The observation is built from `telemetry.read`-class inputs only (the
permission ceiling of `plugins/profiles/sim-lab.profile.json`), with
telemetry quality states carried per value (GOOD/SUSPECT/BAD/MISSING/
ESTIMATED/STALE — §10.2; a MISSING/STALE value is NEVER silently
imputed for training or execution input; the handling of degraded
telemetry is part of the pre-registered evaluation design, P8.19's
uncertainty cases):

| Feature | Contract | Unit/kind |
|---|---|---|
| `p_export_mw` | Active-power export at the point of connection (P > 0 = export — a caller-side sign declaration carried by the NAME) | MW |
| reactive power | Declared per grid model and device profile — NEVER assumed (AGENTS.md; §10.2) | MVAr |
| `soc_fraction` | State of charge at the API boundary | fraction in [0, 1] |
| `v_pu` | Voltage | per-unit (solver representation), > 0 |
| device status | Binary availability | 0 / 1 |

The per-agent observation vector is composed ONLY from the declared
feature kinds above (the closed set mirrors the P8.10 feature contract,
`10_gnn_warmstart.md` §7 — one feature vocabulary across the ML
research line). An observation feature outside the declared set is a
contract refusal, never tolerated "for later".

### 2.5 Action space (a data contract; the filter's input)

| Item | Contract |
|---|---|
| Per-agent action | A continuous bounded setpoint proposal for the agent's asset: the proposed active-power setpoint (MW) for the next decision interval. Discrete on/off actions are OUT of scope: a connect/disconnect is a device-command shape, and the worker's action surface carries setpoint PROPOSALS only (X-2 posture) |
| Closed action kinds | `setpoint_delta`, `hold_current_setpoint` (the second is also the P8.13 §4 fallback kind) — `safety_filter.py::ACTION_KINDS` |
| Permission consumed | `schedule.propose` ONLY. An action claiming any other permission (dispatch/control-shaped) is refused with the named outcome `refused_permission_ceiling` — non-projectable by construction |
| What the action becomes | After the safety filter (§3), the filtered action is emitted as a `candidate_setpoint_vector` PROPOSAL into the P8.05 validation-harness vocabulary (P8.04 closed payload `flexcore.research.marl_shadow.v1`). It is never issued, never a schedule, never a device command |
| Physical-limit inputs | Per-asset declared limits: `p_min_mw`, `p_max_mw`, `ramp_mw_per_interval` (MW family; declared per request pending the platform-side limit registry — a recorded stopgap, §8 of `14_marl_worker.md`) |

### 2.6 Reward (SHAPE only — numbers are pre-registration-gated)

The reward SHAPE is recorded; NO number is: any numeric reward weight,
threshold, or target written in this loop would fabricate the §19.4
precondition (X-8; P8.02 §5). Pre-registration happens in writing
BEFORE any training/tuning run, at the P8.06 freeze at the earliest,
by the Research lead + independent reviewer:

| Term | Shape |
|---|---|
| Service objective | The same objective family the deterministic baseline optimizes (per the P8.06 frozen baseline definitions) — the comparison stays meaningful |
| Constraint-violation penalties | Penalize observed violations of the declared physical limits; the §19.4 metric is constraint-violation FREQUENCY, reported alongside return, never merged into one headline number |
| Filter interaction | Filter activations are REPORTED (activation/infeasibility statistics, P8.15), not hidden inside the reward: a policy tuned to exploit the filter's clamps must be visible as such |
| Reward hacking guard | The evaluation reports the baseline comparison on the SAME cases with the SAME accounting (P8.06 frozen definitions); a "win" that only exists under the research reward is reported as such |

## 3. The safety layer (ledger step 2) — derivation status: NOT YET DERIVED; training BLOCKED

### 3.1 What the ledger requires, and the honest status

The ledger P8.13 step 2 requires a control-barrier-function (CBF) or
safety-projection layer **derived from a valid system model and the
correct relative degree** (§13.3), with the derivation recorded and the
model it depends on named. The ledger's exit check explicitly allows
the alternative outcome: *"or recorded as not yet derived — and
training blocked until it is"*.

**Status: NOT YET DERIVED in this loop — TRAINING IS BLOCKED until a
power-systems reviewer signs the derivation** (the ledger's
verification method: "Power-systems reviewer checks the safety-filter
derivation (model, relative degree) independently; any derivation gap
blocks P8.14's training"). This is a RECORDED GAP, not a silent
substitution — no prose in this record pretends the CBF layer exists.

### 3.2 The model the derivation must start from (named)

| Item | Named content |
|---|---|
| System model | The declared environment model of §2.3: the balanced-radial feeder model behind the platform simulator (cited, `crates/flexcore-simulator`), linearized where the derivation requires it, WITH the model version + generator version + seeds recorded per P8.35 — a derivation against an unnamed model is void |
| Inputs (controls) | The per-asset setpoint proposals of §2.5 (MW setpoints, ramp-limited) |
| Constraint candidates | Declared P/ramp limits (static, implemented — see §3.3); voltage-band, branch-current, and SOC-continuation constraints (NOT implemented; CBF-layer scope) |
| The named trap (§13.3, binding) | **Voltage and branch-current constraints are NOT automatically simple dynamic state constraints**: in a quasi-static feeder model the voltage/branch-current solution is an ALGEBRAIC function of the state and inputs through the power-flow map (v = V(x, u)), so their relative degree w.r.t. the inverter setpoint inputs is generally NOT 1 — a naive CBF h(x) placed on the algebraic output, or a clamp placed on the output value, is unsound (the filter's QP/projection can be infeasible or vacuous at the wrong relative degree). The derivation must compose the constraint through the power-flow map explicitly, or state the model form (e.g. a dynamic-line/Voltage dynamic state) under which the relative degree IS well-defined. Any CBF QP used later inherits this requirement |

### 3.3 What IS implemented now (and its honest scope)

The implemented filter (`apps/worker/python/marl_worker/safety_filter.py`,
the P8.14 deliverable) is the STATIC limit/permission projection — a
SUBSET of the safety layer, explicitly NOT the CBF layer:

| Layer | Scope | Status |
|---|---|---|
| Permission-ceiling gate | The action's permission must be exactly `schedule.propose` (the closed ceiling from `plugins/profiles/sim-lab.profile.json`: telemetry.read, asset.read, schedule.propose); anything else is the named outcome `refused_permission_ceiling` — non-projectable, no clamp turns a forbidden permission into an allowed one | IMPLEMENTED (pure, hand-tested) |
| Static P/ramp projection | The feasible interval `[max(p_min, p_current - ramp), min(p_max, p_current + ramp)]` per asset; proposal outside → clamped to the interval (named outcome `filtered_clamped_to_limits`, activation recorded with before/after) | IMPLEMENTED (pure, hand-tested; arithmetic stated in the self-tests) |
| Infeasible-filter outcome | See §4 | IMPLEMENTED (pure, hand-tested; the fallback's failure is PROVABLE) |
| Voltage / branch-current / SOC-dynamic layer | NOT IMPLEMENTED — the CBF-layer scope (§3.2); a clamp on these would be unsound (wrong relative degree through the power-flow map) | NOT YET DERIVED — training blocked |

No safety or certification claim is made or implied by any layer:
"safe MARL" in the ledger's title names the RESEARCH DESIGN AREA
(§13.3), never an achieved property; zero observed violations from a
finite test set is a test outcome, not a guarantee (X-4).

## 4. The infeasible-filter outcome (ledger step 3) — defined, and provable

A filter can be INFEASIBLE: no safe action exists. The definition, the
fallback, and the proof:

| Item | Content |
|---|---|
| When the filter is infeasible | The feasible interval `[max(p_min, p_current - ramp), min(p_max, p_current + ramp)]` is EMPTY for any asset entry — i.e. the current setpoint sits outside `[p_min, p_max]` AND the ramp is too small to reach the bounds within one interval |
| What happens then | (1) The named outcome `infeasible_no_safe_projection` is produced (the closed vocabulary carries it as a first-class value — an infeasibility is a RESULT, never a crash and never silence); (2) the P8.13 §4 fallback action — `hold_current_setpoint`, re-filtered through the SAME projection — is attempted and RECORDED (`fallback.attempted: true`, viability recorded, never assumed); (3) the whole action is refused: NO proposal is emitted, the envelope is a failed result carrying the full filter report, and the shadow log entry records the infeasibility (`infeasibility.recorded: true`) for the P8.15 statistics |
| Why the fallback provably fails here | The hold-current action (p_proposed = p_current) passes the interval check iff p_current ∈ [p_min, p_max]; but p_current ∈ [p_min, p_max] makes the PRIMARY interval NON-empty (it contains p_current itself, since ramp ≥ 0). Contrapositive: primary interval empty ⟹ p_current outside [p_min, p_max] ⟹ the fallback violates the same bounds. The failure is a THEOREM about the implemented model, stated in `safety_filter.py` header item 4 and pinned by two hand-computed constructions; the fallback is still EXECUTED (not assumed) so the recorded evidence matches the model at run time |
| Never | an unfiltered action passed through silently; a fabricated "safe" value; an infeasibility swallowed into a generic error; a refusal that skips the shadow log |
| The safety-relevant signal | An infeasible filter means the STATE itself is limit-violating (or the declared limits are internally inconsistent with the observed state) — that diagnostic is the output, telemetry-worthy, and counted in the P8.15 infeasibility statistics with the outcome of EVERY occurrence |

## 5. Framework selection (ledger step 4) — DECISION-POINT OPEN, RECOMMENDATION only

The ledger's step 4 ("Select the framework explicitly with version
pins and the API stack it targets") is recorded here as the
DECISION-POINT it actually is: nothing is selected, installed, or
verified in this loop, and the verification the ledger asks for is by
construction a HUMAN gate. What this design fixes is the choice space,
the resolution rule, and the RECOMMENDATION, with labels (agents never
close D-decisions):

| Item | Recorded content | Label |
|---|---|---|
| Report default | RLlib as the named RL framework (§7.4), Gymnasium as the environment API | DEFAULT (report) — not a decision |
| Version status | Ray/RLlib **UNPINNED-REVERIFY**: no version current as of 7 November 2026 verified from a primary source (2.56.1 / 2.57.0 from July–August 2026 secondary snapshots only; RESEARCH.md §7 task 1). Gymnasium **1.3.0** recorded (RESEARCH.md §3), license UNVERIFIED | recorded finding — pin at the human pass |
| API stack | RLlib's **NEW API stack** (RLModule, Learner, EnvRunner, ConnectorV2) is default in recent releases and is RECOMMENDED; the old ModelV2/Policy stack is the recorded alternative. R8-C4 binds: the legacy `rllib_contrib` collection (A2C, DDPG, TD3, MADDPG, QMIX, …) was REMOVED — a design assuming MADDPG/QMIX availability in current RLlib would be WRONG; the CTDE baseline's algorithm must come from the new stack's shipped set (e.g. PPO with a centralized critic) | RECOMMENDATION — recorded; the selection is the human gate's recorded decision |
| PyTorch consequence | RLlib is now **PyTorch-only** (RESEARCH.md §3) — a Ray/RLlib selection FORCES the PyTorch leg of the P8.03 §4 torch/PyG DECISION-POINT (the verified-compatible pair of `10_gnn_warmstart.md` §6, resolved in the SAME isolated environment) | recorded consequence |
| Alternatives | Stable-Baselines3 / CleanRL / a JAX stack: recorded alternatives, each its own pinned, licensed, CVE-checked decision if selected (P8.03 §4) | open alternatives — not selected |
| `andes.rl` extension | The dynamic-track extension candidate (§2.3) — ANDES GPLv3 isolation applies; gate G-4 OPEN | recorded candidate — not selected |
| License/CVE | Ray/RLlib Apache-2.0 family, Gymnasium MIT family — UNVERIFIED until confirmed from primary sources at pin time; whatever is pinned is checked AT PIN TIME by the P8.36 procedure | UNVERIFIED |
| Where the pin lives | `apps/worker/python/marl_worker/requirements-marl.txt` (carries the gate-order procedure and NO pin line) | recorded |
| Status | **DECISION-POINT OPEN** — the worker's `MARL_PIN_STATUS` constant carries this status verbatim | OPEN |

## 6. Provenance, reproducibility, and the follow-on references

| Item | Reference (never duplicated here) |
|---|---|
| Training-data provenance | **P8.35** owns the full record (inventory, ingestion validation, licensing, versioning); this design and the worker carry only the REFERENCE (`training_data_ref`), never the record |
| Held-out sets | The rule is frozen (P8.02 §4: held-out topologies/regimes named at design time); the concrete lists follow the P8.10 §4 RECOMMENDED assignment pattern (§19.3 library families + the P8.19 regime extensions), UNCONFIRMED until the P8.35 manifest and P8.19 artifacts exist — the reviewer's overlap check is the evidence |
| GPU/seed nondeterminism | **P8.38** owns the procedure (seeds, framework nondeterminism controls, what a rerun must reproduce); results are never claimed reproducible beyond it |
| Result keys | The P8.04 closed MARL payload (`flexcore.research.marl_shadow.v1`): `result_kind`, `policy_version`, `training_seed`, `environment_version`, `safety_filter_outcome` — enforced in the worker (`enforce_marl_result_schema`); `safety_filter_outcome`'s closed value set is §3.3's five outcomes |
| Evaluation | **P8.15** (template, EMPTY cells) reports per-case constraint-violation frequency, return, robustness under distribution shift, baseline comparison, and the filter activation/infeasibility statistics computed from the shadow log |
| Shadow log | The worker's in-envelope log (P8.14); the artifact-backed log store is the P8.20 seam |

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| CTDE baseline named | DONE — §2.1 (RECOMMENDED form recorded; agents' scope recorded per §2.2) |
| Safety filter derived from a stated model with correct relative degree (or recorded as not yet derived — and training blocked until it is) | **RECORDED AS NOT YET DERIVED** (§3) — the model the derivation must start from is named (§3.2), the relative-degree trap is stated verbatim, and TRAINING IS BLOCKED; the implemented static filter (§3.3) is honestly scoped as a subset, never passed off as the CBF layer |
| Infeasible-filter outcome defined | DONE — §4, implemented in code with the provable fallback failure, pinned by hand-computed tests |
| Framework + API stack pinned | **NOT PINNED — cannot be in this loop** (nothing installed; DECISION-POINT OPEN with the RECOMMENDATION recorded, §5) — the human gate verifies by install and records the decision |

## 8. Verification method (for the human/reviewer)

1. **CBF derivation gate (blocks training)**: the power-systems reviewer
   independently derives or signs the safety-layer derivation against
   the named model (§3.2) with the relative-degree analysis composed
   through the power-flow map — the ledger's verification method. Until
   then, training stays blocked; the implemented static filter is never
   a substitute.
2. **Framework pin**: verify Ray/RLlib by install in an isolated
   environment (`requirements-marl.txt` gate order), record the version
   and the API stack explicitly, update the DECISION-POINT tables here
   and in `03_research_license_audit.md` §2 — a recorded decision, not a
   silent edit.
3. Run the P8.14 worker's pure self-tests (no framework needed):
   `python apps/worker/python/marl_worker/safety_filter.py`,
   `python apps/worker/python/marl_worker/shadow_log.py`,
   `python apps/worker/python/marl_worker/marl_worker.py`, and the
   pytest suite:
   `PYTHONPATH=apps/worker/python/marl_worker python -m pytest
   apps/worker/python/marl_worker/tests -v`.
4. Confirm the boundary per the P8.14 verification method: the
   code-path inspection from worker output to dispatch must be EMPTY
   (the introspection + AST tests in `tests/test_marl_worker.py`
   operationalize it), and the permission-refusal test must show the
   named outcome with NO proposal.
5. All P8.15 measured cells stay EMPTY until the human/lab run fills
   them via the template's recorded commands.

## 9. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, or scan was
performed for this design.
