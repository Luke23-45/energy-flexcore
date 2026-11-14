<!-- P9.16 — Field acceptance test plan (decision record). PLAN ONLY: the FAT case catalogue, pre-registered structural pass/fail criteria, roles, failure process, and re-run set are written BEFORE any run; every per-site value, numeric threshold, date, and sign-off cell is EMPTY/UNSIGNED. No site exists (P9.01 freeze UNSIGNED); no HIL result exists (P9.15 UNRUN); no customer acceptance criterion has been received. FAT EXECUTION (P9.17) is a human gate — this plan never executes anything. -->
# P9.16 — Field Acceptance Test Plan (FAT/SAT Plan — Pre-Registered; Execution Cells EMPTY)

Date: 2026-11-14. Subtask: P9.16 (`ledger_phase_9.md` §P9.16).
Owner role: Integration engineering (with product and the customer).
Report trace: §23 Stage 9 (exit gate: customer/site and applicable
authorities approve the deployment and its control scope); §19.2 (the
test-layer ladder; item 10 — HIL/commissioning tests run ONLY after
simulation, integration, and cyber reviews pass); §22.3 (release gates;
a production field pilot additionally requires site owner approval,
device/vendor integration tests, local safety review, commissioning, and
any applicable market/interconnection qualification).
Inputs: P9.01 pilot scope (**UNSIGNED — no site exists**), P9.13
commissioning plan (TEMPLATE — every execution cell EMPTY), P9.15 HIL
results (**UNRUN — no rig exists**), P9.11 market/program requirements
(record, no program joined), customer acceptance criteria (**DO NOT
EXIST — no customer is named**). **None of the inputs carries real site
data.**

## 1. Honest status — the plain statement

**No field acceptance test has been planned for a date, executed,
witnessed, or signed, and none is claimed.** No pilot site exists, no
customer acceptance criterion has been received, no HIL result exists to
trace against, and no site has completed commissioning (P9.13 —
UNSCHEDULED). This record delivers what an agent can deliver under
ENTRY_NOTE.md boundary 1:

- the **FAT case catalogue** (§6) — the acceptance cases the customer and
  the applicable authorities will use to judge the pilot, defined BEFORE
  any run, each with its procedure, input/starting state, expected
  result, pass/fail criterion, evidence artifact, and its named HIL
  coverage per `40_hil_field_traceability.md`;
- the **pre-registered threshold discipline** (§6.2): structural
  pass/fail criteria are fixed NOW (they are design-derived, not
  measured); every NUMERIC threshold is registered in the dated
  amendment block BEFORE the first run (X-8 — `39_performance_load_test.md`
  §5 discipline applied here by name) — a threshold set after a result
  was known is INVALID;
- the **traceability rule** (§3): each case names its HIL coverage per
  the 40_ record's rules; a case with no upstream coverage is a recorded
  gap (`UNMAPPED-GAP-OPEN`), never waved through;
- the **acceptance-test environment state** (§4): control-authority mode
  per §21.4, frozen versions, and the version-change/re-run rules stated
  IN ADVANCE;
- the **roles, signatures, and failure process** (§5, §7);
- the **§19.2/§22.3 evidence requirements** (§8).

The plan is a REVIEWED-BASELINE DRAFT until the customer signs §9
(acceptance criterion (d) — NOT MET and NOT CLAIMED). Nothing here runs
a test, grants field write authority, or approves a control scope.

## 2. What the FAT is — and what it is not (scope ceiling)

- The FAT verifies, AT A REAL SITE, that the commissioned FlexCore
  deployment behaves as designed in **read-only/shadow mode first**
  (§21.4) and only then — after shadow acceptance and a P9.26 promotion
  request — in operator-approved mode under direct supervision. The
  shadow-first ordering and its gates are P9.17's execution record;
  this plan PRE-REGISTERS the cases that record will fill.
- **It is not a safety or certification activity** (AGENTS.md §3 rules
  2–3). Site-local protection, switching, and electrical safety remain
  with the customer's local certified controllers (§17.5 invariant 10).
  Every FAT step that touches energized equipment is performed BY THE
  CUSTOMER under THEIR safety rules — FlexCore personnel observe and
  record only.
- **It is not an equipment-conformance activity.** IEEE 1547.1-2020
  conformance testing remains a separate responsibility (§11.3); no FAT
  row may be worded as certifying equipment.
- **FlexCore is supervisory orchestration only** (§14.5): no FAT case
  exercises or implies primary frequency response, synthetic inertia,
  relay protection, inverter inner-loop control, or sub-cycle control.
- **A HIL pass justifies a FAT case's EXECUTION — never its RESULT**
  (`40_hil_field_traceability.md` rule R-2, applied verbatim in §3).

## 3. The traceability rule (each case names its HIL coverage; uncovered = recorded gap)

The ledger files the P9.40 map with this plan BEFORE P9.17 executes.
The rules below bind every filling act on §6's coverage column; they
are `40_hil_field_traceability.md` §3 rules applied to the FAT side:

| # | Rule |
|---|---|
| TR-1 | Every FAT case row carries its upstream coverage (HIL case(s) per P9.15, lab case(s) per P9.14, simulation case(s) per P8.24) THROUGH the P9.40 map — never asserted directly between records. The §6 "Coverage (via P9.40)" column names the mapped HIL case IDs or `UNMAPPED-GAP-OPEN` |
| TR-2 | A case with no upstream row is **recorded as a gap, not covered**: `UNMAPPED-GAP-OPEN`, with disposition (a) define and run the upstream case (a new map version), or (b) a signed CUSTOMER acceptance of the gap filed in P9.27. Never left blank |
| TR-3 | A HIL/lab/simulation pass justifies running the FAT case and pre-declares its expected behaviour; it NEVER justifies, substitutes for, or predicts the FAT RESULT. The field result exists only after the P9.17 run at the site, in the recorded control-authority mode. A FAT case is never marked "passed" because its HIL row passed |
| TR-4 | Coverage records the HIL class actually run (CHIL/PHIL per the P9.15 record). A CHIL pass is never recorded as PHIL coverage |
| TR-5 | HIL evidence on versions differing from the pilot's cannot justify the FAT case (`BLOCKED-version-mismatch`); the §4 frozen-versions rule makes the comparison possible |
| TR-6 | The map version filed with this plan predates the first P9.17 execution record; the frozen dates are the ordering evidence (P9.40 rule R-6) |

**Coverage status at writing: EVERY §6 coverage cell is EMPTY — no HIL
case has run, the P9.40 matrices are EMPTY by design, so no coverage
claim of any kind is possible today.** The cells are filled only when
the P9.15 record carries real case results and the P9.40 map is frozen
as a filled version.

## 4. Acceptance-test environment state (fixed BEFORE the run)

| Property | Rule (stated in advance — never negotiated at run time) |
|---|---|
| Control-authority mode (§21.4) | The FAT starts in **Read-only / shadow** ("None; computes and compares proposed actions against measurements. Writes to devices: No. Default use: Early pilots and commissioning" — verbatim §21.4 row). Operator-approved cases run ONLY after shadow acceptance and the P9.26 promotion request (P9.17 §2). Bounded automation is OUT OF SCOPE (post-1.0; ENTRY_NOTE boundary 3) |
| Frozen versions | Before the first case, the run record freezes: control-plane build/version, edge-agent build (git SHA), adapter versions, site EMS/OpenEMS version, grid-model version (P9.03), device firmware identities (P9.01 freeze), and the deployed configuration manifest (P9.13 C8.1) |
| Version change DURING the run | A version change of ANY frozen component invalidates the AFFECTED cases — every case that exercised the changed component's code path (the affected-subset rule below). Invalidated cases are marked `INVALIDATED-version-change` in the P9.17 record, never quietly left as passes |
| Affected-subset rule (defined in advance) | Per case, the §6 catalogue names the components whose change invalidates it (the "Invalidated by" column). The executor consults this column, not judgement, at a mid-run version change |
| Full re-run triggers | A change to ANY of: the edge-agent build, the grid-model version, the adapter/ACL mapping table, the site enablement/configuration manifest, or the device firmware — forces a FULL re-run of the site's FAT (all cases), because these components sit on every case's path |
| Site entry state | No case is scheduled at a site that has not completed commissioning (P9.13 C8.3 shadow-entry sign-off signed) and closed D6 for the deployment (P9.06 signed record) |

## 5. Roles present during acceptance, and who signs each case

| Role | Present for | Signs |
|---|---|---|
| Customer operator | Every case | The per-case result row AND the shadow review (P9.17 §6) — acceptance is never Neryva-only (the ledger's named failure mode: "acceptance witnessed only by Neryva") |
| Neryva integration engineer (executor) | Every case | The procedure/evidence columns; NOT the pass verdict alone |
| Customer site contact | Every case at their site | The per-case result row alongside the operator |
| Authority witness (where required) | The cases the applicable authority's engagement record (P9.12) designates | The designated cases' result rows; where no authority requires witnessing, the cell records that fact — it is never silently omitted |
| Neryva product owner | The shadow review; any waiver | Waivers (§7) are signed by the CUSTOMER — Neryva cannot waive a customer acceptance criterion |
| Independent reviewer (did not execute) | The post-run review (P9.17 §8) | The review record |

**All signature cells are UNSIGNED** — no role is staffed (no site, no
customer exists). The witness roster per site is filled at plan review
(§9), not at execution.

## 6. FAT case catalogue (cases defined BEFORE any run; per-site result cells EMPTY)

Case IDs are stable and pre-registered so P9.40 can trace them and
P9.17 can fill them. Expected results are DESIGN statements from the
cited sources — they are not results. Per-site values (`<SITE-ID>`,
dates, thresholds' numeric values, personnel) are EMPTY.

### 6.1 The structural pass/fail criteria (pre-registered NOW — these are design-derived, not measured)

"PASS" for a case means: the observed behaviour matched the case's
expected behaviour, on the frozen versions, with the named evidence
filed, in the recorded control-authority mode. Every structural
criterion below is binary and fixed before any run.

### 6.2 Numeric thresholds — NOT registered (X-8; the dated-amendment block)

Any case criterion that is a NUMBER (freshness limits, latency bounds,
accuracy tolerances, staleness windows) is registered in this block,
with basis/definer/date, BEFORE the first case runs — derived from the
deployment's defined model, service, and operational risk (P9.01 scope,
P9.07 risk positions, P9.11 program requirements), never from a
preliminary observation. The block is EMPTY:

| Case | Numeric criterion | Threshold (value + basis) | Registered by | Registration date |
|---|---|---|---|---|
| FAT-T3 | Freshness limit per use case | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| FAT-T3 | Accuracy tolerance per measured point | (EMPTY — not registered) | (EMPTY) | (EMPTY) |
| FAT-C4 | Clock-offset bound the expiry check assumes | (EMPTY — not registered; cross-ref P9.18 §4) | (EMPTY) | (EMPTY) |
| FAT-A1 | Audit-completeness window/bound, if numeric | (EMPTY — not registered) | (EMPTY) | (EMPTY) |

A threshold set or changed AFTER a result was known is INVALID; the
affected case is discarded and re-run under a re-registered threshold.

### 6.3 Telemetry cases at the site (the P9.18 measurements are cross-referenced, not duplicated)

| Case ID | Case | Procedure | Input/starting state | Expected result (design-derived) | Pass/fail criterion (structural, pre-set) | Evidence to file | Coverage (via P9.40) | Invalidated by |
|---|---|---|---|---|---|---|---|---|
| FAT-T1 | Telemetry end-to-end accuracy and freshness at the site, for EVERY adapter named by the FILLED P9.04 design | Compare each adapter's site values against the customer's metering reference at the same instants; compute end-to-end freshness (event_time → queryable) | Site in shadow; frozen versions; P9.13 commissioning complete | Every P9.04 adapter row delivers values in canonical units (MW/MVAr/MVA/MWh; SOC fraction [0,1]; §10.2) with declared signs, correctly timestamped (§10.3) | Every adapter row delivers with the §6.2-registered accuracy/freshness thresholds met; any adapter absent from the flow = fail | Per-adapter comparison table + raw export | via P9.40 ← P9.14 L1/L3 (lab counterparts); HIL coverage: (EMPTY — fill from the P9.15/P9.40 records) | edge build, adapter/ACL mapping, EMS/OpenEMS version, grid-model version, device firmware |
| FAT-T2 | Quality-state mapping at the site (§10.4 six canonical states) | Observe each source's native states map to exactly ONE canonical state (`GOOD`/`SUSPECT`/`BAD`/`MISSING`/`ESTIMATED`/`STALE`), source code retained as diagnostic metadata | Site in shadow; frozen versions | No quality flag silently converted into a normal reading; degraded never upgraded to `GOOD` | Zero upgrades observed at any checked point; every value exactly one canonical state | Quality-state trace per adapter | via P9.40 ← P9.14 L2/L5; the measurement record is P9.18 §3 | edge build, adapter/ACL mapping, EMS/OpenEMS version |
| FAT-T3 | Freshness and staleness per use case | Measure ingestion delay and staleness against the §6.2-registered per-use-case limit; record which quality states each use case accepts | Site in shadow; frozen versions | `STALE` stamped at the freshness limit per use case (the `openems.rs` `stale_after_millis` discipline) | Staleness behaviour matches the registered limits; use-case acceptance sets recorded | Freshness measurement export | via P9.40 ← P9.14 L5; the freshness measurements are P9.18 §6 | edge build, adapter/ACL mapping, grid-model version |
| FAT-T4 | Stale/missing handling end-to-end | Inject a stale feed and a missing feed at the site boundary (coordinated with the customer; declared injection, never accidental) | Site in shadow; frozen versions | The system HOLDS/REJECTS per policy instead of inferring values — §17.5 invariant 9 verbatim: "A missing/uncertain/stale model is not interpreted as a passing feasibility check" | No silently substituted value appears at any consumer; the hold/reject is audited | Injection log + consumer-side trace + audit extract | via P9.40 ← H8/H12 (P8.24 F1/F4 vocabulary); the site-side procedure is P9.18 §7 | edge build, adapter/ACL mapping, grid-model version |

### 6.4 Shadow command-lifecycle cases (no command reaches ISSUED on the field path in shadow)

| Case ID | Case | Procedure | Input/starting state | Expected result (design-derived) | Pass/fail criterion (structural, pre-set) | Evidence to file | Coverage (via P9.40) | Invalidated by |
|---|---|---|---|---|---|---|---|---|
| FAT-S1 | Command lifecycle in shadow (§14.1 gates distinct) | Drive PROPOSED → VALIDATED → FEASIBLE → (approval gate rendered, not exercised against field) in shadow; attempt the forbidden shortcuts | Site in shadow; frozen versions | Validation, feasibility, and authorization remain DISTINCT gates; no `PROPOSED → ISSUED` shortcut exists; shadow compares proposed actions against measurements WITHOUT issuing commands (§21.4) | Zero commands reach the edge's forwarder or any adapter; every gate refusal audited; the lifecycle transitions follow the closed `command_state.rs` table | Command-record + audit extract per case | via P9.40 ← P9.14 L8–L10; HIL H1–H2 counterparts | edge build, control-plane build, grid-model version |
| FAT-S2 | Rejection set at the site boundary (P9.36 vocabulary BY CITATION) | Exercise the CLOSED `PilotAttackCase` set (`crates/edge-agent/src/pilot_rejection_proof.rs`: `forged_signature`, `replayed_captured`, `expired_window`, `out_of_sequence`, `conflicting_idempotency_payload`) as declared bench-level confirmations at the site (the live injection drill is P9.36's deliverable — this case confirms the SAME codes at the site, it does not replace the drill) | Site in shadow; frozen versions | Each case refused with the named rejection code (`signature`/`duplicate`/`expiry`/`sequence`/`idempotency_conflict`); sequence checkpoint unchanged | Zero reach to any adapter; every refusal audited; checkpoint state unchanged after each case | Rejection log + checkpoint extract | via P9.40 ← H7; the drill execution is P9.36 | edge build |
| FAT-S3 | Acknowledgement and tracking semantics | In the shadow/test-lab composition the deployment actually has, walk ACKNOWLEDGED → TRACKING → COMPLETED and the FAILED edges; attempt to infer portfolio-level ack from one device | Site in shadow; frozen versions | Acknowledgement confirms receipt/acceptance, NOT physical delivery (§14.1); partial fleet success represented per asset; no portfolio ack inferred from the first responder | No inferred ack; per-asset representation verified; tracking comparisons recorded against measurements | Lifecycle trace + telemetry comparison | via P9.40 ← H2/H13 (P8.24 F5) | edge build, control-plane build, device firmware |
| FAT-S4 | Expiry and supersede in shadow (§14.3) | Present commands whose validity window elapses; supersede an approved revision; duplicate with conflicting payload | Site in shadow; frozen versions | Expired refused `expiry`; superseded commands never deliver; conflicting duplicate refused `idempotency_conflict`; §14.3 evidence fields present on every record | Every case refused with the named code; zero stale/duplicated/out-of-sequence acceptance | Command-record + audit extract | via P9.40 ← H3/H4/H9/H11 (P8.24 F1/F3) | edge build |
| FAT-S5 | Fallback behaviour (§14.4 per-asset policy) | Trigger each asset/site's RECORDED fallback policy in shadow and observe the recorded outcome path | Site in shadow; frozen versions; fallback policies as configured | Fallback activates per the recorded per-asset/site policy (hold-last-bounded / return-to-local-EMS / stop-accepting / require-operator — §14.4 list); NO universal "set power to zero"; the fallback outcome is recorded, never a silent schedule substitution | The observed fallback matches the configured policy exactly; no unconfigured fallback appears | Fallback activation record + audit extract | via P9.40 ← H5/H14 (P8.24 F6) | edge build, control-plane build, configuration manifest |

### 6.5 Fault-behaviour cases (the P8.24 fault vocabulary F1–F8 BY CITATION)

P8.24 (`ledger/phase-08-advanced-coordination/24_failure_drill.md` §2)
recorded the fault set F1–F8 with per-module expected fail-closed
behaviour; the SAME vocabulary applies at the site. Injection
parameters (delay magnitude, loss rate, skew offsets, stale windows)
are RECORDED PER CASE at execution from the actual injection — no
number is invented here. Injection at the site is COORDINATED WITH THE
CUSTOMER under their operational approval and is performed in shadow.

| Case ID | Fault (P8.24 vocabulary, cited) | Site-side injection | Expected fail-closed behaviour (design-derived) | Pass/fail criterion (structural, pre-set) | Evidence | Coverage (via P9.40) | Invalidated by |
|---|---|---|---|---|---|---|---|
| FAT-F1 | F1 dropped / duplicated messages | Drop/duplicate telemetry frames on the designed site legs (F1/F2 per P9.05) | MISSING/STALE surfaced per §10.4; duplicates detected per dedup rules; no silent wrong value | Every drop/dup handled per the recorded policy; zero silent substitutions | Buffer/audit journal extract + quality trace | via P9.40 ← H8/H9; P9.14 L4/L6 | edge build, adapter/ACL mapping |
| FAT-F2 | F2 reordered messages | Reorder delivery order at the site seam | Refused/recorded per seam; never silently accepted as in-order | Zero out-of-order silent acceptance | Seam log | via P9.40 ← H10 | edge build |
| FAT-F3 | F3 clock skew | Skew the site/edge clock source per the declared injection (`clock_skew_future`/`clock_skew_past` cases) | Expiry checks fail CLOSED on any non-`Synchronized` clock (`clock_quality.rs` hold-on-drift); rejections audited | Commands held/rejected on every non-`Synchronized` state; holds audited (`EDGE_CLOCK_HOLD_ON_DRIFT`) | Clock-quality audit extract | via P9.40 ← H11 | edge build; the measurement record is P9.18 §4 |
| FAT-F4 | F4 stale telemetry | Serve stale readings at the site boundary | STALE carried end-to-end; downstream sees the degraded state; no upgrade | Zero upgrades; consumers record the degraded state | Quality trace | via P9.40 ← H12; P9.14 L5 | edge build, adapter/ACL mapping |
| FAT-F5 | F5 unavailable asset / partial fleet response | Remove an asset from the site set mid-observation | Refusal/recorded abandonment per policy; no fabricated values for the absent asset | Zero fabricated values; abandonment recorded | Dispatch record + audit | via P9.40 ← H13 | control-plane build, grid-model version |
| FAT-F6 | F6 solver timeout | Exhaust the scheduling budget | `NON_CONVERGED_BUDGET_EXHAUSTED`-class recorded failure; fallback per FAT-S5; no invented schedule | No schedule presented from an exhausted budget; failure recorded | Scheduling record + audit | via P9.40 ← H14 | control-plane build, solver/provider versions |
| FAT-F7 | F7 worker/plugin crash | Crash a worker process at the composition root (P8.24 C-2 discipline) | Worker loss isolated; no partial envelope; control plane survives and records the failure | No partial result consumed; failure recorded | Supervision log + audit | via P9.40 ← H15 | control-plane build |
| FAT-F8 | F8 communication loss (FlexCore ↔ site) | Drop the F1 uplink for a bounded interval | Edge buffers locally (bounded store-and-forward); site-local control continues unaffected (§21.2, §17.5 invariant 10); abandonment/audit per policy | Buffered records re-uploaded in order; drops counted+audited; site-local independence confirmed by the customer operator | Buffer/audit extract + operator confirmation | via P9.40 ← H6/H16 | edge build, network design (P9.05) |

### 6.6 Rollback, degradation, operator workflow, and audit cases

| Case ID | Case | Procedure | Input/starting state | Expected result (design-derived) | Pass/fail criterion (structural, pre-set) | Evidence | Coverage (via P9.40) | Invalidated by |
|---|---|---|---|---|---|---|---|---|
| FAT-R1 | Signed-update/rollback AT THE SITE (re-verifies P9.06 UP3, as the signed D6 record requires) | Staged update + failed-health rollback on the INSTALLED site platform (the P9.13 C7.3 counterpart, repeated at FAT because the frozen version set is the FAT's) | Site in shadow; frozen versions; signed bundles available | Rollback to the previous verified bundle occurs on failed health; downgrade only via explicit operator-signed action | Rollback demonstrable on the installed platform; no unsigned/downgrade path observed | Update/rollback log | via P9.40 ← P9.34 verification; HIL coverage: (EMPTY — fill from P9.15/P9.40) | edge build, bundle-signing configuration |
| FAT-D1 | Degraded-mode behaviour (central outage at the site) | Stop the control plane for a bounded interval; observe the site | Site in shadow; frozen versions | Site continues LOCAL operation; site-local protection unaffected (§21.2, §17.5 invariant 10); edge buffers and refuses new commands; recovery drains in order | Site-local independence confirmed; no command issued while disconnected; recovery in order, drops audited | Drill log + audit extract + operator confirmation | via P9.40 ← H6; the full per-policy drill register is P9.20's deliverable | edge build, network design |
| FAT-O1 | Operator workflow (§20) | The customer operator walks the pilot workflow end-to-end in the operator console (§20 workspaces): observe telemetry/quality, review a proposed schedule, walk the approval gate rendering, view the audit trail | Site in shadow; frozen versions | Every §20 workspace step completes with the shadow-mode data; the approval gate is RENDERED but issues nothing to the field in shadow; the console shows exactly-one canonical quality state per value | Zero field writes; every workflow step completed; console values match the recorded telemetry | Console walkthrough record + screenshots | via P9.40 ← P9.14 L1/L2 console legs | console build, control-plane build |
| FAT-A1 | Audit completeness | For EVERY other case in this catalogue, confirm the case's decisions/refusals/transitions appear in the audit trail (the P9.32 discipline) | After the other cases | Every case's evidence is traceable from the audit trail alone | Zero cases without their audit evidence; any gap = fail of FAT-A1 (not of the referenced case) | Audit-trail export mapped to case IDs | via P9.40 ← P9.32; HIL coverage: (EMPTY — fill from P9.15/P9.40) | edge build, control-plane build |

**Operator-approved acceptance cases** (approved command issue,
acknowledgement, tracking, completion — ledger P9.17 step 5) are
DEFINED as the operator-approved re-instantiation of FAT-S1–FAT-S3
under direct operator supervision, executed ONLY after shadow
acceptance and the P9.26 promotion request. Their procedures, expected
results, and criteria are the shadow cases' criteria plus the field
write actually occurring; they are PRE-REGISTERED here and are never
improvised at run time.

## 7. The failure process (fixed BEFORE the run — no silent re-runs)

| Rule | Statement |
|---|---|
| FP-1 | A failed case is recorded AS A FAILURE in the P9.17 execution record the same day, with the observed behaviour, the versions, and the evidence reference |
| FP-2 | The case is retried ONLY after: (a) a RECORDED FIX (what was changed, by whom, which frozen-version fields changed as a result — triggering the §4 re-run rules), or (b) a RECORDED WAIVER signed by the CUSTOMER (with the residual risk noted for P9.27). Never by re-running until it passes |
| FP-3 | A waiver is a CUSTOMER signature on a named case with its observed failure — Neryva cannot grant it, and no authority-witnessed case is waivable without that witness's record |
| FP-4 | A failure on an operator-approved case triggers the P9.17 reversion rule (site back to shadow, recorded; no further commands at that site) — never "continued testing" |
| FP-5 | Every failure and every waiver is carried to P9.27 as a residual-risk entry |

## 8. The §19.2 and §22.3 evidence requirements (what the FAT record must contain to count)

| Requirement | Source | How the FAT satisfies it |
|---|---|---|
| Every command-state transition tested | §22.3 | FAT-S1–FAT-S4 + the operator-approved re-instantiation cover the §14.1 lifecycle edges the pilot's scope exercises; transitions outside the pilot scope are recorded NOT-EXERCISED-with-reason, never silently skipped |
| Authorization and tenant-isolation tests pass | §22.3 | FAT-S1/S2 record the authorization-gate refusals; tenant isolation is evidenced by the P9.08/P9.14 records referenced from the run record — the FAT does not re-invent isolation testing |
| Stale/missing telemetry cases handled | §22.3 | FAT-T4/FAT-F4 + the P9.18 §7 injection record |
| Offline/degraded modes tested | §22.3 | FAT-F8/FAT-D1 |
| Upgrade/rollback tested | §22.3 | FAT-R1 |
| Target control-authority mode passed its own qualification | §22.3 | The shadow cases qualify read-only/shadow; the operator-approved cases qualify operator-approved mode AFTER shadow acceptance and the P9.26 promotion request — never before |
| Deployment-specific cyber and operational risk review complete | §22.3 | The P9.07 assessment (no open must-fix) is a §4 entry-state precondition; open must-fix = the FAT does not start |
| Shadow-mode evaluation: compare proposed actions to existing operator/site behaviour without issuing commands | §19.2 item 9 | Every shadow case computes and compares WITHOUT issuing (§21.4); the comparison evidence is per-case |
| Metrics defined before judging | §19.4 | "Set acceptance thresholds after defining the model, service, benchmark, and operational risk" — the §6.2 X-8 amendment block; dispatch metrics (acknowledgement latency, tracking error, failure/expiry rate, recovery) are recorded per case where the composition can express them, and the composition limits are recorded where it cannot (the P9.39 §7.1 honesty pattern) |
| §19.2(10) ordering | §19.2 | No FAT case is scheduled at a site until simulation (P9.41), integration (P9.14), and cyber (P9.07 + signed D6) reviews have passed — the entry-state table (§4) enforces it structurally |

## 9. Customer review of the plan — NOT REVIEWED (acceptance criterion (d))

| Field | Value |
|---|---|
| Reviewer (customer representative, named) | `<EMPTY>` |
| Applicable authority consulted (per P9.12; or "none required" recorded with its basis) | `<EMPTY>` |
| Review basis (the case catalogue, thresholds, roles, failure process walked with the customer) | `<EMPTY>` |
| Customer acceptance criteria reconciled (the ledger input: the customer's own criteria mapped into §6 or recorded as additions) | `<EMPTY>` |
| Outcome (approved / approved-with-conditions / changes required) | **NOT RECORDED** |
| Conditions / required changes | `<EMPTY>` |
| Signature / date | **UNSIGNED / `<EMPTY>`** |

Until this row is signed, the plan is a reviewed-baseline DRAFT and no
P9.17 execution may be scheduled against it (the ledger's exit check
(d): "the customer has reviewed and agreed the plan").

## 10. Reviewer-verification procedure

1. Confirm §1's plain statement is unchanged: no site, no date, no
   threshold value, no signature exists anywhere in the record.
2. Confirm every §6 case row has ALL of: procedure, input/starting
   state, expected result, pass/fail criterion, evidence artifact,
   coverage column (ledger acceptance criterion (a) — every case has a
   pre-set threshold; a case lacking one is a finding).
3. Confirm §6.2's numeric-threshold block is EMPTY and that the X-8
   pre-registration rule is stated (a threshold fitted to an observed
   result is the ledger's named failure mode — this block is the
   control).
4. Confirm §3's traceability rules match `40_hil_field_traceability.md`
   (TR-2/R-1 gap discipline; TR-3/R-2 the execution-never-result rule;
   TR-5/R-4 version consistency) and that every §6 coverage cell is
   EMPTY at writing.
5. Confirm §4's version-change and re-run rules are stated IN ADVANCE
   and the full-re-run trigger set is explicit (ledger exit check (e)).
6. Confirm §7 forbids silent re-runs and binds waivers to the CUSTOMER
   (ledger exit check (c)).
7. Confirm §9 is UNSIGNED/NOT RECORDED (ledger exit check (d)) and that
   the §2 scope ceiling contains no safety/certification/protection
   wording as a claim.
8. For a filled future plan: confirm the threshold amendment predates
   the first run record, and the P9.40 map version filed with this plan
   predates the first P9.17 execution record (TR-6).

## 11. Filling commands (for the human executor)

```sh
# (a) Documented-tools gate (before and after any edit):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill order: sign the P9.01 freeze (site + customer) → receive the
#     customer's acceptance criteria → reconcile them into §6 (or record
#     additions) → P9.13 commissioning complete (C8.3 signed) → P9.15 HIL
#     results + the FILLED P9.40 map version filed here (TR-6) → sign §9.
#     THEN register the numeric thresholds in §6.2 as a DATED AMENDMENT
#     (value + basis + definer + date, BEFORE the first run). Only then
#     may P9.17 schedule execution.

# (c) Per-site instantiation: copy this plan per site (the site column
#     stays <SITE-ID> until the signed freeze names one); fill the §6
#     coverage cells FROM the P9.40 map rows for this site's case set —
#     never from memory; every unmapped case gets UNMAPPED-GAP-OPEN and
#     an FP-2/§3 TR-2 disposition.

# (d) Execution consumes P9.17 (17_field_acceptance_execution.md) — this
#     plan never records a result; results are recorded ONLY in the
#     P9.17 execution record from actual run output.
```

## 12. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Why it is open |
|---|---|---|
| R-1 | **No site, no customer, no acceptance criteria exist** — the FAT cannot be scheduled for any site until the P9.01 freeze is signed and the customer's criteria are received and reconciled (§9) | §1, §9 |
| R-2 | **Numeric thresholds are unregistered** (§6.2 EMPTY): until the dated amendment, no case result can be judged pass/fail — only recorded. The X-8 registration is a human/product input | §6.2 |
| R-3 | **Coverage is EMPTY because the upstream evidence does not exist** (P9.15 UNRUN, P9.40 matrices EMPTY): every §6 coverage cell is an open gap until the map is filled — execution before a filled map violates TR-6 | §3, §6 |
| R-4 | **Thresholds fitted to observed results** (the ledger's named failure mode): the §6.2 block and FP-2 provide the structural control, but enforcement at fill time is human | §6.2, §7 |
| R-5 | **A mid-run version change silently invalidating earlier cases**: the §4 affected-subset/full-re-run rules are pre-registered, but the invalidation marking in the P9.17 record is human-enforced | §4 |
| R-6 | **Acceptance witnessed only by Neryva** (the ledger's named failure mode): §5 binds customer-operator signatures to every case, but staffing is impossible until a customer exists | §5 |
| R-7 | **Waiver pressure**: schedule pressure at a real pilot tempts FP-3 violations; a customer-signed waiver records a REAL residual risk in P9.27, and an authority-witnessed case waiver additionally needs the witness's record | §7 |
| R-8 | **Operator-approved cases exercised before shadow acceptance** (the P9.17 ordering failure mode): this plan pre-registers the ordering; the gate is P9.17 §2, human-enforced | §6.4; P9.17 |

## 13. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| The FAT itself (any case of §6) | **NOT RUN — banned in this loop; human-gate input; every result cell lives in the P9.17 record and is EMPTY** |
| Numeric threshold registration (§6.2) | **NOT REGISTERED — X-8: registration is a dated amendment BEFORE the run** |
| Customer review (§9) | **NOT REVIEWED — UNSIGNED; acceptance criterion (d) not met and not claimed** |
| Any fabricated site, date, threshold value, or signature | **NONE EXISTS — nothing fabricated** |

Status: **WRITTEN-UNRUN** — a FAT plan is paper; execution is people at
a real site under the P9.17 shadow-first gate, which has not happened
and is not scheduled. Nothing was executed, witnessed, or signed.
