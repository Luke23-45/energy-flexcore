<!-- P19.02 — IEC 62443-4-1 SDL program artifacts. HONEST RECORD: this is a
     process-mapping record against a standard whose practice enumeration is
     REQUIRES-STANDARD-TEXT (the research covers [S-22] ISASecure, not the
     62443-4-1 text; every practice id below is marked UNVERIFIED against
     the standard). It maps EXISTING practices by citation — never restates
     a producer's artifact — and records a gap list of practices with no
     current process owner. No SDL process is certified, assessed, or
     claimed. The not-a-certification discipline of P10.25 §7 (§8) carries
     verbatim. -->
# P19.02 — IEC 62443-4-1 SDL Program Artifacts (existing-practice mapping + process-owner gap list; REQUIRES-STANDARD-TEXT posture)

Date: 2026-11-17. Subtask: P19.02 (`ledger_phase_19.md` §P19.02).
Plan trace: `../plan.md` §2 row P19.02, §3 blocking order row 2 (consumed by
P19.03 component evidence and P19.10 Security-TSC rows). Sources: [S-22]
(ISASecure, isasecure.org — the scheme family; NOT the 4-1 standard text);
the P0.17 threat-model freeze
(`ledger/phase-00-product-freeze/17_threat_model.md`); the ledger-verification
discipline (`../../01_PROGRAM_CONVENTIONS.md` §2, §6).

## 1. Honest status — the plain statement

**FlexCore operates no certified SDL process, and this record claims none.**
What exists today — records discipline, review gates, threat modeling,
ledger verification — is mapped below onto the IEC 62443-4-1 practice
structure as EVIDENCE A CERTIFICATION PROCESS WOULD CONSUME. The 4-1
practice identifiers and their exact requirements are **UNVERIFIED in this
program's research** (the gap analysis's Track B research covered ISASecure
[S-22]; the 62443-4-1 text itself was not consulted) — every practice row
is marked REQUIRES-STANDARD-TEXT and the mapping must be re-validated
against the standard text before any assessment engagement. The gap list
(§5) records practices with no current process owner; owner cells are
EMPTY/PENDING by design.

## 2. The practice structure — REQUIRES-STANDARD-TEXT

The 4-1 secure development lifecycle is organized into practice groups
(SM, SR, SD, SI, SVV, DM, CA, MA) — the widely published group structure —
with numbered practices inside each. **The practice ids, titles, and
requirements below are the program's working skeleton, marked UNVERIFIED;
the standard text is the only authority for their content** (context
constraint 4's discipline, applied to 62443 as to ISO 27001). Nothing in
this record asserts what any practice REQUIRES beyond what the skeleton
name implies; the "existing practice" column is real, the "62443-4-1
requirement" column is a heading awaiting the text.

## 3. Existing-practice mapping (by citation — producers own their content)

| Practice group (UNVERIFIED skeleton) | Existing FlexCore practice | Evidence source (path — cited, never restated) | Owner today | Status against the group |
|---|---|---|---|---|
| SM — Security management (UNVERIFIED ids SM-1..SM-4) | Decision-record discipline: every implemented batch gets a decision record; E-decisions registered, never agent-closed | `../../01_PROGRAM_CONVENTIONS.md` §2.4, §4 | Compliance (this phase) | Process EXISTS as convention; formal security-management plan DOES NOT |
| SM (continuation) | Security consolidation and review gates (the P10.25 input pack with verdict NOT APPROVED) | `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §1, §9 | Security owner — **UNSIGNED** | A review gate exists as a TEMPLATE; no convened review has ever happened |
| SR — Security requirements specification (UNVERIFIED ids SR-1..SR-4) | The §17 threat model freeze: zones, 16 §17.3 controls, 10 §17.4 threats, 10 §17.5 invariants, frozen at Phase 0 | `ledger/phase-00-product-freeze/17_threat_model.md` §3–§6 | Security owner (P0.17); refresh owned by P19.14 (Agent B) | Requirements exist and are frozen; they are NOT labeled to any 62443-4-1 practice and no traceability matrix to standards exists |
| SD — Secure by design (UNVERIFIED ids SD-1..SD-4) | Architecture conventions: DDD + hexagonal, dependency rule enforced by a checker, never-granted permission set, ADR-010 dispatch-authority separation, fail-closed seams (`NoUpdateVerifier`, `MtlsRequirements`) | `../../01_PROGRAM_CONVENTIONS.md` §6; `docs/architecture/adr/ADR-010.md`; `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` §5; `../../phase-14-protocol-device-connectivity/records/03_p14_03_security_design.md` §2 | Architecture (per-phase records) | Design principles recorded per phase; no consolidated secure-design guideline document exists |
| SI — Secure implementation (UNVERIFIED ids SI-1..SI-2) | Coding conventions recorded (conventions §8: fmt/clippy/test gates; no arbitrary dylib; WASM/WIT plugins); the workspace is WRITTEN-UNRUN — the first human build is the gate | `../../01_PROGRAM_CONVENTIONS.md` §8 | Engineering (per-phase) | Conventions exist; NO build has ever executed, so no implementation evidence exists at all |
| SVV — Verification and validation (UNVERIFIED ids SVV-1..SVV-4) | Written-unrun test suites across phases 0–10 (e.g., the P10.04 authz/tenant suite, P10.08 plugin-boundary suite); verification tools run by humans only | `ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`; `.../08_plugin_boundary_tests.md` | Security + engineering | Suites are WRITTEN-UNRUN — zero executed test evidence exists; zero-observed-failure claims are never made from unrun suites (conventions §5) |
| DM — Defect management (UNVERIFIED ids DM-1..DM-4) | Findings registers per record (e.g., P10.19 F-1..F-7, P10.20 F-1..F-5, P10.25 SR-1..SR-8); the F-P10.07-5 recorded defect; LR register | `ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` §8; `.../20_vuln_scan_closure.md` §9; `.../25_security_consolidation.md` §10; `ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` | Per-register owners | Findings recorded with dispositions; no standing defect-management PROCESS (triage SLAs, severity ladder, closure discipline across the program) — P19.12 (Agent B) supersedes the scan-template posture into that program |
| CA — Compiler/toolchain analysis (UNVERIFIED ids CA-1..CA-3) | Toolchain pinning (Phase 7: wasmtime floor, toolchain record); CI gate definitions (UNRUN) | `ledger/phase-07-plugin-sdk/40_toolchain_pinning.md` §3; `.github/workflows/ci.yml` (all jobs UNRUN) | Platform engineering | Pins recorded; no compiler-warning/security-flag analysis has ever run |
| MA — Maintenance and disposal (UNVERIFIED ids MA-1..MA-5) | Update/rollback RC procedures (P10.12), edge update manifest v2 with anti-rollback (P18.02), decommissioning runbook (P18.10) | `ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md`; `../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`; `../../phase-18-edge-fleet-lifecycle-ota/records/23_p18_10_decommissioning_runbook.md` | Platform + edge (per-phase) | Update/maintenance procedures recorded; nothing executed; no standing patch/cadence program (P19.12's scope) |

## 4. What the mapping honestly establishes

- The program HAS practice-shaped raw material (records, gates, threat
  model, findings registers) — the gap analysis's own verdict is "Partial
  (secure SDLC practices exist informally)" (`../../00_GAP_ANALYSIS.md` §4
  Track B row).
- The material is NOT assembled as an SDL program: no security-management
  plan, no practice-to-evidence traceability, no formal verification
  reporting, and no operated process behind the conventions. The tables
  above are the assembly skeleton the human SDL owner fills; they are not
  an assessment and do not score any practice.

## 5. Gap list — practices with no current process owner

| # | Gap | What is missing | Owner cell (EMPTY by design — human assignment) |
|---|---|---|---|
| G-1 | No standing security-management process owner | A named owner for the SDL program itself (policy, scope, review cadence); the P10.25 security owner cells are UNSIGNED/EMPTY | `<PENDING — human assignment>` |
| G-2 | No security-requirements traceability owner | Someone to maintain a requirements→design→test traceability (the §17 controls are frozen but unowned as a living matrix) | `<PENDING — human assignment>` |
| G-3 | No verification-reporting process | Executed test evidence, coverage reporting, and a formal V&V record per release — today suites are WRITTEN-UNRUN and nobody owns turning them into evidence | `<PENDING — human assignment>` |
| G-4 | No defect-management program owner | Triage SLAs, severity ladder, exception expiry — the posture P19.12 upgrades (Agent B; PENDING-P19.12) | `<PENDING — human assignment>` |
| G-5 | No third-party/software-acceptance process | The 4-1 lifecycle for incorporated third-party components (the P19.06 program, record `06_p19_06_supply_chain_security_program.md`) has no named owner | `<PENDING — human assignment>` |
| G-6 | No training/competence record | No security-training process or record exists anywhere in the program's history | `<PENDING — human assignment>` |
| G-7 | No disposal/decommission process owner for control-plane data | P18.10 covers edge decommissioning; the control-plane side (tenant offboarding data disposition) has no owner (intersects P13.15/P13.13 — cited, not re-decided) | `<PENDING — human assignment>` |

## 6. Threshold registration (X-8)

No SLA, cadence, or freshness threshold is set by this record. Where the
SDL program will need them (review cadence, defect triage tiers), they are
registered as REQUIRED-BEFORE-FIRST-RUN inputs and set in the owning
programs (P19.12 by Agent B; access reviews P19.11) — never invented here
(conventions §5; plan §5.1).

## 7. What would change this record's status

1. The standard text of IEC 62443-4-1 is obtained and the §2 skeleton is
   re-validated row by row (human gate; UNVERIFIED register clears only
   then).
2. The human SDL owner assignment fills §5's G-1..G-7 owner cells.
3. An assessment engagement (ISASecure SDLA or equivalent, per P19-E-1's
   owner decision) consumes the assembled evidence — none is scheduled.

## 8. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

Nothing here claims, scores, or pre-empts any SDL certification,
assessment outcome, or conformance.

## 9. Reviewer-verification procedure (human)

1. Confirm §2/§3: every practice id is marked UNVERIFIED /
   REQUIRES-STANDARD-TEXT; no practice requirement is asserted from
   memory.
2. Spot-check five §3 evidence paths against the sources: the source must
   still say what the Status cell reports; any mismatch is a defect of
   THIS record.
3. Confirm §5's owner cells are `<PENDING>` and no human/role is invented.
4. Confirm §8's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 10. Status

**RECORD-ONLY / WRITTEN.** The practice mapping and gap list exist as
documentation; no SDL process is operated, certified, assessed, or
claimed; every owner cell is PENDING by design.
