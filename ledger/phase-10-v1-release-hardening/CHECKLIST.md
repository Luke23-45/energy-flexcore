# Phase 10 — v1.0.0 Pilot-Ready Release Hardening / Closure: Exit-Gate Checklist

**Exit gate (§25 pilot release + §22.3): v1.0.0 is releasable as pilot-ready when every
§22.3 release gate is met against the frozen release candidate, the §22.2 CI gates pass,
the benchmark is reproducible, the deployment/upgrade model is documented and tested, the
security review is complete, the site integration test harness is packaged, and the
go/no-go record (P10.27) says go.**

Each item is a checkbox plus the evidence required. Check an item only when the evidence
exists at the location named and has been inspected. A checked item without evidence is not
checked. **Phase 10 overlaps Stage 9 but is the release gate, not duplicated work** — items
marked "cross-reference" rely on Phase 9 evidence re-verified against the release candidate;
they do not redo the pilot qualification.

## §22.3 release gates (one-for-one)

- [x] **Gate 1 — Documented scope and supported adapters complete** (P10.01). Evidence: **NOT MET - prepared**: evidence = `01_release_scope_freeze.md` (freeze template, UNSIGNED; adapter matrix all UNVERIFIED; candidate id `<PENDING P10.18>`) (WRITTEN-UNRUN; execution cells EMPTY); the signed freeze + candidate identity is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.01)
  signed scope freeze + adapter matrix tied to the release-candidate identifier.
- [x] **Gate 2 — All command-state transitions tested** (P10.03). Evidence: transition test **NOT MET - prepared**: evidence = `03_command_transition_tests.md` + `command_transition_release_matrix.rs` (1,759 lines, 21 tests; 196-pair + 374-cell matrices) WRITTEN-UNCOMPILED/UNRUN (WRITTEN-UNRUN; execution cells EMPTY); the compile and the suite run is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.03)
  report covering every legal §14.1 transition and the enumerated illegal ones.
- [x] **Gate 3 — Authorization and tenant-isolation tests pass** (P10.04). Evidence: **NOT MET - prepared**: evidence = `04_authz_tenant_tests.md` + 4 test files (348-cell matrices; zero-unauthorized-successes asserted by construction, never by run) (WRITTEN-UNRUN; execution cells EMPTY); the test run + RLS verification is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.04)
  authorization/tenancy test report; zero unauthorized successes; RLS verified.
- [x] **Gate 4 — Optimizer outputs independently validated** (P10.05). Evidence: independent **NOT MET - prepared**: evidence = `05_optimizer_validation.md` + 2 test files (result-contract + harness consumption) (WRITTEN-UNRUN; execution cells EMPTY); the test run (no optimizer binary exists) is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.05)
  validation report; infeasible/timeout cases reject/hold, never infer a pass.
- [x] **Gate 5 — Stale/missing telemetry cases handled** (P10.06). Evidence: injection test **NOT MET - prepared**: evidence = `06_telemetry_handling_tests.md` + 2 test files (six-state handling; fail-closed gates) (WRITTEN-UNRUN; execution cells EMPTY); the injection test run + console check is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.06)
  report; zero silent substitutions; console quality distinction verified.
- [x] **Gate 6 — Edge offline/degraded modes tested** (P10.07). Evidence: edge test report on **NOT MET - prepared**: evidence = `07_edge_offline_tests.md` + 2 test files (34 tests; bounded buffering; no stale command issue asserted) (WRITTEN-UNRUN; execution cells EMPTY); the candidate edge build + run is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.07)
  the candidate edge build; bounded buffering and no stale command issue.
- [x] **Gate 7 — Plugin permission boundaries tested** (P10.08). Evidence: permission-boundary **NOT MET - prepared**: evidence = `08_plugin_boundary_tests.md` + 2 test files (38 tests; never-granted refusals; ADR-010 type proof) (WRITTEN-UNRUN; execution cells EMPTY); the test run is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.08)
  test report; zero successful escalation attempts; lifecycle rules hold.
- [x] **Gate 8 — Simulator benchmark results reproducible** (P10.09–P10.10). Evidence: two **NOT MET - prepared**: evidence = `09_benchmark_rerun.md` + `10_benchmark_results_pack.md` (templates; X-8 binding; NO baseline run exists anywhere) (WRITTEN-UNRUN; execution cells EMPTY); threshold registration + two full runs is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.09-P10.10)
  full benchmark runs agreeing within pre-recorded tolerances + results pack with
  thresholds dated before the runs.
- [x] **Gate 9 — Backup/restore and upgrade/rollback tested** (P10.11–P10.13). Evidence: **NOT MET - prepared**: evidence = `11_backup_restore_rc.md` + `12_upgrade_rollback_rc.md` + `13_schema_migration_compat.md` (procedures; run records EMPTY; no update ever built/applied; Postgres pin UNVERIFIED) (WRITTEN-UNRUN; execution cells EMPTY); restore/upgrade/rollback executions on a candidate is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.11-13)
  restore actually performed and verified on the candidate; rolling upgrade and rollback
  verified; migration compatibility windows recorded.
- [x] **Gate 10 — Supported browser/OS matrix tested** (P10.14). Evidence: matrix test report; **NOT MET - prepared**: evidence = `14_browser_os_matrix.md` + `console/SUPPORT_MATRIX.md` (21 cells all `<EMPTY - UNRUN>`) (WRITTEN-UNRUN; execution cells EMPTY); the matrix test run is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.14)
  every §21.4 cell tested or explicitly removed from support.
- [x] **Gate 11 — Target control-authority mode qualified** (P10.15). Evidence: operator-approved **NOT MET - prepared**: evidence = `15_operator_mode_qualification.md` - VERDICT: NOT QUALIFIED (recorded); every requirement EMPTY/UNSIGNED (WRITTEN-UNRUN; execution cells EMPTY); the entire Stage 9 evidence chain is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.15)
  mode qualification record; every bypass attempt failed; no automation envelope enabled.
- [x] **Gate 12 — Deployment-specific cyber and operational risk review complete** (P10.16– **NOT MET - prepared**: evidence = `16_cyber_risk_consolidation.md` + `17_operational_risk_review.md` (trace-only consolidations; deployment-specific cells EMPTY; nothing closed) (WRITTEN-UNRUN; execution cells EMPTY); the deployment-specific review execution is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.16-17)
  P10.17). Evidence: consolidated cyber risk review (Rev. 4 status checked) + operational
  risk review; every risk owned and dated.

## CI, supply chain, and artifacts

- [x] **Full CI gate run passed on the candidate** (P10.18). Evidence: per-gate CI report + **NOT MET - prepared**: evidence = `18_ci_gate_run_record.md` (24 jobs enumerated; additive dispatch-only RC build job; run record EMPTY) (WRITTEN-UNRUN; execution cells EMPTY); the CI run itself is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.18)
  release-candidate identifier and artifact hashes; repository verification commands pass.
- [x] **Public API and plugin contracts frozen** (P10.02). Evidence: Buf lint/breaking report **NOT MET - prepared**: evidence = `02_api_contract_freeze.md` (12 surfaces inventoried; drift procedures; Buf/WIT runs NOT RUN; compatibility promises UNVERIFIED) (WRITTEN-UNRUN; execution cells EMPTY); the tool runs against a candidate is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.02)
  (zero unintended breaking changes) + compatibility matrix.
- [x] **D4 license audit closed** (P10.19). Evidence: release SBOMs (from lockfiles) + license **NOT MET - prepared**: evidence = `19_sbom_license_closure.md` - verdict NOT CLOSED (deny.toml exists from P6.27, never run; no lockfile; LLVM-exception decision RECORDED not closed) (WRITTEN-UNRUN; execution cells EMPTY); lockfile-backed SBOM + scan + legal determination is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.19)
  audit report + signed D4 closure record; every dependency determined; legal review done.
- [x] **Vulnerability scan closed** (P10.20). Evidence: final scan report; every critical/high **NOT MET - prepared**: evidence = `20_vuln_scan_closure.md` - verdict NOT CLOSED (procedures; result table EMPTY; zero versions cleared) (WRITTEN-UNRUN; execution cells EMPTY); the scans is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.20)
  finding fixed or formally accepted (signed, time-bounded).
- [x] **Signed artifacts published** (P10.21). Evidence: signed artifacts + checksums verified **NOT MET - prepared**: evidence = `21_signed_artifacts.md` - verdict NOT CLOSED (no production keys; test-grade digests are NOT release signatures; checksum/signing tables EMPTY) (WRITTEN-UNRUN; execution cells EMPTY); a candidate build + key ceremony + signing is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.21)
  from the publication location + support matrix + Helm chart published.

## Release packaging

- [x] **Site integration test harness packaged** (P10.22). Evidence: versioned harness + **NOT MET - prepared**: evidence = `22_site_harness.md` + `harness/site-integration/` (manifest v1 + README + run-record template; packaged BY CONSTRUCTION) (WRITTEN-UNRUN; execution cells EMPTY); the first human execution is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.22)
  clean-environment run record by a non-author; version-mismatch detection verified.
- [x] **Operator workflow stable** (P10.23). Evidence: walkthrough report; every §20.2 **NOT MET - prepared**: evidence = `23_operator_workflow_stability.md` (procedures pre-registered; check record EMPTY) (WRITTEN-UNRUN; execution cells EMPTY); the walkthrough is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.23)
  principle verified; no state-hiding defect open.
- [x] **Deployment/upgrade model documented** (P10.24). Evidence: deployment and upgrade guide **NOT MET - prepared**: evidence = `24_deployment_upgrade_model.md` + `docs/operations/deployment-upgrade-model.md` (DESIGN-not-deployed; site values PLACEHOLDER) (WRITTEN-UNRUN; execution cells EMPTY); a deployed system to document + review-deploy is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.24)
  matching the tested paths; review-deployed or exception recorded.
- [x] **Security review complete** (P10.25). Evidence: signed v1.0.0 security review with scope, **NOT MET - prepared**: evidence = `25_security_consolidation.md` - verdict NOT APPROVED (sign-off block UNSIGNED) (WRITTEN-UNRUN; execution cells EMPTY); the convened review is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.25)
  limits, and the not-a-certification statement.
- [x] **Release notes complete** (P10.26). Evidence: release notes with known limitations **NOT MET - prepared**: evidence = `26_release_notes.md` + `docs/releases/v1.0.0-release-notes-draft.md` (v1.0.0-draft; limitations section filled from the real registers; review NOT REVIEWED) (WRITTEN-UNRUN; execution cells EMPTY); the review + a real version is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.26)
  cross-checked item-for-item against the P9.27 register; §0.3 non-promises stated.

## Decision and handover

- [x] **Go/no-go decision recorded** (P10.27). Evidence: signed decision record; every §22.3 **NOT MET - prepared**: evidence = `27_go_no_go.md` - decision NOT MADE; the structure returns NO-GO on today's evidence; sign-off UNSIGNED (WRITTEN-UNRUN; execution cells EMPTY); the decision meeting is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.27)
  gate item with a status; conditions/waivers owned and dated. **No publication on a no-go.**
- [x] **v1.0.0 tagged and published** (P10.28). Evidence: tag matching the candidate commit and **NOT MET - prepared-NOT-EXECUTED**: evidence = `28_release_publication.md` (publication PROCEDURE described; precondition chain re-verified - ZERO of eight links satisfied; `git tag -l` = 0 tags verified during P10.28). v1.0.0 IS NOT RELEASED: no tag, no artifact, no signature, no announcement exist, and the loop bans permanently forbid creating any. The publication authorization is a human gate input (UNSIGNED). Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.28)
  hashes; publication verified from outside; notes published with the release.
- [x] **Pilot handover pack delivered** (P10.29). Evidence: versioned handover pack (v1.0.0) + **NOT MET - prepared**: evidence = `29_pilot_handover_pack.md` + `docs/operations/pilot-handover-runbook.md` (DESIGN runbook; acceptance checklist EMPTY) (WRITTEN-UNRUN; execution cells EMPTY); the handover walkthrough is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.29)
  walkthrough record + signed receipt; operators qualified on the release.
- [x] **Post-release monitoring live; automation deferral recorded** (P10.30). Evidence: **NOT MET - prepared**: evidence = `30_post_release_deferral.md` (monitoring PLAN; three signals have no instrumentation; run record EMPTY; bounded-automation deferral carried verbatim - UNGRANTED) (WRITTEN-UNRUN; execution cells EMPTY); a deployment to monitor is a human gate input. Release gate NOT MET - see `31_phase_exit_gate_record.md`. (P10.30)
  monitoring confirmation on the released version; first review date set; deferral record
  stating the approval route for any future bounded automation.
