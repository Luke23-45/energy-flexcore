# P3.29 — Phase 3 security review (§17.2–§17.4)

Date: 2026-11-07. Reviewer: implementation-coordinator (STATIC review; the
named security engineer's review + the scans are gate inputs the human runs —
loop bans). Outcome: **NO UNADDRESSED HIGH-SEVERITY FINDING on static
inspection; scans NOT RUN (recorded below).**

## 1. Tenant isolation (§17.5 invariant 8) — verified statically

- All three P3.28 resources (`/api/v1/flexibility/envelopes`, `/schedules`,
  `/optimization-runs`) authorize via `authorize_tenant_scoped` + tenant-scope
  actions (`flexibility.envelope.read/create`, `schedule.read/propose`,
  `optimization_run.read/create` — no delete/update/approve/dispatch actions
  exist); cross-tenant reads → 404 with no leak (tested:
  `apps/control-plane/tests/api_phase3.rs`).
- Storage: migrations 0013/0014/0015 carry `tenant_id` on every tenant-owned
  row, RLS ENABLE+FORCE, owner `flexcore_migrator`, runtime role
  INSERT/SELECT-only; postgres store impls pass the tenant GUC + an explicit
  tenant filter (defence in depth), DB-gated tests assert cross-tenant → None.
- Solver workers (P3.17 D-P3.17-1): the Rust side sends only the problem IR +
  config + pinned versions through the `Tier1Backend` seam — no tenant data
  beyond the tenant-scoped candidate inputs; worker logs are a later-wave
  adapter concern, and the P1.20/P2.21 no-secrets log rules carry forward
  (log review at the worker-adapter wave is recorded as an OPEN checklist
  item for P4.31's fault/shadow suite to re-verify).

## 2. Authorization on schedule actions — verified

- Creating or re-solving a schedule requires `schedule.propose` (Operator+);
  the actor identity is recorded in the schedule record per §21.3 (P3.22
  revision records carry actor/timestamp on every transition; journal rows
  are immutable).
- Deny-by-default holds: `phase3_default_policies` extends the recorded
  grants; no role bypasses the tenant match (P1.11 invariant).

## 3. Secrets + dependencies — scan status

- Static inspection: no credentials in solver configuration (provider-owned
  config structs carry only solver options), scenario fixtures (JSON pins:
  versions/seeds/tolerances), or log lines (correlation fields only, per the
  P2.21 closed-vocabulary rules).
- **Scans NOT RUN** (loop bans): the human runs the dependency/secret scans —
  `cargo deny check` (HiGHS=MIT via highspy; Clarabel.rs=Apache-2.0; both
  already in the D4 inventory), `gitleaks` (already a CI gate), and the
  CVXPY pin check when the Python worker lockfile lands (P4.02's
  `pandapower==3.5.6` BSD-3-Clause is also a D4 input).
- No high-severity finding exists to address on static inspection.

## 4. Findings

| # | Finding | Severity | Disposition |
|---|---|---|---|
| F1 | Frozen `ScheduleValidated` payload cannot carry the validation-scope caveat (6-field payload) | INFO | Recorded (D-P3.28-2); caveat lives in record fields + OpenAPI + docs; additive serde-default extension path recorded |
| F2 | Solver-worker log review deferred to the worker-adapter wave | LOW | Carried as an explicit open item (P4.31 re-verification) |
| F3 | Human scans pending | — | Gate input; blockers list in HANDOVER.md |

Findings F1–F3 are recorded, none blocks implementation continuation; the
Stage 3 gate review (P3.30/CHECKLIST) requires F3's scans before sign-off.
