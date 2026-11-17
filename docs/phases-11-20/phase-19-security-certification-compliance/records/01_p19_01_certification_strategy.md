<!-- P19.01 — Certification strategy record. HONEST RECORD: this record
     registers a STRATEGY and an OPEN decision (P19-E-1); it closes nothing.
     No certification, attestation, audit, or auditor engagement exists or is
     claimed anywhere. Every timeline cell is EMPTY: observation windows,
     auditor scheduling, and signatures are human/organizational acts that
     cannot be produced by an agent (conventions §5). The not-a-certification
     discipline of P10.25 §7 carries verbatim into this record (§7). -->
# P19.01 — Certification Strategy Record (SOC 2 Type II vs ISO 27001 order; ISASecure SDLA as the edge agent's target; P19-E-1 registered OPEN)

Date: 2026-11-17. Subtask: P19.01 (`ledger_phase_19.md` §P19.01).
Owner role: Compliance (program conventions §2.4 — this record is the
decision record the tick cites).
Plan trace: `../plan.md` §2 row P19.01, §6 row P19-E-1, §5 blocking order
row 1. Sources: [S-80] (SOC 2 Type II mechanics — Vanta/Drata research
pages), [S-22] (ISASecure, isasecure.org), [S-44][S-45] (the US-weighted
commercial precedent: Uplight SOC 2 + penetration tests; Enode SOC 2).
Conventions: `../../01_PROGRAM_CONVENTIONS.md` §4 (E-decisions), §5 (human
gates), §7.3 (certification claims banned).

## 1. Honest status — the plain statement

**No certification process of any kind has started.** No auditor exists,
no observation window has been declared, no readiness assessment has run,
no Statement of Applicability exists, and no certification budget has been
recorded. This record delivers: the certification landscape and the order
question (§3), the registered P19-E-1 decision with its DEFAULT carried,
never closed (§4), the ISASecure SDLA target statement for the edge agent
(§5), the EMPTY timeline table (§6), and the honest dependency statement
(the evidence base does not exist yet — §8). Every timeline cell in §6 is
EMPTY and stays EMPTY until the human owner fills it.

## 2. What this record consumes — and what does not exist

The gap analysis drives the phase: commercial table-stakes #10 ("Security
attestations (SOC 2 Type II minimum)") records FlexCore's posture as
"Nothing: security consolidation verdict NOT APPROVED (P10.25)"
(`../../00_GAP_ANALYSIS.md` §3 row 10). The Phase 0–10 supply-chain
posture this strategy sits on is all NOT CLOSED / WRITTEN-UNRUN, cited at
`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` §10,
`.../20_vuln_scan_closure.md` §10, `.../21_signed_artifacts.md` §12, and
`.../25_security_consolidation.md` §1 — never restated here. The SOC 2
evidence chain over P11–P13 artifacts is P19.10's record (Agent B, records
numbered 21–39 under `../records/` — PENDING-P19.10 until it lands); this
record consumes it by path only.

## 3. The landscape and the order question

| Scheme | What it is (as the research recorded it) | What it consumes | Status for FlexCore today |
|---|---|---|---|
| SOC 2 Type II | An AICPA attestation over controls OPERATED across an observation window; Trust Services Criteria (TSC) with Security mandatory and Availability / Confidentiality / Processing Integrity / Privacy as scoping choices [S-80] | Evidence of control operation over time — the P19.10 TSC map, access reviews (P19.11), vulnerability management (P19.12), IR (P19.16) | Not started; the evidence chain is being mapped, not operated |
| ISO 27001 | An ISMS certification: audited management system (policy, risk assessment, Statement of Applicability, Annex A control catalogue) | A documented, internally audited ISMS. **Annex A specifics are UNVERIFIED in this program's research — every ISO-side statement in this record is REQUIRES-STANDARD-TEXT (context constraint 4)** | Not started; no ISMS document set exists |
| IEC 62443 / ISASecure SDLA | The 62443-4-1 secure-development-lifecycle process certification under the ISASecure scheme family (SDLA = secure development; EDSA/SSA are the device/system schemes) [S-22] | The 4-1 SDL process evidence — mapped in record `02_p19_02_iec_62443_4_1_sdl_program.md` | Not started; practices mapped against existing processes, gap list recorded |

The order question (SOC 2 Type II vs ISO 27001 first, or parallel) is
**P19-E-1 and it is OPEN** — an agent never closes it (conventions §4;
program-level decisions belong to the human owner: certification order is
explicitly named there).

## 4. P19-E-1 — registered OPEN with DEFAULT (a); carried, never closed

| Item | Recorded content |
|---|---|
| Question | Certification order: SOC 2 Type II first, ISO 27001 first, or parallel tracks? |
| Options | (a) SOC 2 Type II first, ISO 27001 follow-on; (b) ISO 27001 first (EU-first market posture); (c) parallel tracks |
| **DEFAULT (implemented as the working posture)** | **(a)** — SOC 2 Type II first for US-market traction (the commercial evidence base is US-weighted [S-44][S-45]); ISO 27001 follow-on; ISASecure SDLA recorded as the edge agent's longer-horizon target [S-22] (§5). The TSC mapping (P19.10, Agent B) is the platform-independent evidence index either way |
| Why the DEFAULT is what it is | The gap analysis's commercial row cites US competitors' SOC 2 posture [S-44][S-45]; SOC 2 Type II is the minimum row in table-stakes #10; ISO 27001's Annex A mapping additionally requires standard text this program does not have (UNVERIFIED) |
| What would close it | The owner recording the market priority and engaging the first auditor (plan §6 P19-E-1 closer column) |
| Status | **OPEN — never closed by an agent.** The DEFAULT is implemented as the ordering in this record's tables; a different order changes sequencing, not the evidence work |

## 5. ISASecure SDLA as the edge agent's certification target

Recorded as the **target** — a longer-horizon human/organizational goal,
never a claim or a schedule:

- The edge agent is the FlexCore surface with a distinct certification
  audience (site/OT operators), and the ISASecure SDLA scheme certifies the
  *development process* behind a product [S-22] — the process this phase
  maps in record `02_p19_02_iec_62443_4_1_sdl_program.md`.
- The edge agent's component-requirement posture is recorded in
  `03_p19_03_iec_62443_4_2_component_mapping.md` (4-2 CR matrix; CR 3.4
  software-integrity evidence anchors to the P18.02 update-manifest record
  `../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`).
- Whether an eventual ISASecure assessment covers the edge agent alone or a
  FlexCore system scope is **UNDECIDED** — recorded as an open scoping
  question for the owner, not assumed.
- ISASecure EDSA (embedded device) / SSA (system) schemes are named in the
  research [S-22]; which scheme(s) FlexCore would eventually enter is
  UNDECIDED and REQUIRES-THE-SCHEME-DOCUMENTS.

## 6. Timeline — every cell EMPTY (human decisions; no date promised)

| Milestone | Owner (human) | Target date | Status |
|---|---|---|---|
| Market-priority decision closing P19-E-1 | human owner | `<EMPTY>` | NOT-STARTED |
| First auditor / assessment body engaged | human owner | `<EMPTY>` | NOT-STARTED |
| SOC 2 Type II observation window declared | human owner | `<EMPTY>` | NOT-STARTED |
| SOC 2 Type I (if the owner chooses a readiness step) | human owner | `<EMPTY>` | NOT-STARTED |
| SOC 2 Type II audit period start / end | human owner | `<EMPTY>` / `<EMPTY>` | NOT-STARTED |
| ISO 27001 follow-on decision (stage 1 / stage 2 scheduling) | human owner | `<EMPTY>` | NOT-STARTED |
| ISASecure SDLA scoping decision | human owner | `<EMPTY>` | NOT-STARTED |
| Budget decision | human owner | `<EMPTY>` | NOT-STARTED |

No date is invented anywhere in this record; the milestone rows exist so
the human owner fills them in one place (plan §7 R-P19-2).

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

This record registers a strategy and an open decision. It does not claim,
schedule, or promise any certification, attestation, audit outcome, or
conformance. The standing version qualification (report §2.1) stands: a
version number implies no regulatory certification or operational
qualification.

## 8. The dependency statement — the evidence base does not exist yet

SOC 2 Type II evidence is evidence of control OPERATION over an observation
window [S-80]. The controls this strategy would evidence are owned by
phases that are themselves written-unrun or not started (P11 audit chain /
logs, P12 drills, P13 identity — plan §5 dependency rule). Until those
producers land and the human operators run the controls, every consumed
cell in the downstream P19 records is PENDING-<producer>. The strategy
therefore states explicitly: **the observation window cannot usefully start
before the P11–P13 control surfaces exist and are operated** — sequencing
fact, not a promise, and not a date.

## 9. Reviewer-verification procedure (human)

1. Confirm §1: no auditor, window, date, budget, or attestation appears;
   every §6 timeline cell is `<EMPTY>`.
2. Confirm §4 registers P19-E-1 as OPEN with DEFAULT (a) carried — not
   decided; the ledger's decision row (`ledger_phase_19.md` §Decision
   points) still reads OPEN.
3. Confirm every ISO 27001-side statement is marked UNVERIFIED /
   REQUIRES-STANDARD-TEXT (context constraint 4) — §3 row 2.
4. Confirm §5 reads as a target statement with the scheme scoping
   UNDECIDED, never as a claim.
5. Confirm §7's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 10. Status

**RECORD-ONLY / WRITTEN — no certification activity exists or is claimed.**
P19-E-1 OPEN, DEFAULT (a) carried. Ticking P19.01 means "implemented
(record written), awaiting human decision/engagement" — the same honest
convention as every prior phase.
