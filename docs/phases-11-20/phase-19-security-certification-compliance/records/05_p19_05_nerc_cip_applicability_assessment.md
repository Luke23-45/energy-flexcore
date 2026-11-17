<!-- P19.05 — NERC CIP applicability assessment. HONEST RECORD: an
     APPLICABILITY ASSESSMENT FRAMEWORK plus a voluntary alignment note, a
     supplier flow-down questionnaire TEMPLATE, and a roadmap monitoring
     record. NO regulatory determination is made, no registration status is
     asserted, and no CIP requirement is claimed satisfied. Every
     CIP-standard-specific statement is REQUIRES-THE-STANDARD-TEXT /
     UNVERIFIED — the research sources [S-23][S-24][S-25] are the white
     paper, the roadmap, and the CIP-015-1 page, not the CIP standards'
     requirement texts. The not-a-certification discipline of P10.25 §7
     (§7) carries verbatim. -->
# P19.05 — NERC CIP Applicability Assessment (registration-trigger framework; voluntary CIP-003 Attachment 12 alignment note; CIP-013-style flow-down questionnaire TEMPLATE; roadmap monitoring)

Date: 2026-11-17. Subtask: P19.05 (`ledger_phase_19.md` §P19.05).
Plan trace: `../plan.md` §2 row P19.05, §3 blocking order row 4 (the
supplier questionnaire is the vendor-assessment instrument P19.06 consumes
— by path). Sources: [S-23] (NERC joint white paper on DER aggregators —
privacy and security impacts), [S-24] (NERC CIP Roadmap, January 2026),
[S-25] (NERC CIP-015-1 page); the recorded posture in
`../context.md` §1 ("CIP currently does not apply to DERAs — a voluntary
CIP-003 Attachment 12 baseline is the recorded posture").

## 1. Honest status — the plain statement

**No NERC registration determination exists, none is made here, and none
can be made by a document.** CIP applicability is a registration and
compliance matter resolved between an entity, its regional entity, and
NERC/FERC processes — human and organizational acts. What this record
delivers: the applicability framework (§3 — the triggers FlexCore-as-
aggregator would be monitored against), the voluntary baseline alignment
note (§4), the CIP-013-style supplier flow-down questionnaire TEMPLATE
(§5 — a template with EMPTY answer cells), and the roadmap monitoring
record with a re-check trigger (§6). Every statement about what a CIP
standard requires is **UNVERIFIED / REQUIRES-THE-STANDARD-TEXT**; the
sources cited cover the white paper, the roadmap, and a standards page —
not the requirement texts themselves.

## 2. The recorded posture (cited, unchanged)

The gap analysis and phase context record (`../context.md` §1): CIP
currently does not apply to DERAs; the voluntary CIP-003 Attachment 12
baseline is the recorded posture; CIP-013-style supply-chain flow-down to
suppliers is adopted voluntarily where CIP does not yet apply; the NERC
CIP roadmap (Jan 2026) is monitored for DERA applicability changes. This
record carries that posture forward as a monitoring framework — it changes
nothing about the posture itself.

## 3. Applicability framework — registration triggers monitored, never resolved

CIP applicability follows registration for Bulk Electric System functions
(context §4 glossary row, per [S-23]). The assessment framework below
lists the triggers a FlexCore deployment operating as (or on behalf of) a
DER aggregator would monitor. Each row is a trigger to WATCH — the
trigger's resolution is a human/regulatory determination, and the
standard-specific content of any resulting obligation is
REQUIRES-THE-STANDARD-TEXT.

| # | Trigger monitored | Why it matters (as the research posture records it) | Monitoring owner | Status |
|---|---|---|---|---|
| T-1 | The deployment performs, or is registered to perform, BES functions (per NERC registration rules — REQUIRES-THE-RULES-TEXT) | CIP obligations attach to registered entities performing BES functions; DER aggregation status alone does not create registration | Deployment compliance owner (human) | NOT-STARTED |
| T-2 | A regulator (NERC/FERC) or regional entity asserts aggregator applicability by order, rule, or guidance | The recorded posture (CIP not applying to DERAs) can change by process, not by software | Compliance owner | NOT-STARTED |
| T-3 | The CIP roadmap's DERA-related work items mature into rule changes [S-24] | The Jan 2026 roadmap is the recorded monitoring surface for future applicability | Compliance owner | MONITORED-SURFACE-ONLY (no monitoring event has occurred) |
| T-4 | A contracted market/utility program imposes CIP-like obligations contractually | Contractual flow-down can arrive before regulatory applicability; it is a contract matter, not a registration one | Commercial owner | NOT-STARTED |
| T-5 | The deployment's sites include a customer entity whose own registered status pulls FlexCore operations into a compliance perimeter | The customer's compliance obligations may cascade by contract | Compliance owner | NOT-STARTED |

**No trigger is asserted as fired. No threshold (MW, asset count, or
otherwise) is invented** — any such threshold would be a regulatory fact
REQUIRING the rules text; where the white paper discusses aggregator
exposure categories [S-23], its discussion is the citation, not a
determination adopted here.

## 4. Voluntary CIP-003 Attachment 12 alignment note

The recorded posture adopts the voluntary baseline concept as an alignment
NOTE, with the same honesty discipline:

| Item | Recorded content |
|---|---|
| What is adopted | The CONCEPT of a voluntary, low-burden cybersecurity baseline for entities not subject to full CIP — as recorded in `../context.md` §1 per [S-23] |
| What is NOT adopted | Any specific CIP-003 Attachment 12 requirement text — the attachment's requirements are REQUIRES-THE-STANDARD-TEXT and are not reproduced, paraphrased as obligations, or claimed followed anywhere in this program |
| How FlexCore relates today | The controls a baseline would look for overlap with the §17.3 control inventory (16 controls, frozen — `ledger/phase-00-product-freeze/17_threat_model.md` §4). That overlap is a posture statement, never a compliance claim |
| Status | Alignment note only. No baseline document is filed, no entity status is asserted, and nothing here states FlexCore "meets" the baseline |

## 5. CIP-013-style supply-chain flow-down questionnaire — TEMPLATE (supplier answers EMPTY)

The CIP-013 concept adopted voluntarily (context §4 glossary): supply-chain
risk management with vendor flow-down. What follows is the questionnaire
TEMPLATE suppliers receive — a template whose questions are this program's
own supply-chain risk questions styled after the CIP-013 flow-down
CONCEPT (the standard's actual requirement text is
REQUIRES-THE-STANDARD-TEXT and is not reproduced). Every answer cell is
EMPTY; no supplier has received it (P19.06 consumes it as the vendor
assessment instrument — by path, not restated there).

| # | Question (template) | Supplier answer | FlexCore review disposition |
|---|---|---|---|
| Q-1 | Does your organization maintain an SBOM for the delivered component? In which format (CycloneDX / SPDX / other)? | `<EMPTY>` | `<EMPTY>` |
| Q-2 | What is your vulnerability-disclosure and coordinated-response process, and what notification SLA do you commit to customers? | `<EMPTY>` | `<EMPTY>` |
| Q-3 | Do you provide VEX statements for affected releases? Under what profile, and with what freshness commitment? | `<EMPTY>` | `<EMPTY>` |
| Q-4 | How is your build/distribution pipeline protected (build provenance, artifact signing, release integrity)? Describe or cite published evidence. | `<EMPTY>` | `<EMPTY>` |
| Q-5 | What is your process for managing third-party components you incorporate (intake, scanning, disposition)? | `<EMPTY>` | `<EMPTY>` |
| Q-6 | What is your patch/release cadence for security fixes, and how are backports handled for supported versions? | `<EMPTY>` | `<EMPTY>` |
| Q-7 | Do you support customer security audits or provide evidence of your own security program (self-assessment, third-party review)? — a request for EVIDENCE, never accepted as a certification claim from the supplier | `<EMPTY>` | `<EMPTY>` |
| Q-8 | Are there components with cryptographic functionality? Which, and under what key-management model? | `<EMPTY>` | `<EMPTY>` |
| Q-9 | What are your support-lifecycle and end-of-life commitments for the delivered component? | `<EMPTY>` | `<EMPTY>` |
| Q-10 | Is any delivered component subject to export, end-use, or sanctions restrictions relevant to the deployment jurisdictions? | `<EMPTY>` | `<EMPTY>` |

Template rules (binding on its use):

1. A supplier's answers are CLAIMS received, never evidence accepted —
   each answer is reviewed per the P19.06 program (§5 row 7 applies to
   Q-7 specifically: any certification or attestation a supplier shows is
   recorded as a RECEIVED DOCUMENT, never re-attested or endorsed here).
2. No answer cell ships pre-filled; no supplier is invented; the template
   is distributed only by the human vendor-assessment owner.
3. The disposition vocabulary for review outcomes is the P10.19
   replace/isolate/comply/escalate set (record
   `06_p19_06_supply_chain_security_program.md` §5 — by path).

## 6. Roadmap monitoring record — with re-check trigger

| Item | Recorded content |
|---|---|
| Monitored surface | The NERC CIP roadmap (Jan 2026) [S-24] and successor publications; NERC/FERC rulemaking notices touching DER aggregators; the white paper's follow-on work [S-23] |
| Monitoring owner | `<EMPTY — human assignment>` |
| Monitoring cadence | Threshold registered (X-8): the monitored surfaces are re-checked **at least once per calendar quarter** and **immediately on any trigger event of §3** (T-2 or a roadmap item maturing into a proposed rule). First re-check: NOT-STARTED |
| Re-check trigger (binding) | ANY of: (a) a published NERC/FERC action proposing CIP applicability for DER aggregations; (b) a roadmap work item moving from study to rulemaking; (c) a contractual trigger of T-4 firing. On any trigger: this record's §3 framework is re-assessed by the compliance owner, the posture in §2 is re-confirmed or revised, and the revision is recorded as a dated amendment to THIS record — never as a silent edit |
| First re-check date | `<EMPTY — no re-check has occurred>` |
| Findings from monitoring to date | NONE — no monitoring event has occurred; zero findings is NOT a clean result, it is an unstarted process |

## 7. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

No regulatory determination, registration status, compliance posture, or
standard-conformance claim of any kind is made. CIP applicability is a
regulatory matter for the humans the framework names.

## 8. Reviewer-verification procedure (human)

1. Confirm §1/§3: no trigger is marked fired; no MW/asset threshold is
   invented; every CIP-requirement-adjacent statement is
   REQUIRES-THE-STANDARD-TEXT.
2. Confirm §4: no Attachment 12 requirement text is reproduced or claimed
   met; the alignment is a concept-level note with its citation.
3. Confirm §5: every supplier answer and disposition cell is `<EMPTY>`;
   no supplier name appears.
4. Confirm §6: the cadence threshold is registered (X-8) with the first
   re-check NOT-STARTED and the trigger rule binding.
5. Confirm §7's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 9. Status

**RECORD-ONLY / WRITTEN.** Framework, alignment note, questionnaire
template, and monitoring record exist as documentation. No regulatory
determination exists; no supplier has been assessed; no monitoring event
has occurred.
