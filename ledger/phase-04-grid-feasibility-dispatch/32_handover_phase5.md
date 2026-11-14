# P4.32 — Phase 4 handover to Phase 5 (§23 Stage 4 exit gate)

Date: 2026-11-11. Status: **all 37 subtasks implemented (P4.01–P4.37);
nothing built, nothing run** — the first `cargo build`/`test --workspace` is
the real proof and remains the human's loop. Stage 4 gate: **NOT HELD**
(prepared, not executed — §4 below); the demonstration, the two clean evidence
packs runs (P4.27), and reviewer sign-offs are gate inputs the human supplies.

## 1. What Phase 5 consumes (exact inputs)

| Input | Contract + version home | Source |
|---|---|---|
| Command lifecycle (14 states / 27 frozen edges) | `crates/flexcore-domain/src/{command_lifecycle,command_state}.rs`; P0.11 oracle; exhaustive 196-cell matrix test `crates/flexcore-domain/tests/transition_matrix.rs` | P4.10, P4.33 |
| §14.3 command invariants + evidence references | `crates/flexcore-application/src/{command_record,command_gates}.rs`; `S143_FIELD_GROUPS` | P4.14 |
| Dispatch REST contracts (read-only) | `apps/control-plane/src/{routes_phase4,dto_phase4}.rs`: GET `/api/v1/dispatch-commands` (+`/{command_id}`), GET `/api/v1/feasibility-checks` (+`/{check_id}`); OpenAPI registered; **no generic execute endpoint (§15.2)** | P4.30 |
| Feasibility results + model health | `crates/flexcore-grid-api/src/{feasibility,verdict,fail_closed,model_health}.rs`; migration `0017_feasibility_checks_p4_30` | P4.03–P4.07 |
| Constrain/return consumption contract | `crates/flexcore-application/src/feasibility_return.rs` — the P5.18–P5.19 closed loop consumes `plan_consumption` | P4.37 |
| Audit-events read API | GET `/api/v1/audit-events` (+`/{event_id}`); `crates/flexcore-storage/src/audit_read.rs` (`AuditEventReader`); action `audit.event.read` | P4.36 |
| Rejection evidence pack | `27_evidence_pack.md` — 70 tests indexed across 8 suites, **UNRUN**, sign-offs NOT SIGNED; P5.04 and the §22.3 release gate cite it | P4.27 |
| Security controls | `crates/flexcore-application/src/dispatch_security.rs` (payload-bound signatures, `SequenceGuard`, `TenantDenialAuditRecord`); P4.28 wiring re-verification recorded | P4.28 |
| Safety-invariant matrix | `29_safety_invariant_matrix.md` — invariants 1–9 mapped to tests, invariant 10 boundary note | P4.29 |
| Fault-injection + shadow method | `crates/flexcore-application/src/shadow_evaluation.rs` (`ShadowReport.evidence_status() = "not_evidence_for_field_dispatch"`); `31_fault_shadow.md` fault table | P4.31 |
| Timing-honesty boundary | §14.5 constants + measured-distribution method in `crates/flexcore-application/src/timing_honesty.rs`; no hard-real-time/frequency/relay claims anywhere | P4.19 |

## 2. Simulation-only boundary (REQUIRED handover section — §23 risk row)

**Every issued command in this platform is issued to the SIMULATOR and nothing
else.** `SimulatorDispatchAdapter` hard-refuses all four non-simulator target
kinds (P4.16 guard, re-verified in `29_safety_invariant_matrix.md` invariant 6).
There is no device write path — a structural source scan is pinned in
`crates/flexcore-application/tests/dispatch_simulator.rs`. Acknowledgement is
NOT physical delivery; the console (P5.13) must label every simulated issue,
acknowledgement, and tracking result as simulated and must never present an
acknowledgement as delivery. Operator-approved field control is v1.0.0, not
now; real protocol adapters are Stage 6.

## 3. Versions and pins actually recorded in Phase 4

pandapower **3.5.6** (grid provider pin, D-P4.01-1 — NOT installed), HiGHS
**1.15.1** via highspy (Tier 1 worker path, D-P3.17-1 — NOT installed),
Clarabel.rs **=0.11.1** (native, Tier 2). Migration count now **0017**
(dispatch/feasibility persistence). D3 (network class) still **OPEN**;
`34_unbalanced_deferral.md` is a scoped Phase-4 deferral under it, not a
resolution. D6 (cyber/OT) OPEN — mTLS explicitly NOT implemented (D-P4.28-5).

## 4. Exit-gate status (§23 Stage 4): NOT HELD — prepared

The gate sentence "invalid, stale, infeasible, unauthorized, duplicate, and
expired candidate commands are rejected in deterministic tests" is **prepared
as code**: 61 rejection-suite tests + 9 matrix tests + 14 security tests +
12 fault tests + 14 shadow tests, all with pre-registered expectations and
in-suite double-run determinism assertions. What is missing, and who holds it:

1. **Execution** — all suites UNRUN (loop bans). The human runs
   `cargo test --workspace` and fills `27_evidence_pack.md` run cells.
2. **Two clean full-pack runs** — procedure in `27_evidence_pack.md` §2; the
   no-partial-re-run rule is recorded there as binding.
3. **Reviewer sign-offs** (power-systems / security / QA per suite) — all
   cells `NOT SIGNED (gate input)` in `27_evidence_pack.md` §3.
4. **Secret/dependency scans** — `gitleaks` + `cargo deny` NOT RUN (P4.28 F);
   the pandapower/CVXPY Python-worker lockfile pin is a D4 input.

Verification tools (P4.32 step 3): `python tools/verify_docs.py` → OK;
`python tools/validate_agent.py` → OK (recorded at close, this date).

## 5. Open items carried to Phase 5 (and later)

- **Unbalanced three-phase model** — deferred from v0.4.0 with trigger
  enumeration T0–T5 in `34_unbalanced_deferral.md`; T2–T5 are uncomputable
  against the current grid-api port (no per-phase data) — recorded gap.
- **P4.35 measured cells EMPTY** — feasibility-gate runtime template
  `35_gate_benchmark_report.md`; no timing surface on limit-checks/diagnostics
  (§2 of the template); the human's benchmark run creates the baseline.
- **P4.28 composition wiring** — the adapter's issue path still records the
  legacy traceability signature; payload-bound verification is enforced at the
  receive point, not yet wired to issued envelopes (Stage-6/composition item).
- **Audit cross-tenant denial logging** — cross-tenant reads are 404 by design
  (no leak, D-P4.36-4); §16.2 denial logging stays on the P4.28
  `SecurityAuditSink` seam — flagged for P5.26 review.
- **Shadow is value-level** (requested vs no-command baseline), not
  consequence-level; not yet wired into a composition root (P4.31 §2).
- **Invariant-7 plugin kernel** — does not exist (v0.5.0); bypass-proof is
  structural (no plugin execution path); P5.17 scoping decision should carry
  this forward.
- **Closed-loop consumption** — `feasibility_return.rs` maps CONSTRAIN/RETURN;
  the actual re-scheduling consumer is P5.18–P5.19 (P3.24's frozen trigger set
  intentionally unmodified).
- **First-failure-only gate/journal surfaces** (D-P4.26-2) and **zero
  clock-skew tolerance** (D-P4.21-3) — recorded limitations, not bugs.
