# Phase 10 — v1.0.0 Pilot-Ready Release Hardening / Closure: Context

**Phase:** 10 (release gate — not a report stage)
**Ledger:** `ledger_phase_10.md` (P10.01–P10.30)

## 1. Background

Phase 9 qualified a customer deployment at named sites. Phase 10 turns that qualification —
plus the product's own gates — into a releasable v1.0.0. The report defines the pilot release
as "stable operator workflow, reproducible benchmark, deployment/upgrade model, security
review, and site integration test harness; operator-approved control by default" (§25), and
§22.3 lists twelve release gates a release cannot be labelled pilot-ready without. Phase 10
is the work of meeting those gates against one frozen build, the release candidate, and
recording the go/no-go decision on the evidence.

**Phase 10 overlaps Stage 9 but is the release gate, not duplicated work** — it closes and
consolidates Stage 9 evidence into a releasable v1.0.0, cross-referencing Phase 9 subtasks
rather than redoing them. The distinction matters in both directions: Phase 10 must not
re-run the pilot qualification (HIL, field acceptance, customer review), and Phase 9 evidence
must not be presented as release evidence without re-verification against the candidate.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §22.3 | The twelve release gates — the phase's backbone; each gate maps to named subtasks (see the ledger's subtask index) |
| §22.2 | CI quality gates — every gate runs in P10.18 |
| §22.1 | ADRs (ADR-001…ADR-012) — the release must not contradict them; a contradiction is a finding, not a footnote |
| §25, §2.1 | Pilot release definition; v1.0.0 capability and control status |
| §21.1–§21.4 | Hosting modes, availability/recovery, observability, build form, distribution, control-authority modes; the browser/OS matrix lives in §21.4 |
| §19.3–§19.4 | Benchmark cases and evaluation metrics; thresholds set after defining model/service/benchmark/risk, never as arbitrary universals |
| §14.1–§14.3 | Command lifecycle — the full transition matrix tested in P10.03 |
| §12.3, §12.5–§12.6 | Optimization tiers, feasibility gate, result contract — P10.05 |
| §10.2–§10.5 | Units/sign, time model, telemetry quality, schema governance (Buf) |
| §16.2 | Tenancy and RLS — P10.04 |
| §17.1–§17.5 | Security model, zones, controls, threat model, safety invariants — P10.16, P10.25 |
| §5.4–§5.7, §9.3 | Plugin execution classes, authority model, SBOM/signatures — P10.08, P10.19 |
| §20.1–§20.2 | Operator console — P10.23 |
| §25.2 | Deferred decisions; D4 final closure for the v1.0.0 artifact set here (P10.19) — first gated before v0.1.0 |
| §24 | Vertical slice — lands here as the reproducibility benchmark (see `PLAN.md` §10) |
| §0.3 | What is deliberately not promised — repeated in the release notes (P10.26) |

## 3. Canonical units, sign, and terminology

- **Units (§10.2):** MW, MVAr, MVA, MWh. `P > 0` = export at the point of connection.
  Reactive-power sign declared per grid model and device profile. SOC as a fraction in
  [0,1] at the API boundary.
- **Telemetry quality (§10.4):** exactly one of `GOOD`, `SUSPECT`, `BAD`, `MISSING`,
  `ESTIMATED`, `STALE` per value.
- **Command lifecycle (§14.1–§14.2):** `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`, `EXPIRED`,
  `CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility, and authorization are
  distinct gates. `SUPERSEDED` and `CANCELLED` are different states.
- **Control authority (§21.4):** Simulation, Read-only/shadow, Operator-approved, Bounded
  automation. Operator-approved is the v1.0.0 default.
- **Terminology (`.agent/WRITING.md`):** "license" (US spelling); "acknowledgement" (noun);
  "operator-approved control" (not "manual control" or "human-in-the-loop"); "read-only /
  shadow mode" (not "passive mode"); "dispatch command" (not "setpoint" for the lifecycle
  object). "Certified" and "conformant" require a test result.
- **Release terms:** "release candidate" = the frozen build under verification (P10.18);
  "go with conditions" = release proceeds only with the listed conditions owned and dated
  (P10.27); "waiver" = an explicit, signed, reasoned, time-bounded exception — never silent.

## 4. Constraints

- **Operator-approved control only at v1.0.0.** Every command requires an authorized human
  approval (§21.4). P10.15 qualifies this mode; nothing in this phase qualifies any other
  mode.
- **Bounded automation is post-1.0**, per deployment, site, and asset class, after separate
  qualification (§21.4, §25). It is not built, enabled, or qualified here; P10.30 records
  the deferral.
- **No certification claims.** v1.0.0 is not certified, not a safety case, and not permission
  for live grid control. The security review (P10.25) and release notes (P10.26) state their
  limits plainly.
- **No protection-grade claims.** FlexCore is supervisory orchestration (§1.6).
- **Simulation-first 0.x; field write authority begins at v1.0.0 with operator approval.**
  The release does not back-fit write authority into any 0.x line.
- **Solver discipline (AGENTS.md).** HiGHS is LP/QP/MIP, not SOCP. Clarabel is convex conic,
  not MISOCP. AC power flow is not AC-OPF. IPOPT is local, not a global-optimality guarantee.
  Enforced in P10.05.
- **One candidate.** All verifications test the same frozen release candidate (P10.18). A
  verification run against any other build does not count.
- **Thresholds before results.** Benchmark acceptance thresholds are recorded before the
  benchmark runs (P10.10); thresholds fitted after the fact are invalid.

## 5. Prior-phase outputs consumed

| Output | From | Consumed by |
|---|---|---|
| Phase 9 exit-gate record + handover pack | P9.30 | P10.01 (scope), P10.16–P10.17 (risk consolidation), P10.26 (limitations), P10.27 (decision pack) |
| Control-scope document + promotion decisions | P9.26 | P10.15 (mode qualification) |
| Known-limitations / residual-risk register | P9.27 | P10.17, P10.26 (carried forward item for item) |
| D6 closure + security assessment | P9.06, P9.07 | P10.16 (consolidation) |
| HIL report, field acceptance records | P9.15, P9.17 | P10.01 (adapter matrix), P10.16–P10.17 (consolidation) — not re-run |
| Lab/field test procedures | P9.14, P9.16–P9.20 | P10.22 (harness content) |
| Backup rehearsal procedure + timings | P9.24 | P10.11 (re-run against the candidate) |
| Operator training, procedures, alert catalogue | P9.21–P9.23, P9.25 | P10.23, P10.29–P10.30 |
| CI gates, contracts, plugin model | Phases 1, 5, 7 | P10.02, P10.08, P10.18 |

If a Phase 9 output is missing or does not match the release candidate, that is a finding
in the consuming subtask — not a reason to proceed on an assumed equivalent.

## 6. D1–D6 relevance

| Gate | Relevance to Phase 10 |
|---|---|
| D1/D2/D3/D5 | Closed (or interim) in Phase 9 for the deployment; Phase 10 consolidates the evidence and checks the release scope does not exceed it (P10.01, P10.16). |
| D4 license and dependency audit | **Final closure for the v1.0.0 artifact set in this phase (P10.19)**; first gated before v0.1.0. An unclosable D4 blocks the go decision (P10.27). |
| D6 cyber and OT boundary review | Closed in Phase 9 (P9.06) and must close **before any read-only pilot** (`.agent/DECISIONS.md` gate); Phase 10 verifies the D6 scope covers the release scope (P10.16). A gap is a go blocker. |

## 7. Open questions

1. Has NIST SP 800-82 Rev. 4 published as final once the comment period closes 30 November
   2026, and do its deltas change any control? (P10.16 — re-check on the review date; no
   final answer is possible before 30 November 2026.)
2. What are the current Syft/Grype/Trivy/ORT release versions and SPDX/CycloneDX format
   versions at the time P10.19 runs? (See `RESEARCH.md` — verify against each tool's own
   repository; do not copy the background versions into a release record.)
3. Which browser/OS cells, if any, fail in P10.14 and must be removed from the supported
   matrix (P10.26)?
4. Is the Helm chart (§21.4, from v1.0.0) published and verified as part of P10.21?
5. What is the support commitment for v1.0.0 (what "supported" means and for how long)?
   (P10.21 — record it, or record that it is undecided.)
6. Which P10.27 conditions, if any, are carried as post-release actions, and what is the
   first post-release review date? (P10.30.)
