<!-- P19.06 — Supply-chain security program. HONEST RECORD: a PROCESS
     document — vendor assessment, supplier SBOM intake, VEX handling,
     third-party component policy. No vendor has been assessed, no supplier
     SBOM has been received, no VEX statement exists or has been
     interpreted, and no third-party component disposition has been made.
     Thresholds are registered (X-8) BEFORE any run that could fill them;
     every dynamic-result cell is EMPTY. The not-a-certification
     discipline of P10.25 §7 (§9) carries verbatim. -->
# P19.06 — Supply-Chain Security Program (vendor assessment process; supplier SBOM intake; VEX handling; third-party component policy)

Date: 2026-11-17. Subtask: P19.06 (`ledger_phase_19.md` §P19.06).
Plan trace: `../plan.md` §2 row P19.06, §3 blocking order row 4 (consumes
the P19.05 questionnaire template by path) and row 6 (the intake format is
what the P19.08 SBOM process produces — by path). Sources: the P10.19
disposition vocabulary and §7 reconciliation posture
(`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` §5, §7);
the P8.03 §2 research pin table (the research-stack SBOM input list);
`../context.md` §4 (VEX glossary row); the P19.05 questionnaire
(`05_p19_05_nerc_cip_applicability_assessment.md` §5).

## 1. Honest status — the plain statement

**No vendor has been assessed, no supplier SBOM or VEX statement has been
received, and no third-party component disposition exists beyond the ones
the 0–10 records already carry.** This record is the standing PROCESS the
program adopts when the human supply-chain owner starts executing it; the
process is WRITTEN, never operated. Thresholds in §6 are registered (X-8)
BEFORE any run that could fill them; every dynamic-result cell in this
record is EMPTY.

## 2. Vendor assessment process (executed by humans; nothing executed)

| Stage | What happens | Instrument (by path) | Status |
|---|---|---|---|
| V-1 Intake | A vendor enters scope when a deployment plans to use their component; the owner sends the P19.05 §5 questionnaire | `05_p19_05_nerc_cip_applicability_assessment.md` §5 (template, answer cells EMPTY) | NOT-STARTED |
| V-2 Evidence review | Answers reviewed as RECEIVED CLAIMS (never accepted as evidence); any supplier certification/attestation document is logged as a received document, never re-attested or endorsed | This record §3 (review discipline) | NOT-STARTED |
| V-3 Technical corroboration | Claims corroborated where possible from primary surfaces: the vendor's own SBOMs/VEX feeds, published advisories, the delivered artifact's actual manifests | The P19.08 SBOM process (per-image intake) | NOT-STARTED |
| V-4 Disposition | The vendor/component gets one of the §5 dispositions; the record of the decision carries the owner and date (human) | §5 vocabulary | NOT-STARTED |
| V-5 Reassessment | Registered vendors re-enter V-2 on the §6 reassessment cadence or on any trigger (new release, new advisory, changed ownership) | §6 thresholds | NOT-STARTED |

## 3. Supplier SBOM intake

1. **Format**: a supplier SBOM is accepted in CycloneDX or SPDX,
   machine-readable, generated FROM the supplier's resolved
   build/lockfiles — a hand-written component list is not an SBOM (the
   P10.19 rule, carried). The exact format-version set accepted is
   verified at intake time per the tooling notes of the producing tool —
   no version is asserted here (the unnamed-tool rule).
2. **Minimum fields** (intake validation — a submission missing any of
   these is INCOMPLETE and does not enter the register): component
   identifier (name + version), dependency relationships, license
   expressions, and a hash or digest binding the SBOM to the delivered
   artifact version.
3. **Binding**: a supplier SBOM is valid for the artifact version it
   digests-binds to; a new delivered version re-enters intake (the
   P10.20 §4 re-scan trigger rule, carried in substance: a check run for
   one version is evidence for that version only).
4. **Register**: intakes land in the supply-chain register (a
   human-owned record; no store exists yet — the register's storage is
   UNDECIDED and its file/table shape is a future deliverable, not
   invented here). Register cells today: `<EMPTY — no intake ever
   received>`.

## 4. VEX handling — interpreted, never auto-trusted

The VEX discipline (context §4 glossary: "a statement of whether a listed
vulnerability actually affects a product"):

| Rule | Content |
|---|---|
| R-1 | A VEX statement is an INPUT TO ANALYSIS, never a resolution. It is never auto-trusted, auto-applied, or treated as a clearance by any tool or process step |
| R-2 | Every received VEX statement is recorded (source, version, affected-product claim, status asserted, date received) — `<EMPTY — none ever received>` |
| R-3 | Interpretation: the statement's claim is checked against OUR artifact inventory and configuration (does the affected component exist in the deployment? in the affected version? on an executable path?) before any "not affected" posture is recorded. The check result is a human-reviewed disposition, not the vendor's word |
| R-4 | A VEX "affected" statement maps to the vulnerability-management program's intake (P19.12 — Agent B, PENDING-P19.12; cited by path) |
| R-5 | A VEX "not affected" posture accepted by review is recorded WITH its reasoning, reviewer, and a re-check date — an acceptance without reasoning is invalid |
| R-6 | No VEX statement creates a clearance in any P10.20-style result table; scan results and VEX interpretations stay structurally separate (conventions §5) |

## 5. Third-party component policy — the P10.19 disposition vocabulary

Carried verbatim in substance from
`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` (§5/§7:
"replace / isolate / comply / escalate-to-legal") and extended from
license findings to supply-chain findings generally:

| Disposition | When it applies | Record requirements |
|---|---|---|
| REPLACE | The component is replaced by another (internal or third-party) | The replacement's own assessment (V-1..V-4) before swap; the old component's removal recorded |
| ISOLATE | The component stays but is contained (e.g., out-of-process worker, no dispatch reach, network-restricted) | The isolation mechanism named; the containment is a reviewed change; simulation/shadow ceiling rules unaffected |
| COMPLY | The finding is accepted and the component stays as-is with the finding acknowledged | Dated, owner-signed acceptance with expiry (§6); carried into release notes the way P10.20 §8 carries accepted findings |
| ESCALATE | The finding needs a decision above the program (legal, license, regulatory, budget) | Routed to the named human owner; the escalation is recorded with the routing date; `<EMPTY — none escalated>` |

No third-party component ships silently with an open finding (the P10.19
C-5 rule, carried). The dispositions recorded in the 0–10 records (e.g.,
the ANDES GPLv3 worker-boundary isolation, P8.03 §3) remain owned by those
records — cited, never re-decided here.

## 6. Thresholds registered (X-8) — before any run that could fill them

| Threshold | Registered value | Applies to | Filling status |
|---|---|---|---|
| Vendor reassessment cadence | Once per calendar year per active vendor, PLUS on any new major version delivery | V-5 | Registered; first reassessment NOT-STARTED |
| Questionnaire response window | 30 calendar days from V-1 dispatch, after which the vendor is recorded NON-RESPONSIVE (a disposition input, not a penalty) | V-1 | Registered; no questionnaire dispatched |
| SBOM intake validity | An intake older than the artifact's current delivered version is STALE and re-enters intake | §3 rule 3 | Registered; register EMPTY |
| VEX re-check window | An accepted "not affected" posture is re-checked at the next vulnerability-program review of that component, and in any case at least once per calendar quarter | §4 R-5 | Registered; no VEX accepted |
| Finding-acceptance expiry | Every COMPLY disposition carries an expiry; an expired acceptance is an open finding (the P10.20 §8 rule, carried) | §5 | Registered; no acceptance exists |

All values above are process thresholds registered before any run; none
has been applied to any real case (there are no cases — the register is
EMPTY).

## 7. Interlock with the never-granted permission set and ADR-010

The vendor-assessment process changes no permission: third-party
components assessed under this program receive capabilities only through
the existing plugin boundary and command lifecycle; no assessment outcome
grants dispatch reach, `secret.read`, or `network.any` (conventions §6.4;
ADR-010 unchanged).

## 8. Handoffs

- **P19.05** (questionnaire): §2 V-1 is the instrument's distribution path.
- **P19.08** (Agent A, record 08): the per-image SBOM process is the
  intake format's production-side counterpart.
- **P19.12** (Agent B): VEX "affected" statements and vendor advisory
  feeds land in the vulnerability-management program's intake.
- **Wave 2 / human owner**: the supply-chain register's storage shape and
  the first vendor engagement are human-gated work.

## 9. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

No vendor attestation, supplier certification, or audit outcome is
claimed, endorsed, or reproduced as FlexCore's own.

## 10. Status

**RECORD-ONLY / PROCESS-WRITTEN / NEVER-OPERATED.** No vendor assessed,
no SBOM received, no VEX interpreted, no disposition made beyond the
0–10 records' own.
