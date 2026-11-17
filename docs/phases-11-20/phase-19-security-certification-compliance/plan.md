# Phase 19 — Security Certification and Compliance Readiness — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/18). Planning baseline only.
Ledger: `ledger_phase_19.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 19 converts the Phase 0–10 security *templates* into standing
*programs* and prepares the evidence structures a certification process
consumes — without ever claiming certification, an attestation, a scan
result, or an audit outcome (context constraint 1). The 0–10 posture it
upgrades: P10.25 NOT APPROVED, P10.19/P10.20/P10.21 all NOT CLOSED,
deny.toml WRITTEN-NEVER-RUN (all cited at their paths in `context.md` §2).

Four workstreams:

1. **Standards posture** — IEC 62443 (4-1 SDL artifacts, 4-2 component
   mapping, 3-2/3-3 zone/conduit documentation, ISASecure SDLA as the edge
   agent's certification target) [S-22][S-163]; NERC CIP applicability
   assessment with the voluntary CIP-003 Attachment 12 baseline and
   CIP-013-style supply-chain flow-down [S-23][S-24][S-25].
2. **Supply chain** — SLSA L1→L2 provenance and cosign keyless signing
   design [S-78][S-79]; SBOM generation process [S-79]; dependency-audit
   rollout around the never-run `deny.toml`; vendor SBOM/VEX intake.
3. **SOC 2 evidence chain** — mapping of P11–P13/P18 artifacts to the Trust
   Services Criteria [S-80]; access-review process; vulnerability-management
   program upgrade; pen-test program (scope and criteria only); GDPR closure
   plan on the P13.13/P13.14 seam [S-86][S-87]; incident response plan v2.
4. **Verification** — written-unrun compliance test/evidence suite additions
   and the decision records.

Every subtask is a **record-only deliverable**: policies, mappings, process
documents, config files written-unrun, and design records. Implementation
records land under `records/` at implementation time, never during planning
(conventions §8). Evidence produced by other phases is collected by citation
— traced by path, never restated (context constraint 3).

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P19.01 | Certification strategy record | SOC 2 Type II vs ISO 27001 order (P19-E-1, DEFAULT: SOC 2 Type II first for US-market traction, ISO 27001 follow-on); IEC 62443 ISASecure SDLA recorded as the edge agent's certification target; timelines are human decisions, no date promised | [S-80][S-22]; conventions §4 |
| P19.02 | IEC 62443-4-1 SDL program artifacts | Process documents mapping existing practices (records discipline, review gates, threat modeling per P0.17, ledger verification) onto the 4-1 practices; gap list of practices with no current process owner | [S-22]; `ledger/phase-00-product-freeze/17_threat_model.md` |
| P19.03 | IEC 62443-4-2 component requirements mapping | Edge agent and control plane modeled as components; FR/CR matrix with justification per CR — CR 3.4 (software integrity) ties to the P18.02 update manifests; CR 2.1 (authorization enforcement) ties to the P13/P14 identity and permission seams | [S-163]; `../phase-18-edge-fleet-lifecycle-ota/plan.md` P18.02 |
| P19.04 | Zone/conduit deployment architecture documentation | 3-2/3-3 posture: reference deployment zones (cloud control plane, edge OT zone, site devices) extending the P0.17 five-zone model; conduits with documented flows; SL targets recorded as RECOMMENDATION, never as achieved posture | [S-22]; P0.17 §3 |
| P19.05 | NERC CIP applicability assessment | Registration triggers for aggregators assessed from the white-paper/roadmap posture; voluntary CIP-003 Attachment 12 alignment note; CIP-013-style supply-chain flow-down questionnaire template for suppliers; roadmap monitoring record with a re-check trigger | [S-23][S-24][S-25] |
| P19.06 | Supply-chain security program | Vendor assessment process; supplier SBOM intake; VEX handling (a VEX statement is interpreted, never auto-trusted); third-party component policy (replace / isolate / comply / escalate, per the P10.19 disposition vocabulary) | P10.19 §7; gap analysis §5 Tier 1 |
| P19.07 | SLSA L1→L2 build provenance plan | GitHub Actions provenance design; cosign keyless signing design (OIDC-issued short-lived certificates); verification points at registry pull and admission; ties to the P10.21 not-yet-existing artifact set | [S-78][S-79]; `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` |
| P19.08 | SBOM generation process | Per-image CycloneDX (Syft-class) procedure extending the P10.19 §6 commands; SBOMs stored next to artifacts; freshness policy; procedures only — never run here (no lockfile exists) | `ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` §6 |
| P19.09 | Dependency auditing rollout | cargo-deny / cargo-audit / pip-audit / npm-audit configs written-unrun as NEW files (deny.toml stays byte-identical, P8.03 §6 precedent); advisory-feed currency policy; every run a human gate | `deny.toml`; `ledger/phase-10-v1-release-hardening/20_vuln_scan_closure.md` §3 |
| P19.10 | SOC 2 evidence chain mapping | Which P11–P13/P18 artifacts evidence which TSCs (Security / Availability / Confidentiality / Processing Integrity / Privacy); gap list with owning phases; the 14-state command lifecycle with its three distinct gates as the Processing Integrity anchor story | [S-80]; `../phase-11-operational-robustness/plan.md` P11.15; P13 rows; P18.02 |
| P19.11 | Access-review process | Quarterly review cadence; mechanics IdP-dependent (P13.01/P13-E-1 — marked PENDING-P13 until the IdP lands); evidence record template with EMPTY cells; findings path (revoke / accept with expiry) | `../phase-13-enterprise-identity-integration/plan.md`; conventions §5 |
| P19.12 | Vulnerability management program upgrade | SLA tiers by severity and exploitability (pre-registered, X-8); scanner cadence per stack; exception process with signed, time-bounded expiry; supersedes the P10.20 template posture — an upgrade record, not a rewrite of the frozen closure file | `ledger/phase-10-v1-release-hardening/20_vuln_scan_closure.md` |
| P19.13 | Penetration-test program | Scope (control plane, edge agent, console, protocol adapters), frequency, vendor-selection criteria (P19-E-3), remediation SLAs; NOT executed and NOT scheduled as done — the engagement is a human gate | [S-44] (Uplight pen-test posture as commercial precedent); context constraint 5 |
| P19.14 | Threat-model refresh | P0.17 extension as design: new surfaces — protocol adapters (P14), market records (P16), OTA (P18), identity (P13); STRIDE-per-surface pass recorded; the frozen P0.17 file is never edited — the refresh is a new record citing it | P0.17; `../phase-14-protocol-device-connectivity/plan.md` P14.01; P16/P13/P18 seams |
| P19.15 | GDPR compliance closure plan | Retention basis per the P13.14 jurisdiction registry; DPIA templates; records-of-processing structure; legal sign-off = human gate (LR-062 direction) | [S-86][S-87]; `../phase-13-enterprise-identity-integration/plan.md` P13.13/P13.14; LR-062 |
| P19.16 | Incident response plan v2 | Severity ladder; grid-ops escalation paths (DSO/market/notification per the P13.11 pipeline and P16 market-record interplay); tabletop exercise template — human-gated, never run | `../phase-13-enterprise-identity-integration/plan.md` P13.11; P16 seams |
| P19.17 | Compliance test/evidence suite additions (written-unrun) | Audit-chain verification tests (against the P11.15 chain design), retention-enforcement tests (P13.13 matrix), access-review data tests, SBOM-freshness check scripts; complete files, never run | conventions §1 item 3; P11.15; P13.13 |
| P19.18 | Decision records + close | P19-E-1 (certification order), P19-E-2 (compliance automation platform adoption at certification time), P19-E-3 (pen-test vendor criteria); consistency audit across every phase-19 artifact; agents close none | conventions §4; `context.md` §3 |

## 3. Blocking order and shared surfaces

| Producer (blocks) | Consumers (what they take, never redefine) |
|---|---|
| P19.01 strategy + P19-E-1 default | P19.02 (SDL ordering), P19.10 (TSC scope order), P19.18 close |
| P19.02 4-1 practice map | P19.03 (component evidence inherits process claims), P19.10 (Security TSC rows) |
| P19.03 CR matrix | P19.04 (component SL placement), P19.17 (CR-derived test list) |
| P19.05 CIP flow-down template | P19.06 (the supplier questionnaire is the vendor-assessment instrument) |
| P19.07 provenance design | P19.08 (SBOM feeds the attested artifact set), P19.17 freshness checks |
| P19.08 SBOM process | P19.09 (audit configs scan what the SBOM describes), P19.06 intake format |
| P19.10 evidence map | P19.11/P19.12/P19.13 evidence-record templates reference its TSC rows; P19.18 audit |
| P19.14 threat-model refresh | P19.13 (pen-test scope derives from surfaces), P19.10 Security TSC gap rows |
| P19.16 IR plan v2 | P19.17 tabletop-related test fixtures; P19.18 audit |

## 4. Deliberate non-goals (this phase does not)

- **No certification is achieved, scheduled as achieved, or promised.** No
  SOC 2 report, ISO 27001 certificate, or ISASecure claim exists; the
  observation window, auditor, and signature are organizational acts.
- **No scan, audit, or pen test runs.** Every scanner named (cargo-deny,
  cargo-audit, pip-audit, npm-audit, Syft-class SBOM generators) stays
  written-unrun; the pen test is scoped, never scheduled as done.
- **deny.toml is not edited** (P8.03 §6 recording-over-editing precedent);
  P19.09 adds new config files alongside it.
- **No producer phase's artifacts are restated or re-decided** — P11 audit
  chain, P12 drills, P13 identity/GDPR records are cited by path.
- **No ISO 27001 Annex A claim** beyond the marked UNVERIFIED/REQUIRES-
  STANDARD-TEXT posture (context constraint 4).
- **No new crates**: the phase's deliverables are records and config files;
  any future automation crate would follow conventions §6.2 at
  implementation, with real code and the additive dependency-rule update.
- **No dispatch-authority change**: ADR-010 and the never-granted permission
  set are untouched; compliance evidence describes the existing gates, it
  never adds a bypass.

## 5. Wave plan

Two agents, per the standing loop discipline.

| Wave | Agent A | Agent B | Shared note |
|---|---|---|---|
| 1 | P19.01–P19.09 (certification strategy + supply chain: strategy, 4-1/4-2/3-2-3-3, CIP, supply-chain program, SLSA/cosign, SBOM, dependency audits) | P19.10–P19.14 (evidence chain + programs: TSC mapping, access review, vulnerability management, pen-test program, threat-model refresh) | Disjoint records; P19.10's TSC map and P19.14's surfaces cross-reference Agent A's P19.02–P19.04 by path (citation, not duplication) |
| 2 | P19.17 (test/evidence suite additions) | P19.15, P19.16, P19.18 (GDPR closure plan, IR plan v2, decision records + close) | The consistency audit (P19.18) covers every wave-1 and wave-2 artifact; both agents' outputs compile into it |

Dependency rules inherited (gap analysis §9): Phase 19 collects evidence
from P11–P13 — those phases' records must exist before the consuming P19
records can cite filled cells; until then every consumed cell is marked
PENDING-<producer subtask>. Each implemented batch gets a decision record
under `records/` and keeps the dependency-rule checker green (conventions
§2, §6.1).

### 5.1 Batch discipline during implementation

- Every implemented batch closes with a decision record under `records/`
  citing the subtask IDs, the seams touched, and the honest status
  ("implemented, awaiting human build/test") — conventions §2.4.
- Thresholds named in the programs (SLA tiers, freshness windows, exception
  expiries, review cadences) are registered BEFORE any run that could fill
  them; dynamic results stay structurally separate (X-8; conventions §5).
- Phase close follows conventions §2.5: HANDOVER, CHECKLIST, and the
  close/exit-gate record are added under this directory only at
  implementation completion (P19.18) — never during planning.
- No crate is added by this phase; if implementation later introduces a
  tooling crate, conventions §6.2 applies additively.

## 6. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | What would close it | Closer |
|---|---|---|---|---|---|
| P19-E-1 | Certification order: SOC 2 Type II or ISO 27001 first (or parallel)? | (a) SOC 2 Type II first, ISO 27001 follow-on; (b) ISO 27001 first (EU-first market posture); (c) parallel tracks | **DEFAULT: (a)** — SOC 2 Type II first for US-market traction (the commercial evidence base is US-weighted [S-44][S-45]), ISO 27001 follow-on; IEC 62443 ISASecure SDLA recorded as the edge agent's longer-horizon target [S-22]; all timelines are human decisions | The owner recording the market priority and engaging the first auditor | human owner |
| P19-E-2 | Adopt a compliance-automation platform (Vanta/Drata class [S-80]) at certification time, or manual evidence collection? | (a) adopt a platform at certification kickoff; (b) manual collection with the P19.10 map as the index; (c) re-decide at kickoff | **DEFAULT: (a)** — evidence over an observation window is exactly what these platforms automate [S-80]; the P19.10 mapping remains the platform-independent index either way; budget is the owner's call | The owner recording the budget decision at certification kickoff | human owner |
| P19-E-3 | Pen-test vendor selection criteria | (a) criteria-based (recorded minimums: methodology disclosure, OT/grid competence, retest terms, reporting quality); (b) incumbent/cheapest qualifying vendor; (c) broker-managed | **DEFAULT: (a)** — the minimum criteria set is recorded in P19.13; the vendor, the engagement date, and the findings remain human gates | The owner accepting the criteria and selecting the vendor at engagement time | human owner |

## 7. Risks

| # | Risk | Basis / mitigation |
|---|---|---|
| R-P19-1 | The evidence chain depends on phases 11–13 landing — the audit chain (P11.15), drills (P12.07/P12.08), and IdP (P13.01) do not exist yet, so mappings cite records that are themselves NOT STARTED | Every consumed cell is marked PENDING-<producer subtask> (plan §5 dependency rule); the mapping tables are structurally complete so producers fill, never re-shape, them |
| R-P19-2 | Certification timelines are organizational — observation windows, auditor scheduling, and signatures cannot be produced by agents | No date is promised anywhere (P19-E-1); the strategy record states the dependency explicitly and leaves every timeline cell EMPTY |
| R-P19-3 | Scanner/audit configs are unrun — deny.toml has never executed, the new configs never run; the first run may false-fail or surface findings | Pre-registration discipline (X-8) before any run; zero-finding is never claimed from an unrun config (the P10.20 §1 rule); findings get dispositions, not silence |
| R-P19-4 | ISO 27001 mapping is incomplete without the standard text — Annex A specifics are UNVERIFIED in the research (gap analysis §11) | Mapping done against SOC 2 TSCs [S-80]; every ISO-side statement marked UNVERIFIED / REQUIRES-STANDARD-TEXT (context constraint 4) |
| R-P19-5 | The 0–10 base is WRITTEN-UNRUN — the first build may surface recorded defects (e.g., F-P10.07-5), and every 0–10 security record is a template, not a verdict | Program-level standing risk (program README boundary 1); planning proceeds, implementation cannot precede a green human build |
| R-P19-6 | Honesty wording — any text implying certification, an attestation, a scan outcome, or an executed audit is a compliance failure | Context constraint 1 + §8 acceptance criteria; every phase-19 record carries the not-a-certification statement |

## 8. Acceptance criteria (honest)

1. Mappings, policies, process documents, config files, and test files exist
   **written-unrun**; no subtask claims a run, a scan, an audit outcome, or
   verified behavior.
2. **No certification, attestation, or conformance is claimed** (conventions
   §7.3); the verbatim not-a-certification discipline of P10.25 §7 carries
   into every phase-19 record.
3. **No pen test is executed or scheduled as done** — scope, frequency,
   criteria, and SLAs are defined; the engagement cells stay NOT-STARTED.
4. Evidence is **collected by citation**: P11/P12/P13 producers are traced
   by path, never restated; unfilled cells are marked PENDING-<producer>,
   never assumed filled.
5. Every ISO 27001 Annex A statement is marked **UNVERIFIED / REQUIRES-
   STANDARD-TEXT**; the TSC mapping is the only criteria mapping presented
   as grounded.
6. `deny.toml` stays **byte-identical**; new configs are new files; the
   first human run of anything remains the verification point.
7. The dependency rule stays green: `python -I
   tools/check_dependency_rule.py` after every implementation batch (this
   phase adds no crate; the check is run and quoted, never asserted).
8. `python tools/verify_docs.py` and `python tools/validate_agent.py` are
   green after every documentation change in this phase (results quoted,
   never asserted).
