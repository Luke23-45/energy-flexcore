# Phase 8 — Advanced Coordination Research: Exit-Gate Checklist

| Field | Value |
|---|---|
| Phase | 8 — Stage 8 Advanced coordination research |
| Report trace | §23 Stage 8 |
| Release | v1.x — Advanced coordination; v2.x — Advanced grid intelligence |

## §23 Stage 8 exit gate (verbatim substance)

> Every research module is compared to strong deterministic baselines and
> independently validated.

Each item below mirrors that gate (plus the §23 Stage 8 deliverables list).
Tick an item only when the named evidence exists. Evidence is a file, a
test run, or a signed record — not an assertion in chat.

## Deliverables (§23 Stage 8)

- [ ] **ADMM/DMPC provider with clear convexity assumptions** — Evidence: formulation + assumptions note (reviewer-signed) + provider behind the `Optimizer` contract + residuals/termination/infeasibility/fallback implementation. (P8.07, P8.08)
- [ ] **GNN warm-start/screening experiment** — Evidence: experiment design (roles, topology scope, held-out sets, leakage controls) + isolated inference worker + evaluation on unseen topologies by end-to-end feasible solve time. (P8.10–P8.12)
- [ ] **MARL simulation/shadow worker** — Evidence: CTDE design + safety-filter derivation (reviewed) + shadow worker with action logger + evaluation under held-out regimes, outages, delays, uncertainty. (P8.13–P8.15)
- [ ] **Dynamic simulator integration for frequency/stability research** — Evidence: ANDES worker (pinned version, GPLv3 isolation verified) + model/case record + dynamic evaluation under disturbances. (P8.16, P8.17)
- [ ] **Benchmark paper/report** — Evidence: the report itself (P8.26) + independent review record with no unaddressed major comment.

## Exit gate

- [ ] **ADMM/DMPC compared to strong deterministic baselines** — Evidence: P8.09 evaluation report; frozen baselines (P8.06) compared on the same cases; wins and losses reported; no global-optimality claim outside the stated convexity assumptions.
- [ ] **GNN compared to strong deterministic baselines** — Evidence: P8.12 evaluation report; non-GNN baseline (cold start + heuristic warm start) compared on unseen topologies; end-to-end feasible solve time is the headline metric; every counted success passed independent validation.
- [ ] **MARL compared to strong deterministic baselines** — Evidence: P8.15 evaluation report; deterministic optimization and rule-based/PI baselines compared on the same cases; filter statistics complete; finite-test-set limitation stated.
- [ ] **Every research module independently validated** — Evidence: P8.05 harness results per module; harness owned separately from module authors; deliberately-wrong cases caught; harness version recorded per validation.
- [ ] **All results reproducible** — Evidence: P8.20 reproducibility records (seeds, versions, artifacts); at least one result per module passed the rerun test by a second engineer; non-reproducing results marked and excluded.
- [ ] **Cross-module comparison complete** — Evidence: P8.23 comparison report on the same cases/seeds/metrics; no missing cells without a reason.
- [ ] **Proposal-only boundary enforced** — Evidence: worker response types cannot carry an authorization token (P8.04 type inspection); no code path from any research worker to dispatch (P8.14 boundary test); shadow log complete.
- [ ] **License audit covers the research stack** — Evidence: P8.03 audit table; ANDES GPLv3 isolation design confirmed by legal; PyTorch/PyG pair is a verified-compatible pin. (P8.03)
- [ ] **Failure-mode drill complete** — Evidence: P8.24 drill report; zero faults let an invalid proposal pass validation or crash the control plane.
- [ ] **Security review complete with zero open high/critical findings** — Evidence: P8.25 research security-review report (model files, training data, worker outputs as untrusted input).
- [ ] **CI gates green on the research code** — Evidence: CI run showing Python Ruff/type/pytest, Rust gates, scenario regression incl. research benchmark cases, seeded reproducibility check. (P8.28)
- [ ] **Operator console shows research proposals as proposals** — Evidence: console walkthrough record; no research output displayed as executed. (P8.27)
- [ ] **Per-feature qualification record written (not granted)** — Evidence: P8.29 record per module listing what field use would require, with the "not qualified" statement per module. (P8.29)
- [ ] **Retrospective and exit-gate record written** — Evidence: retrospective + open-items register + exit-gate record containing the "research only, not field-qualified" limitation in plain language. (P8.30)

- [ ] **Research-worker tenant isolation enforced** — Evidence: scoping rule + cross-tenant negative-test record; unapproved cross-tenant model serving refused. (P8.31)
- [ ] **Model artifacts signed and verified at load** — Evidence: tampered/unsigned/revoked artifact refusal record. (P8.32)
- [ ] **Research-worker identity, mTLS, and secret handling** — Evidence: identity design + revoked-identity refusal + literal-secret rejection tests. (P8.33)
- [ ] **Research proposals carry validity windows** — Evidence: expired/superseded/missing-window proposals rejected. (P8.34)
- [ ] **Training-data provenance, license, and poisoning controls** — Evidence: dataset inventory + quarantine record; dataset version linked to the model artifact. (P8.35)
- [ ] **Research-stack CVE check at pin time** — Evidence: advisory-check record per pinned version with dates. (P8.36)
- [ ] **Unit and property tests exist per research worker** — Evidence: suites running in CI with per-worker coverage recorded. (P8.37)
- [ ] **Training/inference reruns reproduce within pre-recorded tolerances** — Evidence: rerun record per module with GPU-nondeterminism disclosure. (P8.38)
- [ ] **HELICS/ANDES cross-checks pass** — Evidence: time-step mismatch detection, ANDES vs pandapower comparison, federate-dropout detection. (P8.39)

## What this gate does NOT establish

- It does not establish safety, field qualification, or dispatch authority
  for any research module. Per-feature qualification (§2.1) is a separate,
  later decision (Stage 9); shadow/simulation is the ceiling of this phase.
- It does not establish global optimality, convergence, or generalization
  beyond the stated assumptions, tested topologies, and tested regimes.
  A finite test set with zero observed violations is not a universal safety
  guarantee (§13.3).
- It does not establish capability for network classes, topologies, or
  scales that were not tested (notably not a 10,000-bus network, §19.3).
