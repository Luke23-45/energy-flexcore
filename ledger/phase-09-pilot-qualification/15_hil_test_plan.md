<!-- P9.15 — Hardware-in-the-loop test plan and execution (decision record). PLAN + EXECUTION TEMPLATE ONLY: every run/result cell EMPTY; rig NOT PROCURED; runs UNSCHEDULED. No HIL rig exists anywhere in this effort; no platform has been selected; no real-time performance figure is claimed anywhere in this record. -->
# P9.15 — Hardware-in-the-Loop Test Plan and Execution (Plan + EMPTY Execution Template)

Date: 2026-11-14. Subtask: P9.15 (`ledger_phase_9.md` §P9.15).
Owner role: Power systems engineering (with the HIL lab / vendor partner).
Report trace: §23 Stage 9; §19.1 (HIL track — "Does command/control
integration behave correctly with real controller hardware?"); §19.2(10)
(HIL/commissioning tests run ONLY after simulation, integration, and
cyber reviews pass); §11.3 (conformance testing under IEEE 1547.1
remains a separate responsibility); RESEARCH.md §4 (HIL practice,
platforms, PHIL interface-algorithm requirement, same-profiles-and-seeds
validation pathway).
Inputs: P9.14 lab results (**NOT EXECUTED**), P9.06 D6 closure
(**OPEN**), P9.07 assessment with no open must-fix (**not written in
this wave**), HIL platform access (**NOT PROCURED**), the pilot grid
model (P9.03 — **template**), the pilot adapter set (P9.04 —
**template**), pilot equipment/firmware identities (P9.01 freeze —
**UNSIGNED**). None of the inputs exists.

## 1. Honest status — the plain statement

**No HIL rig exists, none is procured, and no HIL run is scheduled.**
No real-time target, no amplifier, no DUT equipment, no lab access, and
no pilot equipment identity exist; the ledger's own prerequisites (P9.14
results, D6 closure, P9.07 clean assessment) are all unmet. This record
delivers what an agent can deliver under ENTRY_NOTE boundary 6:

- the **HIL rig requirements** (§3 — real-time target, amplifier/
  interface for PHIL, the DUT boundary, calibration/validation
  evidence), as requirements a procurement must satisfy — not a
  selection (no platform is selected; the platform-SELECTION cell is
  EMPTY);
- the **test matrix** (§5) — the IEEE 1547.1-2020-derived cases where
  they apply plus the FlexCore-specific command-lifecycle cases, with
  the delay/loss/skew injections aligned to the P8.24 fault vocabulary
  BY CITATION and the P9.36 attack cases;
- the **§19.1–§19.2 trace requirements** that make this record usable
  by P9.40 (§6 — case IDs, disturbance profiles, and seeds designed for
  cross-stage reuse);
- the **execution record as an EMPTY template** (§7), with pre-gate
  evidence rows that are all UNMET at writing;
- the honest claim boundary (§8): **no real-time performance claim** of
  any kind is made or may be made from this record; HIL evidence does
  not certify equipment (§11.3).

Nothing was simulated, run, procured, or scheduled. Every measured cell
in §7 is deliberately EMPTY (UNRUN — human gate input).

## 2. Scope decision — CHIL vs PHIL (the ledger's step 1; SELECTION EMPTY)

The ledger requires the plan to state which HIL class applies and why,
and forbids claiming PHIL evidence from a CHIL-only run. The distinction
(RESEARCH.md §4): **CHIL** puts the real CONTROLLER (the edge agent /
site EMS controller) against a simulated grid over signal-level I/O;
**PHIL** puts real POWER EQUIPMENT (e.g. an inverter) in the loop via a
power amplifier and an interface algorithm with a stability condition.

| Question | Recorded position |
|---|---|
| What does the pilot's §19.1 question need? | "Does command/control integration behave correctly with real controller hardware?" — i.e. the FlexCore supervisory integration chain (edge agent → adapter → controller → equipment response) with REAL CONTROLLER HARDWARE |
| CHIL applicability | Applies whenever the DUT is the controller/EMS/edge hardware. Minimum required scope for this subtask |
| PHIL applicability | Required ONLY if a case's evidence claim is about real POWER equipment behaviour (e.g. ride-through of the actual inverter). **Selection cell: `<EMPTY — decided by the human power-systems owner once the pilot equipment and the lab capability are known>`** |
| Anti-conflation rule (ledger step 1, verbatim discipline) | A CHIL-only run is recorded as CHIL-only; NO PHIL evidence claim may be derived from it. §7's per-case rows carry a CHIL/PHIL applicability column so the evidence class is per-case, not per-report |

## 3. HIL rig requirements (what a procurement must satisfy; nothing procured)

| # | Requirement | Detail | Status |
|---|---|---|---|
| R1 | Real-time target | A real-time simulator platform of the class used for this work — the RESEARCH.md §4 verified examples: OPAL-RT (ePHASORSIM/HYPERSIM, RT-LAB), RTDS/RSCAD (FPGA-based), Typhoon HIL — capable of running the pilot grid model (P9.03) in real time at a recorded time step | **NOT PROCURED — no platform selected; selection cell EMPTY** |
| R2 | Amplifier / interface (PHIL only, if selected) | A power amplifier between the real-time simulation and the DUT equipment, with the interface algorithm RECORDED (e.g. Ideal Transformer Method per RESEARCH.md §4) and its stability condition stated for the planned operating range | **NOT PROCURED — N/A unless PHIL selected** |
| R3 | DUT boundary (explicit, written) | Exactly which hardware is IN the loop (edge gateway hardware? site EMS controller? inverter?) and which parts remain simulated. The DUT boundary is drawn so that §17.5 invariant 10 holds by construction: site-local protection and safety stay OUTSIDE FlexCore's test boundary with their own local controllers — FlexCore never substitutes for them | Boundary cell per case: `<EMPTY>` |
| R4 | Signal interface to the DUT | The physical/signal interface (I/O, communication protocol at the pilot's adapter set per P9.04) by which the real controller talks to the simulated grid; must exercise the SAME adapter code path the field uses | `<EMPTY>` |
| R5 | Rig calibration/validation evidence | The platform's last calibration/validation date and record, filed BEFORE the first case. **A run on an unvalidated rig is not HIL evidence** (ledger step 2, binding) | `<EMPTY>` |
| R6 | Time-step and latency disclosure | The rig's time step and the measured transport latency of the DUT interface, recorded per run — used to interpret results, never to claim real-time performance (§8) | `<EMPTY>` |
| R7 | Waveform/capture retention | Per-case raw captures retained and referenced from the run record (the reviewer's spot-check input, ledger verification method) | `<EMPTY>` |
| R8 | Version pinning | DUT hardware/firmware versions, rig software versions, edge-agent build (git SHA), grid-model version — all pinned in the run record BEFORE the run | `<EMPTY>` |

**No rig exists.** R1–R8 are requirements for the human procurement; the
procurement decision itself is a human/organizational gate outside this
record (ENTRY_NOTE boundary 1).

## 4. Pre-gates (§19.2(10)) — ALL UNMET

§19.2 item 10 forbids HIL/commissioning tests until simulation,
integration, and cyber reviews pass. The gate evidence table (ledger
step 4); every row UNMET at writing:

| Gate | Evidence required | Status |
|---|---|---|
| Simulation reviews pass | §22.2 scenario-regression suite re-run on the pilot configuration (P9.41) with failures dispositioned | **UNMET — P9.41 UNRUN** |
| Integration reviews pass | P9.14 lab integration report: read-only end-to-end + command path demonstrably disabled + fault subset recorded | **UNMET — P9.14 NOT EXECUTED** |
| Cyber reviews pass | P9.07 assessment with no open must-fix findings; D6 closed (P9.06 signed) | **UNMET — P9.07 not written; D6 OPEN** |
| Order rule | The gates are evidenced BEFORE the first HIL case runs; the evidence references are filled into §7's pre-gate block at execution | **UNMET** |

An HIL run before these gates pass is the ledger's named failure mode
("HIL run before the cyber review passes — §19.2 forbids this"); the run
template (§7) captures the gate references first, and any UNMET gate
aborts scheduling.

## 5. Test matrix (cases defined now; runs UNSCHEDULED)

Case IDs are assigned now so P9.40 can trace them (§6). Expected-
behaviour cells are DESIGN statements from the cited sources — they are
not results.

### Part A — FlexCore-specific command/control integration cases (the §19.1 question)

| Case ID | Case | Code/vocabulary basis (cited) | Expected behaviour (design-derived, UNRUN) | CHIL/PHIL |
|---|---|---|---|---|
| H1 | Approved command delivery end-to-end | P9.14 part B path; `command_forward.rs` (`CommandForwardingEngine::forward_verified`, separate allowlist + policy re-checks) | An operator-approved command in the (lab-approved-for-test) enabled configuration reaches the DUT controller via the SAME adapter path, after ALL verification gates pass | CHIL |
| H2 | Acknowledgement | §14 lifecycle (`crates/flexcore-domain/src/command_state.rs`: ISSUED → ACKNOWLEDGED edge 13) | DUT ack returns through the uplink and advances the lifecycle exactly along the closed transition table; no shortcut state reachable | CHIL |
| H3 | Command expiry | §14.3; `command_verify.rs` verify step 2 (injected clock; clock-quality fail-closed) | A command whose validity window elapses before delivery is refused `expiry` at the edge; the DUT never sees it | CHIL |
| H4 | Supersede | §14 lifecycle SUPERSEDED transitions (`command_state.rs` closed table) | A superseded command never delivers; the superseding command follows H1's path | CHIL |
| H5 | Fallback activation | §14.4 fallback states; `dmpc.rs` `FallbackOutcome` discipline (P8.24 F6: budget exhausted → recorded fallback, never a fabricated result) | Fallback activates per the recorded fallback policy and the fallback outcome is recorded; no silent schedule substitution | CHIL |
| H6 | Communication loss between FlexCore and the DUT | P8.24 F8 vocabulary (communication loss; abandoned-participant discipline) | Loss is visible as staleness/failure per the recorded policy; site-local control (outside FlexCore) continues unaffected (§21.2, §17.5 invariant 10) | CHIL |
| H7 | Rejection set at the DUT boundary | P9.36 attack cases — the CLOSED `PilotAttackCase` set (`crates/edge-agent/src/pilot_rejection_proof.rs`: `forged_signature`, `replayed_captured`, `expired_window`, `out_of_sequence`, `conflicting_idempotency_payload`) | Each attack case injected toward the DUT chain is refused with the named rejection code (`signature`/`duplicate`/`expiry`/`sequence`/`idempotency_conflict`); ZERO reach to the DUT; sequence checkpoint unchanged | CHIL |

### Part B — delay/loss/skew injections (aligned with the P8.24 fault vocabulary BY CITATION)

P8.24 (`ledger/phase-08-advanced-coordination/24_failure_drill.md` §2)
recorded the fault set F1–F8 with per-module expected fail-closed
behaviour; the SAME vocabulary applies here at the HIL interface. The
injection parameters (delay magnitude, loss rate, skew offsets) are
RECORDED PER CASE at execution — no number is invented here; each case
row carries an EMPTY parameter cell.

| Case ID | Fault (P8.24 vocabulary, cited) | Injection at the HIL seam | Expected fail-closed behaviour (design-derived, UNRUN) | Injection parameters |
|---|---|---|---|---|
| H8 | F1 dropped messages | Drop telemetry/command frames on the DUT interface | Missing handled per quality states (§10.4) — MISSING/STALE surfaced, never a silent wrong value | `<EMPTY>` |
| H9 | F1 duplicated messages | Replay frames | Duplicate detected/handled per dedup and sequence rules; no double-application at the DUT | `<EMPTY>` |
| H10 | F2 reordered messages (P8.24 C-1 inherited where the seam is call-return) | Reorder delivery order | Refused/recorded per seam; never silently accepted as in-order | `<EMPTY>` |
| H11 | F3 clock skew | Skew the clock source per `clock_skew_future`/`clock_skew_past` cases (`conformance.rs`) | Expiry checks fail CLOSED on any non-`Synchronized` clock; rejections audited | `<EMPTY>` |
| H12 | F4 stale telemetry | Serve stale readings to the DUT chain | STALE state carried; downstream sees the degraded state; no upgrade | `<EMPTY>` |
| H13 | F5 unavailable asset / partial fleet response | Remove an asset from the simulated fleet mid-run | Refusal/recorded-abandonment per the recorded policy; no fabricated values for the absent asset | `<EMPTY>` |
| H14 | F6 solver timeout | Exhaust the scheduling budget | `NON_CONVERGED_BUDGET_EXHAUSTED`-class recorded failure; fallback per H5; no invented schedule | `<EMPTY>` |
| H15 | F7 worker/plugin crash | Crash a worker process at the composition root (P8.24 C-2: real-process kills are composition-root runs) | Worker loss isolated; no partial envelope; control plane survives and records the failure | `<EMPTY>` |
| H16 | F8 communication loss (participant killed mid-iteration) | Kill the link/participant mid-run | Abandonment recorded with participant and iteration named; NO proposal/schedule from an abandoned run | `<EMPTY>` |

### Part C — IEEE 1547.1-2020 conformance-test-procedure cases WHERE THEY APPLY (ledger step 3)

The ledger requires these cases "where they apply" — applicability
depends on the pilot equipment (P9.01/P9.04: EMPTY) and on whether PHIL
is selected (§2). The case list follows the verified RESEARCH.md §4
material (IEEE 1547.1-2020 procedures; the Sandia/SIRFN automated PHIL
practice; SunSpec SVP automation where the lab supports it). **HIL
execution of these procedures supports FlexCore's integration claim; it
does NOT certify the equipment** (§11.3; §8 below).

| Case ID | 1547.1-2020 procedure family | Applies when | Applicable? | Run parameters |
|---|---|---|---|---|
| H17 | Volt-var | The pilot inverter/EMS exposes a volt-var function in its P9.04 capability profile | `<EMPTY>` | `<EMPTY>` |
| H18 | Volt-watt | Same condition | `<EMPTY>` | `<EMPTY>` |
| H19 | Constant power factor | Same condition | `<EMPTY>` | `<EMPTY>` |
| H20 | Ride-through (voltage/frequency) | Same condition; PHIL is the established method for ride-through against real inverters (RESEARCH.md §4) | `<EMPTY>` | `<EMPTY>` |
| H21 | Frequency response | Same condition | `<EMPTY>` | `<EMPTY>` |

Anti-conflation rule (restated): a case marked run under CHIL-only is
recorded as CHIL-only; no PHIL claim may be derived from it; an
applicability cell left `<EMPTY>` keeps the case NOT-APPLICABLE-PENDING,
never silently passed.

## 6. §19.1–§19.2 trace requirements (feeds P9.40)

P9.40 builds the map FAT case → HIL case(s) → lab case(s) → simulation
case(s) with the disturbance profile and seed used at each stage
(RESEARCH.md §4: the same disturbance profiles and seeds at each stage
so evidence is comparable). To make that possible, THIS record binds
every case to its cross-stage identity NOW:

| Trace element | Rule recorded now |
|---|---|
| Case identity | The H-case IDs above are stable; P9.40 references them, never renumbers |
| Profile identity | Each Part B injection names its P8.24 fault row (F1–F8); the lab counterparts are the P9.14 Part C cases (F11–F18); the simulation-stage counterparts are the P8.24 drill cells (F1–F8 per module). The SAME fault-vocabulary row must be used across stages; where a stage could not reuse the profile (P8.24 C-1/C-2 inheritance), the exception is recorded per §7's per-case notes — never papered over |
| Seed/config identity | Any seeded input is pinned in the §7 configuration record and reused verbatim across stages where the stage supports it; a seed that cannot cross a stage boundary (e.g. real-time rig vs deterministic sim) is recorded as an exception with its reason |
| Mapping input for P9.40 | P9.40 fills its map FROM the filled §7 records; this record pre-registers only the identity scheme above. No FAT case exists yet (P9.16 UNRUN) — no mapping is claimed here |

## 7. Execution record — EMPTY template (the human gate input)

**EVERY CELL EMPTY. No case has run; no pass, fail, or
not-applicable exists. Filling any cell from anything other than the
actual run output of the actual rig/DUT versions is fabrication
(AGENTS.md rule 1).**

### 7.1 Pre-gate evidence block (filled BEFORE the first case)

| Gate | Evidence reference | Status at execution |
|---|---|---|
| P9.41 regression re-run | | (EMPTY — UNMET) |
| P9.14 lab report | | (EMPTY — UNMET) |
| P9.07 assessment (no open must-fix) | | (EMPTY — UNMET) |
| P9.06 signed D6 closure | | (EMPTY — UNMET) |

### 7.2 Rig and configuration record (filled BEFORE the run)

| Field | Value (EMPTY — fill at execution) |
|---|---|
| HIL scope selected (CHIL / CHIL+PHIL) with the §2 rationale | |
| Platform (make/model) + time step | |
| Amplifier + interface algorithm + stability condition (PHIL only) | |
| Rig calibration/validation date + record reference (R5) | |
| DUT identity: hardware, firmware (INSTALLED, fleet-confirmed) | |
| DUT boundary statement (R3 — what is real, what is simulated) | |
| Edge-agent build (git SHA) / control-plane version / grid-model version / adapter profile version | |
| Signal interface description (R4) | |
| Seeds/config identifiers (§6) | |
| Run dates / executors / power-systems reviewer (NOT the runner) | **UNSIGNED** |

### 7.3 Per-case result table

| Case ID | CHIL/PHIL class run | Result (PASS / FAIL / NOT-APPLICABLE-with-reason) | Observed behaviour (from the actual output) | Equipment/firmware version under test | Capture reference (R7) |
|---|---|---|---|---|---|
| H1 | (EMPTY — UNRUN) | | | | |
| H2 | (EMPTY — UNRUN) | | | | |
| H3 | (EMPTY — UNRUN) | | | | |
| H4 | (EMPTY — UNRUN) | | | | |
| H5 | (EMPTY — UNRUN) | | | | |
| H6 | (EMPTY — UNRUN) | | | | |
| H7 | (EMPTY — UNRUN) | | | | |
| H8 | (EMPTY — UNRUN) | | | | |
| H9 | (EMPTY — UNRUN) | | | | |
| H10 | (EMPTY — UNRUN) | | | | |
| H11 | (EMPTY — UNRUN) | | | | |
| H12 | (EMPTY — UNRUN) | | | | |
| H13 | (EMPTY — UNRUN) | | | | |
| H14 | (EMPTY — UNRUN) | | | | |
| H15 | (EMPTY — UNRUN) | | | | |
| H16 | (EMPTY — UNRUN) | | | | |
| H17 | (EMPTY — UNRUN) | | | | |
| H18 | (EMPTY — UNRUN) | | | | |
| H19 | (EMPTY — UNRUN) | | | | |
| H20 | (EMPTY — UNRUN) | | | | |
| H21 | (EMPTY — UNRUN) | | | | |

**Per-case pass/fail semantics (fixed BEFORE the run):** PASS = observed
behaviour matches the case's expected behaviour, on the recorded
equipment/firmware version, with a retained capture (R7). A case marked
passed on equipment/firmware differing from the pilot's is INVALID
(ledger acceptance criterion (c)) — the version column is audited row by
row. NOT-APPLICABLE requires a written reason (e.g. PHIL not selected ⇒
H20 not-applicable-with-reason), never silence.

### 7.4 Failures-and-dispositions table

| Failed case | Disposition (fixed-and-re-run / accepted limitation filed to P9.27 / blocks the pilot) | Evidence | Status |
|---|---|---|---|
| (none — exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

The ledger's failure-handling rule: every failure gets one of the three
dispositions; a failure without a disposition blocks P9.17/P9.28.

## 8. What a HIL run would and would not establish

**A completed HIL run would establish ONLY:** that the named DUT
hardware/firmware, against the named rig and pilot configuration, behaved
as the §5 matrix expects for the injected cases on the recorded dates,
with retained captures — i.e. §19.1's question ("does command/control
integration behave correctly with real controller hardware?") answered
for THAT hardware and THAT rig, per case.

**It would NOT establish** (none of these may be written about it):

1. **Any real-time performance claim.** The rig's time step (R6) is
   disclosed for interpretation; no latency, throughput, or real-time
   capability figure for FlexCore is claimed or derivable from this
   record. Performance measurement at pilot scale is P9.39's deliverable.
2. **Equipment certification or conformance.** IEEE 1547.1-2020
   conformance testing remains a separate responsibility (report §11.3;
   RESEARCH.md §4: "HIL evidence supports FlexCore's integration claim;
   it does not certify the equipment"). No "1547.1-compliant" wording may
   appear about the pilot equipment because of a P9.15 row.
3. **Safety or protection-grade properties** (AGENTS.md §3 rules 2–3):
   site-local protection stays outside the DUT boundary (R3); HIL proves
   supervisory integration behaviour, never grid safety.
4. **PHIL-grade evidence from a CHIL-only run** (§2 anti-conflation rule,
   per-case enforced by §7.3's class column).
5. **Field behaviour.** The field path is P9.13 (commissioning) + P9.17
   (field acceptance, shadow first); HIL de-risks it (P9.40's map), it
   does not replace it.
6. **Evidence from firmware not under test.** A firmware bump between run
   and pilot invalidates the affected rows (ledger risk row; §7.3 audit).

## 9. Reviewer-verification procedure

1. Confirm §1's plain statement is unchanged: NOT PROCURED / UNSCHEDULED /
   UNRUN everywhere; no measurement, date, or signature exists.
2. Confirm every §4 gate row reads UNMET, every §5 applicability/parameter
   cell is `<EMPTY>`, and every §7 cell is EMPTY / UNRUN / UNSIGNED.
3. Confirm §2's anti-conflation rule and §8's claim boundary are intact
   (no PHIL claim from CHIL; no certification wording; no real-time
   performance claim).
4. Traceability check (for a filled future record): (a) the pre-gate
   references resolve and predate the first case; (b) the rig
   calibration/validation record is filed (R5) — a run on an unvalidated
   rig voids the record; (c) every case row carries the equipment/
   firmware version actually under test and a capture reference; (d) the
   independent reviewer is a power-systems reviewer who did not run the
   tests, and at least one case is spot-checked by re-running it or
   inspecting the raw capture (the ledger's verification method).
5. Confirm §6's identity scheme is what P9.40 consumes (stable IDs; same
   fault-vocabulary rows across stages; seed reuse with recorded
   exceptions).
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the plan and template are written; no rig exists.

## 10. Filling commands (for the human executor)

```sh
# (a) Documented-tools gate (before and after filling):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill order: close the §4 pre-gates (P9.41 → P9.14 → P9.07 → signed
#     P9.06 D6) and record their references in §7.1; procure/select the
#     rig (§2/§3 — human gate) and file the calibration record (R5);
#     pin DUT/edge/grid-model versions into §7.2 BEFORE the run.

# (c) Execute Part A → Part B → Part C in order (a failed Part A case
#     stops the run until dispositioned). Fill §7.3 from the ACTUAL run
#     output the same day; retain captures per R7; disposition failures
#     in §7.4.

# (d) Independent review: a power-systems reviewer who did not run the
#     tests re-runs or capture-inspects at least one case and signs §7.2.

# (e) Claim-phrase check on the filled report (§8 discipline):
grep -n "certified\|compliant\|conformant\|real-time performance" ledger/phase-09-pilot-qualification/15_hil_test_plan.md
#     The only permitted occurrences are inside the quoted boundary statements.
```

No procurement, simulation, rig connection, or test execution happened
in this writing loop (banned; ENTRY_NOTE boundary 1); every execution
cell stays EMPTY until people at a real rig fill it.

## 11. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No HIL rig exists or is procured; lab/vendor access unestablished — P9.15 is blocked behind P9.14 (itself blocked behind a nonexistent lab environment) | §1, §4 |
| HIL run before the cyber review passes (§19.2 forbids it) — prevented structurally by the §4 gate table, enforcement is human | §4; ledger risk row |
| PHIL interface instability mistaken for equipment failure — the interface algorithm and its stability condition must be recorded per case (R2) before any PHIL result is interpreted | §3 R2; ledger risk row |
| A test passed on lab firmware differing from field firmware silently invalidates the affected rows — the §7.3 version column audit is the control | §7.3; ledger risk row |
| Rig unvalidated or calibration lapsed — a run on an unvalidated rig is not HIL evidence; the R5 filing is a hard precondition | §3 R5; ledger step 2 |
| HIL evidence misread as equipment certification or as a real-time performance claim — prevented by the §8 boundary; conformance testing under IEEE 1547.1 remains a separate responsibility (§11.3) | §8 |
| P9.40 map risk inherited: profiles/seeds reused by name while parameters differ between stages — §6 binds identity now; cross-stage parameter equality is verified only when P9.40 fills its map | §6 |
| Pilot equipment/adapter set undefined (P9.01/P9.04 templates) — the Part C applicability cells cannot be set; the matrix risks over- or under-scoping at execution | §5 Part C |

Status: **WRITTEN-UNRUN** — a HIL plan cannot be "run" by an agent; no
rig, no DUT, no lab access, and no pilot equipment identity exist; every
execution cell is EMPTY and stays EMPTY until the human execution fills
it from actual run output. Nothing was procured, scheduled, or executed,
and no real-time performance or certification claim is made.
