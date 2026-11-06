# Phase 10 — v1.0.0 Pilot-Ready Release Hardening / Closure: Ledger

**Phase:** 10 (release gate — not a report stage)
**Release target:** v1.0.0 pilot-ready (operator-approved control by default; §2.1, §21.4)
**Status:** Not started. No subtask is complete until its acceptance criteria are met and the
evidence named in the subtask exists and has been checked.
**Research cut-off:** 7 November 2026 (report §27). Nothing in this ledger invents a version,
date, or certification status after that cut-off.

> **What Phase 10 is — and is not.** Phase 10 overlaps Stage 9 but is the **release gate,
> not duplicated work**. Stage 9 (Phase 9) qualifies a customer deployment at named sites.
> Phase 10 closes and consolidates that evidence — plus the §22.3 release gates, the §22.2
> CI gates, the §19.3–§19.4 benchmarks, and the §21 deployment model — into a releasable
> v1.0.0. Where a Phase 10 subtask depends on Phase 9 evidence, it **cross-references the
> Phase 9 subtask and re-verifies it against the release candidate; it does not redo the
> pilot qualification**. A Phase 10 subtask that re-runs a test does so against the frozen
> release candidate build, which is a different artifact from the pilot build — that is
> verification, not duplication.
>
> FlexCore v1.0.0 is not certified, not a safety case, and not permission for live grid
> control. Bounded automation is post-1.0, per deployment, site, and asset class, after
> separate qualification (§21.4, §25).

## How to read this ledger

- Every subtask has an ID (`P10.01` … `P10.30`), a report trace (`§N` / `§N.M`), inputs,
  detailed steps, a concrete deliverable, testable acceptance criteria, an owner role,
  dependencies, a verification method, and risks/failure modes.
- Checkbox `- [ ]` marks an incomplete subtask. Check it only when the acceptance criteria
  are met and the evidence is filed in the location the subtask names.
- The §22.3 release gates are covered one-for-one: P10.01 (§22.3 item 1), P10.03 (item 2),
  P10.04 (item 3), P10.05 (item 4), P10.06 (item 5), P10.07 (item 6), P10.08 (item 7),
  P10.09–P10.10 (item 8), P10.11–P10.13 (item 9), P10.14 (item 10), P10.15 (item 11),
  P10.16–P10.17 (item 12). The mapping is stated in each subtask so a gate can never be
  claimed covered by the wrong subtask.
- Canonical units throughout: MW, MVAr, MVA, MWh; `P > 0` = export at the point of
  connection; reactive-power sign declared per grid model and device profile; SOC as a
  fraction in [0,1] at the API boundary (§10.2). Telemetry quality is exactly one of
  `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE` (§10.4). Use "license" (US
  spelling) and "acknowledgement" (per `.agent/WRITING.md`).
- Open gates carried into this phase: D4 (license and dependency audit) was gated **before
  v0.1.0**; its **final closure for the v1.0.0 release artifact set** is here (P10.19) — P10.19
  is a re-closure for this artifact set, not the first D4 closure. D6 (cyber and OT boundary
  review) must close **before any read-only pilot** — the stricter D6 gate from
  `.agent/DECISIONS.md`, not merely before promotion beyond read-only mode. D1–D3 and D5 must
  be closed for the deployment jurisdiction, network class, and protocol mix before the
  deployment is promoted beyond read-only mode (§22.3). Closure evidence for D1–D3, D5, D6
  comes from Phase 9 (P9.01–P9.06, P9.12) and is consolidated here, not re-decided.

## Phase exit gate

**Exit gate (§25 pilot release + §22.3): v1.0.0 is releasable as pilot-ready when every
§22.3 release gate is met against the frozen release candidate, the §22.2 CI gates pass,
the benchmark is reproducible, the deployment/upgrade model is documented and tested, the
security review is complete, the site integration test harness is packaged, and the
go/no-go record (P10.27) says go.** The gate is met only when every subtask in this ledger
is checked and the `CHECKLIST.md` in this directory is complete with the evidence it names.

## Subtask index

| ID | Title | Trace |
|----|-------|-------|
| P10.01 | Release scope freeze and supported-adapter matrix | §22.3 (1), §2.1 |
| P10.02 | Public API and plugin-contract freeze | §21.4, §10.5, §9.3 |
| P10.03 | Command-state transition test suite — full matrix | §22.3 (2), §14 |
| P10.04 | Authorization and tenant-isolation test suite | §22.3 (3), §16.2, §17 |
| P10.05 | Optimizer independent validation | §22.3 (4), §12 |
| P10.06 | Stale/missing telemetry handling verification | §22.3 (5), §10.4 |
| P10.07 | Edge offline/degraded-mode tests | §22.3 (6), §18, §21.2 |
| P10.08 | Plugin permission-boundary tests | §22.3 (7), §5.7, §9.3 |
| P10.09 | Reproducible benchmark — full rerun | §22.3 (8), §19.3–§19.4 |
| P10.10 | Benchmark results pack and thresholds record | §19.4, §22.3 (8) |
| P10.11 | Backup/restore test on the release candidate | §22.3 (9), §21.2, §17.3 |
| P10.12 | Upgrade/rollback test on the release candidate | §22.3 (9), §21.2, §5.6 |
| P10.13 | Schema migration compatibility test | §22.2, §10.5, §21.2 |
| P10.14 | Browser/OS support-matrix test | §22.3 (10), §21.4 |
| P10.15 | Control-authority qualification — operator-approved mode | §22.3 (11), §21.4 |
| P10.16 | Deployment-specific cyber risk review (consolidation) | §22.3 (12), §17 |
| P10.17 | Deployment-specific operational risk review | §22.3 (12), §17.4, §21.2 |
| P10.18 | Full CI gate run and release-candidate build | §22.2 |
| P10.19 | SBOM and license audit closure — D4 | §22.2, §22.3, §25.2 (D4), §9.3 |
| P10.20 | Vulnerability and dependency scan closure | §22.2, §17.3 |
| P10.21 | Signed artifacts, checksums, and support matrix | §21.4, §9.3 |
| P10.22 | Site integration test harness — packaged and runnable | §25, §23 Stage 9 |
| P10.23 | Operator-workflow stability check | §20, §25 |
| P10.24 | Deployment and upgrade model documentation | §21.1–§21.2, §25 |
| P10.25 | Security review consolidation report | §17, §25 |
| P10.26 | Release notes and known limitations | §2.1, §25 |
| P10.27 | Go/no-go decision record | §22.3 |
| P10.28 | v1.0.0 tagging and release publication | §21.4, §22.3 |
| P10.29 | Pilot handover pack (operations runbook) | §21.2–§21.3, §23 Stage 9 |
| P10.30 | Post-release monitoring and bounded-automation deferral record | §21.3, §21.4, §25 |

---

## Subtasks

### P10.01 — Release scope freeze and supported-adapter matrix

- [ ] **P10.01 Release scope freeze and supported-adapter matrix**

- **Objective:** Freeze, in writing, exactly what v1.0.0 contains and which adapters it
  supports — §22.3 gate item 1 ("documented scope and supported adapters are complete").
- **Report trace:** §22.3 (release gate 1); §2.1 (v1.0.0 definition).
- **Inputs / prerequisites:** Phase 9 exit-gate record (P9.30); the §2.1 v1.0.0 capability
  list; the P9.04 integration design.
- **Steps:**
  1. Write the v1.0.0 scope document: capabilities included (stable operator workflow,
     reproducible benchmark, deployment/upgrade model, security review, site integration
     test harness — §25), capabilities excluded, and the asset/protocol boundary (§2.2–§2.3).
  2. Build the supported-adapter matrix: each adapter, its version, its profile/variant,
     the pilot it was qualified against (cross-reference P9.04/P9.15), and its conformance
     status — with the rule that implementing an adapter is not a conformance claim (§2.3).
  3. Record the frozen release-candidate identifier (commit/tag, build ID) that the scope
     applies to. Any change to the candidate after this point re-opens this subtask.
  4. Have product and engineering sign the scope freeze.
- **Deliverable / output:** v1.0.0 scope freeze document + supported-adapter matrix,
  tied to the release-candidate identifier.
- **Acceptance criteria / exit check:** (a) every adapter in the matrix has a version,
  profile, and qualification cross-reference (P9.04/P9.15) or is marked unsupported;
  (b) no adapter is described as certified/conformant without a cited test result;
  (c) the scope document names the release-candidate identifier; (d) the freeze is signed.
- **Owner role:** Product (with engineering).
- **Dependencies:** P9.30 (Phase 9 exit-gate record). Blocks P10.02, P10.18, P10.26.
- **Verification method:** Trace each matrix row to its Phase 9 evidence; confirm the
  candidate identifier matches the build under test in P10.18.
- **Risks / failure modes:** An adapter qualified in the lab but never at a pilot site
  listed as supported; scope written against a moving candidate so the release notes
  describe a different build.

### P10.02 — Public API and plugin-contract freeze

- [ ] **P10.02 Public API and plugin-contract freeze**

- **Objective:** Freeze the public contracts for the 1.x line, as §21.4 requires at v1.0.0,
  and prove the freeze with breaking-change detection.
- **Report trace:** §21.4 (v1.0.0 freezes the public API and plugin contracts for the 1.x
  line); §10.5 (Buf lint/breaking); §9.3 (SemVer, compatibility matrix).
- **Inputs / prerequisites:** P10.01 scope freeze; the REST/OpenAPI surface (§15.1–§15.2),
  Protobuf schemas (§10.5), plugin WIT interfaces and manifests (§5.5, §9.3); the NATS
  event vocabulary and JetStream topology (§15.3–§15.5; P0.14, P1.09, P1.14, P2.16 —
  cross-references, not re-builds); the §10.1 canonical entities and §10.2–§10.3 units/time
  contracts (P0.07, P0.09).
- **Steps:**
  1. List the frozen public contracts: REST API (versioned base path, e.g. `/api/v1`),
     Protobuf schemas, plugin WIT interfaces, plugin manifests, **and the NATS event
     vocabulary with its subject mapping** (§15.3) plus the JetStream stream configuration
     (per-stream retention, replication, acknowledgement, replay, and dead-letter/
     quarantine policy, §15.4).
  2. Run Buf lint and Buf breaking-change detection against the previous published
     contracts (§10.5, §22.2); record the result. Any breaking change must be deliberate,
     versioned, and recorded — not silent.
  3. Run plugin manifest and WIT compatibility validation (§22.2) against the frozen
     contracts.
  4. Run an OpenAPI diff / breaking-change check of the frozen REST surface against the
     previous published OpenAPI spec (Buf covers Protobuf only — it does not cover REST):
     verify every §15.2 candidate resource is present, every route is in the generated
     (not hand-written) spec, and no generic "execute arbitrary command" endpoint exists
     (§15.2). Record the diff result; any breaking REST change must be deliberate,
     versioned, and recorded like a Protobuf one.
  5. Verify the frozen contracts cover all fourteen §10.1 canonical entities (Tenant,
     Portfolio, Site, Asset, AssetState, CapabilityEnvelope, GridModel, ForecastBundle,
     Schedule, DispatchCommand, DispatchExecution, MarketCommitment, PluginInstance,
     AuditRecord) and carry the §10.2 units/sign rules (MW/MVAr/MVA/MWh, `P > 0` export,
     reactive-power sign declared per grid model/device profile, SOC as a fraction in
     [0,1] at the API boundary) and the §10.3 time fields (`event_time`, `ingest_time`,
     `valid_from`/`valid_until`, source sequence/version, UTC internally). Verify no
     public contract exposes a raw database schema (§10.5).
  6. Verify the frozen NATS vocabulary names all twenty-six §15.3 events with the same
     names and subject mapping as the P1.14 topology, and that every JetStream stream
     carries its §15.4 retention/replication/ack/replay/dead-letter policy. An event
     renamed, dropped, or re-subjected without a versioned change is a breaking change.
  7. Publish the compatibility matrix (§9.3): contract version ↔ plugin/API version,
     including the event-vocabulary and stream-configuration versions.
  8. Record the SemVer rule for the 1.x line: the 0.x line could break contracts with a
     migration note; from v1.0.0 the public contracts are frozen (§21.4).
- **Deliverable / output:** Frozen-contract list (REST + Protobuf + WIT/manifests + NATS
  event vocabulary and stream configuration) + Buf lint/breaking report + OpenAPI
  diff/breaking report + compatibility matrix.
- **Acceptance criteria / exit check:** (a) Buf lint passes with zero errors; (b) Buf
  breaking reports zero unintended breaking changes (any intended change is listed with
  its versioning decision); (c) manifest/WIT validation passes; (d) the compatibility
  matrix covers every public contract; (e) the OpenAPI diff reports zero unintended
  breaking REST changes, every §15.2 resource is present, and no generic execute
  endpoint exists; (f) all fourteen §10.1 entities are represented, canonical units/sign
  (including SOC in [0,1] and declared Q sign) and §10.3 time fields hold in the frozen
  contracts, and no raw database schema is exposed; (g) all twenty-six §15.3 events are
  frozen with their subject mapping and every stream carries its §15.4 policy.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01. Blocks P10.18, P10.21.
- **Verification method:** Re-run Buf lint and breaking detection from a clean checkout and
  compare the output to the filed report; re-run the OpenAPI diff and the event-vocabulary
  list against the P1.14 topology and confirm the same result.
- **Risks / failure modes:** A breaking change introduced after the freeze by a late fix;
  a contract changed without a version bump, silently breaking a pilot integration.

### P10.03 — Command-state transition test suite — full matrix

- [ ] **P10.03 Command-state transition test suite — full matrix**

- **Objective:** Prove every command-state transition is tested against the release
  candidate — §22.3 gate item 2 ("all command-state transitions are tested").
- **Report trace:** §22.3 (release gate 2); §14.1–§14.3.
- **Inputs / prerequisites:** P10.01 scope freeze; the §14.1 state machine; Phase 9 shadow
  dry-run evidence (P9.19 — cross-reference, do not redo the shadow run).
- **Steps:**
  1. Enumerate the full transition matrix from the §14.1 state diagram: every legal
     transition (including REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK)
     and the illegal transitions that must be refused.
  2. For each transition, run a deterministic test against the release candidate with
     pinned inputs and record the outcome. Include: stale, duplicated-with-conflicting-
     payload, out-of-sequence, outside-capability, and missing-evidence candidates — all
     must be rejected (§14.3).
  3. Verify per-asset representation of partial fleet success: a portfolio-level
     acknowledgement is not inferred from the first responding device (§14.3).
  4. Verify the §14.3 invariants on test commands (unique ID, validity window, sequence,
     idempotency key, evidence references).
  5. Record the suite result with the release-candidate identifier; any failure blocks the
     release (P10.27) until fixed and re-run.
- **Deliverable / output:** Command-state transition test report (matrix, per-transition
  outcome, candidate ID).
- **Acceptance criteria / exit check:** (a) every legal transition in §14.1 has a passing
  test; (b) every enumerated illegal transition is refused; (c) all five rejection classes
  (§14.3) are demonstrated; (d) partial-fleet success is represented per asset.
- **Owner role:** Platform engineering (with QA/test).
- **Dependencies:** P10.01, P10.18 (release candidate — every test in this subtask runs against the candidate identifier from P10.18); P9.19 (cross-reference). Blocks P10.27.
- **Verification method:** Independent re-run of the suite from the filed configuration;
  confirm the transition list matches §14.1 exactly (no transition omitted).
- **Risks / failure modes:** A rarely-used transition (e.g. SUPERSEDED from TRACKING) never
  tested; a test that passes because the test harness, not the platform, enforces the rule;
  the suite run against a different build than the release candidate.

### P10.04 — Authorization and tenant-isolation test suite

- [ ] **P10.04 Authorization and tenant-isolation test suite**

- **Objective:** Prove authorization and tenant isolation hold on the release candidate —
  §22.3 gate item 3.
- **Report trace:** §22.3 (release gate 3); §16.2; §17.3–§17.4; §5.7.
- **Inputs / prerequisites:** P10.01 scope freeze; Phase 9 identity/access evidence (P9.08)
  and tenancy evidence (P9.10) — cross-referenced, re-run against the candidate.
- **Steps:**
  1. Re-run authorization tests against the candidate: each role attempts actions above
     its permission (including `dispatch.authorize` from a plugin path, §5.7) and is
     denied, with the denial audited.
  2. Re-run tenant-isolation tests: cross-tenant read, write, and command attempts are
     denied and audited; `tenant_id` scoping is verified on every tenant-owned record
     type (§16.2).
  3. Verify RLS as defence in depth: confirm the application runtime role does not own
     tenant tables and does not bypass RLS (§16.2, [R25]); test all service roles,
     including `FORCE ROW LEVEL SECURITY` where configured.
  4. Verify tenant context is derived from authenticated claims/authorized context, never
     from untrusted request-body fields (§16.2).
  5. Verify edge identities are scoped to one tenant and one approved site/asset set (§16.2).
  6. Verify the P9.10 retention/residency design is actually configured on the candidate
     (§16.3): a retention value per data class (raw high-frequency telemetry, normalized
     telemetry, aggregated intervals, command/audit evidence, market records, simulation
     artifacts, model/forecast outputs), configured from the customer contract — not a
     universal period hard-coded in code — plus the data-residency setting and the
     personal-data export/deletion procedure from P9.10.
- **Deliverable / output:** Authorization and tenant-isolation test report (cases, outcomes,
  audit extracts, RLS verification).
- **Acceptance criteria / exit check:** (a) every unauthorized attempt is denied and
  audited — zero unauthorized successes; (b) no service role bypasses RLS; (c) tenant
  context is never taken from a request body; (d) every edge identity is scoped to one
  tenant/site set; (e) every §16.3 data class has a retention value configured on the
  candidate (no hard-coded universal period), with the residency setting and the
  export/deletion procedure present.
- **Owner role:** Security (with platform engineering).
- **Dependencies:** P10.01, P10.18 (release candidate); P9.08, P9.10 (cross-references). Blocks P10.27.
- **Verification method:** Independent execution of a sample of negative cases against the
  candidate; inspect the audit log for the denial records.
- **Risks / failure modes:** A test that only checks the happy path; RLS verified in a
  configuration where the application runs as table owner; a plugin path that bypasses the
  authorization check the API enforces.

### P10.05 — Optimizer independent validation

- [ ] **P10.05 Optimizer independent validation**

- **Objective:** Prove optimizer outputs are independently validated on the release
  candidate — §22.3 gate item 4. The optimizer never validates itself.
- **Report trace:** §22.3 (release gate 4); §12.3, §12.5–§12.6; §9.2.
- **Inputs / prerequisites:** P10.01 scope freeze; the §12.6 optimization-result contract;
  the pilot grid model (P9.03) and the Phase 9 shadow evidence (P9.19).
- **Steps:**
  1. For each optimizer provider in the release, record the declared problem class and
     solver capability (§12.3 tiers; §7.4 solver selection rule) — HiGHS as LP/QP/MIP
     only, Clarabel as continuous conic only, IPOPT as local only.
  2. Validate optimizer outputs with an independent path: run the candidate injections
     through the §12.5 feasibility gate (AC power flow / three-phase unbalanced power flow
     on the versioned network model) and compare against the optimizer's own claims.
  3. Check the §12.6 result contract on every validation run: status, problem class,
     solver/provider version, objective value and units, diagnostics, residuals, runtime,
     input snapshot, grid-model version, uncertainty/assumptions, warnings.
  4. Include infeasible and timeout cases: the validator must report non-convergence and
     model coverage limitations explicitly, and the system must reject or hold the command
     per policy — never infer feasibility from a failed or stale model (§12.5, §17.5
     invariant 9).
  5. Record the validation evidence per run (P9.19 cross-reference for the shadow cases;
     new runs here against the candidate).
- **Deliverable / output:** Optimizer independent-validation report (provider/class table,
  validation runs, residuals, infeasible/timeout handling).
- **Acceptance criteria / exit check:** (a) every provider's declared class matches the
  problem it solved; (b) every optimizer output in the validation set was checked by the
  independent feasibility path; (c) infeasible/timeout cases result in reject/hold, never
  in an inferred pass; (d) every run carries the full §12.6 contract.
- **Owner role:** Power systems engineering (independent of the optimizer provider owner).
- **Dependencies:** P10.01, P10.18 (release candidate); P9.03, P9.19 (cross-references). Blocks P10.27.
- **Verification method:** Re-run one validation case from the filed inputs and compare the
  residuals; confirm the validator and the optimizer are different code paths.
- **Risks / failure modes:** The "independent" check reusing the optimizer's own model;
  an SOCP relaxation assumed exact for reverse power flow or voltage upper bounds (§12.5);
  a solver described by the wrong class (e.g. HiGHS called an SOCP solver).

### P10.06 — Stale/missing telemetry handling verification

- [ ] **P10.06 Stale/missing telemetry handling verification**

- **Objective:** Prove stale and missing telemetry cases are handled on the release
  candidate — §22.3 gate item 5.
- **Report trace:** §22.3 (release gate 5); §10.4; §17.4–§17.5.
- **Inputs / prerequisites:** P10.01 scope freeze; Phase 9 telemetry verification (P9.18)
  — cross-referenced, re-run against the candidate.
- **Steps:**
  1. For each telemetry quality state (§10.4), verify the system's behaviour on the
     candidate: `GOOD` accepted; `SUSPECT`/`BAD` handled per the use-case policy;
     `MISSING`/`STALE` handled by hold/reject, never by silent substitution;
     `ESTIMATED` visibly distinguished from measured (§20.2).
  2. Inject stale telemetry (age beyond the use-case freshness limit) and missing telemetry
     for each pilot asset class, and record the outcome per use case (schedule, feasibility,
     dispatch).
  3. Verify the freshness limits each use case applies are the recorded ones (P9.18), not
     defaults that were never set.
  4. Verify a stale/missing model is not interpreted as a passing feasibility check (§17.5
     invariant 9) in any path.
  5. Verify the operator console distinguishes measured, estimated, stale, and simulated
     values visually (§20.2) on the candidate build.
  6. Verify `ESTIMATED` values are also flagged as estimated in the telemetry/query API
     responses (§15.2), not only in the console — a consumer of the API must be able to
     tell an estimate from a measurement without opening the console.
- **Deliverable / output:** Stale/missing telemetry verification report (injection cases,
  per-use-case outcomes, console evidence).
- **Acceptance criteria / exit check:** (a) every injection case has a recorded hold/reject
  outcome — zero silent substitutions; (b) no feasibility check passes on stale/missing
  input; (c) `ESTIMATED` values are visually distinct from measured values in the console and
  flagged as estimated in telemetry/query API responses.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01, P10.18 (release candidate); P9.18 (cross-reference). Blocks P10.27.
- **Verification method:** Re-run one stale-injection and one missing-injection case against
  the candidate; inspect the console rendering for the quality distinction.
- **Risks / failure modes:** A freshness limit that exists in a document but not in code;
  an `ESTIMATED` value displayed identically to a measured value; a feasibility path that
  substitutes the last-known value when telemetry is missing.

### P10.07 — Edge offline/degraded-mode tests

- [ ] **P10.07 Edge offline/degraded-mode tests**

- **Objective:** Prove edge offline and degraded modes are tested on the release candidate —
  §22.3 gate item 6.
- **Report trace:** §22.3 (release gate 6); §18.1–§18.3; §21.2.
- **Inputs / prerequisites:** P10.01 scope freeze; Phase 9 fallback drills (P9.20) and
  backup rehearsal (P9.24) — cross-referenced, re-run against the candidate edge build.
- **Steps:**
  1. Test edge offline behaviour on the candidate: link loss → local buffering (bounded,
     §18.3) → store-and-forward on recovery; verify no command is issued from a stale
     buffered state (P9.24 cross-reference).
  2. Test edge degraded modes: adapter failure, certificate expiry warning, local audit
     buffer behaviour, and remote upgrade with staged/rollback capability (§18.1).
  3. Test the central-outage case: site-local fallback and autonomous local EMS continue;
     no dependence on the cloud for protection (§17.4, §21.2).
  4. Test command verification at the edge: signature/expiry/sequence verification (§18.1)
     rejects a forged, replayed, or expired command before forwarding.
  5. Record per-test: trigger, observed behaviour, time to fallback/recovery, versions.
- **Deliverable / output:** Edge offline/degraded-mode test report (cases, outcomes,
  timings, versions).
- **Acceptance criteria / exit check:** (a) every listed degraded mode has a passing test
  on the candidate edge build; (b) offline buffering is bounded and store-and-forward is
  verified; (c) no stale buffered command is issued; (d) a forged/replayed/expired command
  is rejected at the edge.
- **Owner role:** Platform engineering (with integration engineering).
- **Dependencies:** P10.01, P10.18 (release candidate); P9.20, P9.24 (cross-references). Blocks P10.27.
- **Verification method:** Re-run the link-loss test against the candidate and inspect the
  buffer/forward log; confirm the edge build version matches the release candidate.
- **Risks / failure modes:** An edge build that differs from the release candidate; an
  unbounded buffer that exhausts local storage on a long outage; an expired command
  accepted because the edge clock drifted (§17.4).

### P10.08 — Plugin permission-boundary tests

- [ ] **P10.08 Plugin permission-boundary tests**

- **Objective:** Prove plugin permission boundaries hold on the release candidate — §22.3
  gate item 7.
- **Report trace:** §22.3 (release gate 7); §5.7; §9.3; §5.4–§5.6.
- **Inputs / prerequisites:** P10.01 scope freeze; the plugin manifest/permission model
  (§5.5, §5.7); Phase 7 conformance suite.
- **Steps:**
  1. For each plugin in the release (built-in, WASM, gRPC worker), verify its declared
     capabilities and requested permissions against its manifest, and that activation
     validates manifest, publisher, version, capabilities, and permissions (§9.3).
  2. Attempt privilege escalation from an untrusted plugin: access to an ungranted
     capability (e.g. `dispatch.authorize`, `secret.read`, `network.any`), ambient
     filesystem/network/database access from a WASM component, and direct device-protocol
     access. Each attempt must fail and be logged.
  3. Verify WASM sandbox limits are enforced (memory, fuel/time, concurrency, output size,
     call rate — §5.4 Class B) and that signatures/provenance are required for production
     plugin installation (§9.3).
  4. Verify plugin lifecycle safety (§5.6): no activation or upgrade mid-dispatch-transaction;
     drain → stop → replace, with the version change recorded; no hot-reload of safety or
     dispatch providers in the 1.x line.
  5. Verify no plugin can self-upgrade or fetch executable code at runtime (§9.3).
- **Deliverable / output:** Plugin permission-boundary test report (plugins tested,
  escalation attempts, outcomes, lifecycle checks).
- **Acceptance criteria / exit check:** (a) every escalation attempt failed and was logged
  — zero successes; (b) sandbox limits are enforced and evidenced; (c) lifecycle rules
  hold for a plugin upgrade during an active dispatch transaction (upgrade refused or
  deferred until drain); (d) no plugin self-upgraded or fetched code at runtime.
- **Owner role:** Security (with platform engineering).
- **Dependencies:** P10.01, P10.18 (release candidate). Blocks P10.27.
- **Verification method:** Independent execution of at least one escalation attempt per
  plugin class; inspect the host capability-policy enforcement log.
- **Risks / failure modes:** A built-in plugin tested less rigorously than a WASM one
  because it is "trusted"; a sandbox limit configured but not enforced by the host;
  a plugin upgrade applied mid-transaction in a lab profile and carried into the release.

### P10.09 — Reproducible benchmark — full rerun

- [ ] **P10.09 Reproducible benchmark — full rerun**

- **Objective:** Re-run the full simulator benchmark against the release candidate and prove
  the results are reproducible — §22.3 gate item 8 ("simulator benchmark results are
  reproducible").
- **Report trace:** §22.3 (release gate 8); §19.3–§19.4.
- **Inputs / prerequisites:** P10.01 scope freeze; the §19.3 benchmark cases; the §24
  vertical-slice replay procedure; pinned solver versions, seeds, and configs.
- **Steps:**
  1. Freeze the benchmark set for v1.0.0 from §19.3: IEEE 13-bus, 34-bus, 123-bus
     distribution feeders (where licensing and model availability are confirmed), IEEE
     39-bus and 118-bus transmission systems for selected studies, synthetic portfolios
     (BESS, PV, flexible load, EVSE), multi-feeder/substation-constraint cases, and the
     stressed scenarios (PV ramps, forecast error, communication loss, asset dropout,
     reverse power flow, topology change).
  2. Run the full set against the release candidate with pinned inputs, solver versions,
     random seeds, and expected outcomes/tolerances (§19.2 item 5).
  3. Re-run the full set a second time from a clean environment and compare: results must
     reproduce within the recorded tolerances. Record both runs in full.
  4. Run the full §24 vertical slice end-to-end against the release candidate — steps 1–10
     (tenant/portfolio/site/grid model + four simulated assets → telemetry with quality
     flags → 24 h flexibility → LP/MILP/QP schedule → feeder power-flow validation →
     PROPOSED command transitions → simulator-only dispatch → acknowledgement/tracking/
     partial failure/timeout → console audit trail), not only the replay step — and then
     run the step-10 replay: replay the same scenario with the same versions and verify
     reproducibility. The piecemeal gate re-verifications (P10.03–P10.08) do not substitute
     for one end-to-end slice run on the candidate.
  5. Record every run with configs, seeds, model versions, solver versions, and outputs
     (§7.4 benchmarking; §9.2 provenance).
- **Deliverable / output:** Benchmark run records (two full runs + vertical-slice replay),
  with configs, seeds, versions, and outputs.
- **Acceptance criteria / exit check:** (a) every case in the frozen set ran in both runs;
  (b) the two runs agree within the pre-recorded tolerances — any deviation is
  investigated, not averaged away;   (c) the vertical-slice replay reproduces; (d) no case
  was dropped from the set without a recorded reason; (e) the full §24 slice (steps 1–10)
  ran end-to-end on the candidate, with the run record filed alongside the replay.
- **Owner role:** Platform engineering (with power systems).
- **Dependencies:** P10.01, P10.18 (release candidate). Blocks P10.10, P10.27.
- **Verification method:** Independent re-run of a sample of cases (at least one feeder,
  one synthetic portfolio, one stressed scenario) from the filed configs and comparison
  against the filed results.
- **Risks / failure modes:** A benchmark that "reproduces" because the tolerance was widened
  after the fact; a feeder case run on a different model version than recorded; a
  dropped stressed scenario that would have failed.

### P10.10 — Benchmark results pack and thresholds record

- [ ] **P10.10 Benchmark results pack and thresholds record**

- **Objective:** Publish the benchmark results with the §19.4 metrics and the acceptance
  thresholds they were judged against — thresholds defined before the runs, not fitted after.
- **Report trace:** §19.4; §22.3 (release gate 8).
- **Inputs / prerequisites:** P10.09 run records.
- **Steps:**
  1. Compute the §19.4 metrics for the benchmark set: scheduling (objective value,
     schedule deviation, feasible interval count, runtime); asset model (SOC/power
     violations, ramp violations, availability prediction error); grid feasibility (voltage
     violations, branch/transformer overload, AC power-flow convergence, residuals);
     flexibility (available vs delivered MW/MWh, uncertainty calibration, shortfall);
     dispatch (acknowledgement latency, tracking error, failure/expiry rate, recovery time);
     platform (availability, throughput, p95/p99 API/ingest latency, recovery, tenant
     isolation).
  2. Record the acceptance threshold for each metric and the basis for it (§19.4: "Set
     acceptance thresholds after defining the model, service, benchmark, and operational
     risk" — do not set arbitrary universal targets such as "zero violations in 1,000
     cases proves safety").
  3. Record the result per metric against its threshold, and any metric that did not meet
     its threshold with its disposition (fix, waive with rationale, or block release).
  4. State plainly what the benchmark does not establish: the first distribution feeder
     tests do not establish capability for a 10,000-bus utility network (§19.3).
  5. Publish the pack with the release-candidate identifier.
- **Deliverable / output:** v1.0.0 benchmark results pack (metrics, thresholds with bases,
  per-metric results, dispositions, scope statement).
- **Acceptance criteria / exit check:** (a) every §19.4 metric area applicable to the
  release has a recorded threshold with a stated basis; (b) every threshold was recorded
  before the P10.09 runs (evidence: dated threshold record); (c) every missed threshold
  has a disposition; (d) the pack states the benchmark's limits.
- **Owner role:** Platform engineering (with power systems).
- **Dependencies:** P10.09. Blocks P10.26, P10.27.
- **Verification method:** Check the threshold record's date against the P10.09 run dates;
  recompute one metric from the raw run outputs.
- **Risks / failure modes:** Thresholds fitted to the results; a metric omitted because it
  would have failed; benchmark results presented as a safety proof (§19.4 forbids this).

### P10.11 — Backup/restore test on the release candidate

- [ ] **P10.11 Backup/restore test on the release candidate**

- **Objective:** Prove backup and restore work on the release candidate — §22.3 gate item 9
  (first half). This re-runs the Phase 9 rehearsal against the candidate; it does not
  re-write the procedure (cross-reference P9.24).
- **Report trace:** §22.3 (release gate 9); §21.2; §17.3.
- **Inputs / prerequisites:** P10.01 scope freeze; P9.24 rehearsal procedure and measured
  timings (cross-reference); the release-candidate deployment.
- **Steps:**
  1. Execute the P9.24 procedure against the release candidate: back up PostgreSQL, NATS
     stream state, and durable command/audit records; restore into an isolated environment.
  2. Verify the restored system: command/audit records complete and consistent, outbox
     state correct, idempotent replay holds (§6.3/§15.5 — no command re-issued by replay).
  3. Measure and record the restore time and data-loss window actually observed on the
     candidate; judge them against the customer requirements recorded in P9.24 (§21.2:
     do not set RPO/RTO without customer and service requirements).
  4. Verify the schema version of the restored data matches the candidate (§10.5, P10.13).
  5. Verify the restored NATS JetStream stream configuration matches the P1.14 design
     (subjects, streams, retention/replication/ack/replay/dead-letter policy) and that
     pending outbox entries and inbox/idempotency records survived the restore: replay
     from the restored state issues no duplicate command (§15.5). Perform the restore
     with scoped JetStream restore authorization only — a restore to an unauthorized
     stream name must be refused (this is the CVE-2026-33222 restore-authorization
     surface; see Phase 1 `RESEARCH.md`).
- **Deliverable / output:** Release-candidate backup/restore test report (steps, measured
  time/loss, verification results).
- **Acceptance criteria / exit check:** (a) a restore was performed on the candidate and
  verified — not just a backup job observed; (b) restored records are complete and
  consistent;   (c) no command is re-issued by replay; (d) measured time/loss meet the
  recorded customer requirements or the shortfall has a disposition; (e) the restored
  stream configuration matches the P1.14 design, outbox/inbox state survived intact, and
  the restore authorization was scoped (unauthorized stream-name restore refused).
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01, P10.18 (release candidate); P9.24 (cross-reference). Blocks P10.27.
- **Verification method:** Inspect the restored database's audit-record count against the
  source; confirm the candidate identifier matches.
- **Risks / failure modes:** The test run against the pilot build instead of the candidate;
  a restore that succeeds with an older schema; replay re-issuing a command that was
  already issued before the backup.

### P10.12 — Upgrade/rollback test on the release candidate

- [ ] **P10.12 Upgrade/rollback test on the release candidate**

- **Objective:** Prove the release can be upgraded to and rolled back from, on the
  candidate — §22.3 gate item 9 (second half).
- **Report trace:** §22.3 (release gate 9); §21.2; §5.6 (plugin lifecycle).
- **Inputs / prerequisites:** P10.01 scope freeze; the deployment model (P10.24 draft);
  the previous release (or the pilot build) as the upgrade source.
- **Steps:**
  1. Define the upgrade path: from the previous release (or pilot build) to the v1.0.0
     candidate, including schema migrations (P10.13) and plugin upgrades (§5.6 drain
     rules).
  2. Perform a rolling upgrade in a staging environment that mirrors the pilot deployment;
     verify health/readiness probes, and that in-flight dispatch transactions are not
     interrupted by a plugin activation/upgrade mid-transaction (§5.6).
  3. Verify the upgraded system: version identifiers, contract compatibility (P10.02),
     command/audit continuity, and a smoke dispatch-lifecycle pass in shadow.
  4. Perform a rollback to the previous version; verify the system returns to a consistent
     state (data, schema compatibility window, plugin versions) and the rollback artifacts
     are retained (§9.3: pin plugin dependencies and retain rollback artifacts).
  5. Verify pending outbox entries and inbox/idempotency records survive the upgrade and
     the rollback without loss or duplicate publication (§15.5): entries pending before
     the upgrade are published exactly once after it, and entries already delivered are
     not re-published.
  6. Record timings, the exact steps, and any manual intervention required.
- **Deliverable / output:** Upgrade/rollback test report (path, steps, timings, verification
  results, retained artifacts).
- **Acceptance criteria / exit check:** (a) the rolling upgrade completed with no
  in-flight dispatch transaction corrupted; (b) the upgraded system passes the smoke
  lifecycle check; (c) rollback returned the system to a consistent, verified state;
  (d) rollback artifacts are retained and identified; (e) pending outbox/inbox state
  survived the upgrade and the rollback with no loss and no duplicate publication.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01, P10.13, P10.18 (release candidate — the upgrade target). Blocks P10.24, P10.27.
- **Verification method:** Inspect the staging upgrade/rollback log; confirm the plugin
  versions before and after match the recorded artifacts.
- **Risks / failure modes:** A migration with no compatibility window, so rollback is
  impossible; a plugin upgraded mid-transaction; a rollback tested only on an empty
  database.

### P10.13 — Schema migration compatibility test

- [ ] **P10.13 Schema migration compatibility test**

- **Objective:** Prove schema migrations are safe across the upgrade/rollback window —
  a §22.2 CI gate item ("migration tests and schema compatibility").
- **Report trace:** §22.2; §10.5; §21.2 (schema migrations with compatibility windows).
- **Inputs / prerequisites:** P10.01 scope freeze; the migration set for v1.0.0; P10.02
  contract freeze.
- **Steps:**
  1. List every migration in the v1.0.0 candidate, with its compatibility window (which
     prior versions can run against the new schema and vice versa).
  2. Test forward migration on a copy of production-shaped data (pilot-shaped, not empty):
     migrate, verify constraints and data integrity, verify the application works.
  3. Test the rollback window: run the prior application version against the migrated
     schema for the declared window, or prove the migration is reversible — record which
     applies per migration.
  4. Run Buf breaking-change detection on any Protobuf schema change (§10.5); record the
     result (P10.02 cross-reference). Record REST/OpenAPI and NATS event-subject changes
     in the same breaking-change record — a migration that renames a subject or drops a
     REST resource is a breaking change even if no Protobuf file changed.
  5. Record any migration that is irreversible, with its justification and the operational
     consequence (e.g. rollback requires restore from backup — which must then have been
     tested in P10.11).
- **Deliverable / output:** Migration compatibility test report (migrations, windows,
  forward/rollback results, irreversible items).
- **Acceptance criteria / exit check:** (a) every migration has a recorded compatibility
  window; (b) forward migration passes on production-shaped data; (c) every irreversible
  migration is listed with its rollback consequence;   (d) no silent breaking Protobuf, REST, or event-subject change.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01, P10.02. Blocks P10.12, P10.18.
- **Verification method:** Re-run one forward migration on a fresh copy and compare the
  resulting schema; inspect the compatibility windows against the deployment model.
- **Risks / failure modes:** A migration tested only on an empty schema; an irreversible
  migration discovered during a real rollback; a Protobuf change that breaks a pilot
  integration without a version bump.

### P10.14 — Browser/OS support-matrix test

- [ ] **P10.14 Browser/OS support-matrix test**

- **Objective:** Prove the supported browser/OS matrix is tested on the release candidate —
  §22.3 gate item 10.
- **Report trace:** §22.3 (release gate 10); §21.4 (build/distribution form).
- **Inputs / prerequisites:** P10.01 scope freeze; the §21.4 support matrix.
- **Steps:**
  1. Record the matrix from §21.4: Operator Console — current and previous major versions
     of Chrome, Edge, Firefox, Safari; Control Plane — Linux x86-64 and ARM64 OCI image
     plus native binary (Linux container preferred; Windows/macOS only for development);
     WASM Plugin Host — Linux x86-64/ARM64 container; Numerical/Research Workers — Linux
     Python containers pinned by lockfile; Site Edge Agent — Linux x86-64/ARM64 container
     or systemd service; CLI/Simulation Lab — CLI on Linux, macOS, Windows; simulators in
     Linux containers.
  2. Test the operator console on each browser in the matrix (current and previous major
     version): load the pilot workflows (§20.1), exercise an approval flow end-to-end, and
     verify the state distinctions (§20.2) render correctly.
  3. Test the control-plane, worker, and edge artifacts on their stated runtimes; verify
     the CLI on Linux, macOS, and Windows.
  4. Record the exact browser/OS versions tested and the result per cell. A cell that was
     not tested is marked untested, not assumed.
  5. Record any matrix cell that fails with its disposition (fix, or remove from the
     supported matrix with a release-notes entry in P10.26).
- **Deliverable / output:** Browser/OS matrix test report (matrix, versions tested,
  per-cell results, dispositions).
- **Acceptance criteria / exit check:** (a) every cell in the §21.4 matrix has a tested
  result or an explicit removal from the supported matrix; (b) the approval flow passes
  on every supported browser; (c) no untested cell is listed as supported.
- **Owner role:** Platform engineering (with frontend engineering for the console cells).
- **Dependencies:** P10.01, P10.18 (release candidate). Blocks P10.21, P10.27.
- **Verification method:** Spot-check one browser cell by re-running the approval flow on
  the recorded version; confirm the artifact hashes tested match the candidate.
- **Risks / failure modes:** Testing only the newest browser version and claiming the
  previous major version; a console rendering failure on one browser that hides a command
  state distinction (§20.2); the CLI tested only on Linux.

### P10.15 — Control-authority qualification — operator-approved mode

- [ ] **P10.15 Control-authority qualification — operator-approved mode**

- **Objective:** Qualify the target control-authority mode — operator-approved control — for
  v1.0.0, on the release candidate — §22.3 gate item 11.
- **Report trace:** §22.3 (release gate 11); §21.4; §14.
- **Inputs / prerequisites:** P10.01 scope freeze; Phase 9 control-scope document (P9.26)
  and shadow/field acceptance evidence (P9.17, P9.19) — cross-referenced; P10.03, P10.04.
- **Steps:**
  1. State the qualification target precisely: operator-approved mode, per §21.4 — each
     command requires an authorized human approval; writes to devices only after approval.
  2. Verify on the candidate that no command can reach ISSUED without an APPROVED state
     with a valid authorization decision reference, approved scope, limits, and expiry
     (§14.3) — test the bypass paths: expired approval, approval for a different scope,
     approval by an unauthorized role, approval after supersede.
  3. Verify mode enforcement: the mode is a deployment/per-site setting (§21.4); confirm
     the candidate enforces the configured mode per site, that mixed modes by site/asset
     class behave as configured, and that no plugin can select or raise the mode (§5.7,
     P10.08 cross-reference).
  4. Verify the human-approval UX on the candidate console: the approver sees the command,
     its validation and feasibility evidence, and its scope/limits before approving (§20.2);
     a proposed setpoint is never displayed as if executed.
  5. Record the qualification result per site/asset class from Phase 9 (P9.26 promotion
     decisions) and confirm the candidate configuration matches it.
  6. Record explicitly that bounded automation is **not** qualified by this subtask and is
     not enabled in the release configuration.
- **Deliverable / output:** Control-authority qualification record for operator-approved mode
  (bypass tests, mode-enforcement checks, UX checks, per-site confirmation).
- **Acceptance criteria / exit check:** (a) every bypass attempt failed — no command reached
  ISSUED without a valid, in-scope, unexpired approval; (b) mode enforcement matches the
  Phase 9 control-scope document per site; (c) the approver UX shows evidence before
  approval; (d) no automation envelope is enabled in the candidate configuration.
- **Owner role:** Security (with product and platform engineering).
- **Dependencies:** P10.01, P10.03, P10.04, P10.08, P10.18 (release candidate); P9.26 (cross-reference). Blocks P10.27.
- **Verification method:** Independent execution of one bypass attempt per class against
  the candidate; inspect the running mode configuration per site.
- **Risks / failure modes:** An approval that never expires, so a stale approval authorises
  a later command; a scope check that compares only the asset ID and not the limits;
  an automation envelope left enabled from a lab profile (§5.8).

### P10.16 — Deployment-specific cyber risk review (consolidation)

- [ ] **P10.16 Deployment-specific cyber risk review (consolidation)**

- **Objective:** Consolidate the deployment-specific cyber risk review for the release —
  §22.3 gate item 12 (cyber half). This consolidates Phase 9 evidence (P9.06, P9.07) against
  the release candidate; it does not redo the pilot security assessment.
- **Report trace:** §22.3 (release gate 12); §17; §25.2 (D6).
- **Inputs / prerequisites:** P10.01 scope freeze; P9.06 D6 closure record; P9.07 security
  assessment and findings list; P10.18 candidate build (for the re-check); the NIST
  SP 800-82 Rev. 4 status (see `RESEARCH.md` — re-check whether the final has published).
- **Steps:**
  1. Confirm the P9.07 assessment still covers the candidate: re-check each §17.4 threat
     control against the candidate configuration (not the pilot build) and record any drift.
  2. Confirm every must-fix finding from P9.07 is closed with evidence, or has a signed,
     time-bounded acceptance carried into P10.26/P10.27.
  3. Re-check the NIST SP 800-82 Rev. 4 status: the comment period closes 30 November 2026,
     so this check cannot return a "final" answer before that date. If a final Rev. 4 has
     published, record its deltas against the Rev. 3 baseline used in
     P9.07 and assess whether any delta changes a control in this release. If it has not
     published — including when this review runs before 30 November 2026 — record that
     Rev. 3 remains the baseline, record the draft status, and record a dated re-check
     (a go decision before 30 November 2026 must carry that re-check as a P10.27
     condition, not assume the final matches the draft).
  4. Confirm the D6 closure (P9.06) covers the deployment the release ships to; if the
     release adds a site, network path, or adapter beyond the D6 scope, record that as a
     gap that blocks the go decision for that scope.
  5. Produce the consolidated cyber risk statement for the release: threats, controls,
     residual risks, and their owners.
- **Deliverable / output:** Consolidated deployment cyber risk review (drift check, findings
  closure, Rev. 4 status note, residual-risk statement).
- **Acceptance criteria / exit check:** (a) every §17.4 threat control re-checked against
  the candidate; (b) no must-fix finding open without a signed, time-bounded acceptance;
  (c) the Rev. 4 status is recorded as checked on the review date (final or still draft);
  (d) the D6 scope covers the release scope, or the gap is recorded as a go blocker.
- **Owner role:** Security.
- **Dependencies:** P10.01; P9.06, P9.07 (cross-references); P10.18 (candidate). Blocks
  P10.25, P10.27.
- **Verification method:** Spot-check two threat controls against the candidate
  configuration; confirm the Rev. 4 status against the NIST CSRC page on the review date.
- **Risks / failure modes:** The review consolidating the pilot build's evidence while the
  candidate drifted (new dependency, changed default); Rev. 4 final text assumed to match
  the draft; a new site added to the release after D6 closed.

### P10.17 — Deployment-specific operational risk review

- [ ] **P10.17 Deployment-specific operational risk review**

- **Objective:** Complete the deployment-specific operational risk review for the release —
  §22.3 gate item 12 (operational half).
- **Report trace:** §22.3 (release gate 12); §17.4; §21.2; §14.4.
- **Inputs / prerequisites:** P10.01 scope freeze; P9.20 fallback drills; P9.23 incident
  procedure; P9.24 backup rehearsal; P9.25 observability verification; P9.27 risk register
  — all cross-referenced.
- **Steps:**
  1. Consolidate the operational risks from Phase 9: fallback adequacy per asset/site
     (P9.20), incident-response readiness (P9.23), restore capability (P9.24), and
     observability (P9.25).
  2. Assess the operational risks specific to the release: upgrade/rollback failure
     (P10.12), migration failure (P10.13), a bad release reaching the pilot (mitigated by
     P10.18/P10.27), operator error under the new console (P10.23), and a central outage
     during an active dispatch (§17.4, §21.2).
  3. For each risk, record impact, assessed likelihood with its basis, owner, mitigation
     or monitoring, and review date. Do not record a likelihood without a basis.
  4. Confirm the P9.27 residual-risk register is carried forward complete into P10.26
     (release notes) — no item dropped.
  5. Record the operational risk statement for the release and its go/no-go input to P10.27.
- **Deliverable / output:** Deployment operational risk review (consolidated risks,
  owners, review dates, go/no-go input).
- **Acceptance criteria / exit check:** (a) every Phase 9 operational risk is carried
  forward or closed with evidence; (b) every release-specific operational risk has an
  owner and a review date; (c) the P9.27 register matches the release-notes limitations
  (P10.26) item for item; (d) no risk is recorded without a likelihood basis.
- **Owner role:** Product / operations (with security and platform engineering).
- **Dependencies:** P10.01, P10.18 (release candidate); P9.20, P9.23–P9.25, P9.27 (cross-references); P10.11 (candidate restore result), P10.12, P10.23.
  Blocks P10.25, P10.27.
- **Verification method:** Cross-check the review against the P9.27 register item by item;
  confirm each new risk has an owner.
- **Risks / failure modes:** A Phase 9 risk dropped because "the release fixes it" without
  evidence; an operational risk with no owner; the central-outage-during-dispatch case
  never assessed because it is "unlikely".

### P10.18 — Full CI gate run and release-candidate build

- [ ] **P10.18 Full CI gate run and release-candidate build**

- **Objective:** Run every §22.2 CI quality gate against the frozen candidate and produce
  the release-candidate build the other subtasks verify.
- **Report trace:** §22.2.
- **Inputs / prerequisites:** P10.01 scope freeze; P10.02 contract freeze; P10.13 migration
  tests.
- **Steps:**
  1. Run the full §22.2 gate list on the candidate commit: `cargo fmt --check`; `cargo
     clippy --workspace --all-targets -- -D warnings` (allowlist only by explicit review);
     `cargo test --workspace`; Python Ruff + type checks + pytest; frontend lint/type-check/
     test/build; Buf lint and breaking-change detection; plugin manifest and WIT
     compatibility validation; migration tests and schema compatibility; container/
     dependency scanning; SBOM generation; license/security review; scenario regression suite.
  2. Record the result of every gate with its log/output location. A gate that is skipped
     is recorded as skipped with its reason — a skipped gate is not a passed gate.
  3. Build the release candidate from the gated commit and record its identifier (commit,
     tag, build ID, artifact hashes). This identifier is the one P10.03–P10.17 verify
     against; if the candidate changes, the affected verifications re-run.
  4. Run the repository verification commands (`python tools/verify_docs.py`,
     `python tools/validate_agent.py`) and record their output (AGENTS.md §6).
- **Deliverable / output:** CI gate report (per-gate result + logs) + release-candidate
  identifier and artifact hashes.
- **Acceptance criteria / exit check:** (a) every §22.2 gate ran and passed, or a failed/
  skipped gate has a recorded disposition approved for the release; (b) the candidate
  identifier is recorded and matches the artifacts verified in P10.03–P10.17; (c) the
  repository verification commands pass.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.01, P10.02, P10.13. Blocks P10.03–P10.17 (candidate identity),
  P10.21, P10.27.
- **Verification method:** Re-run the CI gates from a clean checkout of the candidate commit
  and compare the results to the filed report.
- **Risks / failure modes:** A gate passing on a different commit than the candidate; a
  clippy allowlist added without explicit review; the scenario regression suite skipped as
  "slow" and recorded as passed; the §22.2 "license/security review" CI gate being read as
  the D4 closure — it is only the automated scan. The legal D4 determination is P10.19;
  P10.18 must not be recorded as D4 closed.

### P10.19 — SBOM and license audit closure — D4

- [ ] **P10.19 SBOM and license audit closure — D4**

- **Objective:** Close decision D4 for the v1.0.0 release: audit the license compatibility of
  the release's protocol stacks, solvers, models, and datasets, and generate the release SBOM.
- **Report trace:** §22.2 (SBOM generation; license/security review); §22.3; §25.2 (D4);
  §9.3 (SBOMs for releases and plugin packages).
- **Inputs / prerequisites:** P10.01 scope freeze; P10.18 candidate; the Phase 9 handover
  note on any new pilot dependency (P9.30); the tooling background in `RESEARCH.md`.
- **Steps:**
  1. Generate an SBOM for the release candidate and for each plugin package (§9.3) in a
     machine-readable format (SPDX or CycloneDX — see `RESEARCH.md` for the format
     versions to verify at time of use), from the lockfiles, not from a hand-written list.
  2. Audit every dependency's license for Apache-2.0 compatibility (the recommended core
     license, §25.1): protocol stacks, solvers, models, datasets, and third-party
     dependencies (§25.2 D4). Use a license scanner (e.g. ORT; see `RESEARCH.md`) and
     record the tool and version actually used.
  3. For each dependency whose license is incompatible, unclear, or requires an obligation
     (notice, source disclosure, copyleft), record the finding with its disposition:
     replace, isolate, comply with the obligation, or escalate to legal. No incompatible
     dependency ships silently.
  4. Verify the SBOM is complete against the lockfiles (spot-check: every direct
     dependency appears; a sample of transitive dependencies appears).
  5. Record the D4 closure: the audit result, the SBOM locations, the dispositions, and
     the legal review. If D4 cannot close (e.g. an incompatible required dependency), the
     release does not go (P10.27).
  6. Confirm no third-party agent skill, document, or code was copied into the repository
     without a license check (`.agent/PROVENANCE.md` rule; AGENTS.md §11).
- **Deliverable / output:** Release SBOMs + license audit report + D4 closure record.
- **Acceptance criteria / exit check:** (a) SBOMs exist for the release and each plugin
  package, generated from lockfiles; (b) every dependency has a license determination;
  (c) every incompatible/unclear license has a disposition; (d) legal has reviewed the
  audit; (e) the D4 closure record is signed.
- **Owner role:** Legal + engineering (D4 owners per `.agent/DECISIONS.md`).
- **Dependencies:** P10.01, P10.18. Blocks P10.21, P10.27.
- **Verification method:** Independent regeneration of the SBOM from the lockfiles and
  comparison; confirm the scanner version recorded matches the tool run.
- **Risks / failure modes:** An SBOM generated from a manifest instead of the lockfile,
  missing transitive dependencies; a GPL/AGPL dependency discovered after the release
  shipped; a dataset or model with an unexamined license (D4 explicitly covers models and
  datasets, not just code).

### P10.20 — Vulnerability and dependency scan closure

- [ ] **P10.20 Vulnerability and dependency scan closure**

- **Objective:** Close vulnerability and dependency scanning for the release candidate: every
  finding triaged, every critical/high finding fixed or formally accepted.
- **Report trace:** §22.2 (container/dependency scanning); §17.3 (vulnerability management
  and dependency scanning).
- **Inputs / prerequisites:** P10.18 candidate; P10.19 SBOMs (scan the SBOM, not just the
  source tree).
- **Steps:**
  1. Run container scanning and dependency vulnerability scanning against the candidate
     artifacts and the SBOMs (tools per `RESEARCH.md` — e.g. Grype, Trivy, OSV-Scanner;
     record the tool and version actually used).
  2. Triage every finding: severity, affected component, exploitability in this deployment
     (a vulnerability in an unused code path is still recorded, with its reasoning).
  3. Fix or formally accept each critical/high finding; an acceptance is signed, time-
     bounded, and carried into P10.26/P10.27. No critical/high finding is left untriaged.
  4. Re-scan after fixes and record the final scan result against the candidate identifier.
  5. Record the vulnerability-management process for the pilot period: how new disclosures
     are received, triaged, and patched after release.
- **Deliverable / output:** Vulnerability scan report (final scan, triage, dispositions) +
  post-release vulnerability process note.
- **Acceptance criteria / exit check:** (a) the final scan ran against the candidate
  artifacts and SBOMs; (b) every critical/high finding is fixed or formally accepted
  (signed, time-bounded); (c) no finding is untriaged; (d) the post-release process is
  written.
- **Owner role:** Security (with platform engineering).
- **Dependencies:** P10.18, P10.19. Blocks P10.27.
- **Verification method:** Re-run the scanner against the candidate SBOM and compare the
  finding list to the filed triage.
- **Risks / failure modes:** Scanning the source tree but not the shipped container image;
  a finding accepted without a time bound; a scanner version recorded that differs from
  the one run.

### P10.21 — Signed artifacts, checksums, and support matrix

- [ ] **P10.21 Signed artifacts, checksums, and support matrix**

- **Objective:** Publish the release artifacts signed, checksummed, and with their support
  matrix — so a pilot site can verify what it installs.
- **Report trace:** §21.4 (every release publishes checksums, an SBOM, and a support matrix);
  §9.3 (signatures/provenance for production plugin installation); §18.3 (signed and
  verified update bundles).
- **Inputs / prerequisites:** P10.18 candidate; P10.19 SBOMs; P10.14 matrix test results.
- **Steps:**
  1. Produce the release artifacts per §21.4: Operator Console static bundle; Control
     Plane Linux x86-64/ARM64 OCI image plus native binary; WASM Plugin Host container;
     Numerical/Research Worker container images; Site Edge Agent package/OCI image with
     signed updates; CLI/Simulation Lab release archives and OCI images; Helm chart
     (from v1.0.0, §21.4).
  2. Sign the artifacts and publish checksums for every artifact (§21.4); record the
     signing identity and the verification procedure.
  3. Publish the support matrix: artifact ↔ supported browsers/OSs (from P10.14), contract
     versions (from P10.02), and the SBOM locations (from P10.19).
  4. Verify the published artifacts: download/verify checksums and signatures from the
     publication location (not from the build directory) and confirm the edge update
     bundle verifies on a test edge host (§18.3).
  5. Record the distribution channels (§21.4) and the support commitment for the release
     (what "supported" means and for how long — or record that it is undecided).
- **Deliverable / output:** Published release artifacts + checksums + signatures + support
  matrix + publication-verification record.
- **Acceptance criteria / exit check:** (a) every §21.4 artifact exists, is signed, and has
  a published checksum; (b) checksums/signatures verify from the publication location;
  (c) the support matrix matches the P10.14 tested cells; (d) the Helm chart is published
  (v1.0.0 requirement, §21.4).
- **Owner role:** Platform engineering.
- **Dependencies:** P10.14, P10.18, P10.19. Blocks P10.28.
- **Verification method:** Independent download and verification of one artifact per class
  from the publication location.
- **Risks / failure modes:** An artifact published unsigned; a checksum published for a
  different build than the candidate; a support matrix that lists an untested browser/OS
  cell as supported.

### P10.22 — Site integration test harness — packaged and runnable

- [ ] **P10.22 Site integration test harness — packaged and runnable**

- **Objective:** Package the site integration test harness as a shippable part of v1.0.0
  (§25 pilot release), runnable by a site integrator — not just by its authors.
- **Report trace:** §25 (pilot release: "site integration test harness"); §23 Stage 9.
- **Inputs / prerequisites:** P10.01 scope freeze; the Phase 9 lab/field test procedures
  (P9.14, P9.16–P9.17) as the harness's test content; P10.18 candidate.
- **Steps:**
  1. Define the harness contents: the lab integration checks (P9.14), the field acceptance
     cases (P9.16), the telemetry/clock checks (P9.18), the command-lifecycle dry run
     (P9.19), and the fallback drills (P9.20) — as runnable checks with pass/fail output,
     not as documents.
  2. Package the harness with the release (versioned with the candidate), with a README
     that states its prerequisites, how to run it, and how to read its output.
  3. Test the harness end-to-end in a clean environment by a person who did not write it,
     against a staging deployment: record the run, the result per check, and any check
     that could not run.
  4. Verify the harness refuses to run (or clearly warns) when pointed at a deployment
     whose versions do not match the candidate, so a pass is never recorded against the
     wrong build.
  5. Record the harness version and its location in the release notes (P10.26).
- **Deliverable / output:** Packaged site integration test harness (versioned) + clean-
  environment run record.
- **Acceptance criteria / exit check:** (a) the harness runs end-to-end in a clean
  environment by a non-author; (b) every check produces a pass/fail, not a silent skip;
  (c) the harness detects a version mismatch; (d) the harness is versioned with the release.
- **Owner role:** Integration engineering (with platform engineering).
- **Dependencies:** P10.01, P10.18; P9.14, P9.16–P9.20 (content cross-references). Blocks
  P10.26, P10.28.
- **Verification method:** Independent clean-environment run by a second person; confirm
  the version-mismatch detection with a deliberately mismatched deployment.
- **Risks / failure modes:** A harness that only its author can run; a check that silently
  skips when its prerequisite is missing; a harness version that drifts from the release.

### P10.23 — Operator-workflow stability check

- [ ] **P10.23 Operator-workflow stability check**

- **Objective:** Prove the operator workflow is stable for the pilot release (§25: "stable
  operator workflow") on the candidate console.
- **Report trace:** §20; §25 (pilot release).
- **Inputs / prerequisites:** P10.01 scope freeze; P10.14 browser tests; Phase 9 training
  evidence (P9.21) and procedures (P9.22) — cross-referenced.
- **Steps:**
  1. Walk the full operator workflow on the candidate console (§20.1 workspaces): Overview,
     Portfolios, Sites, Assets, Grid, Schedules, Dispatch, Market & Programs, Simulation
     Lab, Plugins & Integrations, Audit & Reports, Administration.
  2. Exercise the dispatch workflow end-to-end in shadow: propose → validate → feasibility
     → approve → hold before issue, with the evidence shown at each step (§20.2).
  3. Verify the §20.2 design principles on the candidate: operational state and freshness
     prominent; measured/estimated/stale/simulated visually distinct; proposed never shown
     as executed; candidate/feasible/approved/issued/acknowledged/measured visually
     distinct; rejected commands explained with the failing constraint and evidence; model
     version and timestamp shown for feasibility outcomes; explicit confirmation for
     high-impact actions; emergency actions visually separate from normal scheduling.
  4. Verify role-specific screens: a role sees only its relevant controls (§20.2).
  5. Record any workflow defect with its disposition; a defect that hides a command state
     or an approval blocks the release.
- **Deliverable / output:** Operator-workflow stability report (walkthrough, per-principle
  checks, defects and dispositions).
- **Acceptance criteria / exit check:** (a) every §20.1 workspace loads and functions on
  the candidate; (b) every §20.2 principle is verified with evidence; (c) no defect that
  hides a command state, an approval, or a rejection reason is open.
- **Owner role:** Product (with frontend engineering and the customer operator lead).
- **Dependencies:** P10.01, P10.14, P10.18 (release candidate console build); P9.21, P9.22 (cross-references). Blocks P10.17, P10.27.
- **Verification method:** Independent walkthrough by a second operator (ideally the
  customer operator) on the candidate console.
- **Risks / failure modes:** A console regression that only appears on the previous major
  browser version (P10.14 cross-check); a rejected command shown without its reason, so
  the operator cannot act on it; an emergency action placed where it can be hit by mistake.

### P10.24 — Deployment and upgrade model documentation

- [ ] **P10.24 Deployment and upgrade model documentation**

- **Objective:** Document the deployment and upgrade model for v1.0.0 (§25 pilot release)
  so a customer can deploy, operate, and upgrade it without Neryva in the room.
- **Report trace:** §21.1–§21.2; §25.
- **Inputs / prerequisites:** P10.01 scope freeze; P10.11–P10.13 test results; P10.21
  artifacts.
- **Steps:**
  1. Document the deployment modes supported by the release (§21.1: local developer,
     public demo/lab, customer private cloud, customer on-premises, hybrid) and which mode
     the pilot uses.
  2. Document the deployment procedure per mode: components, Helm chart usage (from
     v1.0.0), database/broker setup, edge enrolment, and the health/readiness probes.
  3. Document the upgrade model validated in P10.12: rolling upgrades, rollback, schema
     migration compatibility windows (P10.13), and plugin upgrade rules (§5.6).
  4. Document the degraded modes (§21.2) and the rule that a central outage must not
     disable site-local protective controls or force an unsafe device state.
  5. Document backup/restore and disaster recovery as validated in P10.11, with the
     measured timings.
  6. Have the document reviewed by a person who did not write it, by following it to deploy
     the candidate in a clean environment (or record why that was not possible and what
     was done instead).
- **Deliverable / output:** v1.0.0 deployment and upgrade guide (modes, procedures, upgrade/
  rollback, degraded modes, backup/restore).
- **Acceptance criteria / exit check:** (a) every §21.1 mode the release claims is
  documented; (b) the upgrade/rollback procedure matches the P10.12 tested path;
  (c) degraded modes and the central-outage rule are documented; (d) the guide was
  review-deployed or the exception is recorded.
- **Owner role:** Platform engineering.
- **Dependencies:** P10.11, P10.12, P10.13, P10.21. Blocks P10.29.
- **Verification method:** Follow-the-guide deployment by a second person; compare the
  resulting deployment against the P10.12 staging result.
- **Risks / failure modes:** Documentation describing a deployment path that was never
  tested; a Helm chart documented but not published (P10.21 cross-check); an upgrade
  procedure that omits the plugin drain rule.

### P10.25 — Security review consolidation report

- [ ] **P10.25 Security review consolidation report**

- **Objective:** Produce the single security review for v1.0.0 (§25 pilot release:
  "security review") from the consolidated cyber and operational reviews.
- **Report trace:** §17; §25.
- **Inputs / prerequisites:** P10.16 cyber review; P10.17 operational risk review; P10.04,
  P10.08, P10.20 test reports.
- **Steps:**
  1. Assemble the review: the §17.3 control status for the candidate, the §17.4 threat
     coverage (from P10.16), the authorization/tenant-isolation results (P10.04), the
     plugin-boundary results (P10.08), and the vulnerability-scan closure (P10.20).
  2. Record the security review's scope and limits: what was reviewed, what was not (e.g.
     a customer-specific control that belongs to the customer, not to FlexCore), and the
     review date.
  3. List the residual security risks with owners and review dates (from P10.16/P10.17),
     and confirm each appears in the release notes (P10.26).
  4. State plainly that the review is not a certification, not a safety case, and not
     permission for live grid control.
  5. Have the review signed by the security owner.
- **Deliverable / output:** v1.0.0 security review report (signed).
- **Acceptance criteria / exit check:** (a) every §17.3 control has a status for the
  candidate; (b) every residual risk has an owner and a review date; (c) the scope/limits
  and the not-a-certification statement are present; (d) the review is signed.
- **Owner role:** Security.
- **Dependencies:** P10.04, P10.08, P10.16, P10.17, P10.20. Blocks P10.26, P10.27.
- **Verification method:** Cross-check the review's control statuses against the source
  subtask reports; confirm the residual-risk list matches P10.16/P10.17.
- **Risks / failure modes:** A review that describes the product design instead of the
  candidate; a residual risk omitted from the release notes; the review's limits left
  unstated so a reader over-reads it.

### P10.26 — Release notes and known limitations

- [ ] **P10.26 Release notes and known limitations**

- **Objective:** Write the v1.0.0 release notes with the known limitations stated plainly —
  including every limitation from the Phase 9 register.
- **Report trace:** §2.1 (v1.0.0 definition); §25.
- **Inputs / prerequisites:** P10.01 scope; P10.10 benchmark pack; P10.22 harness;
  P9.27 limitations/risks register (must be carried forward complete); P10.16–P10.17,
  P10.25.
- **Steps:**
  1. Write the release notes: what v1.0.0 contains, the supported adapters (P10.01), the
     frozen contracts (P10.02), the benchmark summary (P10.10), the deployment/upgrade
     model (P10.24), and the support matrix (P10.21).
  2. Write the known-limitations section from the P9.27 register, item for item — plus any
     new limitation found in Phase 10 (benchmark limits from P10.10, browser/OS cells
     removed in P10.14, accepted vulnerabilities from P10.20). No item is dropped or
     softened.
  3. State the §0.3 non-promises that are relevant to the release (no hard-real-time or
     protection-grade control; no primary frequency response or synthetic inertia; no
     universal market qualification or FERC Order 2222 compliance; no certified protocol
     conformance from adapter implementation alone; no safe field dispatch without
     site-specific acceptance).
  4. State the control-authority position: operator-approved control by default; bounded
     automation is post-1.0 and not enabled.
  5. Have the notes reviewed by product and security for accuracy — every factual claim
     traced to its source subtask.
- **Deliverable / output:** v1.0.0 release notes (including known limitations).
- **Acceptance criteria / exit check:** (a) every P9.27 register item appears in the known-
  limitations section or is closed with evidence; (b) every factual claim traces to a
  source subtask; (c) the §0.3 non-promises and the bounded-automation position are
  stated; (d) the notes are reviewed and signed off by product and security.
- **Owner role:** Product (with security).
- **Dependencies:** P10.01, P10.10, P10.14 (browser/OS cells removed), P10.15 (control-authority position), P10.16, P10.17, P10.19 (license/SBOM limits), P10.20 (accepted vulnerabilities), P10.22, P10.25; P9.27 (cross-reference).
  Blocks P10.27, P10.28.
- **Verification method:** Item-by-item cross-check of the known-limitations section against
  the P9.27 register and the Phase 10 findings; confirm no claim lacks a source.
- **Risks / failure modes:** A limitation softened into marketing language; a P9.27 item
  dropped; a benchmark claim in the notes that the P10.10 pack does not support.

### P10.27 — Go/no-go decision record

- [ ] **P10.27 Go/no-go decision record**

- **Objective:** Record the formal go/no-go decision for the v1.0.0 release, on the evidence —
  not on schedule pressure.
- **Report trace:** §22.3.
- **Inputs / prerequisites:** P10.01–P10.26 complete (or explicitly waived — see below);
  the Phase 9 exit-gate record (P9.30).
- **Steps:**
  1. Assemble the decision pack: the §22.3 gate evidence (P10.01–P10.17), the CI report
     (P10.18), D4 closure (P10.19), vulnerability closure (P10.20), signed artifacts and
     support matrix (P10.21), site integration test harness (P10.22), operator-workflow
     stability check (P10.23), deployment/upgrade guide (P10.24), security review
     (P10.25), release notes (P10.26), and the Phase 9 exit-gate record (P9.30).
  2. Hold the decision meeting with product, engineering, security, power systems, and
     legal. Record attendees, the evidence presented, and the questions raised with their
     answers.
  3. Record the decision: **go** / **go with conditions** (conditions listed, with owners
     and dates) / **no-go** (reasons listed, with the subtasks that re-open).
  4. Any waiver of an incomplete subtask must be explicit, signed, reasoned, and
     time-bounded — a silent gap is not a waiver. A waived §22.3 gate item must name the
     compensating control and its expiry.
  5. If the decision is no-go, record the release as not released and the re-entry
     conditions. Do not publish the release (P10.28) on a no-go.
- **Deliverable / output:** Go/no-go decision record (pack contents, attendees, decision,
  conditions, waivers).
- **Acceptance criteria / exit check:** (a) the meeting was held with all five functions
  present (or an absence recorded with its reason); (b) every §22.3 gate item has a
  recorded status; (c) every condition/waiver has an owner and a date; (d) the decision
  is recorded and signed.
- **Owner role:** Product (chair); the decision is the meeting's, not one person's.
- **Dependencies:** P10.01–P10.26; P9.30. Blocks P10.28.
- **Verification method:** Inspect the signed record against the decision pack; confirm no
  §22.3 gate item is unrecorded.
- **Risks / failure modes:** A go decision on schedule pressure with an open must-fix
  finding; a waiver with no expiry that becomes permanent; a decision recorded by one
  person without the meeting.

### P10.28 — v1.0.0 tagging and release publication

- [ ] **P10.28 v1.0.0 tagging and release publication**

- **Objective:** Tag and publish v1.0.0 — only on a go decision (P10.27).
- **Report trace:** §21.4; §22.3.
- **Inputs / prerequisites:** P10.27 go decision; P10.21 published artifacts; P10.26
  release notes.
- **Steps:**
  1. Confirm the P10.27 decision is go (or go with conditions, with the conditions met or
     explicitly carried as post-release actions with owners and dates).
  2. Tag the release-candidate commit as v1.0.0 (Semantic Versioning) and record the tag,
     the commit, and the artifact hashes.
  3. Publish the release: artifacts (P10.21), SBOMs (P10.19), release notes (P10.26),
     deployment guide (P10.24), and the test harness (P10.22) through the recorded
     distribution channels.
  4. Verify the publication from the outside: download the artifacts from the publication
     location, verify checksums/signatures, and confirm the release notes match the tag.
  5. Announce the release through the agreed channel, with the known limitations visible —
     not buried.
- **Deliverable / output:** v1.0.0 tag + publication record + external verification record.
- **Acceptance criteria / exit check:** (a) the P10.27 decision is go; (b) the tag matches
  the release-candidate commit and artifact hashes; (c) every published artifact verifies
  from the publication location; (d) the release notes (with limitations) are published
  with the release.
- **Owner role:** Platform engineering (with product).
- **Dependencies:** P10.21, P10.26, P10.27; P10.22, P10.24. Blocks P10.29.
- **Verification method:** Independent download-and-verify of the published artifacts
  against the recorded hashes.
- **Risks / failure modes:** Publishing on a no-go; a tag pointing at a different commit
  than the tested candidate; the release notes published without the limitations section.

### P10.29 — Pilot handover pack (operations runbook)

- [ ] **P10.29 Pilot handover pack (operations runbook)**

- **Objective:** Hand the pilot operators a complete operations pack for the released version,
  so the pilot can run without the release team in the room.
- **Report trace:** §21.2–§21.3; §23 Stage 9.
- **Inputs / prerequisites:** P10.24 deployment guide; P10.28 publication; Phase 9
  procedures (P9.22, P9.23) and qualification register (P9.21) — updated to the released
  version.
- **Steps:**
  1. Assemble the handover pack: deployment guide (P10.24), release notes (P10.26),
     operations manual (P9.22, updated to v1.0.0), incident-response procedure (P9.23),
     backup/restore and DR procedure (P10.11), alert catalogue (P9.25), control-scope
     document (P9.26), on-call rota, and escalation contacts.
  2. Verify every procedure in the pack refers to the released version (not the pilot
     build) and the frozen grid-model/adapter versions.
  3. Walk the pack with the pilot operators and the on-call rota; record the walkthrough
     and correct the pack where the walkthrough diverges.
  4. Confirm the operators who will run the release are qualified on it (P9.21 register
     checked against the released version; re-qualify where the release changed the
     workflow).
  5. Record the handover: who received the pack, when, and the open items (conditions
     from P10.27) they inherit.
- **Deliverable / output:** Pilot handover pack (versioned, v1.0.0) + walkthrough record +
  handover receipt.
- **Acceptance criteria / exit check:** (a) every document in the pack refers to v1.0.0;
  (b) the walkthrough happened with the pilot operators; (c) every operator running the
  release is qualified on it; (d) the handover receipt is signed.
- **Owner role:** Product / operations (with platform engineering).
- **Dependencies:** P10.24, P10.28; P9.21–P9.23, P9.25, P9.26 (cross-references). Blocks
  P10.30.
- **Verification method:** Inspect the pack for version references; confirm the walkthrough
  record and the qualification register.
- **Risks / failure modes:** A runbook that describes the pilot build, not the release;
  an operator qualified on a superseded workflow; an on-call rota that is out of date on
  day one.

### P10.30 — Post-release monitoring and bounded-automation deferral record

- [ ] **P10.30 Post-release monitoring and bounded-automation deferral record**

- **Objective:** Set up post-release monitoring for the pilot, and record — in writing —
  that bounded automation is deferred, so no one enables it by drift.
- **Report trace:** §21.3; §21.4; §25.
- **Inputs / prerequisites:** P10.28 publication; P10.29 handover; P9.25 alert catalogue.
- **Steps:**
  1. Confirm post-release monitoring is live for the pilot: the §21.3 correlation fields,
     the measured signals (ingestion delay, stale telemetry, queue depth, plugin health,
     solver duration/status, command delivery, acknowledgement, tracking deviation), and
     the alert catalogue (P9.25) with named recipients.
  2. Define the post-release review cadence: what is reviewed, by whom, and how often
     (first review date recorded), including the review of the P10.27 conditions and the
     P10.26 limitations.
  3. Record the bounded-automation deferral: bounded automation is post-1.0, per
     deployment, site, and asset class, after separate qualification (§21.4). Record that
     no automation envelope exists in the v1.0.0 configuration, who would have to approve
     any future automation (per deployment/site/asset class), and what qualification that
     approval would require.
  4. Record the known-issue and incident intake for the pilot period: where issues are
     filed, who triages them, and the link to the vulnerability process (P10.20).
  5. Close the phase: verify the `CHECKLIST.md` in this directory is complete with
     evidence locations.
- **Deliverable / output:** Post-release monitoring confirmation + review cadence +
  bounded-automation deferral record + Phase 10 exit-gate record.
- **Acceptance criteria / exit check:** (a) monitoring and alerts are confirmed live on
  the released version; (b) the first post-release review date is set; (c) the deferral
  record states the approval route and qualification required for any future automation;
  (d) the `CHECKLIST.md` is complete.
- **Owner role:** Product / operations (with platform engineering).
- **Dependencies:** P10.28, P10.29.
- **Verification method:** Inspect a live trace/record from the released deployment for
  the §21.3 correlation fields; confirm the deferral record is filed with the release notes.
- **Risks / failure modes:** Monitoring configured for the staging deployment, not the
  pilot; an automation envelope enabled later without the recorded approval route;
  P10.27 conditions never reviewed because no review date was set.

---

## Traceability note

Every subtask above traces to §22.3 (release gates), §22.2 (CI gates), §21 (deployment),
§19.3–§19.4 (benchmarks), or §25 (pilot release). Where the report is silent on a release
detail (for example, the exact SBOM tool — the report requires SBOMs, §9.3/§22.2, but does
not name a tool), this ledger requires the tool actually used to be recorded with its
version rather than assuming a default. Where a subtask consolidates Phase 9 evidence, the
cross-reference names the Phase 9 subtask; the Phase 10 subtask re-verifies that evidence
against the release candidate and records drift — it does not repeat the pilot qualification.
