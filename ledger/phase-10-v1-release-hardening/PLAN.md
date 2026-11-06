# Phase 10 — v1.0.0 Pilot-Ready Release Hardening / Closure: Plan

**Phase:** 10 (release gate — not a report stage)
**Release target:** v1.0.0 pilot-ready; operator-approved control by default (§2.1, §21.4)
**Ledger:** `ledger_phase_10.md` (P10.01–P10.30)

## 1. Scope

### In scope

- Closing and consolidating the evidence for a releasable v1.0.0: every §22.3 release-gate
  item verified against the frozen release candidate, the §22.2 CI gates run, the §19.3–
  §19.4 benchmark reproduced, the §21 deployment/upgrade model documented and tested, the
  security review completed, the site integration test harness packaged, and the go/no-go
  decision recorded (§25 pilot release).
- The release artifacts themselves: signed, checksummed, with SBOMs and a support matrix
  (§21.4), and the pilot handover pack for the released version.

### Relationship to Phase 9 (stated explicitly, as required)

**Phase 10 overlaps Stage 9 but is the release gate, not duplicated work.** Stage 9
(Phase 9) qualifies a customer deployment at named sites. Phase 10 closes and consolidates
Stage 9 evidence into a releasable v1.0.0. Where a Phase 10 subtask depends on Phase 9
evidence, it cross-references the Phase 9 subtask (e.g. P10.05 cross-references P9.03/P9.19;
P10.16 cross-references P9.06/P9.07) and **re-verifies that evidence against the release
candidate**; it does not redo the pilot qualification. The only re-runs in Phase 10 are
verifications against a different artifact (the frozen release candidate) or consolidations
of evidence into release form (security review, release notes, go/no-go record). If a
Phase 10 re-verification finds drift between the pilot build and the candidate, that is a
finding — not a reason to re-run Phase 9.

### Non-goals (explicitly out of scope)

- **Re-qualifying the pilot.** Pilot qualification is Phase 9's job. Phase 10 does not
  re-run HIL, field acceptance, or the customer acceptance review.
- **Bounded automation.** Not built, not enabled, not qualified. Bounded automation is
  post-1.0, per deployment, site, and asset class, after separate qualification (§21.4,
  §25). P10.30 records the deferral in writing.
- **New features, adapters, or markets.** The release scope is frozen in P10.01; anything
  not in the freeze is a post-1.0 item.
- **Certification of anything.** v1.0.0 is not certified, not a safety case, and not
  permission for live grid control. The security review (P10.25) states its limits plainly.
- **A second pilot or a production rollout beyond the approved control scope (P9.26).**

## 2. Architecture decisions in play

| Decision | Status in this phase | Report |
|---|---|---|
| D4 license and dependency audit | **Final closure here (P10.19)** for the v1.0.0 artifact set — first gated before v0.1.0: SBOMs from lockfiles, per-dependency license determination, legal review | §25.2 |
| D1/D2/D3/D5/D6 | Closed (or interim) in Phase 9 for the deployment; D6 must close **before any read-only pilot**. Phase 10 consolidates the closure evidence (P10.16) and blocks the go decision if the release scope exceeds the D6 scope | §25.2, §22.3 |
| Public API/plugin contracts frozen | v1.0.0 freezes the public API and plugin contracts for the 1.x line (§21.4); the 0.x line could break contracts with a migration note — that ends here (P10.02) | §21.4, §10.5 |
| Control-authority mode | Operator-approved is the v1.0.0 default and is qualified in P10.15; mode is a deployment/per-site setting, never selectable by a plugin (§21.4, §5.7) | §21.4 |
| Solver discipline | HiGHS LP/QP/MIP, Clarabel continuous conic, IPOPT local — enforced in the independent validation (P10.05) | §7.4, §12.3 |

## 3. Component breakdown

| Component / workstream | Subtasks | Notes |
|---|---|---|
| Release freeze | P10.01–P10.02 | Scope + adapter matrix; API/contract freeze with Buf breaking detection |
| §22.3 gates 2–8 | P10.03–P10.10 | Command transitions, auth/tenancy, optimizer validation, stale/missing telemetry, edge modes, plugin boundaries, reproducible benchmark + thresholds |
| §22.3 gate 9 + migrations | P10.11–P10.13 | Backup/restore, upgrade/rollback, migration compatibility |
| §22.3 gates 10–12 | P10.14–P10.17 | Browser/OS matrix, control-authority qualification, cyber + operational risk reviews |
| Build and supply chain | P10.18–P10.21 | Full CI run + candidate build, D4 SBOM/license closure, vulnerability closure, signed artifacts |
| Release packaging | P10.22–P10.26 | Test harness, operator-workflow check, deployment/upgrade guide, security review, release notes |
| Decision and handover | P10.27–P10.30 | Go/no-go, tagging/publication, handover pack, post-release monitoring + automation deferral |

## 4. Data contracts

- **Canonical units and signs (§10.2):** MW, MVAr, MVA, MWh; `P > 0` = export; reactive-power
  sign declared per model/profile; SOC in [0,1] at the API boundary. The release does not
  change any contract — P10.02 freezes them.
- **Telemetry quality (§10.4):** six canonical states; verified on the candidate in P10.06.
- **Command invariants (§14.3):** verified across the full transition matrix in P10.03.
- **Optimization result contract (§12.6):** status, problem class, solver/provider version,
  objective and units, diagnostics, residuals, runtime, input snapshot, grid-model version,
  uncertainty/assumptions, warnings — required on every validation run in P10.05.
- **Schema governance (§10.5):** Protobuf via Buf lint/breaking; migrations with compatibility
  windows (P10.13).
- **Audit (§10.1):** actor, action, object, before/after references, decision, evidence,
  timestamp, correlation ID — the go/no-go pack is itself auditable (P10.27).

## 5. Interfaces

- **REST/OpenAPI (§15.1–§15.2):** versioned base path (e.g. `/api/v1`); frozen at v1.0.0.
- **gRPC/Protobuf (§10.5):** internal worker contracts; Buf-governed.
- **Plugin WIT interfaces and manifests (§5.5, §9.3):** frozen; compatibility matrix published.
- **NATS JetStream (§15.4–§15.5):** at-least-once delivery; idempotent consumers; transactional
  outbox — re-verified through the backup/restore and upgrade tests (P10.11–P10.12).
- **Operator console (§20):** browser matrix per §21.4; workflow stability checked in P10.23.
- **Site edge agent (§18):** signed update bundles; offline/degraded behaviour tested in P10.07.

## 6. Sequencing

1. **Freeze** (P10.01–P10.02): scope, adapter matrix, public contracts. The freeze names the
   release-candidate identifier; later changes re-open the affected subtasks.
2. **Build the candidate** (P10.18, with P10.13 migrations): full CI gate run; the candidate
   is the artifact every verification subtask tests.
3. **Verify the §22.3 gates against the candidate** (P10.03–P10.17): command transitions,
   auth/tenancy, optimizer validation, telemetry handling, edge modes, plugin boundaries,
   benchmark rerun, backup/restore, upgrade/rollback, browser/OS matrix, control-authority
   qualification, cyber + operational risk reviews. These run in parallel where they do not
   share an artifact; P10.10 follows P10.09, P10.12 follows P10.13. Every verification
   subtask names P10.18 (the release candidate) in its Dependencies field, so no gate
   verification can run against an unidentified build.
4. **Close the supply chain** (P10.19–P10.21): D4 SBOM/license audit, vulnerability closure,
   signed artifacts and support matrix.
5. **Package the release** (P10.22–P10.26): test harness, operator-workflow check, deployment/
   upgrade guide, security review, release notes with known limitations.
6. **Decide, publish, hand over** (P10.27–P10.30): go/no-go, tagging/publication, handover
   pack, post-release monitoring and the bounded-automation deferral record.

Relative effort is ordered by dependency, not by calendar: the benchmark rerun, the
upgrade/rollback test, and the D4 audit dominate elapsed time (compute, staging, and legal
review respectively). No calendar dates are promised in this ledger.

## 7. Roles

| Role | Owns |
|---|---|
| Product | Scope freeze (P10.01), operator workflow (P10.23), release notes (P10.26), go/no-go chair (P10.27), handover (P10.29–P10.30) |
| Platform engineering | Contract freeze (P10.02), command/auth/telemetry/edge tests (P10.03, P10.06–P10.07), benchmark (P10.09–P10.10), backup/upgrade/migrations (P10.11–P10.13), CI + candidate (P10.18), artifacts (P10.21), harness (P10.22), deployment guide (P10.24), publication (P10.28) |
| Power systems | Optimizer validation (P10.05), benchmark (P10.09–P10.10) |
| Security | Auth/tenancy (P10.04), plugin boundaries (P10.08), control-authority qualification (P10.15), cyber review (P10.16), vulnerability closure (P10.20), security review (P10.25) |
| Legal | D4 license audit (P10.19, with engineering); go/no-go participant |
| Operations | Operational risk review (P10.17), handover and post-release monitoring (P10.29–P10.30) |
| Customer operator | Operator-workflow walkthrough (P10.23), handover walkthrough (P10.29) |

## 8. Risks and mitigations

| Risk | Mitigation (in this phase) |
|---|---|
| Candidate drift (verifications run against different builds) | Single candidate identifier from P10.18; every verification subtask records it |
| Thresholds fitted to benchmark results | P10.10 requires thresholds dated before the P10.09 runs |
| D4 finds an incompatible dependency late | P10.19 runs before the go decision; an unclosable D4 blocks the release (P10.27) |
| A §22.3 gate waived silently | P10.27 requires explicit, signed, time-bounded waivers with compensating controls |
| Release scope exceeds the D6 closure | P10.16 records the gap as a go blocker |
| Automation enabled by drift | P10.15 verifies no automation envelope in the candidate; P10.30 records the deferral |
| Release notes omit a Phase 9 limitation | P10.26 cross-checks the notes against the P9.27 register item for item |

## 9. What must NOT be built yet

- Bounded automation in any form.
- New features, adapters, or market integrations beyond the P10.01 freeze.
- Any certification, conformance, or safety-case claim for v1.0.0, an adapter, or a site.
- A production rollout beyond the approved control scope (P9.26).
- A v1.1.0 scope decision inside this phase — post-1.0 items are listed as deferred, not
  decided.

## 10. How the §24 vertical slice lands here

The §24 vertical slice is the reproducibility proof for the release: P10.09 re-runs the
slice's replay step (step 10 — replay the same scenario with the same versions and verify
reproducibility) against the release candidate, and P10.10 publishes the results with
thresholds. The slice's simulated dispatch steps (7–8) are not release evidence for field
behaviour; field evidence comes from Phase 9 (P9.15 HIL, P9.17 field acceptance), which
Phase 10 consolidates (P10.16–P10.17) rather than repeating. The slice lands in Phase 10 as
a benchmark, not as a pilot.
