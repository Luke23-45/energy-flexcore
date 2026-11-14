# P8.39 — Co-simulation and dynamic cross-checks (record — every measured cell EMPTY)

Date: 2026-11-14. Subtask: P8.39 (`ledger_phase_8.md` §P8.39). Owner role:
Research engineer. Report trace: §19.1 (multi-domain co-simulation,
dynamic stability); §19.2(6). Inputs: P8.16 (`16_andes_integration.md` —
the ANDES worker and its `steady_state_power_flow` study type); P8.18
(`18_helics_cosim.md` — the research federation, its exit-report
discipline, and the P5.16 granted-time cross-check); P8.01 simulator
adapters; P8.05 (`05_06_harness_baselines.md` — the independent
validation harness that owns the proposal-vs-limits cross-checks);
P8.02 (`02_research_scope_freeze.md` — prohibitions X-5/X-8);
ENTRY_NOTE.md boundaries 2 and 4.

## 1. Status of this record — WRITTEN-UNRUN; every measured cell EMPTY

**Every cross-check result cell in this document is EMPTY.** Per the
phase boundaries (ENTRY_NOTE boundary 4), the ANDES runs, the
pandapower runs, and the HELICS federation runs are **human/lab gate
inputs**: ANDES and HELICS were **NEVER installed** in this repository's
history, no federation was formed, no power flow ran, and no cross-check
comparison was ever computed. No figure, tolerance value, delta, or
verdict from a real comparison appears anywhere below, and none was
computed by any code in this loop (nothing was installed, built, or
executed).

What exists and is verifiable to the extent the loop allows:

- the **cross-check DESIGN** (§2–§3) — what a cross-check establishes
  and what it does not, the pair list, and the comparison rules, all
  written **BEFORE any run** (X-8), with the tolerance value cells EMPTY
  until they are pre-registered by a dated amendment of §3.2;
- the **pure stdlib cross-check module**
  (`apps/worker/python/andes_worker/cosim_crosschecks.py`,
  `flexcore-cosim-crosscheck/1`) — ADDITIVE to the P8.16 worker (no
  existing worker logic modified; no new `import andes` anywhere): four
  checkers over INJECTED data, each pinned by HAND-COMPUTED self-tests
  with the arithmetic stated in the test. WRITTEN, NEVER EXECUTED;
- the **pytest mirror**
  (`apps/worker/python/andes_worker/tests/test_cosim_crosschecks.py`) —
  WRITTEN, NEVER RUN;
- the **filling procedure** (§7) and the **reviewer-verification
  procedure** (§8) — the exact commands the human/lab pass runs.

**The test-arithmetic / cross-check-result distinction (stated
explicitly):** the self-tests assert hand-derived values over
constructed inputs (e.g. schedule `[2.0, 3.0, -1.0]` MW vs dynamic
endpoints `[2.0, 3.05, -1.0]` MW → deltas `[0.0, 0.05, 0.0]` → max
0.05 MW within a pre-registered 0.1 MW; pandapower `[1.000, 0.980,
1.020]` pu vs ANDES PF `[1.000, 0.981, 1.020]` pu → max delta 0.001 pu).
That is **test-grade arithmetic pinning the checker definitions** — it
is NOT a cross-check measurement, NOT evidence about any case, and it
fills no cell below.

## 2. What a co-simulation cross-check establishes — and what it does NOT (the design, fixed BEFORE any run)

**The defining statement (verbatim, carried in every record the module
builds):**

> A cross-check between two model classes is AGREEMENT EVIDENCE ON THE
> OVERLAPPING DOMAIN ONLY — it is never a validation of either model
> against reality, never a safety case, and never a statement about the
> real grid (§19.1).

Consequences of that statement, applied to every pair below:

| A cross-check DOES establish | A cross-check does NOT establish |
|---|---|
| That two DECLARED models agree, within a pre-registered tolerance, on the quantities and samples their domains OVERLAP (the shared time samples; the shared bus/branch index) | Validation of either model against reality — no field measurement exists anywhere in this phase (simulation/shadow only) |
| That a disagreement beyond tolerance EXISTS and is EXPLAINED (model conventions, model scope, solver settings) — the explanation is recorded with the cell | That either model is "correct": agreement is evidence about the pair, never a correctness certificate; disagreement is evidence about the pair, never a bug verdict by itself |
| Transport-level facts about the HELICS coordinator (declared time steps, granted times, completion) | Any power-system result: HELICS is TRANSPORT, not physics (§19.1) — "the federation synchronized" says nothing about grid behaviour |
| Steady-state consistency between two power-flow tools on one case (X-5 discipline preserved) | Transient stability: a steady-state power flow (either tool) is NOT transient-stability evidence — that remains the ANDES TDS/eigenvalue track's separate question (X-5) |
| Agreement evidence under the pre-registered tolerance | Any threshold fitted after seeing a difference (X-8): a beyond-tolerance result is recorded AND explained, never absorbed by widening the tolerance |

**X-5 structural separation, enforced in code (not prose):** steady-state
and dynamic results are NEVER averaged or merged. In
`cosim_crosschecks.py` the merge is **unrepresentable**:

1. every compared series lives inside its own `series_blocks` entry that
   MUST declare `model_class` from the closed vocabulary
   `("steady_state", "dynamic")` (`CLASS_DECLARATION_REQUIRED` refusal
   otherwise);
2. the `comparison` block carries **DELTAS ONLY** — a `values`/`series`
   key inside it is refused (`COMPARISON_CARRIES_VALUES_REFUSED`);
3. any merged/averaged/blended-series key anywhere in a record is
   refused (`MERGED_SERIES_REFUSED` — `FORBIDDEN_MERGE_KEYS`);
4. `assert_structural_separation()` walks every assembled record and
   enforces 1–3 before the record can exist.

The endpoint pair (CC-4) therefore compares steady-state schedule
ENDPOINT values with dynamic-simulation END-OF-WINDOW values on the
identical shared time base — the overlapping domain — and nothing else:
the steady-state series says nothing about the dynamic trace between its
endpoints, and the dynamic trace says nothing about the steady state
between schedule intervals.

## 3. Cross-check pairs and comparison rules (frozen BEFORE any run; X-8)

### 3.1 The pairs

The comparison rule for each pair is stated NOW, before any run. The
case ids, tool versions, and per-run parameters are `RECORD-AT-RUN`
fields — never invented here.

| Pair | What is compared | Comparison rule (frozen now) | Tolerance value | Status |
|---|---|---|---|---|
| CC-1 — HELICS declared time-step consistency | Each federate's declared time step, from the federation config | ALL declared steps must be EQUAL; any mismatch is DETECTED and REPORTED with the mismatching federates named, and any interaction claim on the mismatched federation is REFUSED (`INTERACTION_CLAIM_REFUSED`) — never silently absorbed | n/a (exact equality — no tolerance exists for a configuration fact) | EMPTY — run pending |
| CC-2 — federate completion / dropout | Each federate's final granted time and granted-step count vs the scenario end | All final granted times must be EQUAL; a federate below the scenario end (or, without a configured end, below the latest federate) is recorded as a DROPOUT with the federate NAMED — never treated as "no change"; any detected condition refuses the interaction claim | n/a (exact equality on granted times) | EMPTY |
| CC-3 — ANDES power flow vs pandapower power flow | Bus-voltage magnitudes (pu) and branch flows (MW/MVAr) from the SAME case with the SAME inputs, run through both tools | Element-by-element on the IDENTICAL bus/branch index (same ids, SAME ORDER — a reordered or partial index list is refused, never silently aligned); verdict = max abs delta vs the pre-registered tolerance; a beyond-tolerance difference is recorded AND explained (model conventions, model scope), never absorbed | **EMPTY — pre-register in §3.2 before the first run** | EMPTY |
| CC-4 — quasi-static schedule endpoints vs dynamic simulation endpoints | The steady-state schedule's endpoint values (MW/MVAr) vs the ANDES TDS end-of-window values, per interval | Sample-by-sample on the IDENTICAL shared time base (mismatched grids refused, never interpolated); verdict = max abs delta vs the pre-registered tolerance; agreement asserted on the overlapping domain ONLY | **EMPTY — pre-register in §3.2 before the first run** | EMPTY |

**Pairs owned elsewhere (referenced, not duplicated):** the ledger's
broader cross-check examples — ADMM dispatch vs power-flow feasibility,
and MARL proposal vs physical limits — are implemented as the **P8.05
independent validation harness** (`05_06_harness_baselines.md` §2: AC
power flow feasibility, asset capability, SOC/ramp/duration, command
invariants, with its own deliberately-wrong test suite and separate
ownership). P8.39 does not duplicate them; every CC-3/CC-4 input that
originates from a research module's proposal carries that harness's
validation outcome with it (a proposal that failed P8.05 validation is
not a valid cross-check input).

### 3.2 Tolerance pre-registration (X-8) — the EMPTY registry

Per X-8 and the ledger's named risk ("a cross-check tolerance fitted
after seeing the difference"): the tolerance VALUES for CC-3 and CC-4
are **registered by a dated amendment to this section BEFORE the first
comparison is run**, on stated bases (the quantity's operational
significance, the tools' declared tolerances, the sampling grid), never
fitted after seeing a delta, never set as a universal target. The
module enforces this in code: a comparison with `tolerance=None` is
refused (`THRESHOLD_NOT_PRE_REGISTERED`); no default exists anywhere.

| Pair | Quantity | Pre-registered tolerance | Basis (stated at registration) | Registered |
|---|---|---|---|---|
| CC-3 | voltage_pu | EMPTY — REGISTER-BEFORE-FIRST-RUN | EMPTY | EMPTY (dated amendment pending) |
| CC-3 | p_mw | EMPTY — REGISTER-BEFORE-FIRST-RUN | EMPTY | EMPTY |
| CC-3 | q_mvar | EMPTY — REGISTER-BEFORE-FIRST-RUN | EMPTY | EMPTY |
| CC-4 | p_mw | EMPTY — REGISTER-BEFORE-FIRST-RUN | EMPTY | EMPTY |
| CC-4 | q_mvar | EMPTY — REGISTER-BEFORE-FIRST-RUN | EMPTY | EMPTY |

Sign discipline (§10.2): `P > 0` = export at the point of connection;
the sign convention is DECLARED per series and a comparison across
different declared conventions is refused (`SIGN_CONVENTION_MISMATCH`) —
reactive-power sign is declared per grid model and device profile,
never assumed.

## 4. Cross-check result tables — ALL MEASURED CELLS EMPTY

Rules for whoever fills these tables (recorded now, before any number):

- Cells come from the module's checker reports (fed with the run
  records per §7), never from memory, never estimated. An in-band
  refusal (`TIME_BASE_MISMATCH`, `INDEX_BASE_MISMATCH`,
  `THRESHOLD_NOT_PRE_REGISTERED`, `SIGN_CONVENTION_MISMATCH`,
  `INTERACTION_CLAIM_REFUSED`) is recorded AS THE REFUSAL in the cell —
  never as a blank and never as a number.
- Every filled row carries its tool versions and case reference — a
  number without its model/case scope is invalid and must not be
  entered (the P8.16/P8.18 discipline).
- A beyond-tolerance CC-3/CC-4 delta is recorded WITH its explanation
  in the same row; the tolerance is never widened to fit (X-8).
- Units are canonical: MW/MVAr/MWh, voltage pu (solver representation),
  time s (§10.2).

### 4.1 CC-1 — HELICS declared time-step consistency

| Federate (RECORD-AT-RUN) | Declared time step (s) | Config source | Mismatch detected? | Mismatching federates named | Interaction claim |
|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

**Honest scope note:** the checker here is the DETECTION LOGIC over the
injected declared configuration and exit reports. Whether the LIVE
HELICS federation itself detects/refuses a mid-run mismatch is exactly
the ledger's acceptance criterion and is **UNRUN** (helics was never
installed); the live confirmation belongs to the human/lab pass, which
feeds the recorded configs and exit reports into the checkers (§7) and
records what the federation itself did.

### 4.2 CC-2 — federate completion and dropout

| Federate (RECORD-AT-RUN) | Final granted time (s) | Granted steps | Scenario end (s) | Dropout detected? | Federate named | Interaction claim |
|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

### 4.3 CC-3 — ANDES power flow vs pandapower power flow (same case, same inputs)

| Case (RECORD-AT-RUN) | pandapower version | ANDES version | Quantity | Elements compared | Max abs delta | Pre-registered tolerance | Within tolerance? | Explanation if beyond | Case ref |
|---|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | voltage_pu | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | p_mw | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | EMPTY | EMPTY | q_mvar | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

### 4.4 CC-4 — quasi-static schedule endpoints vs dynamic simulation endpoints

| Case/interval set (RECORD-AT-RUN) | Schedule source (ref) | Dynamic run ref (case + ANDES version + model scope) | Quantity | Shared samples | Max abs delta | Pre-registered tolerance | Within tolerance? | Model/case ref |
|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

## 5. Model-class boundary statements (verbatim)

The three discipline statements this subtask was required to carry,
verbatim from the phase boundaries, plus the module's own:

1. **Steady-state PF is NOT transient stability:** "A steady-state
   power flow is NOT transient-stability evidence" (X-5; §19.1) — the
   CC-3 comparison is a steady-state consistency check only; it says
   nothing about frequency nadir, ROCOF, or transient response, and it
   never substitutes for the ANDES TDS/eigenvalue track (P8.17) or the
   distribution quasi-static feasibility pipeline (Phase 4).
2. **HELICS is TRANSPORT not physics:** "HELICS synchronizes federates;
   it is not itself the power-system physics model" (§19.1; P8.18 §5) —
   CC-1/CC-2 are statements about the coordinator's synchronization,
   never power-system results.
3. **Dynamic results structurally separate (X-5):** steady-state and
   dynamic results are never averaged or merged — enforced in code by
   `assert_structural_separation` (§2), with the comparison block
   carrying deltas only and every series living in its declared-class
   block.
4. **Cross-check = agreement evidence on the overlapping domain only**
   (§2's defining statement) — carried verbatim in every record the
   module builds (`BOUNDARY_STATEMENTS` in `cosim_crosschecks.py`).
5. **Proposals only (ADR-010):** cross-check records are research
   diagnostics; the record type carries no authorization token and no
   command field (`FORBIDDEN_RESULT_KEYS` scan,
   `PROPOSAL_ONLY_REFUSED`).

## 6. What the module computes vs what the human run supplies

| Supplied by | Content |
|---|---|
| `cosim_crosschecks.py` (written, never run) | the four checkers over INJECTED data with the refusal discipline (time-base/index exactness, sign-convention declaration, pre-registered tolerance); the in-code X-5 structural separation and the ADR-010 proposal-only scan; the record assembler carrying the boundary statements verbatim; hand-computed self-tests pinning every definition |
| The human/lab run | the federation runs (per P8.18's procedure — helics installed only after gate G-6 and the P8.36 advisory check); the ANDES `steady_state_power_flow` and TDS runs (per P8.16's procedure — gate G-4); the pandapower PF runs (Phase 5 pipeline) on the SAME case; the case alignment that produces the identical index/time bases (§8.2 gap); the §3.2 tolerance pre-registration (dated amendment BEFORE the first comparison); the fills of §4 |

## 7. Exact filling commands (the human/lab gate inputs)

Recorded verbatim; all from the repository root, in the isolated
research environment (ANDES/HELICS installed per the P8.16/P8.18
procedures, AFTER gates G-4/G-6 and the P8.36 advisory check):

```sh
# 0. The pure self-tests (no ANDES/HELICS/pandapower needed):
python apps/worker/python/andes_worker/cosim_crosschecks.py
#    expected: every hand-computed self-test PASSes (endpoint deltas
#    [0.0, 0.05, 0.0] MW → max 0.05 vs 0.1; vector deltas [0.0, 0.001,
#    0.0] pu → max 0.001 vs 0.005; time-step mismatch {1.0, 2.0} →
#    flexcore_grid named; dropout at 1800.0 s of 3600.0 s named; the
#    refusal suite).

# 1. The pytest mirror:
PYTHONPATH=apps/worker/python/andes_worker python -m pytest \
  apps/worker/python/andes_worker/tests -v

# 2. The import-graph discipline (P8.16 §6(a) must be UNCHANGED — this
#    module adds NO new andes import):
grep -rn "import andes" apps crates python --include=*.py --include=*.rs
#    expected: exactly one hit, inside andes_dynamic_worker.py

# 3. Documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# 4. FIRST RUN — the fills of §3.2 and §4 (human/lab inputs):
#    a. Register the §3.2 tolerances by a DATED AMENDMENT of this record
#       BEFORE any comparison (X-8);
#    b. CC-1/CC-2: run the P8.18 federation per its recorded procedure;
#       feed the declared federation config into federate_time_step_check
#       and each federate's exit report into granted_time_check (with the
#       configured scenario end); record the reports in §4.1/§4.2 — and
#       record what the LIVE federation itself did with the mismatch/
#       dropout cases (the ledger's acceptance criterion);
#    c. CC-3: run the ANDES steady_state_power_flow study (P8.16 worker)
#       and the pandapower power flow (Phase 5 pipeline) on the SAME case
#       with the SAME inputs; align the bus/branch indices EXPLICITLY
#       (recorded in the case ref column — never silently aligned);
#       inject the two vectors per quantity into vector_agreement with
#       the registered tolerance; record §4.3 including the explanation
#       for any beyond-tolerance delta;
#    d. CC-4: inject the schedule endpoints and the ANDES TDS
#       end-of-window values into endpoint_agreement (identical time
#       base; registered tolerance); record §4.4;
#    e. independent re-run of the mismatch case from the recorded inputs
#       (the ledger's verification method) and confirm the checker
#       reports the same detected condition.
```

## 8. Reviewer-verification procedure

```sh
# (a) Re-run steps 0-3 of §7 and confirm the outputs match §1's claims.
# (b) Hand-verify ONE checker case independently of the module (the
#     ledger's verification method): recompute the time-step mismatch
#     detection and one endpoint delta BY HAND from the constructed
#     inputs in the module's self-tests and confirm the module's figures.
# (c) Inspect the X-5 enforcement: read assert_structural_separation and
#     confirm every series block requires a declared model class, the
#     comparison block refuses values, and the merge keys are refused.
# (d) Confirm the proposal-only scan (ADR-010) refuses an
#     authorization-shaped key on the assembled record.
# (e) Confirm no measured cell of §4 was filled without the §7
#     procedure, and that every beyond-tolerance cell carries its
#     explanation and cites its case ref.
```

## 9. Contract gaps → Phase 5/7 follow-ups (recorded, not invented)

1. **No shared case-exchange format between the ANDES worker and the
   Phase 5 pandapower pipeline.** CC-3 requires the SAME case expressed
   in both tools, with bus/branch ids that align into the identical
   index base `vector_agreement` demands. No such common case manifest
   exists; producing the alignment (explicit, recorded, never silent) is
   the recorded follow-up for the Phase 5/worker owners — never invented
   here, because a silently "aligned" index list would be exactly the
   fabrication the module refuses (`INDEX_BASE_MISMATCH`). Until the
   manifest exists, CC-3 is blocked.
2. **Live-federation mismatch/dropout behaviour is UNRUN (gate G-6 +
   the helics pin carried UNVERIFIED):** the ledger's acceptance
   criterion says the FEDERATION detects and reports the mismatch (or
   refuses to run). The checkers here implement the detection logic over
   the injected records; whether HELICS itself detects/refuses mid-run
   can only be recorded at the first live run (P8.18's procedure). No
   behaviour is claimed for the uninstalled tool.
3. **The P5.16 orchestrator research-role seam (carried from P8.18
   §6.1):** the exit reports `granted_time_check` consumes come from
   federate runs; the `research_controller` role is still not validable
   by `apps/sim-runner/src/helics_orchestration.rs` — the additive
   role-variant change remains the orchestrator owner's recorded
   follow-up.
4. **Message-level time-sync audit (carried from P8.18 §6.3):**
   `granted_time_check` inspects final granted times and step counts
   (the P5.16 recorded scheme); a deeper per-message timestamp audit
   remains a future task, not claimed here.
5. **Tolerance-registry ownership clarification (P8.38 boundary):**
   P8.38 owns the RERUN reproduction tolerances per module; the CC-3/CC-4
   AGREEMENT tolerances registered in §3.2 are a separate registry owned
   by this record's dated amendment. Recorded here to prevent the two
   registries being conflated (neither substitutes for the other).
6. **ANDES case candidates remain recorded ASSUMPTIONS (P8.16 §3):**
   the CC-3/CC-4 case ids are `RECORD-AT-RUN`; that ANDES ships the
   named sample cases was never verified in this loop.

## 10. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Time-step mismatch detected/reported | Detection logic WRITTEN with hand-computed self-tests (mismatch detected, federates named, interaction claim refused); the LIVE federation confirmation **UNRUN** — the human/lab gate (§4.1 note) |
| ANDES vs pandapower comparison recorded within (or explained beyond) tolerance | Checker WRITTEN (identical index base enforced; sign convention declared; tolerance pre-registered or refused); the comparison **UNRUN** — tolerance cells EMPTY until the §3.2 amendment; no figure exists |
| Federate dropout detected | Detection WRITTEN (dropout named, never "no change"); live confirmation **UNRUN** |
| Verification method (independent re-run of the mismatch case) | Procedure recorded (§7 step 4e, §8b); **UNRUN** |

## 11. Non-claims (restated)

No validation of any model against reality — agreement evidence on the
overlapping domain only; no safety, certification, or protection claim;
no transient-stability claim from any steady-state comparison (X-5); no
power-system result from any HELICS transport check; no merged or
averaged cross-class figure (X-5, enforced in code); no tolerance fitted
after seeing a difference (X-8, refused in code); no field authority
(proposals only, ADR-010); no figure from a real run exists anywhere in
this record or its code — the self-tests' toy arithmetic pins the
checker definitions and is not a measurement. WRITTEN-UNRUN throughout.
