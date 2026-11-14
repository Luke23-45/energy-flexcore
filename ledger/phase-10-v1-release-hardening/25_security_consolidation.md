<!-- P10.25 — Security review consolidation report. HONEST RECORD: this is
     the ASSEMBLED security position for v1.0.0, prepared FOR the
     reviewers — the review itself has NOT been convened: no reviewers,
     no meeting, no date, no verdict. VERDICT = NOT APPROVED. The
     reviewer sign-off block is UNSIGNED. The report consolidates BY
     CITATION (ENTRY_NOTE.md boundary 5): every consolidated item from
     16_cyber_risk_consolidation.md, the operational areas from
     17_operational_risk_review.md, the P10.19/P10.20/P10.21
     supply-chain closures (all three verdicts NOT CLOSED), and the
     test-suite findings P10.04/P10.08 — traced by path, never
     restated, never closed, never re-decided. The residual-risk
     posture is the honest negative one: no residual set is claimable
     (no assessment verdict exists; every owner/review-date cell
     EMPTY). No certification, safety-case, or protection-grade claim
     appears anywhere except the verbatim no-claims discipline quoted
     in §7. WRITTEN-UNRUN. -->
# P10.25 — v1.0.0 Security Review Consolidation Report (VERDICT: NOT APPROVED — prepared for the reviewers; no review convened; sign-off UNSIGNED)

Date: 2026-11-14. Subtask: P10.25 (`ledger_phase_10.md` §P10.25).
Owner role: Security. Report trace: §17; §25 (pilot release:
"security review").
Inputs (all cited, none restated as new work): P10.16 cyber risk
consolidation (`ledger/phase-10-v1-release-hardening/16_cyber_risk_consolidation.md`);
P10.17 operational risk review
(`ledger/phase-10-v1-release-hardening/17_operational_risk_review.md`);
P10.04 authorization/tenant-isolation suite
(`ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`);
P10.08 plugin permission-boundary suite
(`ledger/phase-10-v1-release-hardening/08_plugin_boundary_tests.md`);
P10.20 vulnerability scan closure
(`ledger/phase-10-v1-release-hardening/20_vuln_scan_closure.md`);
P10.19 SBOM/license closure (D4)
(`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md`);
P10.21 signed artifacts record
(`ledger/phase-10-v1-release-hardening/21_signed_artifacts.md`);
P10.15 operator-mode qualification
(`ledger/phase-10-v1-release-hardening/15_operator_mode_qualification.md`);
P9.07 deployment security assessment
(`ledger/phase-09-pilot-qualification/07_deployment_security_assessment.md`);
P9.06 D6 record
(`ledger/phase-09-pilot-qualification/06_ot_it_boundary_d6.md`);
P9.27 register
(`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md`);
P8 security review
(`ledger/phase-08-advanced-coordination/25_security_review.md`).

## 1. Honest status — the plain statement

**This report is the final assembled security position for the v1.0.0
release, and it is prepared FOR the reviewers — the review has NOT been
convened.** No review meeting exists, no reviewer has been named, no
review date has been set, and no verdict has been rendered by any
human. The recorded verdict of this report is therefore:

> **VERDICT: NOT APPROVED.** No security review has been convened for
> the v1.0.0 release candidate. This document is the input pack the
> security review convenes on; it is not itself the review.

Consequences, stated plainly:

- The reviewer sign-off block (§9) is **UNSIGNED**. The ledger's
  acceptance item (d) ("the review is signed") is NOT met by this
  record and is not claimed.
- Nothing in this report closes, accepts, or upgrades any carried
  item. Every item in §5–§6 is cited from its source record with the
  status the source itself records; the source owns the wording, the
  status, and the closure conditions.
- The release candidate this review would examine **does not exist**
  (P10.18 UNRUN — `18_ci_gate_run_record.md`; the candidate
  identifier in `01_release_scope_freeze.md` §6.1 is
  `<PENDING P10.18>`). A review that describes the product design
  instead of the candidate is the ledger's named failure mode; this
  record refuses it by marking every candidate-dependent cell EMPTY.

Ticking P10.25 in the ledger means "implemented, awaiting human
review/signature" — the consolidation report exists; the security
review it prepares for is the human gate.

## 2. Scope and limits of the review (ledger step 2)

### 2.1 What this report consolidates (reviewed material, by citation)

| # | Review input | Source (path) | State as recorded there |
|---|---|---|---|
| S-1 | The consolidated deployment-facing cyber-risk table (Phase 6/7/8 registers + Phase 9 security design set, trace-only) | `16_cyber_risk_consolidation.md` §7 (§7.1–§7.5) | Every item OPEN at source; nothing closed by the consolidation |
| S-2 | The §17.4 threat-control re-check procedure against the candidate | `16_cyber_risk_consolidation.md` §3 (RC-1..RC-4) | PREPARED, UNRUN — every drift/verdict cell `<EMPTY>`/`<PENDING P10.18>` |
| S-3 | The deployment-specific framing (site, hardware, contracts, IdP, installed versions) | `16_cyber_risk_consolidation.md` §2 (D-1..D-6) | ALL EMPTY — no deployment exists |
| S-4 | NIST SP 800-82 baseline status | `16_cyber_risk_consolidation.md` §5 | Rev. 3 remains the baseline; Rev. 4 DRAFT as of the last primary check (2026-11-07); the dated CSRC re-check is carried as a P10.27 condition; no lookup performed in this loop |
| S-5 | D6 scope coverage | `16_cyber_risk_consolidation.md` §6; `ledger/phase-09-pilot-qualification/06_ot_it_boundary_d6.md` | D6 OPEN/UNSIGNED; no signed scope exists — every release scope addition is a go blocker for that scope |
| S-6 | The Phase 9 operational risk areas (fallback, training, market procedures, incident response, backup/DR, observability, clock/freshness, audit loss window) | `17_operational_risk_review.md` §2 | Carried by citation, all OPEN, every assessment cell EMPTY/UNSIGNED per source |
| S-7 | Release-specific operational risk structures (R-1..R-7, incl. the F-P10.07-5 clock-median defect) | `17_operational_risk_review.md` §3 | Structures only; impact/likelihood/owner/mitigation/review-date all `<EMPTY>` |
| S-8 | Authorization and tenant-isolation suite (matrices, RLS structural preconditions, claims-derived tenant context) | `04_authz_tenant_tests.md` §3 | WRITTEN-UNRUN; RLS/database and retention/residency verification recorded as open human-gate steps; findings F-P10.04-1..F-P10.04-3 recorded |
| S-9 | Plugin permission-boundary suite (escalation matrices, sandbox limits, lifecycle, no self-upgrade) | `08_plugin_boundary_tests.md` §3/§5/§6 | WRITTEN-UNRUN; findings F-P10.08-1..F-P10.08-4 recorded; zero observed failures are NOT claimed from an unrun suite |
| S-10 | Vulnerability and dependency scan closure | `20_vuln_scan_closure.md` | **NOT CLOSED** — zero scans have ever run; result table EMPTY; zero versions cleared (§6) |
| S-11 | SBOM and license audit closure (D4) | `19_sbom_license_closure.md` | **NOT CLOSED** — no SBOM exists, no scan has run, no legal review; the models/datasets leg has no audit surface at all (F-3 there) |
| S-12 | Signed artifacts, checksums, support matrix | `21_signed_artifacts.md` | Nothing built, signed, checksummed, or published; no production signing key material exists; the test-grade FNV-1a-64 digests are NOT release signatures |
| S-13 | Control-authority position (operator-approved mode) | `15_operator_mode_qualification.md` | **NOT QUALIFIED** — every mode-entry requirement EMPTY/UNSIGNED/NOT-MET; bounded automation not enabled |
| S-14 | The §17.3 control inventory and §17.4 threat/control table (T1–T10, C1–C16) | `ledger/phase-09-pilot-qualification/07_deployment_security_assessment.md` §9/§10 | Design positions cited; every evidence/status/residual cell `<EMPTY>` — the assessment TEMPLATE has never run (LR-039) |

### 2.2 What was NOT reviewed (the limits, stated so a reader cannot over-read this report)

- **No candidate build.** P10.18 is UNRUN; there is no artifact to
  inspect, so no control status is recorded against a candidate —
  only design positions and their unrun test suites.
- **No deployment, site, hardware, or customer contract.** The
  deployment-specific layer (D-1..D-6 of `16_cyber_risk_consolidation.md`
  §2) is entirely EMPTY. A customer-specific control that belongs to
  the customer, not to FlexCore, cannot be reviewed here because no
  customer exists to review it for.
- **No executed penetration test, red team, or fuzzing campaign.** No
  record of any such activity exists in this repository's history;
  none is claimed.
- **No production cryptographic posture.** Production crypto (CSPRNG,
  SHA-256-or-stronger signing under reviewed key management) is a
  recorded DEPLOYMENT REQUIREMENT, not an implemented capability
  (`21_signed_artifacts.md` §5; P9.09 LR-054..LR-061).
- **No physical/hardware security review.** Every P6.34 hardware
  determination is PENDING PILOT HARDWARE SPEC.
- **No review date for the review itself.** The assembly date
  (2026-11-14) is the date of this report; the review's own date is
  set when the review is convened.

## 3. The assembled control and threat position (ledger step 1)

The ledger step-1 assembly, as pointers with the source-owned status —
no cell is filled here that its source left empty:

| Ledger step-1 component | Source of record | Assembled state |
|---|---|---|
| §17.3 control status for the candidate | `07_deployment_security_assessment.md` §10 (C1–C16) via `16_cyber_risk_consolidation.md` §8 | Design positions cited; deployed status `<EMPTY>` for all sixteen — no candidate exists |
| §17.4 threat coverage | `07_deployment_security_assessment.md` §9 (T1–T10) via `16_cyber_risk_consolidation.md` §3 | Controls cited per threat; the candidate drift re-check (RC-2..RC-4) UNRUN; drift verdicts `<EMPTY>` |
| Authorization/tenant-isolation results (P10.04) | `04_authz_tenant_tests.md` | Suite WRITTEN-UNRUN over real crate seams (448 allow cells asserted structurally); executed evidence `<EMPTY>` — zero unauthorized successes is NOT claimable from an unrun suite |
| Plugin-boundary results (P10.08) | `08_plugin_boundary_tests.md` | Suite WRITTEN-UNRUN; executed evidence `<EMPTY>` — zero successful escalations is NOT claimable from an unrun suite |
| Vulnerability-scan closure (P10.20) | `20_vuln_scan_closure.md` | NOT CLOSED; result table EMPTY; zero versions cleared; the carried advisory state (wasmtime floor, NATS CVE surface, P8.36 NOT-EXECUTED table) is cited there, not re-verified |

## 4. The supply-chain closures (P10.19 / P10.20 / P10.21) — all three NOT CLOSED

The release-facing supply-chain position, consolidated by verdict lines
from the owning records (each verdict is the source's own words in
substance, carried unsoftened):

| Record | Verdict (as recorded there) | What is missing for closure |
|---|---|---|
| P10.19 — SBOM and license audit closure (D4) | **NOT CLOSED** (`19_sbom_license_closure.md` §10) | SBOMs generated from lockfiles (none exist — no Cargo.lock, no console lockfile, no resolved Python environment); license scan run; per-dependency determinations; dispositions; legal review; the D4 owners' signature. The models/datasets leg has NO audit surface (F-3 there) |
| P10.20 — Vulnerability and dependency scan closure | **NOT CLOSED** (`20_vuln_scan_closure.md` §10) | A final scan against candidate artifacts and SBOMs (no artifact, no SBOM); triage of every finding; critical/high fixed or signed-time-bounded; re-scan recorded against the candidate identifier |
| P10.21 — Signed artifacts, checksums, support matrix | **NOT CLOSED** (`21_signed_artifacts.md` §12) | The candidate build; checksums over real artifacts; production signing under the DEPLOYMENT REQUIREMENT (no key material exists; `NoUpdateVerifier` refuses everything); support matrix assembled only from filled P10.14 cells (all EMPTY); publication verification |

Consequence carried into P10.26/P10.27: **a go decision that consumes
these records as-is is a go on an unaudited, unscanned, unsigned
artifact set that does not exist** — each record states this itself;
this report carries the statements forward unchanged.

## 5. The consolidated findings register (trace-only; nothing closed)

Every security-relevant finding carried by the input records, cited by
path and identifier. The source row owns the wording; this table is the
reviewer's single entry index.

### 5.1 Cross-register security items (from `16_cyber_risk_consolidation.md` §7 — cited, not restated)

| Register | Handle range / ids | Security posture (as recorded at source) |
|---|---|---|
| Phase 6 register (`ledger/phase-06-edge-protocol/30_retrospective_release_gate.md` §2) | O-1..O-11 (O-12 CLOSED at source) | D5/D6/D4 open; OpenEMS REST hop cleartext refused fail-closed; runner wiring, console embedding, authz interim gates recorded; deferred protocols |
| Phase 7 register (`ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md` §2) | O-1..O-16 | Verification gate UNRUN; D1–D6 open; license verifications UNVERIFIED; compromised-HOST posture with unauthenticated host↔control-plane transports (O-15); P7.29 security clearance UNRUN |
| Phase 8 register + P8.25/P8.36 records (`ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md` §4; `25_security_review.md`; `36_cve_pin_check.md`) | O-2..O-13; F-1..F-4; G-1/G-2; S-1..S-3 | **P8 F-1 (HIGH)**: unguarded `torch.load(...)` at the GNN artifact load — OPEN, blocks the with-framework activation, unreachable today (guarded import; no framework; no artifact). F-2/F-3 review gaps; F-4 signing seam not implemented; advisory lookups NOT-EXECUTED — zero pins cleared |
| Phase 9 security design set (`06_ot_it_boundary_d6.md` §10; `07_deployment_security_assessment.md` §15; `08_identity_access_approval.md` §12; `09_secrets_certs_keys.md` §12; `35_safety_invariant_map.md` §8; `36_forged_command_drill.md` §9) | LR-031..LR-061; LR-152..LR-162 | D6 OPEN/UNSIGNED; no security assessment exists (LR-039); no IdP/MFA; no CA/secrets manager/HSM; production crypto absent (keyed checksum; FNV-1a-64); forged-command drill UNSCHEDULABLE; checksum-grade primitives force re-runs |
| P9.27 register (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md`) | LR-001…LR-211 + 42 carried pointers | The complete consolidation handle; version `pilot-limitations-register-v0.1-draft`; every acceptance cell `<EMPTY — UNSIGNED>`. Count discrepancy recorded: the file numbers LR-001…LR-211 while P10.01 §10 cites LR-001..LR-225 (`16_cyber_risk_consolidation.md` §7 finding C-5) — the register owner's dated amendment, not reconciled here |

### 5.2 Phase 10 findings carried into this report

| Finding | Source (path + §) | Carried posture |
|---|---|---|
| F-P10.04-1..F-P10.04-3 | `04_authz_tenant_tests.md` §4 | Observations/constraint recorded, not fixed (authz `Action::ALL` absent; behavioural grant assertion; the 401-not-403 distinction) |
| F-P10.05-1..F-P10.05-3 | `05_optimizer_validation.md` §6 | Observations/constraint recorded (closed §12.6 vocabulary; provider warning text; test-grade digest posture) |
| F-P10.06-1..F-P10.06-3 | `06_telemetry_handling_tests.md` §6 | Observations/constraint recorded (`age > limit` staleness boundary vs clock skew; envelope invalidation on quality transitions; `Degraded` → `ClockQuality::Unsynchronized` mapping) |
| F-P10.07-1..F-P10.07-4 | `07_edge_offline_tests.md` §6 | Observations/constraints recorded (at-least-once redelivery; audited drop-oldest with audit-journal sizing as a deployment obligation; journal encryption at rest OPEN; keyed-checksum command signature is tamper-EVIDENCE, not a guarantee) |
| **F-P10.07-5** | `07_edge_offline_tests.md` §6 | **Defect in an EXISTING test, recorded, not fixed**: the P6.31 unit test `the_tolerance_boundary_is_respected_inclusively` in `crates/edge-agent/src/clock_quality.rs` (~703–712) asserts a `Degraded` outcome the even-count median arithmetic does not produce — the assertion would FAIL on the first human run. Operationally relevant: the clock-quality state gates command expiry (freshness/hold behaviour). Fix options belong to the file's owner; `17_operational_risk_review.md` §3 R-7 carries it |
| F-P10.08-1..F-P10.08-4 | `08_plugin_boundary_tests.md` §6 | Audit-completeness gap and recorded constraints, not fixed |
| P10.19 F-1..F-7 | `19_sbom_license_closure.md` §8 | No SBOM; no scan; models/datasets leg unauditable; LLVM-exception allowlist question carried as a RECORDED DECISION (DEFAULT/RECOMMENDATION) |
| P10.20 F-1..F-5 | `20_vuln_scan_closure.md` §9 | Zero scans; inputs do not exist; carried advisory state inherits zero clearance; Python CI scanner gap persists |
| P10.21 F-1..F-6 | `21_signed_artifacts.md` §10 | No artifact; no production key material; signing tool unrecorded; zero verified support cells; channels/commitment UNDECIDED |
| P10.16 C-1..C-6 | `16_cyber_risk_consolidation.md` §9 | Candidate re-check unrun; "no must-fix finding open" UNEVALUABLE (LR-039); D6 gap a go blocker; Rev. 4 re-check bound to the review date; LR count discrepancy; nothing closed |
| P10.17 O-1..O-5 | `17_operational_risk_review.md` §6 | No operational risk closed; release-specific risks unassessed; F-P10.07-5 carried; LR count discrepancy; P9.27 → P10.26 reconciliation procedure |

## 6. Residual-risk posture (ledger step 3)

**No residual-risk set is claimable, and this report does not invent
one.** The ledger step-3 deliverable ("residual security risks with
owners and review dates") requires three cells this record cannot fill:

| Residual-risk component | Why it is EMPTY |
|---|---|
| The residual set itself | A residual risk is what remains AFTER an assessment. No assessment exists: the P9.07 findings register is `<EMPTY>` (LR-039; `16_cyber_risk_consolidation.md` §4 — the "no must-fix finding open" check is UNEVALUABLE, not satisfied), and no candidate/deployment exists to assess against. What exists is the OPEN-item surface of §5 — carried OPEN, not residual |
| Owners | Named-person cells are `<EMPTY>` everywhere in the carried registers (no person exists in this design; `17_operational_risk_review.md` §1). The source registers' owner/venue wording is carried verbatim in §5 |
| Review dates | No review has been convened; no acceptance is signed; no expiry exists. The only dated forward obligation already recorded is the NIST CSRC Rev. 4 re-check, bound to the human review/go date as a P10.27 condition (`16_cyber_risk_consolidation.md` §5) |

**The carry-forward obligation to the release notes (ledger step 3,
final clause):** every item in §5 that constitutes a known limitation
or accepted-risk candidate must appear in the P10.26 known-limitations
section item for item — no item dropped, none softened. P10.26 is a
concurrent-wave deliverable (`26_release_notes.md` + the release-notes
draft); the item-for-item cross-check against the P9.27 register is
recorded there as its own procedure (the register's §7 rule: never
paraphrase into softer language) and is completed at the human
release-notes review. This report records the OBLIGATION, not its
completion — the completion is verifiable by the reviewer procedure in
§10.

## 7. The not-a-certification statement (verbatim discipline)

The following discipline governs every reading of this report. Quoted
verbatim from `AGENTS.md` §3 (rules 2–3):

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

And the standing version qualification (report §2.1, verbatim):

> A version number does not imply regulatory certification or
> operational qualification. The control-authority mode actually
> enabled at a site is governed by §21.4 and the release gates in §22.3.

This security review is therefore: not a certification, not a safety
case, and not permission for live grid control. It is a supervisory
platform's release-gate input, prepared for human reviewers, on a
candidate that does not yet exist, with a verdict of NOT APPROVED.

## 8. What an approved review would establish (evidence conditions, never a promise)

If the security review is convened and APPROVED, its approval would —
by the ledger's acceptance criteria — establish each of the following,
each verifiable against evidence that does not exist today. These are
the EMPTY cells the reviewers fill; they are not claims made by this
report:

| # | An approved review would establish | Evidence it would consume (today: does not exist / UNRUN) |
|---|---|---|
| E-1 | Every §17.3 control has a recorded status FOR THE CANDIDATE (not the design) | The P10.18 candidate identity (P10.01 §6.1 `<PENDING P10.18>`) and the filled C1–C16 statuses (`07_deployment_security_assessment.md` §10) |
| E-2 | The §17.4 threat controls were re-checked against the candidate configuration, with drift findings recorded and dispositioned | The RC-1..RC-4 procedure of `16_cyber_risk_consolidation.md` §3, executed |
| E-3 | The authorization/tenant-isolation suite RAN on the candidate with zero unauthorized successes and the denials audited | `04_authz_tenant_tests.md` suites executed; audit-log inspection against the deployed candidate |
| E-4 | The plugin permission-boundary suite RAN with zero successful escalation attempts and enforced sandbox limits evidenced | `08_plugin_boundary_tests.md` suites executed; the host capability-policy enforcement log |
| E-5 | The vulnerability scan RAN against the candidate artifacts and SBOMs; every critical/high finding fixed or formally accepted (signed, time-bounded); none untriaged | `20_vuln_scan_closure.md` §3 procedure executed; §6/§7 filled |
| E-6 | D4 is CLOSED for the release artifact set: SBOMs from lockfiles, license determinations, dispositions, legal review, D4 owners' signature | `19_sbom_license_closure.md` §6/§7 executed and signed |
| E-7 | D6 is SIGNED for the deployment the release ships to, covering the release scope (any uncovered addition is a go blocker for that scope) | The P9.06 closure convened and signed per deployment (`16_cyber_risk_consolidation.md` §6) |
| E-8 | The residual-risk list is complete, owned, and dated, and matches the release notes item for item | The filled residual set; the P9.27 → P10.26 reconciliation result recorded in P10.26 |
| E-9 | The scope/limits and the not-a-certification statement survive the review unsoftened | The reviewer's §2/§7 confirmation |
| E-10 | The signature block (§9) is signed by the security owner | The human signature |

An approval without E-1..E-10 evidence is not an approval of this
report's subject matter; it would be an approval of a different, undescribed
release.

## 9. Reviewer sign-off block — UNSIGNED

| Item | Value |
|---|---|
| Security review convened (date, attendees) | `<EMPTY — no review has been convened>` |
| Reviewing security owner (name, role) | `<EMPTY>` |
| Review of the §17.3 control statuses against the candidate | `<EMPTY — UNRUN>` |
| Review of the residual-risk carry-forward into P10.26 | `<EMPTY — UNRUN>` |
| Findings accepted / rejected / returned for rework | `<EMPTY>` |
| **VERDICT** | **NOT APPROVED** (recorded by this report; changes only by the convened review) |
| **Security owner signature** | **UNSIGNED — `<EMPTY>`** |

The signature row is filled by a human security owner at the convened
review. No agent signature appears or ever appears here.

## 10. Findings recorded by this report

| # | Finding | Disposition |
|---|---|---|
| SR-1 | The review is NOT APPROVED and the sign-off is UNSIGNED because no review has been convened — the report is the input pack, not the review | Binding on this record |
| SR-2 | The candidate this review would examine does not exist (P10.18 UNRUN); every candidate-dependent cell is EMPTY, never assumed | §2.2; carried to the convened review |
| SR-3 | All three supply-chain closures (P10.19 D4, P10.20 scan, P10.21 signing) are NOT CLOSED; a go consuming them as-is is a go on an unaudited, unscanned, unsigned, non-existent artifact set | §4; carried to P10.27 |
| SR-4 | "No must-fix finding open" is UNEVALUABLE, not satisfied — the P9.07 findings register is EMPTY because no assessment exists (LR-039) | §6; carried to P10.27 |
| SR-5 | D6 is OPEN/UNSIGNED with no signed scope — every release scope addition is a go blocker for that scope | §5.1; carried to P10.27 |
| SR-6 | The residual-risk set, owners, and review dates are unfillable today; the P9.27 → P10.26 carry-forward obligation is recorded as a procedure, its execution owned by P10.26's review | §6 |
| SR-7 | LR count discrepancy (LR-211 on disk vs LR-225 cited in P10.01 §10) carried unchanged from P10.16 §7 / P10.17 §4 | Register owner's dated amendment; not reconciled here |
| SR-8 | The NIST SP 800-82 Rev. 4 re-check is bound to the human review/go date as a recorded P10.27 condition; Rev. 3 remains the baseline; no lookup was performed in this loop | §2.1 S-4 |

## 11. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: the verdict line reads NOT APPROVED,
   no convening/attendee/date cell is filled, and the §9 signature
   block is UNSIGNED.
2. Spot-check five rows of §2.1/§5 against their source records: the
   path must be correct and the source row must still say what the
   Status cell reports. Any mismatch is a defect of THIS report —
   never fixed by editing the source register.
3. Confirm §4 quotes each supply-chain verdict in substance unsoftened
   (P10.19 §10, P10.20 §10, P10.21 §12).
4. Confirm §6 records the carry-forward obligation without claiming
   its completion, and the §7 verbatim discipline matches
   `AGENTS.md` §3 rules 2–3 and report §2.1 exactly.
5. Confirm §8 reads as evidence conditions, not as claims or promises.
6. Confirm no certification/safety/protection-grade claim appears
   outside §7's verbatim quotations
   (`grep -niE "certif|safety case|protection-grade|safety-grade" ledger/phase-10-v1-release-hardening/25_security_consolidation.md`
   returns only §7 and this line's context).
7. Confirm the ledger tick reads "implemented, awaiting human
   review/signature".

## 12. Filling commands (human, at execution time)

```sh
# The convened review fills, IN ORDER:
#   1. The §9 convening cells (date, attendees) at the review session.
#   2. E-1..E-5 only FROM the executed records: the P10.18 candidate
#      identity, the RC-2..RC-4 drift verdicts, the run reports of the
#      P10.04/P10.08 suites, the P10.20 scan outputs — cite the runs,
#      never restate them here.
#   3. E-6/E-7 from the signed D4 (P10.19) and signed D6 (P9.06)
#      closures — signatures are the owners', never an agent's.
#   4. The residual-risk set with named owners and review dates,
#      then the item-for-item match against the P10.26 limitations.
#   5. The verdict and the signature — last.

# Candidate identity source (when it exists):
grep -n "PENDING P10.18" ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md

# Carry-forward cross-check aids (after P10.26 lands):
grep -c "^| LR-" ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md
grep -n "LR-225" ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md

# Repository verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 13. Status

**WRITTEN-UNRUN.** A consolidation report cannot be "run": the review
session, the candidate examination, the executed suites, the scans, the
signed D4/D6 closures, the residual-risk judgement, and the signature
are the human gates and have not happened. No review was convened; no
verdict was rendered by any human; nothing is closed or accepted. The
P10.25 ledger tick reads "implemented, awaiting human review/signature".
