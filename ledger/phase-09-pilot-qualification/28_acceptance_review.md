<!-- P9.28 — Customer and site acceptance review (REVIEW TEMPLATE + evidence-checklist assembly). The review structure is written; EVERY attendee cell is EMPTY/UNSIGNED; the acceptance-evidence checklist assembles the phase's planned evidence BY RECORD PATH with each item's current status honestly WRITTEN-UNRUN/EMPTY; the acceptance criteria are PRE-REGISTERED against FAT results that are EMPTY; the review OUTCOME is EMPTY — no review has been convened, nothing is accepted. The review itself is the HUMAN/ORGANIZATION gate (ENTRY_NOTE.md boundary 1). No decision, condition, waiver, or signature exists or is claimed. -->
# P9.28 — Customer and Site Acceptance Review (Review Template — OUTCOME EMPTY, nothing accepted)

Date: 2026-11-14. Subtask: P9.28 (`ledger_phase_9.md` §P9.28).
Owner role: Product (chair); customer (decision). Report trace:
§23 Stage 9 exit gate; §22.3. Inputs per the ledger: P9.01–P9.27
(or an explicitly signed waiver — none exists); P9.12 authority
tracker (`12_authority_engagement.md` §9 — NOT-STARTED); P9.27
register (`27_limitations_residual_risk.md` — v0.1-draft, every
assessment cell EMPTY).

## 1. Honest status — the plain statement

**No acceptance review has been convened, no attendee has been
nominated, no evidence pack has been presented, no decision has been
recorded, and nothing is accepted.** There is no pilot customer, no
site, no deployment, no field acceptance result, and no execution
evidence of any kind (the standing Phase 1–8 caveat: all code is
WRITTEN-UNRUN). The Phase 9 exit gate — "customer/site and applicable
authorities approve the deployment and its control scope" — is NOT
met, is not claimed met, and cannot be met on this record.

This record delivers the REVIEW STRUCTURE only: who attends (§3),
the acceptance-evidence checklist assembled by record path with each
item's honest current status (§4), the pre-registered acceptance
criteria tied to FAT results that are EMPTY (§5), the review
procedure (§6), the decision/waiver record shapes with every cell
EMPTY (§7), the empty OUTCOME (§8), and what acceptance would and
would not establish (§9). The chair may not fill §7's decision cells
from anything except the convened review's own signed output; filling
them from this template would be fabrication.

The ledger's acceptance criteria for P9.28 are NOT met and are NOT
claimed: (a) no review was held — there is nobody to hold it with;
(b) no site has a recorded decision — no site exists; (c) no
condition has an owner or a date; (d) no waiver exists, signed or
otherwise. The ledger's named failure modes — "acceptance granted on
a slide deck instead of the evidence pack", "a condition accepted
verbally and never recorded", "a site approved while its HIL or field
acceptance record is still open" — are structurally guarded by §4's
status column and §7's decision rules, but the guard is paper until
humans execute the review.

## 2. Why no review can be convened yet — the dependency statement

| Dependency | Current status | Consequence |
|---|---|---|
| P9.01 scope freeze | TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md` | no customer legal name and no site ID exist; there is no one to invite and nothing to review |
| P9.02–P9.03 (reconciliation, network model) | TEMPLATE/PLAN — `02_site_asset_reconciliation.md`, `03_network_model_build.md` | the pilot configuration the review would judge is undefined |
| P9.06 D6 closure | OPEN, UNSIGNED — `06_ot_it_boundary_d6.md` | a review held without signed D6 would accept an unresolved cyber boundary |
| P9.07 security assessment | TEMPLATE, no findings assessed — `07_deployment_security_assessment.md` | the "no must-fix finding open at P9.28" check is UNEVALUABLE (LR-039) |
| P9.12 authority tracker | NOT-STARTED — `12_authority_engagement.md` §9 | precondition approvals unknown; a precondition approval pending blocks the site's decision (§7 rule D-3) |
| P9.16 FAT plan / P9.17 FAT execution | PLAN pre-registered / RUN RECORD EMPTY — `16_field_acceptance_plan.md`, `17_field_acceptance_execution.md` | the review's primary evidence (field acceptance results) does not exist |
| P9.26 control-scope document | PREPARED TEMPLATE, NOT APPROVED — `26_control_scope_document.md` | the control scope the review would approve is a design, not a signed document |
| P9.27 limitations register | v0.1-draft, assessments EMPTY — `27_limitations_residual_risk.md` | per §7.2 of that register, a site is not approved while a must-affecting row is unsigned — and every row is unsigned |

## 3. Review structure — who attends (every cell EMPTY/UNSIGNED)

The review is convened by Neryva product (chair) at the request of
the customer after the evidence pack (§4) is assembled. Attendance is
per §23 Stage 9 and ledger step 2: the customer, the site owners,
and Neryva (product, security, power systems, platform). The
independent witness role exists so acceptance is never witnessed
only by Neryva (the P9.16 named failure mode, LR-114). **No person
is named, nominated, or invited; every cell is `<EMPTY>`/UNSIGNED.**

| Role | Attendee name | Affiliation | Attends as | Signature / date |
|---|---|---|---|---|
| Customer operator / decision-maker (the ledger: "customer (decision)") | `<EMPTY>` | `<EMPTY>` | Decision — approval per site | `<EMPTY — UNSIGNED>` |
| Customer site owner (per site; §22.3 "site owner approval") | `<EMPTY>` | `<EMPTY>` | Decision — site scope | `<EMPTY — UNSIGNED>` |
| Customer OT owner (per the P9.06 D6 review composition) | `<EMPTY>` | `<EMPTY>` | Evidence review — boundary decisions | `<EMPTY — UNSIGNED>` |
| Neryva product (CHAIR) | `<EMPTY>` | `<EMPTY>` | Chair — presents the pack, records the decision | `<EMPTY — UNSIGNED>` |
| Neryva security | `<EMPTY>` | `<EMPTY>` | Evidence — P9.07 assessment, D6, P9.31/P9.32 proofs | `<EMPTY — UNSIGNED>` |
| Neryva power systems | `<EMPTY>` | `<EMPTY>` | Evidence — P9.03 model, P9.15 HIL, P9.40 traceability | `<EMPTY — UNSIGNED>` |
| Neryva platform lead | `<EMPTY>` | `<EMPTY>` | Evidence — P9.18/P9.19/P9.24/P9.25/P9.39 operational proofs | `<EMPTY — UNSIGNED>` |
| Independent witness | `<EMPTY>` | `<EMPTY>` | Witness — confirms the review followed §6 and the decision matches the pack | `<EMPTY — UNSIGNED>` |
| Customer operator attendee (P9.21-qualified, for the operator workflow evidence) | `<EMPTY>` | `<EMPTY>` | Evidence — P9.21 qualification, P9.22 procedures | `<EMPTY — UNSIGNED>` |

Rules (structural, not fillable):

| # | Rule |
|---|---|
| A-1 | The review is NOT quorate without the customer decision-maker and the site owner for every site under decision present (in person or by recorded attendance) |
| A-2 | The independent witness signs the review record only after verifying §6 was followed; a witness who did not attend does not sign |
| A-3 | The chair does not hold a decision vote; the decision is the customer's (ledger owner role) |
| A-4 | No attendee signs for another; a signature cell filled by anyone other than the named attendee invalidates the record |

## 4. Acceptance-evidence checklist — assembled BY RECORD PATH (each item's current status honest)

The pack the review consumes is the phase's own record set. Each row
names the record path and that record's ACTUAL current status — the
review may not proceed past any row whose status is not a filled,
verified record (rule E-1 below). **No row below is filled; every
status is the record's honest current state as of 2026-11-14.**

| # | Pack item | Record path | What the review checks | Current status |
|---|---|---|---|---|
| E-01 | Pilot scope freeze (customer, sites, assets, service, exclusions, signatories) | `ledger/phase-09-pilot-qualification/01_pilot_scope_freeze.md` | signed by customer and Neryva product; no "and similar sites" language; bounded automation excluded | TEMPLATE — UNSIGNED |
| E-02 | Site and asset register reconciliation (zero unresolved discrepancies) | `.../02_site_asset_reconciliation.md` | clean final pass; capability provenance marked | TEMPLATE — UNRUN |
| E-03 | Network model build + validation (frozen version, residuals within pre-set thresholds, coverage limitations recorded) | `.../03_network_model_build.md` | model version frozen by ID; provider matches declared class | PLAN — WRITTEN-UNRUN |
| E-04 | Customer integration design (adapters, versions, profiles, mappings, failure behaviour) | `.../04_customer_integration_design.md` | no adapter called conformant without a cited P9.37 result | TEMPLATE — EMPTY |
| E-05 | Site network and connectivity design (zone/flow diagram, allowlist, customer OT-owner review) | `.../05_site_network_design.md` | no inbound internet flow into OT; customer review recorded | TEMPLATE — EMPTY |
| E-06 | OT/IT boundary review — D6 closure record, signed | `.../06_ot_it_boundary_d6.md` | signed by security owner and customer OT owner; no unresolved rejected boundary | OPEN — UNSIGNED |
| E-07 | Deployment security assessment (findings list; no open must-fix at this review) | `.../07_deployment_security_assessment.md` | every §17.4 threat has a control + testing subtask; Rev. 4 cited as draft only | TEMPLATE — EMPTY |
| E-08 | Identity, access, and approval design (role matrix, negative-test evidence) | `.../08_identity_access_approval.md` | no shared approval-capable account; separation of duties | DESIGN — UNTESTED |
| E-09 | Secrets, certificates, and key management (inventory, rotation test) | `.../09_secrets_certs_keys.md` | no secret in source control; rotation test passed | TEMPLATE — UNRUN |
| E-10 | Data residency, retention, and privacy design | `.../10_data_residency_retention.md` | per-class retention with basis; tenancy negative test passed | TEMPLATE — UNRUN |
| E-11 | Market / program requirements capture (or explicit no-participation record) | `.../11_market_program_requirements.md` | rules from program documentation, not assumed | TEMPLATE — EMPTY |
| E-12 | Authority-engagement tracker (P9.12) | `.../12_authority_engagement.md` §9 | every authority APPROVED / APPROVED-WITH-CONDITIONS / NOT-REQUIRED-with-basis; pending items have follow-up dates | NOT-STARTED |
| E-13 | Site commissioning records (every step passed per site) | `.../13_commissioning_plan.md` | pass/fail per step with filed evidence; D6 closed before shadow entry | PLAN — UNSCHEDULED |
| E-14 | Lab integration test — read-only end-to-end | `.../14_lab_integration_test.md` | command path demonstrably disabled in default lab config | TEMPLATE — UNRUN |
| E-15 | HIL test plan + execution report | `.../15_hil_test_plan.md` | per-case pass/fail on pilot equipment/firmware versions; failures dispositioned | PLAN — RIG NOT PROCURED, UNRUN |
| E-16 | FAT plan (pre-registered cases, thresholds, roles, failure process) | `.../16_field_acceptance_plan.md` | thresholds set before the run; customer-reviewed plan | PLAN — CUSTOMER REVIEW UNSIGNED |
| E-17 | FAT execution record — shadow results, shadow review (SIGNED), promotion request, operator-approved results | `.../17_field_acceptance_execution.md` | shadow before promotion; signed customer shadow review; reversions recorded | RUN RECORD — EMPTY, UNRUN |
| E-18 | Telemetry quality and clock-sync verification | `.../18_telemetry_clock_verification.md` | measured offsets within expiry bound; stale/missing injection holds | TEMPLATE — UNRUN |
| E-19 | Command-lifecycle dry run (shadow) | `.../19_command_dry_run.md` | gates distinct; no ISSUED state for any dry-run command | TEMPLATE — UNRUN |
| E-20 | Fallback and degraded-mode drills + fallback register (operator-reviewed) | `.../20_fallback_drills.md` | witnessed drill; no drill failure open; local protection independent | REGISTER EMPTY — UNRUN/UNWITNESSED |
| E-21 | Operator training and qualification register | `.../21_operator_training.md` | every approval-right holder passed the practical exercise | REGISTER EMPTY — NO TRAINEE |
| E-22 | Market/product operational procedures | `.../22_market_operational_procedures.md` | walked through against a shadow case | DRAFT-INACTIVE |
| E-23 | Incident response procedure + tabletop record + rota | `.../23_incident_response.md` | tabletop held with the customer; rota current | PROCEDURE WRITTEN — TABLETOP UNHELD, ROTA UNSIGNED |
| E-24 | Backup/restore and DR rehearsal report | `.../24_backup_dr_rehearsal.md` | restore actually performed; edge-outage test run | PLAN — UNRUN |
| E-25 | Observability and alerting verification | `.../25_observability_alerting.md` | critical alerts triggered and received by a named recipient | TEMPLATE — UNRUN |
| E-26 | Control-scope document + deployed-configuration verification + promotion decisions | `.../26_control_scope_document.md` | running configuration matches document field-for-field; no plugin mode change | PREPARED TEMPLATE — NOT APPROVED |
| E-27 | Known-limitations and residual-risk register (customer-acknowledged) | `.../27_limitations_residual_risk.md` | every limitation traced; customer acknowledgement on file | v0.1-DRAFT — ASSESSMENTS EMPTY, UNSIGNED |
| E-28 | Edge rejection proof (forged/replayed/expired/out-of-sequence) | `.../31_edge_rejection_proof.md` | executed at the real edge with audit records | CODE WRITTEN-UNRUN |
| E-29 | Audit integrity + edge audit-buffer proof | `.../32_audit_integrity_proof.md` | tamper detection; zero-loss buffer upload; export for P9.29 | CODE WRITTEN-UNRUN |
| E-30 | Edge enrollment ceremony + identity lifecycle | `.../33_edge_enrollment.md` | ceremony executed at the site | CODE WRITTEN-UNRUN |
| E-31 | Signed edge-update verification + rollback | `.../34_signed_update_verification.md` | update verified before activation; rollback exercised | CODE WRITTEN-UNRUN |
| E-32 | §17.5 safety-invariant map (pilot) | `.../35_safety_invariant_map.md` | map filed; no invariant claimed guaranteed | MAP — EMPTY CELLS |
| E-33 | Live incident drill: forged-command containment | `.../36_forged_command_drill.md` | drill held per P9.23 | UNSCHEDULABLE (P9.23 tabletop UNHELD) |
| E-34 | Adapter conformance suites (pilot config) | `.../37_adapter_conformance_pilot.md` | suites executed in the pilot configuration | TEMPLATE — UNRUN |
| E-35 | Pilot shadow replay (§24 step 10) | `.../38_pilot_shadow_replay.md` | replay recomputes, not reuses; tolerances pre-registered | TEMPLATE — UNRUN |
| E-36 | Pilot performance and load test | `.../39_performance_load_test.md` | §19.4 metrics within pre-registered thresholds | TEMPLATE — UNRUN |
| E-37 | HIL-to-field traceability map | `.../40_hil_field_traceability.md` + `hil-field-traceability-map-v1.json` | FAT cases trace to HIL coverage; gaps recorded | MAP RULES — MATRIX EMPTY |
| E-38 | Scenario-regression suite re-run (pilot config) | P9.41 — record NOT YET WRITTEN (HANDOVER wave 6: pending; the ledger index lists P9.41) | suite green on pilot config | RECORD MISSING — P9.41 OPEN |

Rules (structural, not fillable):

| # | Rule |
|---|---|
| E-1 | The review does not vote on any site while that site's E-rows are not all filled-and-verified; an open row is an open subtask, and the ledger's rule "a subtask that cannot be evidenced is open, not passed" applies verbatim |
| E-2 | Where a subtask is incomplete, the only path past it is a waiver per §7.3 — explicit, customer-signed, reasoned, time-bounded; a silent gap is not a waiver (ledger step 4) |
| E-3 | The review cites RECORD PATHS, never summaries of them; the P9.27 rule "the trace is a pointer, never the full description" applies to the pack |

## 5. Acceptance criteria (PRE-REGISTERED, tied to the FAT results — which are EMPTY)

The criteria below are registered NOW, before any review is convened
and before any FAT result exists, so that the decision is judged
against pre-set criteria and never fitted to a presentation (the
"slide deck instead of the evidence pack" failure mode). The FAT
results they reference are EMPTY (`17_field_acceptance_execution.md`
§5 — no case has run; §6 — the shadow review has never been held).

| # | Criterion | Evidence it binds to | Current state of that evidence |
|---|---|---|---|
| C-1 | Every FAT case for the site passed, or failed with a customer-signed waiver per the P9.16 failure process | `17_...md` §5 per-case results; `16_...md` §6 catalogue and §7 failure process | EMPTY — no case has run |
| C-2 | The shadow review was held and signed by the customer operator BEFORE any promotion request | `17_...md` §6 shadow-review record | UNSIGNED — no review held |
| C-3 | Every shadow case passed; tracking error and every rejected command are in the shadow-review record with reasons | `17_...md` §5 shadow results | EMPTY |
| C-4 | Operator-approved acceptance cases (approved issue, acknowledgement, tracking, completion) passed only after shadow acceptance, under direct operator supervision; any failure reverted the site to shadow | `17_...md` §5; `26_...md` §6 promotion decisions | NEVER EXECUTED; no promotion requested |
| C-5 | Commissioning completed at the site with every step passed and D6 closed before shadow entry | `13_...md` execution; `06_...md` D6 record | UNSCHEDULED; D6 OPEN |
| C-6 | No must-fix security finding open at the review | `07_...md` findings list | UNEVALUABLE — findings not assessed (LR-039) |
| C-7 | No drill failure open at the review | `20_...md` drill report | UNRUN |
| C-8 | The limitations register is customer-acknowledged for the items affecting the site under decision | `27_...md` §6 acceptance matrix | EVERY ROW UNSIGNED |
| C-9 | Every authority precondition for the site (P9.12 tracker) is APPROVED or APPROVED-WITH-CONDITIONS-verified | `12_...md` §9 | NOT-STARTED |
| C-10 | The deployed configuration matches the control-scope document field-for-field (mode, sites, assets, limits) | `26_...md` §9 verification matrix | UNRUN |

Rule C-X (structural): a site is recorded APPROVED only when every
criterion C-1…C-10 is met for that site on filled evidence, or the
criterion's gap carries a §7.3 waiver. **Not one criterion is
currently met; none can be evaluated on evidence that does not
exist.**

## 6. Review procedure (per the ledger steps; every execution cell EMPTY)

1. **Assemble the pack** (ledger step 1 = §4's checklist): collect
   the records, verify each row's status is filled-and-verified or
   attach a draft waiver for it; circulate the pack to all §3
   attendees before the session.
2. **Convene and record attendance** (§3 table filled at the
   session): customer, site owners, Neryva product/security/power
   systems/platform, independent witness.
3. **Present the evidence by pack row**, not by summary: each §4 row
   is walked with its record path on screen; the witness confirms
   the presented artifact is the filed record.
4. **Record the questions raised and their answers** (ledger step 2)
   in the §7.1 session record.
5. **Record the decision per site** (ledger step 3): approved for
   the stated control scope / approved with conditions (conditions
   listed, with owners and dates) / not approved — §7.2.
6. **Record waivers** (ledger step 4): explicit, customer-signed,
   with rationale and expiry — §7.3.
7. **Record non-approvals** (ledger step 5): what must change and
   which subtasks re-open; the phase is NOT recorded complete — §7.2.
8. **Sign**: every §3 attendee signs; the witness signs after
   checking §6 was followed.

## 7. Decision record — ALL CELLS EMPTY (no review has been convened)

### 7.1 Session record (template — EMPTY)

| Field | Value |
|---|---|
| Session reference | `<EMPTY>` |
| Date, time, location / channel | `<EMPTY>` |
| Attendees (§3 rows filled) | `<EMPTY>` |
| Pack version reviewed (§4 rows + record versions) | `<EMPTY>` |
| Questions raised with answers (per ledger step 2) | `<EMPTY>` |
| Witness attestation | `<EMPTY — UNSIGNED>` |
| Chair signature | `<EMPTY — UNSIGNED>` |

### 7.2 Per-site decision table (template — EMPTY; no site exists)

| Site ID | Decision (approved / approved-with-conditions / not approved) | Control scope approved (cites the signed P9.26 document version) | Conditions (each with owner and date) | Subtasks re-opened if not approved | Decision-maker signature |
|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNSIGNED>` |

Rules (structural, not fillable):

| # | Rule |
|---|---|
| D-1 | No site is approved without its field acceptance record (ledger verification method) — C-1…C-4 must be met on filled evidence |
| D-2 | A decision recorded for a site whose E-rows are open is INVALID; the witness rejects it |
| D-3 | A site with a pending precondition authority approval (P9.12) is recorded at most "approved with conditions" naming that precondition — and per P9.29 step 5 it still does not operate under operator-approved control until the authority approves |
| D-4 | An approval applies ONLY to the stated control scope (the signed P9.26 document version cited in the decision row) and ONLY to the named site — never to the product, any other site, or any asset class outside the scope |

### 7.3 Waiver record (template — EMPTY; no waiver exists)

| Waiver ref | Subtask / criterion waived | Rationale | Expiry (time-bounded) | Customer signature | Date |
|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNSIGNED>` | `<EMPTY>` |

A waiver records a REAL residual risk: it must be carried to the
P9.27 limitations register as a dated amendment (that register's
§1/§7 procedure — not an in-place edit) and re-examined at its expiry.

## 8. The review OUTCOME — EMPTY

| Outcome field | Value |
|---|---|
| Review convened? | **NO** |
| Pack presented? | **NO** — the pack rows exist as templates, not evidence |
| Sites decided | **NONE** — no site exists |
| Conditions recorded | **NONE** |
| Waivers recorded | **NONE** |
| Phase 9 exit gate met by this record? | **NO** — the gate ("customer/site and applicable authorities approve the deployment and its control scope") is unmet; partial approval (e.g. read-only/shadow only) would still not close Phase 9 (ledger phase-exit note) |

**Nothing is accepted. No acceptance exists or is claimed.**

## 9. What acceptance would and would not establish

What a recorded, signed acceptance (per §7.2) WOULD establish:

| # | Established |
|---|---|
| W-1 | The named customer accepted the named deployment at the named sites, for the stated control scope, on the evidence pack as reviewed |
| W-2 | The site-specific acceptance required by §21.4/§22.3 for the operator-approved mode promotion exists — one of the two §22.3 inputs (this acceptance + the applicable-authority approval, P9.29) |
| W-3 | The accepted control scope is the signed P9.26 document version cited in the decision row — no wider |

What it would NOT establish (each verbatim-bounded):

| # | Not established | Basis |
|---|---|---|
| N-1 | Any certification or safety case — not of FlexCore, any adapter, any inverter, or any site | Ledger scope note; AGENTS.md §3 rule 2 |
| N-2 | Bounded automation — post-1.0, per deployment, site, and asset class, after separate qualification | §21.4 mode row (verbatim in `26_...md` §8); ENTRY_NOTE boundary 3 |
| N-3 | Field write authority beyond operator-approved: every command still requires an authorized human approval; each approval covers that command only | §21.4 operator-approved row (verbatim in `26_...md` §3) |
| N-4 | Any dispatch authority for plugins — `dispatch.authorize`/`dispatch.issue` are never granted to plugins regardless of acceptance | `NEVER_GRANTED_PERMISSIONS` (`26_...md` §7); §5.7 |
| N-5 | Protection-grade, primary-response, or sub-cycle capability — FlexCore is supervisory orchestration only | AGENTS.md §3 rule 3; `27_...md` L-6 |
| N-6 | Approval of any OTHER site, asset class, or customer — acceptance is site-specific (LR-123, LR-161: one-site evidence) | §7.2 rule D-4 |
| N-7 | Phase 9 closure by itself — the exit gate also requires the applicable-authority approval (P9.29) and every other subtask checked | Ledger phase-exit gate; §23 Stage 9 |

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement stands: no attendee named, no
   decision recorded, no waiver, no signature anywhere in this
   record; §8's outcome table reads NO/NONE in every row.
2. Confirm §4's status column is honest: spot-check at least five
   E-rows against their record paths and confirm each status matches
   the record's own headline; any row found filled would be a defect
   in THAT record, not permission to proceed here.
3. Confirm §5's criteria are registered BEFORE any FAT result
   exists (this file's date vs `17_...md`'s empty run record) and
   that no criterion cites a number produced after the fact (X-8
   discipline).
4. Confirm §7's tables are entirely `<EMPTY>`/UNSIGNED and that
   rules A-1…A-4, E-1…E-3, D-1…D-4, and C-X are intact.
5. Confirm §9's not-established table quotes the verbatim bases and
   adds no capability claim.
6. When a real review is convened: verify the signed session record
   against the evidence pack row by row (ledger verification method:
   "confirm the decision per site matches the evidence pack — no
   site approved without its field acceptance record"), the witness
   attestation, and each waiver's signature and expiry.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the review TEMPLATE is written; no review was held.

## 11. Filling commands (for the human executor)

- Assemble the pack by §4: for each row, copy the record path into
  the review pack and verify its status by reading the record's own
  headline — never by trusting a summary.
- Convene the review only after every E-row is filled-and-verified
  or carries a drafted §7.3 waiver; fill §3 at the session; run §6.
- Fill §7.1/§7.2/§7.3 during and immediately after the session; a
  condition accepted verbally and never recorded does not exist.
- File the signed session record as the P9.28 evidence named in
  `CHECKLIST.md`; hand the per-site decisions to P9.29 (its packs
  attach "the customer acceptance record").
- Carry every waiver and condition to the P9.27 limitations register
  AS A DATED AMENDMENT under that register's versioning procedure
  (`27_...md` §1/§7 — new LR-ids, never an in-place edit); P9.30's
  exit record then cites the amended register version.
- Unblock order: this record blocks P9.29 (which needs §7.2's
  decisions) and P9.30 (which needs the waivers/conditions carried).

## 12. Open risks for P9.27 (to be recorded AS DATED AMENDMENTS to the register — not as edits to it)

Per `27_limitations_residual_risk.md` §1/§7, changes to the register
are DATED AMENDMENTS assigning new LR-ids; this record does not edit
the register. The rows below are the amendment content a register
amendment should trace:

| Proposed LR-id (next free) | Source row | Traced risk (short) |
|---|---|---|
| LR-212 | 28 §2 | The review cannot be convened: every dependency P9.01–P9.27 is a template or UNRUN — the acceptance decision has no input |
| LR-213 | 28 §4 E-1 | Acceptance granted on a slide deck instead of the evidence pack — guarded by E-1 structurally, but the guard is paper until humans execute it |
| LR-214 | 28 §5 | Criteria pre-registered against FAT results that are EMPTY — a review held before any FAT result exists would evaluate nothing |
| LR-215 | 28 §7.3 | A waiver records a REAL residual risk and must reach the P9.27 register as a dated amendment; a waiver that never reaches the register is an invisible acceptance of risk |
| LR-216 | 28 §7.2 D-3 | A site approved while a precondition authority approval is pending — the P9.29 ordering rule must hold at the promotion, not just at the review |
| LR-217 | 28 §8 | The Phase 9 exit gate is unmet; any record implying customer acceptance exists would be fabrication — the outcome table is the guard |
| LR-218 | 28 §9 | Acceptance over-read: a signed site acceptance misread as certification, bounded-automation permission, or fleet-wide approval |

Status: **WRITTEN-UNRUN** — a review template cannot be "run"; the
convened review, the attendees, the evidence presentation, the
decisions, and the signatures are the human gates
(ENTRY_NOTE.md boundary 1). No review has been convened, nothing is
accepted, and no acceptance is claimed.
