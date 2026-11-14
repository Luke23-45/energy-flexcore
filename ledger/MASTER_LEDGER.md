# FlexCore Master Ledger

> **FINALIZATION — Ledger v1.0, 2026-11-07.** Status: **FINALIZED**.
> 11 phases, **381 subtasks** (verified by independent re-count:
> 30/32/31/31/37/33/34/43/39/41/30), 69 files. Reviewed in 10 iterations
> plus a final independent review: **170 findings — 0 Critical / 24 High /
> 95 Medium / 51 Low** (the log's total row previously read 100/46; the
> per-iteration rows sum to 95/51 — corrected in the final review). All 24
> Highs have owning subtasks (open work, not unrecorded gaps). Final review
> fixed 8 further errors: the 100/46 arithmetic error, a stale Rust 1.99.0
> row in P7 RESEARCH.md (current is 1.99.1), a stale `P9.01 … P9.30` intro
> (phase has 41 subtasks), four handover dependency ranges narrower than
> their Inputs (P2.29, P3.30, P4.32, P5.30), and a PostgreSQL 18.6 row
> upgraded from secondary to primary-confirmed. Repo checks pass
> (`verify_docs.py`, `validate_agent.py`, exit 0); `git status` shows only
> untracked `ledger/`; nothing committed or pushed. Full record, sign-off
> checklist, research spot-checks, and the honest open-items list:
> [`FINALIZATION.md`](FINALIZATION.md). "Finalized" describes the *plan* —
> all 381 subtasks are unchecked and no implementation work is done. The
> blocking next decision is Phase 0, D1–D3 (jurisdiction/market, first
> portfolio/service, network class).

- Report: `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
  (research cut-off 7 November 2026)
- Foundations: `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0`
  (canonical; wins on any conflict — AGENTS.md §2)
- Ledger cut-off: 7 November 2026. Research in each phase's `RESEARCH.md`
  was checked against primary sources on that date; re-verify versions at
  implementation time.
- Status at creation: **all 381 subtasks unchecked** (`- [ ]`). Nothing in
  this ledger is done. Nothing is committed or pushed.
- How to use this file: pick the next phase whose entry gate is met, open
  its directory, work its `ledger_phase_<N>.md` in order. Tick a subtask
  only with evidence (see `README.md`).

## 1. Phase map

Phases 0–9 map one-to-one to report §23 Stages 0–9. Phase 10 has **no**
report stage: it is the v1.0.0 release gate (§25 pilot release + §22.3
release gates). It **overlaps Stage 9 but is not duplicated work** — it
closes and consolidates Stage 9 evidence into a releasable v1.0.0,
cross-referencing Phase 9 subtasks rather than redoing them (stated in
`phase-10-v1-release-hardening/ledger_phase_10.md` and `PLAN.md`).

| Phase | Report stage | Goal | Release | Subtasks |
|---|---|---|---|---|
| 0 — Product freeze | Stage 0 (§23) | Freeze scope, vocabulary, contracts, threat model, ADRs; give every §25.2 gate an owner, venue, and date | Pre-v0.1.0 | P0.01–P0.30 (30) |
| 1 — Rust foundation | Stage 1 (§23) | Cargo workspace, DDD/hexagonal separation, Axum API, PostgreSQL + tenant context, NATS JetStream + outbox, tracing/metrics, CI | v0.1.0 base | P1.01–P1.32 (32) |
| 2 — Asset + telemetry | Stage 2 (§23) | Registry (tenant/portfolio/site/asset), canonical telemetry model + units, simulator source, quality/stale detection, current-state + historical views, deterministic replay | v0.1.0 | P2.01–P2.31 (31) |
| 3 — Flexibility + scheduling | Stage 3 (§23) | BESS/load/EVSE capability models, flexibility envelopes, LP/MILP/QP scheduler with declared problem class, solver result/diagnostics contract, schedule comparison | v0.2.0 | P3.01–P3.31 (31) |
| 4 — Grid feasibility + dispatch | Stage 4 (§23) | Grid-model provider, AC power-flow validation, voltage/thermal/transformer constraints, command lifecycle + audit, human approval + signed commands, simulation-only dispatch adapter | v0.3.0 | P4.01–P4.37 (37) |
| 5 — Operator console + simulator | Stage 5 (§23) | Fleet/sites/assets/grid/schedule/dispatch workspaces, simulator adapters + scenario replay, failure injection, operator audit + reports | v0.3.0 | P5.01–P5.33 (33) |
| 6 — Edge + protocol | Stage 6 (§23) | Rust edge agent, outbound mTLS enrollment, OpenEMS integration, telemetry buffering/recovery, one protocol adapter with a real pilot partner, conformance tests | v0.4.0 | P6.01–P6.34 (34) |
| 7 — Plugin SDK | Stage 7 (§23) | Manifests + schema, capability seams, built-in Rust provider registry, profiles/bundles, Wasmtime/WIT sandbox prototype, Python gRPC worker template, conformance suite | v0.5.0 | P7.01–P7.43 (43) |
| 8 — Advanced coordination | Stage 8 (§23) | ADMM/DMPC provider, GNN warm-start/screening, MARL simulation/shadow worker, dynamic simulator integration, benchmark paper/report | v1.x / v2.x research | P8.01–P8.39 (39) |
| 9 — Pilot qualification | Stage 9 (§23) | Customer-specific integration + network design, security assessment, OT/IT boundary review, commissioning plan, HIL + field acceptance tests, operational procedures | v1.0.0 pilot | P9.01–P9.41 (41) |
| 10 — v1.0.0 release hardening | §25 pilot release + §22.3 | Stable workflow, reproducible benchmark, deployment/upgrade model, security review, site-integration test harness, operator-approved control default — the release gate | **v1.0.0** | P10.01–P10.30 (30) |
| **Total** | | | | **381** |

Subtask counts are by "Acceptance criteria" lines in each
`ledger_phase_<N>.md` (grep-verified 7 Nov 2026). The build pass created
336 subtasks; review iterations 2–6 added 45 (P2 +1, P3 +1, P4 +5, P5 +3,
P6 +4, P7 +11, P8 +9, P9 +11). Phases 0, 1, 10 added none — their gaps
were fixed in place.

## 2. Status dashboard (ledger FINALIZED — v1.0, 2026-11-07; work all unchecked at start)

Ledger status: **FINALIZED — v1.0** (see `FINALIZATION.md`). Work status as of the 2026-11-14 implementation close: **366/381 subtasks ticked** with code, tests, and decision records on disk — NOTHING installed, built, tested, scanned, signed, or run anywhere in the program (every suite WRITTEN-UNRUN; every gate NOT HELD / NOT MET, recorded per phase). The 15 unticked boxes are P0/P1 human-gate criteria (reviewer sign-offs, gate dates, first build/CI execution) — deliberately open, never claimed.

| Phase | Subtasks | Done | Exit gate met? | Blocking decision |
|---|---|---|---|---|
| 0 | 30 | 26 | - [ ] | D1, D2, D3 (owner + venue + date, or decision) |
| 1 | 32 | 21 | - [ ] | D4 (before v0.1.0) |
| 2 | 31 | 31 | - [ ] | — |
| 3 | 31 | 31 | - [ ] | — |
| 4 | 37 | 37 | - [ ] | — |
| 5 | 33 | 33 | - [ ] | — |
| 6 | 34 | 34 | - [ ] | D5 (before v0.4.0) |
| 7 | 43 | 43 | - [ ] | — |
| 8 | 39 | 39 | - [ ] | — |
| 9 | 41 | 41 | - [ ] | D6 (before any read-only pilot) |
| 10 | 30 | 30 | - [ ] | D1–D6 closure for the deployment; go/no-go (P10.27) |
| **Total** | **381** | **366** | | |

## 3. Entry / exit gates

Entry gate for a phase = previous phase's exit gate met, plus the
decision gates in the table above. The §23 exit gate for each phase is
restated in its `CHECKLIST.md` with the evidence required per item.

| Phase | Entry gate | Exit gate (§23, abbreviated) |
|---|---|---|
| 0 | Report v1.1 + foundations available; reviewers named (engineering, product, power systems) | Engineering, product, and power-systems reviewers agree on the first workflow and protected invariants; every §25.2 gate has an owner, a decision venue, and a date. Physical control work stays blocked until the relevant gates close. |
| 1 | Phase 0 exit gate | One end-to-end non-control workflow works through API, application use case, database, and event publication. |
| 2 | Phase 1 exit gate | Synthetic BESS/PV/load/EVSE portfolio telemetry is ingested, normalized, queried, and replayed deterministically. |
| 3 | Phase 2 exit gate | Schedules pass asset/time-coupled constraints on defined synthetic cases. |
| 4 | Phase 3 exit gate | Invalid, stale, infeasible, unauthorized, duplicate, and expired candidate commands are rejected in deterministic tests. (Exhaustive 14×14 transition matrix now owned by P4.33.) |
| 5 | Phase 4 exit gate | An operator can run a full simulated VPP scenario and inspect every scheduling and dispatch decision. |
| 6 | Phase 5 exit gate; D5 decided (first protocol + asset mix) | Read-only end-to-end site integration works in a lab; command path remains disabled unless explicitly approved for a test environment. |
| 7 | Phase 6 exit gate | Swap one optimizer/grid provider without changing application/domain code; untrusted plugin cannot access ungranted capabilities. |
| 8 | Phase 7 exit gate | Every research module is compared to strong deterministic baselines and independently validated. |
| 9 | Phase 8 exit gate (for the modules the pilot uses); D6 review complete before any read-only pilot | Customer/site and applicable authorities approve the deployment and its control scope. |
| 10 | Phase 9 exit gate + all §22.3 release gates evidenced | v1.0.0 go decision recorded (P10.27) with the full decision pack; operator-approved control is the default; no unresolved Critical/High item. |

## 4. Dependencies between phases

Linear by design (§23 is ordered by dependency and risk), with these
named cross-links — a phase must not assume an output from a later phase:

- P0 → all: frozen contracts (units/sign §10.2, telemetry quality §10.4,
  command state machine §14.1, grid-model contract, threat model,
  ADRs 001–012, §24 slice map). Later phases reference `P0.xx` IDs.
- P1 → P2–P10: workspace/crate boundaries, migrations, outbox, NATS
  topology, API/OpenAPI, CI gates (§22.2).
- P2 → P3: registry + telemetry + replay. **Known gap closed in
  iteration 2:** no phase produced forecasts — P3.31 now owns the
  synthetic forecast fixture (P3.12's input), since §23 Stage 2 has no
  forecast deliverable.
- P3 → P4: schedules + solver diagnostics contract (§12.6); P4.37
  consumes the §12.5 step-8 RETURN loop (previously no consumer).
- P4 → P5: command lifecycle + audit events; P5.09 consumes the audit
  API (P4.36 now owns `/api/v1/audit-events`).
- P5 → P6: console + simulator handover (P5.30); P6 lab runs reuse
  P5 scenarios.
- P6 → P7: edge/protocol evidence feeds the plugin conformance suite;
  adapter lifecycle beyond P6.17/P6.20 belongs to Phase 7.
- P7 → P8: plugin host + capability seams are what research modules
  plug into (P8 modules are providers, never kernel replacements).
- P8 → P9/P10: only research modules that passed the Phase 8 exit gate
  may be considered for pilot/shadow use; none may dispatch (ADR-010).
- P9 → P10: pilot evidence (HIL/FAT, security assessment, OT/IT
  review, commissioning) is consolidated by Phase 10, not redone.

## 5. D1–D6 gate mapping

| ID | Decision | Gate | Owner | Status | Ledger owner |
|---|---|---|---|---|---|
| D1 | Target jurisdiction and market | Phase 0 | Product + legal | OPEN | P0 (owner/venue/date or decision); P9.11, P10.27 carry it if still open |
| D2 | First portfolio and service | Phase 0 | Product | OPEN | P0; P9.03/P9.04 proceed only on recorded interim positions if open |
| D3 | Network class and model fidelity | Phase 0 | Power systems | OPEN | P0; P4.34 owns the unbalanced-model deferral/trigger |
| D4 | License and dependency audit | Before v0.1.0 | Legal + engineering | OPEN | P1.27/P1.28 (inventory + cargo-deny gate); **P10.19 closes it for the v1.0.0 artifact set** — if no earlier audit exists, it blocks the go decision |
| D5 | First protocol and asset mix | Before v0.4.0 | Product + integration | OPEN | P6.01 (must name OCPP *edition* as well as version); P9.04 |
| D6 | Cyber and OT boundary review | Before any read-only pilot | Security | OPEN | P6.03 (TLS floor), P9 (OT/IT review), P10.16 (NIST SP 800-82 Rev. 4 — draft until comments close 30 Nov 2026; no "final" answer before then) |

Recommended defaults (NOT decisions): balanced radial feeders for 0.x;
pandapower as primary power-flow provider; unbalanced three-phase from
v0.4.0; meshed transmission out of scope; Apache-2.0 core license
pending D4. A default used for simulation must never be written up as
decided.

## 6. The §24 vertical slice — where it lands

The build-first slice (§24, 10 steps) spans v0.1.0–v0.3.0 and lands
across Phases 0–5; it is **not** all promised in v0.1.0. Iteration 10
walked all 10 steps and confirmed every step is owned by a named
subtask with testable acceptance (mapping table in
`_iterations/iteration-10/findings.md`):

| §24 step | Owner |
|---|---|
| 1. Tenant, portfolio, site, grid model, 4 simulated assets (BESS, PV, flexible load, EVSE) | P0.24 (slice map), P1, P2 |
| 2. Timestamped telemetry with quality flags | P2 |
| 3. Flexibility over a defined 24-hour horizon | P3 |
| 4. LP/MILP/QP portfolio schedule | P3 |
| 5. Validate candidate vs asset capability + feeder power-flow | P3.28 (asset/time-coupled only) + P4 |
| 6. PROPOSED command; validation/rejection/approval transitions | P4 |
| 7. Simulator-only dispatch adapter | P4 |
| 8. Ack, tracking, partial failure, timeout | P4 (+ P5 failure injection) |
| 9. Full decision/audit trail in the web console | P5 |
| 10. Replay same scenario, same versions, verify reproducibility | P5 + P9.38 (pilot shadow replay) + P10.09 (on the release candidate) |

## 7. Definition of done (whole ledger)

The ledger is done — i.e. FlexCore is pilot-ready — only when **all**
of the following hold:

1. Every phase's exit gate is met with evidence in its CHECKLIST.md.
2. Every §22.3 release gate is evidenced (each maps one-for-one to a
   named P10 subtask; see `phase-10-v1-release-hardening/ledger_phase_10.md`).
3. All command-state transitions are tested (exhaustive matrix, P4.33),
   authorization and tenant-isolation tests pass, optimizer outputs are
   independently validated, stale/missing telemetry cases are handled,
   edge offline/degraded modes are tested, plugin permission boundaries
   are tested, simulator benchmarks are reproducible, backup/restore and
   upgrade/rollback are tested, the §21.4 browser/OS matrix is tested,
   the operator-approved control-authority mode has passed its own
   qualification, and a deployment-specific cyber + operational risk
   review is complete.
4. A production field pilot additionally has site-owner approval,
   device/vendor integration tests, local safety review, commissioning,
   and any applicable market/interconnection qualification.
5. No Critical or High item is open in `_iterations/` (see §8).
6. The v1.0.0 go decision is recorded (P10.27) with the full decision
   pack. Bounded automation is NOT part of v1.0.0 — it is post-1.0, per
   deployment, site, and asset class, after qualification.

## 8. Open items (honest, as of 7 Nov 2026)

- **Critical: none found in any iteration (0 across iterations 1–10).**
- **High: 24 found across iterations 1–10; all now have owning
  subtasks** — they are open *work* (tests/proofs to execute before
  their phase's exit gate can honestly pass), not unrecorded gaps.
  The two most load-bearing: P5.31 (alarm contract — §20.1 requires
  "key alarms" in Overview but no phase defined the alarm model) and
  P4.33 (exhaustive command-transition matrix — §22.3 requires *all*
  transitions tested; the draft tested only a representative set).
  Others include P7.33–P7.35 (audit integrity, plugin-host trust
  boundary, ADR-006 no-dylib CI gate), P8.31/P8.32 (tenant isolation,
  model-artifact signing), P9.31/P9.32/P9.37 (hostile replay/forgery
  test, audit integrity + edge buffer, adapter conformance suites
  executed in pilot configuration).
- **Medium (deferred, now applied in final assembly):** iteration-09
  M-9.3 (P7 Wasmtime CVE affected ranges — 115889 fixed on main in
  43.0.0, so 43.0.0–47.x affected only by 115893; added GHSA-m9f7-9w4p-gv38)
  and M-9.4 (P1/P2 NATS v2.15.2 published 26 Oct 2026, not 5 Nov; added
  sibling CVE-2026-33223). Both corrected in the phase RESEARCH.md files.
- **Medium/Low open verifications (by design, re-check at build time):**
  React/TypeScript/Vite versions (P5 RESEARCH — secondary sources only);
  OpenADR 3.1 certification profiles; SPP/CAISO FERC 2222 dates not
  independently re-checked; PyTorch 2.15 vs PyG 2.8.0 skew (P8);
  RLlib latest Ray version; OpenDSS/GridLAB-D licenses (D4 audit);
  IEEE 1547 revision (P1547) publication date. Each is recorded as an
  open verification task in the owning phase's RESEARCH.md or ledger —
  none is asserted as fact.
- **Deferred with reasons (logged in findings):** I5-S2/S3 (signing-key
  compromise procedure; secret-handle expiry/rotation — need design
  decisions the report does not supply); I6-S1/S2 (broker-latency bound
  needs P7.26's first measurement; manifest-schema migration — manifest
  is v1-only); I3-S1/S2/S3 (adapter lifecycle → Phase 7; screen-reader
  check not required by §20.2; OpenEMS EVSE/load lab coverage depends
  on D5); report TOC cosmetic (§0 unnumbered — report out of scope).
- **Correction to an iteration claim:** the iteration-10 reviewer
  reported "332 subtasks total confirmed" — that was the pre-review
  build count and is stale. The verified total after all patches is
  **381** (initial build 336 + 45 added in iterations 2–6; see §1).
  (The build pass itself created 336 subtasks, not 332 as stated in
  one handoff — recount: 30+32+30+30+32+30+30+32+30+30+30 = 336.)

## 9. Cross-phase risks

1. **D1/D2/D3/D5 still open at pilot time** — Phase 9 can only proceed
   on recorded interim positions with change conditions (P9.27); §22.3
   forbids promotion beyond read-only until they close for the
   deployment jurisdiction, network class, and protocol mix.
2. **D4 license audit** — ANDES (GPLv3) and PyPSA (GPL) are live D4
   issues; worker-boundary isolation + legal confirmation required
   (P8); the audit gates v0.1.0 and is re-closed for v1.0.0 (P10.19).
3. **Wasmtime sandbox** — pin ≥50.0.0 (CVSS 9.9 escapes CVE-2026-115889/
   115893 fixed only in ≥50.0.0 / 48.0.7 / 49.0.4 / 36.0.19); WASI 0.3
   unusable on stable Rust until 1.100 ships (12 Nov 2026).
4. **NATS durability** — Jepsen 2.12.1 finding (write loss under file
   corruption; default fsync every 2 min, not before ack): outbox +
   idempotency keys are the guarantee; fsync policy is a deployment
   decision. JetStream restore surface carries CVE-2026-33222/33223
   (P10.11).
5. **Solver-class discipline** — a single mislabelled solver (e.g.
   claiming SOCP from HiGHS, or global optimality from IPOPT) would
   invalidate feasibility evidence; every phase restates the discipline.
6. **Research-to-field leakage** — GNN/MARL/ADMM outputs are proposals
   only (ADR-010); the protected kernel (authorization, tenant
   isolation, audit, command lifecycle, final dispatch validation) is
   non-replaceable by any plugin.
7. **Version drift** — every RESEARCH.md is a 7 Nov 2026 snapshot;
   Rust 1.100, NIST SP 800-82 Rev. 4 (comments close 30 Nov 2026),
   pandapower 4.0.0, and FERC 2222 RTO go-lives all move after the
   cut-off. Re-verify at implementation time; do not pin from memory.
8. **Alarm model unowned until P5.31** — the console Overview cannot
   honestly show "key alarms" (§20.1) until the alarm contract is
   defined; escalate with owner/date if it is deferred.
