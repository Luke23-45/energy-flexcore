<!-- P9.29 — Applicable-authority approval pack (PACK TEMPLATE + assembly structure). The pack structure assembles: scope freeze, network model validation, security assessment, OT/IT D6 record, FAT evidence, control-scope document, limitations register, the P9.12 engagement evidence, and the audit-export bundle capability from P9.32 by citation — every item's assembly status TEMPLATE/EMPTY. The submission/approval workflow is UNSIGNED and NO AUTHORITY IS NAMED. What an approval would permit (operator-approved mode entry, v1.0.0 ladder verbatim) and what it would never permit (bounded automation post-1.0; dispatch-authority plugins — never-granted list verbatim) are stated verbatim. Nothing submitted, nothing approved, nothing claimed. -->
# P9.29 — Applicable-Authority Approval Pack (Pack Template — nothing submitted, no authority named, no approval exists)

Date: 2026-11-14. Subtask: P9.29 (`ledger_phase_9.md` §P9.29).
Owner role: Product (with legal and power systems). Report trace:
§23 Stage 9 exit gate ("applicable authorities approve"); §22.3.
Inputs per the ledger: P9.12 tracker (`12_authority_engagement.md`
§9 — NOT-STARTED, every row `<EMPTY>`); P9.28 acceptance decisions
(`28_acceptance_review.md` §7.2 — NONE RECORDED); P9.26 control-scope
document (`26_control_scope_document.md` — PREPARED TEMPLATE, NOT
APPROVED).

## 1. Honest status — the plain statement

**No authority has been named, no pack has been assembled for a named
authority, nothing has been submitted, no submission reference
exists, and no approval, condition, or decision exists or is
claimed.** No pilot customer, site, or jurisdiction exists (D1 OPEN),
so even the applicable-authority LIST cannot be filled — it comes
from the P9.12 tracker (§4 of that record), whose four authority rows
are all `<EMPTY>` / NOT-STARTED. Approval is a HUMAN/ORGANIZATION
gate (ENTRY_NOTE.md boundary 1); an agent cannot submit to or obtain
an approval from an authority, and this record does not pretend to.

This record delivers the PACK STRUCTURE only: what each authority
pack assembles, item by item, with each item's honest assembly status
— all TEMPLATE/EMPTY (§3) — the submission/approval workflow, which
is UNSIGNED end to end (§4), what an authority approval WOULD permit
(operator-approved mode entry per the v1.0.0 release ladder, quoted
verbatim, §5) and what it would NEVER permit (bounded automation —
post-1.0 verbatim; dispatch authority for plugins — the kernel's
never-granted list verbatim, §6), the decision tracker with every
row EMPTY (§7), and the reviewer/filling procedures (§8–§9).

The ledger's acceptance criteria for P9.29 are NOT met and are NOT
claimed: (a) no authority has a submitted pack or a not-required
basis — every authority row is NOT-STARTED; (b) no required approval
has a decision or a follow-up date; (c) no site operates under any
control mode at all, so criterion (c)'s prohibition is trivially
satisfied but nothing is evidenced; (d) no authority conditions
exist to track. The named failure modes — "an authority assumed to
have approved because it did not object", "a pack submitted with a
superseded control-scope version", "a precondition approval pending
while the site is already issuing commands" — are structurally
guarded by §4's vocabulary and §5/§7's rules, but the guard is paper
until humans execute the submissions.

## 2. Why no pack can be assembled or submitted yet — the dependency statement

| Dependency | Current status | Consequence |
|---|---|---|
| D1 (jurisdiction and market) | OPEN — `.agent/DECISIONS.md`; `01_...md` §3.H | the applicable authorities, the locally adopted interconnection standard, and the submission channels are unrecorded |
| P9.12 authority tracker | NOT-STARTED — `12_authority_engagement.md` §9 | no authority row, no approval-needed definition, no submission reference — P9.29 assembles FOR authorities P9.12 identifies, and there are none yet |
| P9.12 §8 evidence-pack structure | TEMPLATE — every source status UNAVAILABLE | this record's §3 is the pack-level instantiation of that structure; it inherits every EMPTY source |
| P9.28 acceptance decisions | NONE — `28_acceptance_review.md` §7.2 (no review convened) | the ledger makes P9.28 an input to P9.29: the packs attach the customer acceptance record, which does not exist |
| P9.26 control-scope document | PREPARED TEMPLATE, NOT APPROVED — `26_control_scope_document.md` | a pack citing a control-scope document that no one signed would submit a design as if it were a decision |
| P9.32 audit-export capability | CODE WRITTEN-UNRUN — `32_audit_integrity_proof.md` | the integrity evidence the pack attaches is citable by path but does not exist as a produced export (§3 row P-09) |
| P9.03 network model | PLAN — WRITTEN-UNRUN — `03_network_model_build.md` | no frozen model version, no validation residuals — the technical submission has no model to cite |

## 3. The pack structure — what each authority pack assembles (every item TEMPLATE/EMPTY)

The ledger step 1: for each authority in the P9.12 tracker, assemble
the pack IT requires — "do not send an authority a pack written for a
different authority's requirements". The item list below is the
COMMON skeleton; the per-authority subset, format, and language are
selected at assembly time from the authority's own requirements
(P9.12 §7 step 3), never assumed. **Every item's assembly status is
TEMPLATE/EMPTY as of 2026-11-14.**

| # | Pack item | Source (record path) | What it contributes | Assembly status |
|---|---|---|---|---|
| P-01 | Scope freeze extract (customer, sites, assets, service, exclusions, signatories) | `01_pilot_scope_freeze.md` | what deployment is being approved | TEMPLATE — UNSIGNED |
| P-02 | Network model reference: frozen model version, provider, class, validation residuals, coverage limitations | `03_network_model_build.md` | the grid model behind the control scope | PLAN — WRITTEN-UNRUN; NO MODEL VERSION EXISTS |
| P-03 | Interconnection standard/profile per site (as locally adopted; category, utility profile, settings) | `12_authority_engagement.md` §5 | the standard the site operates under | TEMPLATE — EMPTY |
| P-04 | Deployment security assessment (§17.4 threat→control table, findings list) | `07_deployment_security_assessment.md` | the cyber posture of the deployment | TEMPLATE — EMPTY; FINDINGS NOT ASSESSED |
| P-05 | OT/IT boundary review — D6 closure record (per deployment, signed) | `06_ot_it_boundary_d6.md` | the reviewed zone boundaries and flow decisions | OPEN — UNSIGNED |
| P-06 | FAT evidence: pre-registered plan + execution record (shadow results, signed shadow review, operator-approved results) | `16_field_acceptance_plan.md`; `17_field_acceptance_execution.md` | the site-specific acceptance results | PLAN PRE-REGISTERED; RUN RECORD EMPTY, UNRUN |
| P-07 | Control-scope document (per site/asset class; mode, limits, validity, expiry) | `26_control_scope_document.md` | the control scope approval is sought FOR | PREPARED TEMPLATE — NOT APPROVED |
| P-08 | Limitations and residual-risk register (customer-acknowledged version) | `27_limitations_residual_risk.md` | what the deployment does not do and still risks | v0.1-DRAFT — ASSESSMENTS EMPTY, UNSIGNED |
| P-09 | Authority-engagement evidence: P9.12 tracker rows for THIS authority (approval needed, submission ref, status) + any required-study results | `12_authority_engagement.md` §6/§9 | the engagement trail and decision context | NOT-STARTED |
| P-10 | Audit-export bundle capability, BY CITATION to P9.32: the audit-integrity mechanism (append-only hash chain + platform-retained anchor, `32_audit_integrity_proof.md` §1) and the P9.32 ledger step "verify an audit export exists for authority review (P9.29) with the integrity evidence attached" | `32_audit_integrity_proof.md` §1–§5 | the authority-reviewable audit export and its integrity evidence | CODE WRITTEN-UNRUN — NO EXPORT EXISTS; the capability is cited, never produced here |
| P-11 | Customer acceptance record (the P9.28 per-site decision, signed) | `28_acceptance_review.md` §7.2 | the customer/site decision the ledger makes an input | NONE — NO REVIEW CONVENED |
| P-12 | HIL evidence (only where the authority requires it) | `15_hil_test_plan.md` | equipment-in-the-loop results on pilot firmware | PLAN — RIG NOT PROCURED, UNRUN |
| P-13 | Market registration/aggregation evidence (only if P9.11 recorded participation) | `11_market_program_requirements.md` §6.C | market-operator qualification evidence | TEMPLATE — EMPTY; D1 OPEN |

Rules (structural, not fillable):

| # | Rule |
|---|---|
| P-r1 | A pack is assembled PER AUTHORITY from that authority's own requirements — never a generic pack forwarded around (ledger step 1; P9.12 §7 step 3) |
| P-r2 | Every item cites its SOURCE RECORD PATH and VERSION; an item filled from a summary or a superseded version is invalid — the "pack submitted with a superseded control-scope version" failure mode is guarded by requiring the signed `26_...md` document version in P-07 |
| P-r3 | An item whose source record is not filled-and-verified is either left out (if the authority does not require it) or attached as an explicitly marked TEMPLATE/UNRUN artifact with the gap stated — never presented as evidence |
| P-r4 | P-10 attaches the P9.32-produced export and its integrity evidence; this record never generates an export or restates the integrity mechanism's guarantees — the honesty bound of `32_...md` §2 (tamper-EVIDENCE, not tamper-PROOF) applies verbatim to anything the pack claims about audit integrity |

## 4. Submission and approval workflow — UNSIGNED, no authority named

The workflow below is the procedure the humans follow; every
execution cell is EMPTY and no authority name exists anywhere.

| Step | Action | Performed by | Record cell | Status |
|---|---|---|---|---|
| S-1 | Select the authority's pack subset per its requirements (P-r1) | Product + legal | `<EMPTY>` | NOT-STARTED |
| S-2 | Verify each included item against its source record version (P-r2/P-r3) | Product | `<EMPTY>` | NOT-STARTED |
| S-3 | Submit; record the submission reference and date IMMEDIATELY (ledger step 2) | The party the process designates (customer or Neryva per contract — `<EMPTY>`) | `<EMPTY>` | NOT-STARTED |
| S-4 | Track the response in §7 (ledger step 3): APPROVED / APPROVED-WITH-CONDITIONS / NOT-REQUIRED (with cited basis) / PENDING with a NAMED follow-up date | Product | `<EMPTY>` | NOT-STARTED |
| S-5 | Record authority conditions, assign owners, and verify them BEFORE the affected site operates under the approved scope (ledger step 4) | Product + power systems | `<EMPTY>` | NOT-STARTED |
| S-6 | If a precondition approval is not granted, the site does not operate under operator-approved control (ledger step 5) | Product (scope) | `<EMPTY>` | NOT-STARTED |

Status vocabulary (fixed, inherited from `12_...md` §9): `NOT-STARTED`,
`UNSCHEDULED`, `PREPARING`, `SUBMITTED` (with reference + date),
`PENDING` (with named follow-up date), `APPROVED`,
`APPROVED-WITH-CONDITIONS` (conditions tracked to verification),
`NOT-REQUIRED` (with cited basis). **A pending item with no named
follow-up date is open, not "in hand"** (ledger step 3). Approval is
never assumed from silence — "no objection ≠ approved" is the named
P9.29 failure mode and the scope rule in `12_...md` §3.

## 5. What an authority approval WOULD permit — operator-approved mode entry per the v1.0.0 ladder (verbatim)

An authority approval (recorded APPROVED in §7) would — together
with the P9.28 customer/site acceptance — permit the affected site
to enter **operator-approved control**, the v1.0.0 mode. The ladder
row that defines this ceiling, quoted verbatim from the report (§2.1,
via `26_control_scope_document.md` §2 — the same ladder appears in
`AGENTS.md` §4):

> | **v1.0.0 — Pilot-ready FlexCore** | Stable operator workflow, reproducible benchmark, deployment/upgrade model, security review, site integration test harness | Operator-approved control is the default; production dispatch only after customer-specific acceptance and commissioning |

And the §21.4 mode row that governs what that mode does, verbatim
(`26_...md` §3):

> | Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |

And the §21.4 promotion rule, verbatim:

> "Control-authority mode is a deployment and per-site setting, not
> something a plugin can select. A deployment may run mixed modes by
> site or asset class. Promotion to a higher-authority mode requires
> the release gates in §22.3 plus site-specific acceptance."

And the §22.3 production-pilot sentence, verbatim (report §22.3):

> "A production field pilot additionally requires site owner approval,
> device/vendor integration tests, local safety review, commissioning,
> and any applicable market/interconnection qualification. Unresolved
> Phase 0 gates in §25.2 must be closed for the deployment
> jurisdiction, network class, and protocol mix before that deployment
> is promoted beyond read-only mode."

Scope of what the approval permits (structural): entry of the named
site into operator-approved mode for the signed P9.26 control-scope
version cited in the pack — each command still requiring an
authorized human approval, and only after every §22.3 gate and the
site-specific acceptance are evidenced. **No approval is recorded; no
site has entered any mode; nothing is permitted yet.**

## 6. What an authority approval would NEVER permit

| # | Never permitted | Verbatim basis |
|---|---|---|
| N-1 | Bounded automation, at any release level, for this pilot | §21.4 mode row: *"Bounded automation | Pre-approved policy envelope with limits, expiry, and automatic fallback | Yes, inside the envelope only | **Post-1.0, per deployment, site, and asset class after qualification**"* — and `ENTRY_NOTE.md` boundary 3: *"Bounded automation is post-1.0. Nothing in Phase 9 grants field write authority; the control-scope document (P9.26) is a design with the operator-approval gate recorded, not an approval."* |
| N-2 | Dispatch authority for any plugin: `dispatch.authorize` (authorize dispatch) or `dispatch.issue` (issue physical command) granted to a plugin | The kernel's never-granted list, verbatim (`crates/flexcore-kernel/src/manifest.rs`, via `26_...md` §7): `pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"];` — a manifest requesting one is rejected before composition, lifecycle, or any broker exists (P7.11 re-refuses at grant time; profile ceilings refuse with `PROFILE_PERMISSION_CEILING_NEVER_GRANTED`) |
| N-3 | Plugin bypass of authorization, tenant isolation, audit, command lifecycle, or final dispatch validation | §5.7 verbatim rows: *"Authorize dispatch | `dispatch.authorize` | Never granted to general plugins"* and *"Issue physical command | `dispatch.issue` | Owned by protected dispatch service and protocol adapter under a signed authorization token"*; §5.7 closing sentence: *"The platform kernel owns final authorization and dispatch state transitions. An optimizer, GNN, MARL policy, or external plugin cannot call device protocols directly."* |
| N-4 | Protection-grade, primary-frequency-response, synthetic-inertia, or sub-cycle capability for anyone | AGENTS.md §3 rule 3; report §0.3; `27_...md` L-6 (supervisory orchestration only) |
| N-5 | Certification or a safety case — of FlexCore, any adapter, any inverter, or any site | Ledger scope note; AGENTS.md §3 rule 2; §2.1 closing sentence: *"A version number does not imply regulatory certification or operational qualification."* |
| N-6 | Approval of any site, asset class, or deployment other than the one the pack names — an approval is site- and scope-specific | §5 scope rule; `12_...md` (D6 per deployment, scope per site); LR-123/LR-161 (one-site evidence) |
| N-7 | Operation beyond the locally adopted interconnection profile recorded in P-03 | `12_...md` §5 (standard "as locally adopted"; FlexCore makes no equipment-conformance claim — IEEE 1547.1-2020 testing is the equipment vendor's responsibility, §11.3) |

No approval, condition, waiver, or scope wording can override N-1–N-7:
they are kernel- and report-level invariants, not pack-level terms.

## 7. Decision tracker — the deliverable, ALL ROWS EMPTY

One row per P9.12 authority; the tracker is filled only from primary
sources (the authority's own correspondence; the scope rule in
`12_...md` §3). **No authority exists to fill a row for.**

| Authority (P9.12 §4 row) | Pack items included (§3 subset per P-r1) | Submission ref | Submission date | Status | Decision | Conditions (each with owner + verification state) | Follow-up date |
|---|---|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Rules (structural, not fillable):

| # | Rule |
|---|---|
| T-1 | Every P9.12 authority ends at APPROVED, APPROVED-WITH-CONDITIONS, or NOT-REQUIRED-with-cited-basis — no item silently disappears (ledger acceptance (a)) |
| T-2 | Every required approval has a recorded decision or a named follow-up date; a PENDING row without one is OPEN (ledger acceptance (b)) |
| T-3 | No site operates under operator-approved control while a precondition approval for that site is pending (ledger acceptance (c); step S-6) |
| T-4 | Authority conditions are tracked to verification before the affected site operates under the approved scope (ledger acceptance (d); step S-5) |
| T-5 | A pack submitted with a superseded P9.26 control-scope version is void; the pack and the decision must cite the SAME signed document version |

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement stands: no authority named, no
   submission reference, no decision, no condition, no approval
   anywhere in this record; §7's tracker is entirely
   `<EMPTY>`/NOT-STARTED.
2. Confirm §3's assembly-status column is honest: spot-check at
   least five P-rows against their source records' headlines; any
   found filled would be a defect in THAT record, not evidence here.
3. Confirm P-10 cites P9.32 BY PATH and section (`32_...md` §1–§5 and
   the ledger step) and does not restate or extend the integrity
   guarantees — the §2 honesty bound (tamper-EVIDENCE) carries
   verbatim into anything the pack says.
4. Confirm §5 quotes the v1.0.0 ladder row, §21.4 rows, and §22.3
   sentence verbatim, and that §6's N-1–N-7 rows carry their verbatim
   bases unchanged; no wording anywhere narrows or softens them.
5. When packs are assembled and submitted: verify each §7 row traces
   to authority correspondence (primary source), each SUBMITTED row
   has a reference and date, each PENDING row has a named follow-up
   date, each NOT-REQUIRED row cites its basis, and each pack's P-07
   cites the signed control-scope version the decision grants (T-5).
6. Confirm no site was promoted to operator-approved control while a
   §7 precondition row was pending (T-3) — cross-check against the
   P9.26 §6 promotion decisions.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the pack TEMPLATE and workflow are written; nothing
   was submitted or approved.

## 9. Filling commands (for the human executor)

- After P9.12 §4 is filled (post-D1-closure, post-signed freeze):
  instantiate §3 per authority (P-r1), selecting only the items that
  authority requires; mark each included item's version and date.
- Verify each included item against its source record (P-r2/P-r3);
  attach TEMPLATE/UNRUN artifacts only with the gap explicitly
  marked.
- Submit (S-3) and record the reference and date in §7 immediately;
  track decisions with the §9 vocabulary; set follow-up dates on
  every PENDING row (T-2).
- Record and verify conditions (S-5) BEFORE the affected site
  operates under the approved scope; check T-3 at every promotion.
- File the filled tracker as the P9.29 evidence named in
  `CHECKLIST.md`; hand every authority condition to the P9.27
  limitations register AS A DATED AMENDMENT (that register's §1/§7
  procedure) so P9.30's exit record and Phase 10 cannot lose it.
- Unblock order: P9.12 → P9.28 → P9.29 → P9.30. While §7 is empty,
  no site may be promoted beyond read-only mode on the strength of
  an approval that does not exist.

## 10. Open risks for P9.27 (to be recorded AS DATED AMENDMENTS to the register — not as edits to it)

Per `27_limitations_residual_risk.md` §1/§7, changes to the register
are DATED AMENDMENTS assigning new LR-ids; this record does not edit
the register. The rows below are the amendment content a register
amendment should trace:

| Proposed LR-id (next free) | Source row | Traced risk (short) |
|---|---|---|
| LR-219 | 29 §2 | No pack can be assembled: P9.12 has no authority rows, P9.28 has no acceptance decisions, P9.26 is unsigned — every P-01…P-13 source is TEMPLATE/EMPTY |
| LR-220 | 29 §3 P-r3 | A TEMPLATE/UNRUN artifact attached to a pack without the explicit gap marker could be misread as evidence by the authority |
| LR-221 | 29 §4 | Approval assumed from silence (no objection ≠ approved) — the named failure mode; the vocabulary and scope rule guard it on paper only |
| LR-222 | 29 §3 P-07 / T-5 | A pack submitted with a superseded control-scope version voids the pack; version-citation discipline is human-enforced |
| LR-223 | 29 §5 | An authority approval over-read as certification, bounded-automation permission, or plugin dispatch authority — N-1–N-7 must hold at decision time, not just in this template |
| LR-224 | 29 §7 T-3 | A precondition approval pending while the site already issues commands — the ordering rule binds the promotion, and nothing yet enforces it mechanically beyond the P9.26 §6 record |
| LR-225 | 29 §3 P-10 | The audit-export capability cited to the authority is CODE WRITTEN-UNRUN — an export promised in a pack does not exist until P9.32's step-4 verification runs |

Status: **WRITTEN-UNRUN** — a pack template and workflow cannot be
"run"; the authority identification, pack assembly, submissions,
decisions, and signatures are the human gates (ENTRY_NOTE.md
boundary 1). No authority has been named, nothing has been
submitted, nothing is approved, and none of it is claimed.
