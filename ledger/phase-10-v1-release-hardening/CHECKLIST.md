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

- [ ] **Gate 1 — Documented scope and supported adapters complete** (P10.01). Evidence:
  signed scope freeze + adapter matrix tied to the release-candidate identifier.
- [ ] **Gate 2 — All command-state transitions tested** (P10.03). Evidence: transition test
  report covering every legal §14.1 transition and the enumerated illegal ones.
- [ ] **Gate 3 — Authorization and tenant-isolation tests pass** (P10.04). Evidence:
  authorization/tenancy test report; zero unauthorized successes; RLS verified.
- [ ] **Gate 4 — Optimizer outputs independently validated** (P10.05). Evidence: independent
  validation report; infeasible/timeout cases reject/hold, never infer a pass.
- [ ] **Gate 5 — Stale/missing telemetry cases handled** (P10.06). Evidence: injection test
  report; zero silent substitutions; console quality distinction verified.
- [ ] **Gate 6 — Edge offline/degraded modes tested** (P10.07). Evidence: edge test report on
  the candidate edge build; bounded buffering and no stale command issue.
- [ ] **Gate 7 — Plugin permission boundaries tested** (P10.08). Evidence: permission-boundary
  test report; zero successful escalation attempts; lifecycle rules hold.
- [ ] **Gate 8 — Simulator benchmark results reproducible** (P10.09–P10.10). Evidence: two
  full benchmark runs agreeing within pre-recorded tolerances + results pack with
  thresholds dated before the runs.
- [ ] **Gate 9 — Backup/restore and upgrade/rollback tested** (P10.11–P10.13). Evidence:
  restore actually performed and verified on the candidate; rolling upgrade and rollback
  verified; migration compatibility windows recorded.
- [ ] **Gate 10 — Supported browser/OS matrix tested** (P10.14). Evidence: matrix test report;
  every §21.4 cell tested or explicitly removed from support.
- [ ] **Gate 11 — Target control-authority mode qualified** (P10.15). Evidence: operator-approved
  mode qualification record; every bypass attempt failed; no automation envelope enabled.
- [ ] **Gate 12 — Deployment-specific cyber and operational risk review complete** (P10.16–
  P10.17). Evidence: consolidated cyber risk review (Rev. 4 status checked) + operational
  risk review; every risk owned and dated.

## CI, supply chain, and artifacts

- [ ] **Full CI gate run passed on the candidate** (P10.18). Evidence: per-gate CI report +
  release-candidate identifier and artifact hashes; repository verification commands pass.
- [ ] **Public API and plugin contracts frozen** (P10.02). Evidence: Buf lint/breaking report
  (zero unintended breaking changes) + compatibility matrix.
- [ ] **D4 license audit closed** (P10.19). Evidence: release SBOMs (from lockfiles) + license
  audit report + signed D4 closure record; every dependency determined; legal review done.
- [ ] **Vulnerability scan closed** (P10.20). Evidence: final scan report; every critical/high
  finding fixed or formally accepted (signed, time-bounded).
- [ ] **Signed artifacts published** (P10.21). Evidence: signed artifacts + checksums verified
  from the publication location + support matrix + Helm chart published.

## Release packaging

- [ ] **Site integration test harness packaged** (P10.22). Evidence: versioned harness +
  clean-environment run record by a non-author; version-mismatch detection verified.
- [ ] **Operator workflow stable** (P10.23). Evidence: walkthrough report; every §20.2
  principle verified; no state-hiding defect open.
- [ ] **Deployment/upgrade model documented** (P10.24). Evidence: deployment and upgrade guide
  matching the tested paths; review-deployed or exception recorded.
- [ ] **Security review complete** (P10.25). Evidence: signed v1.0.0 security review with scope,
  limits, and the not-a-certification statement.
- [ ] **Release notes complete** (P10.26). Evidence: release notes with known limitations
  cross-checked item-for-item against the P9.27 register; §0.3 non-promises stated.

## Decision and handover

- [ ] **Go/no-go decision recorded** (P10.27). Evidence: signed decision record; every §22.3
  gate item with a status; conditions/waivers owned and dated. **No publication on a no-go.**
- [ ] **v1.0.0 tagged and published** (P10.28). Evidence: tag matching the candidate commit and
  hashes; publication verified from outside; notes published with the release.
- [ ] **Pilot handover pack delivered** (P10.29). Evidence: versioned handover pack (v1.0.0) +
  walkthrough record + signed receipt; operators qualified on the release.
- [ ] **Post-release monitoring live; automation deferral recorded** (P10.30). Evidence:
  monitoring confirmation on the released version; first review date set; deferral record
  stating the approval route for any future bounded automation.
