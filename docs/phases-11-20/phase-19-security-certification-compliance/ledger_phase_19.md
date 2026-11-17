# Phase 19 — Security Certification and Compliance Readiness — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 18 of 18 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/65_phase_19_close_and_consistency_audit.md. No certification, attestation, scan, or audit outcome exists; deny.toml byte-identical; the human-gate queue is in CHECKLIST.md.

**Gate.** No implementation of this phase starts before (a) human review of
this roadmap and (b) the first human build of the Phase 0–10 workspace is
green (program conventions §3; program README, hard status boundary 1).
Every run, scan, sign-off, threshold, and date produced during
implementation is a HUMAN GATE INPUT recorded UNRUN / EMPTY / UNSIGNED.

**Tick convention (carried verbatim from Phases 0–10).** A checkbox is
ticked only as "implemented, awaiting human build/test" with a
decision-record citation — `- [x] ... (decision record:
\`records/NN_slug.md\`)` — never as "tested", "verified", "passed", or
"works". Tick flips use the exact-match count==1 method or Write/Edit;
decision records are created at implementation time under `records/`.

## Subtask register

- [x] **P19.01 — Certification strategy record.** SOC 2 Type II vs ISO 27001 (decision record: `records/01_p19_01_certification_strategy.md`)
      order (P19-E-1, DEFAULT: SOC 2 Type II first for US-market traction,
      ISO 27001 follow-on); ISASecure SDLA as the edge agent's certification
      target; timelines human decisions [S-80][S-22]. Evidence: the record
      cites the strategy and the open E-decision; no auditor, date, or
      attestation exists or is claimed.
- [x] **P19.02 — IEC 62443-4-1 SDL program artifacts.** Process docs mapping (decision record: `records/02_p19_02_iec_62443_4_1_sdl_program.md`)
      existing practices (records discipline, review gates, threat modeling)
      to the 4-1 practices; gap list [S-22]. Evidence: the record cites the
      practice map and gap list; no SDL process is certified or claimed.
- [x] **P19.03 — IEC 62443-4-2 component requirements mapping.** Edge agent (decision record: `records/03_p19_03_iec_62443_4_2_component_mapping.md`)
      + control plane as components; FR/CR matrix — CR 3.4 ties to P18.02
      update manifests, CR 2.1 ties to P13/P14 [S-163]. Evidence: the record
      cites the matrix; no component assessment is claimed.
- [x] **P19.04 — Zone/conduit deployment architecture documentation.** 3-2/3-3 (decision record: `records/04_p19_04_zone_conduit_deployment_architecture.md`)
      posture: reference zones (cloud control plane, edge OT zone, site
      devices), conduits, SL targets as RECOMMENDATION [S-22]. Evidence: the
      record cites the architecture doc; no SL level is achieved or claimed.
- [x] **P19.05 — NERC CIP applicability assessment.** Registration triggers (decision record: `records/05_p19_05_nerc_cip_applicability_assessment.md`)
      for aggregators; voluntary CIP-003 Attachment 12 alignment note;
      CIP-013-style supplier flow-down questionnaire template; roadmap
      monitoring record [S-23][S-24][S-25]. Evidence: the record cites the
      assessment and templates; no regulatory determination is made.
- [x] **P19.06 — Supply-chain security program.** Vendor assessment process, (decision record: `records/06_p19_06_supply_chain_security_program.md`)
      supplier SBOM intake, VEX handling, third-party component policy.
      Evidence: the record cites the program; no vendor has been assessed.
- [x] **P19.07 — SLSA L1→L2 build provenance plan.** GitHub Actions (decision record: `records/07_p19_07_slsa_build_provenance_plan.md`)
      provenance, cosign keyless signing design, verification points
      (registry/admission) [S-78][S-79]. Evidence: the record cites the
      plan; no artifact is signed and no provenance exists.
- [x] **P19.08 — SBOM generation process.** Per-image CycloneDX (Syft-class), (decision record: `records/08_p19_08_sbom_generation_process.md`)
      stored next to artifacts, freshness policy; procedures only — never
      run here. Evidence: the record cites the procedure; no SBOM exists
      (P10.19 posture stands).
- [x] **P19.09 — Dependency auditing rollout.** cargo-deny/cargo-audit/ (decision record: `records/09_p19_09_dependency_auditing_rollout.md`)
      pip-audit/npm-audit configs written-unrun as new files (deny.toml
      byte-identical); advisory-feed currency policy. Evidence: the record
      cites the configs; no scan has run and zero findings are NOT claimed.
- [x] **P19.10 — SOC 2 evidence chain mapping.** Which P11–P13/P18 artifacts (decision record: `records/21_p19_10_soc2_evidence_chain.md`)
      evidence which TSCs (Security/Availability/Confidentiality/Processing
      Integrity/Privacy); gap list with owning phases; the 14-state command
      lifecycle as the Processing Integrity anchor story [S-80]. Evidence:
      the record cites the map; unfilled cells are PENDING-<producer>.
- [x] **P19.11 — Access-review process.** Quarterly cadence, IdP-dependent (decision record: `records/22_p19_11_access_review_process.md`)
      mechanics (P13, marked PENDING-P13), evidence records. Evidence: the
      record cites the process and EMPTY evidence template; no review has
      run.
- [x] **P19.12 — Vulnerability management program upgrade.** SLA tiers by (decision record: `records/23_p19_12_vulnerability_management_program.md`)
      severity/exploitability, scanner cadence, exception process with
      expiry; supersedes the P10.20 template posture — an upgrade record,
      not a rewrite. Evidence: the record cites the program and the upgrade
      linkage; no scan or SLA clock exists.
- [x] **P19.13 — Penetration-test program.** Scope (control plane, edge (decision record: `records/24_p19_13_penetration_test_program.md`)
      agent, console, protocol adapters), frequency, vendor-selection
      criteria (P19-E-3), remediation SLAs; NOT executed. Evidence: the
      record cites the program; no test is scheduled or performed.
- [x] **P19.14 — Threat-model refresh.** P0.17 extension as design for new (decision record: `records/25_p19_14_threat_model_refresh.md`)
      surfaces (protocol adapters P14, market records P16, OTA P18, identity
      P13); STRIDE-per-surface pass recorded; the frozen P0.17 file is never
      edited. Evidence: the record cites the refresh; P0.17 stays
      byte-identical.
- [x] **P19.15 — GDPR compliance closure plan.** Retention basis per the (decision record: `records/61_p19_15_gdpr_compliance_closure_plan.md`)
      P13.14 jurisdiction registry, DPIA templates, records-of-processing
      structure; legal sign-off = human gate [S-86][S-87]. Evidence: the
      record cites the plan; no legal determination or sign-off exists.
- [x] **P19.16 — Incident response plan v2.** Severity ladder, grid-ops (decision record: `records/62_p19_16_incident_response_plan_v2.md`)
      escalation (DSO/market/notification per P13.11/P16 interplay),
      tabletop template (human-gated). Evidence: the record cites the plan
      and template; no exercise has run.
- [x] **P19.17 — Compliance test/evidence suite additions (written-unrun).** (decision record: `records/41_p19_17_compliance_test_suite.md`)
      Audit-chain verification tests, retention-enforcement tests,
      access-review data tests, SBOM-freshness check scripts. Evidence: the
      record cites the test files; execution is a human gate and no test
      has run.
- [x] **P19.18 — Decision records + close.** P19-E-1 (certification order), (decision record: `records/65_phase_19_close_and_consistency_audit.md`)
      P19-E-2 (compliance automation platform adoption at certification
      time), P19-E-3 (pen-test vendor criteria); consistency audit across
      every phase-19 artifact. Evidence: the records compile options,
      defaults, and closing evidence; agents close none.

## Decision points (all OPEN — human-owned)

| ID | Question | DEFAULT (label) | Closer |
|---|---|---|---|
| P19-E-1 | Certification order: SOC 2 Type II vs ISO 27001 first (or parallel) | SOC 2 Type II first for US-market traction, ISO 27001 follow-on; ISASecure SDLA the edge agent's longer-horizon target; timelines human decisions | human owner |
| P19-E-2 | Compliance automation platform (Vanta/Drata class) adoption at certification time | Adopt at certification kickoff; the P19.10 mapping stays the platform-independent index; budget is the owner's call | human owner |
| P19-E-3 | Pen-test vendor selection criteria | Criteria-based with recorded minimums (methodology disclosure, OT/grid competence, retest terms, reporting quality); vendor and engagement remain human gates | human owner |
