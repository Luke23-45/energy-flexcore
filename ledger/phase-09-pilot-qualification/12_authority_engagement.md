<!-- P9.12 — Interconnection and authority engagement (engagement PLAN + tracker TEMPLATE). Every engagement UNSCHEDULED / NOT-STARTED; no authority name, agreement status, or submission reference invented. Engagement itself is a HUMAN/ORGANIZATION gate. -->
# P9.12 — Interconnection and Authority Engagement (Plan + Tracker Template)

Date: 2026-11-14. Subtask: P9.12 (`ledger_phase_9.md` §P9.12).
Owner role: Product (with power systems for technical submissions).
Report trace: §23 Stage 9; §22.3 ("any applicable market/interconnection
qualification"); §2.3.
Inputs: P9.01 freeze (TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md`);
P9.03 network model (per `ledger_phase_9.md` §P9.03 — check its record for
actual status); P9.11 market/program note (TEMPLATE, no market contact —
`11_market_program_requirements.md` §4 EMPTY); the applicable interconnection
requirements — IEEE 1547-2018 **as adopted in the jurisdiction**, per
`RESEARCH.md` §2.

## 1. Honest status — the plain statement

**No authority has been identified by name, contacted, or engaged, and no
engagement is scheduled. No interconnection agreement exists or is claimed,
and no agreement status is asserted.** Which authorities apply depends on the
pilot jurisdiction (D1 OPEN) and on whether the pilot participates in a
market/program (P9.11 EMPTY) — so even the authority LIST cannot be filled
yet, only structured. This record delivers the engagement PLAN: the
authority-inventory structure (§4), the per-site interconnection
standard/profile template (§5), the studies register (§6), the engagement
sequence (§7), the evidence-pack structure feeding P9.29 (§8), and the
engagement tracker with every row UNSCHEDULED/NOT-STARTED (§9).

The ledger's acceptance criteria for P9.12 are NOT met and are NOT claimed:
(a) no applicable authority is listed by name; (b) no item has an
approved/submitted/not-required status — every status is NOT-STARTED;
(c) the per-site interconnection standard/profile is a template with EMPTY
cells. Engagement is a HUMAN/ORGANIZATION gate (ENTRY_NOTE boundary 1);
an agent cannot engage an authority and this record does not pretend to.

## 2. Why nothing can be engaged yet — the dependency statement

| Dependency | Status | Consequence |
|---|---|---|
| D1 (target jurisdiction and market) | OPEN — `.agent/DECISIONS.md`; P9.01 §3.H | the jurisdiction determines the distribution utility/DSO, the interconnection authority, and the locally adopted interconnection standard — none can be named |
| P9.01 freeze | TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md` | no named site, so no per-site interconnection record (§5) can exist |
| P9.03 network model | ledger dependency of this subtask | the network model is an input to any distribution impact study or technical submission |
| P9.11 market/program | TEMPLATE, EMPTY — `11_market_program_requirements.md` | a market-operator authority row exists ONLY if P9.11 records participation (Q10 registration path) |
| §22.3 (report) | binding text | "A production field pilot additionally requires site owner approval, device/vendor integration tests, local safety review, commissioning, and any applicable market/interconnection qualification. Unresolved Phase 0 gates in §25.2 must be closed for the deployment jurisdiction … before that deployment is promoted beyond read-only mode." |

## 3. Scope rule — no authority name or status is ever invented

An authority name, an agreement reference, a submission reference, a decision
date, or a "not required" basis enters §9 ONLY from primary sources: the
signed freeze, the customer's records, the authority's own correspondence, or
the program documentation captured by P9.11. A name recalled from memory, a
status assumed from silence ("they did not object, so it is approved" — the
named P9.29 failure mode), or a status filled from a neighbouring
jurisdiction's practice is treated as UNVERIFIED and the reviewer rejects it
(§10, check 3).

## 4. Authority inventory — STRUCTURE ONLY, ALL IDENTITY CELLS EMPTY

The ledger's step-1 list, structured. Each row's identity is `<EMPTY>` and
each row is UNSCHEDULED / NOT-STARTED. No row may be struck without a cited
"not required" basis (§9 status vocabulary).

| Row | Authority role | Authority name | Approval needed | Status |
|---|---|---|---|---|
| 1 | Distribution utility / DSO | `<EMPTY>` | `<EMPTY — e.g. an interconnection application, an operational-notification duty; filled from the jurisdiction's process, never assumed>` | NOT-STARTED |
| 2 | Interconnection authority | `<EMPTY — may be the same entity as the DSO; record only what the jurisdiction's process names, never assume>` | `<EMPTY>` | NOT-STARTED |
| 3 | Market operator (if any) | `<EMPTY — exists only if P9.11 §4 records participation>` | `<EMPTY — e.g. registration/aggregation approval per P9.11 Q10>` | NOT-STARTED |
| 4 | Other applicable authority | `<EMPTY — named only from the customer's records or the jurisdiction's process; a permitting body, a safety regulator — never invented>` | `<EMPTY>` | NOT-STARTED |

The interconnection AGREEMENT itself is a contract between the customer/site
owner and the authority: whether one exists for `<SITE-ID-1>` is
`<EMPTY — per signed freeze §3.B and customer records>`; FlexCore is not a
party to it and this record neither asserts nor supersedes its status.

## 5. Per-site interconnection standard and profile — ALL CELLS EMPTY

The standard applies **as locally adopted**: `RESEARCH.md` §2 records that
adoption varies by jurisdiction (one US state division was still anchored to
the 2003 edition in mid-2026) and that IEEE 1547-2018 is in revision ballot —
the 2018 edition's requirements must be re-checked when the revision
publishes (a P9.27 watch item). FlexCore's use of IEEE 1547-2018 Clause 5
capabilities depends on category, utility profile, and equipment (§11.3);
FlexCore makes NO equipment conformance claim — conformance testing of DER
equipment is IEEE 1547.1-2020's domain and the equipment vendor's
responsibility (§11.3, `RESEARCH.md` §2).

| Field (per site) | Value |
|---|---|
| Site ID (signed freeze §3.B) | `<SITE-ID-1>` |
| Interconnection standard as locally adopted (edition, amendment) | `<EMPTY — from the jurisdiction's adopted code, cited>` |
| Category / abnormal-performance profile (per the adopted edition) | `<EMPTY>` |
| Utility profile / settings (Clause 5 capabilities in force at the site) | `<EMPTY>` |
| Equipment-side conformance status (IEEE 1547.1-2020, vendor's) | `<EMPTY — vendor's evidence; never claimed by FlexCore>` |
| Locally adopted edition ≠ 2018? (superseded-edition watch) | `<EMPTY>` |

## 6. Authority-required studies register — ALL CELLS EMPTY

The ledger's step 4: any study the authority requires (e.g. a distribution
impact study, cf. IEEE 1547.7) is recorded with who performs it. IEEE
1547.7-2013 is in revision per `RESEARCH.md` §2. FlexCore does not perform
distribution impact studies; the performer is the authority's designated
party or the customer's engineer, recorded from the jurisdiction's process.

| Study | Required by (authority row, §4) | Basis (process document, cited) | Performer | Status |
|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED |

A study discovered late — during field acceptance rather than engagement —
is the ledger's named failure mode; the §7 sequence front-loads the question
to guard against it, and the residual risk is carried to P9.27 (§11).

## 7. Engagement sequence — the plan (every step NOT-STARTED)

1. **Derive the authority list.** From the signed freeze (site, jurisdiction),
   the D1 closure, the customer's records (existing interconnection
   agreement, if any), and the P9.11 capture (market-operator row). Output:
   §4 rows filled with names and the approval each requires.
2. **Classify each approval.** Precondition for site operation (blocks P9.28
   for that site, ledger dependency line) vs informational/notify-only. A
   "not required" classification needs a cited basis (§9 vocabulary).
3. **Prepare the submission per authority** from the §8 evidence pack —
   never a pack written for a different authority's requirements (P9.29
   step 1).
4. **Submit.** A human action by the party the process designates (the
   customer or Neryva per the agreement — `<EMPTY>` per contract); record
   the submission reference and date in §9 immediately.
5. **Track to decision.** Status vocabulary per §9; a PENDING item with no
   named follow-up date is open, not "in hand" (P9.29 step 3).
6. **Record conditions.** Where an approval carries conditions, record them,
   assign owners, and verify before the affected site operates under the
   approved scope (P9.29 step 4).
7. **Hand to P9.29.** The filled tracker plus per-authority evidence packs
   feed the P9.29 approval-pack submission; this record owns engagement and
   tracking, P9.29 owns the pack submission and decisions.
8. **Schedule.** Every engagement is UNSCHEDULED: dates are set by humans
   against the pilot schedule after step 1, and none is invented here.

## 8. Evidence-pack structure (feeding P9.29) — per authority, every source-status cell UNAVAILABLE

P9.29 assembles, per P9.12 authority, the pack that authority requires
(scope, interconnection standard/profile, HIL evidence where relevant,
control scope, customer acceptance record). The sources below are named by
subtask; every source status is honestly NOT AVAILABLE while the pilot does
not exist.

| Pack item | Source subtask / record | Status |
|---|---|---|
| Scope extract (customer, sites, assets, service, exclusions) | P9.01 — `01_pilot_scope_freeze.md` | TEMPLATE, UNSIGNED |
| Network model reference (versioned, frozen, with validation residuals) | P9.03 — `03_network_model_build.md` | TEMPLATE, WRITTEN-UNRUN |
| Interconnection standard/profile per site | §5 of this record | TEMPLATE, EMPTY |
| Study results (if a §6 study is required) | §6 of this record | NOT-STARTED |
| HIL evidence (where the authority requires it) | P9.15 — UNRUN per ENTRY_NOTE boundary 6 | UNRUN |
| Control-scope document | P9.26 — not yet written (wave 7) | NOT WRITTEN |
| Customer acceptance record | P9.17 — shadow acceptance, UNRUN | UNRUN |
| Market registration/aggregation evidence (if P9.11 recorded participation) | P9.11 §6.C | TEMPLATE, EMPTY |

## 9. Engagement tracker — the deliverable, ALL ROWS NOT-STARTED

Status vocabulary (fixed; from P9.29's decision vocabulary plus the
pre-engagement states): `NOT-STARTED`, `UNSCHEDULED`, `PREPARING`,
`SUBMITTED` (with reference + date), `PENDING` (with named follow-up date),
`APPROVED`, `APPROVED-WITH-CONDITIONS` (conditions tracked to verification),
`NOT-REQUIRED` (with cited basis). No other value is valid; a tracker with a
value outside this vocabulary is rejected by the reviewer.

| Authority (§4 row) | Approval needed | Submission ref | Submission date | Status | Decision | Follow-up date |
|---|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | NOT-STARTED | `<EMPTY>` | `<EMPTY>` |

Rules (pre-printed, not fillable): (i) every listed item ends at APPROVED,
APPROVED-WITH-CONDITIONS, or NOT-REQUIRED-with-basis — no item stays
unlisted and none silently disappears (ledger acceptance (b)); (ii) a
"not required" without a cited basis is invalid; (iii) where an approval is
a precondition for a site's operation and it is not granted, that site does
not operate under operator-approved control (P9.29 step 5).

## 10. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged and no authority name, agreement
   status, submission reference, or decision exists anywhere in the record.
2. Confirm every §4/§5/§6/§9 identity/status cell is `<EMPTY>` or
   NOT-STARTED and every status value is within the §9 vocabulary.
3. Traceability check for a filled (future) tracker: every authority row
   traces to the customer's records, the freeze, the P9.11 capture, or
   authority correspondence (primary sources); every "not required" cites a
   basis; every SUBMITTED row carries a reference.
4. Confirm §5 marks the standard "as locally adopted" with the
   superseded-edition watch intact and no FlexCore equipment-conformance
   claim.
5. Confirm §8's source-status column is honest (TEMPLATE/UNRUN/NOT WRITTEN)
   and P9.29 receives nothing but statuses from this record.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the plan and tracker are written; no engagement happened.

## 11. Filling commands (for the human executor)

- Fill §4 rows 1–4 after the D1 closure and the signed freeze, from the
  customer's records and the jurisdiction's interconnection process
  (primary sources); strike a row only with a cited not-required basis.
- Fill §5 per site from the locally adopted standard text and the utility's
  published profile; cite the adoption instrument.
- Fill §6 from the authority's process documents; record the performer.
- Fill §9 as submissions happen (§7 steps 4–6); set the schedule in §7 step 8
  against the pilot schedule once step 1 is complete.
- File the completed tracker as the P9.12 evidence named in `CHECKLIST.md`
  ("Authorities engaged"); hand §8 + §9 to P9.29.
- Downstream unblock order: this record feeds P9.29 (approval pack) and
  blocks P9.28 where an approval is a precondition. While §9 is empty,
  P9.29 has nothing to assemble and no site may be promoted beyond
  read-only mode on the strength of an approval that does not exist.

## 12. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| D1 OPEN — the jurisdiction, therefore the authorities and the locally adopted interconnection standard, are unrecorded; engagement cannot begin | §2, §4; `.agent/DECISIONS.md` |
| No authority has been engaged; every engagement is UNSCHEDULED and the schedule depends on a freeze that is UNSIGNED | §1, §7 step 8 |
| An authority engaged late, after the pilot schedule is fixed — the named failure mode, structurally front-loaded by §7 but dependent on human execution | ledger P9.12 risk row; §7 |
| Assuming a neighbouring jurisdiction's adoption of IEEE 1547-2018 applies locally | §3, §5; `RESEARCH.md` §2 |
| A required impact study discovered during field acceptance | ledger P9.12 risk row; §6 |
| IEEE 1547-2018 revision in ballot — the 2018 edition's requirements may change; a filled §5 must be re-checked when the revision publishes | §5; `RESEARCH.md` §2 (P9.27 watch item) |
| Approval assumed from silence (no objection ≠ approved) — the P9.29 failure mode the §9 vocabulary and §3's scope rule guard against | §3, §9; ledger P9.29 risk row |
| Superseded-edition adoption (e.g. a jurisdiction still anchored to IEEE 1547-2003) making a 2018-based settings record wrong for the site | §5; `RESEARCH.md` §2 |

Status: **WRITTEN-UNRUN** — an engagement plan cannot be "run" by an agent;
engagement is a HUMAN/ORGANIZATION gate (ENTRY_NOTE boundary 1). No
authority has been contacted and no engagement has been scheduled. No pilot
exists and none is claimed.
