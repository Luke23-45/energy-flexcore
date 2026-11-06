# Iteration 3 — Completeness / coverage vs §23 Stages 4–6 exit gates and §2 release scope

- **Reviewer:** Iterations 3+4 reviewer (P4–P6 scope)
- **Date:** 2026-11-07
- **Scope:** `ledger/phase-04-grid-feasibility-dispatch/`, `ledger/phase-05-operator-console-simulator/`, `ledger/phase-06-edge-protocol/`
- **Grounding:** report `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` §2, §22.3, §23 Stages 4–6, §24; `.agent/DECISIONS.md` (D1–D6 OPEN); `AGENTS.md`.
- **Method:** re-read §23 Stages 4–6 deliverables + exit gates and §2.1/§2.3 line by line against every P4/P5/P6 subtask; script-checked §-traces and cross-phase ID references (see Iteration 4 findings for the trace audit); read all three RESEARCH.md files and all three CHECKLIST.md files.
- **Verdict:** the three phases are substantially complete against their gates — all Stage 4 deliverables, all 12 §20.1 workspaces, all Stage 6 deliverables, and all six deterministic-rejection categories are covered. The gaps below are real but none is a missing deliverable: one required workspace element has no owning contract (High), one §22.3 release-gate demand is only sampled (High), and the rest are unowned sub-behaviours (Medium) or wording/record gaps (Low).

## (a) Missing subtasks

### I3-01 — High — No subtask owns the alarm model/contract behind §20.1 item 1 "key alarms"
- **File + subtask patched:** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, new **P5.31**.
- **Evidence:** §20.1 item 1 requires Overview to show "key alarms". P5.02 step 1 displays alarms "as defined by prior phases — record any missing alarm contract as a gap, do not invent one". No prior phase defines one: there is no Alarm event in §15.3, no alarm subtask in Phase 2 (grep: zero "alarm" hits in `ledger_phase_2.md`), and the only later owner is P9.25 "alert catalogue" (Stage 9 pilot) — too late for the v0.3.0 console. P5.02 records the gap but no subtask owns resolving it, so the required Overview element is unbuildable as specified.
- **Patch applied:** P5.31 — alarm-contract gap ownership: define the minimal alarm event/contract (source, severity, state, acknowledgement, tenant scope) or formally escalate to the owning phase with owner + date; P9.25 is the consumer, not the definer.

### I3-02 — High — §22.3 demands ALL command-state transitions tested; P4.10 tests only a "representative set" of illegal ones
- **File + subtask patched:** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, new **P4.33**.
- **Evidence:** §22.3 release gate: "all command-state transitions are tested". P4.10 step 4: "Test every legal transition and a representative set of illegal transitions". The legal side is exhaustive; the illegal side is explicitly sampled, so the §22.3 gate cannot be evidenced from P4.10 alone. (P4.10's legal-transition enumeration itself was verified complete against the §14.1 diagram — see Iteration 4.)
- **Patch applied:** P4.33 — exhaustive transition-matrix test: all 14 states × all possible targets, legal and illegal, generated from the P4.10 table, not hand-picked.

### I3-03 — Medium — Unbalanced three-phase power flow: named in §12.5 step 4, never owned
- **File + subtask patched:** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, new **P4.34**.
- **Evidence:** §12.5 step 4 requires "the appropriate AC power-flow or three-phase unbalanced power-flow model". P4.05's objective mentions "(or three-phase unbalanced)" but no step defines when unbalanced applies, which provider performs it, or the trigger for adding it. The D3 default (balanced radial, §25.2) is recorded in P4.01 but the unbalanced deferral/trigger itself has no subtask; P4.32/P5.30 handovers only repeat "unbalanced three-phase from v0.4.0" without a trigger definition.
- **Patch applied:** P4.34 — unbalanced-model deferral/trigger record: default balanced radial in Phase 4, named trigger cases (e.g. a named unbalanced feeder case) that force a second provider, and the provider-selection rule reused from P4.01.

### I3-04 — Medium — Feasibility-gate runtime is never measured (§21.3 solver duration)
- **File + subtask patched:** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, new **P4.35**.
- **Evidence:** §21.3 requires measuring "solver duration/status"; §19.4 grid-feasibility metrics list convergence/residuals but no runtime. P5.24 measures scheduling runtime (reusing P3.27) and P5.23 records §19.4 metrics, but no P4 subtask measures power-flow/feasibility-gate duration, so a slow gate has no baseline and no regression signal.
- **Patch applied:** P4.35 — feasibility runtime/diagnostics measurement per model size, recorded with conditions, wired into the §19.4 metric set.

### I3-05 — Medium — `/api/v1/audit-events` (§15.2) has no owning subtask anywhere
- **File + subtask patched:** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, new **P4.36**.
- **Evidence:** §15.2 lists 16 candidate resources including `/api/v1/audit-events`; Phase 0 froze the list. Phase 1 built the audit crate/port (P1.10) but no REST resource; P4.30 exposes only `/api/v1/feasibility-checks` and `/api/v1/dispatch-commands`; P5.09 consumes audit data without naming the API it reads from. The audit trail is therefore writable but has no specified read API for the console.
- **Patch applied:** P4.36 — audit-events read API (verify-first: if Phase 1 already exposes it, record the endpoint and close; otherwise build the tenant-scoped, append-only read API Phase 5 consumes).

### I3-06 — Medium — Console never displays the current tenant (§16.2)
- **File + subtask patched:** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, new **P5.32**.
- **Evidence:** §16.2 requires tenant context derived from authenticated claims; P5.26 enforces it at API + console. But no subtask requires the console to *display* the current tenant, so an operator can act under the wrong tenant without any visible cue. P5.03 tests role visibility, P5.26 tests denial — neither shows the tenant.
- **Patch applied:** P5.32 — tenant-context display + tenant-switch guard: persistent tenant indicator on every workspace, switch flow re-derives context, no tenant taken from a request field.

### I3-07 — Medium — Live updates: disconnect handled, out-of-order/duplicate delivery not (§15.4 at-least-once)
- **File + subtask patched:** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, new **P5.33**.
- **Evidence:** §15.4: JetStream is at-least-once; consumers must be idempotent. P5.25 handles connection loss and re-sync, but no step handles out-of-order or duplicate live events: a stale dispatch/telemetry event arriving late could overwrite a fresher displayed state. P4.24 solves the same problem for commands (idempotency/sequence); the console stream has no equivalent.
- **Patch applied:** P5.33 — live-update ordering/dedup: per-stream sequence, stale-event discard, duplicate suppression, test with reordered/redelivered events.

### I3-08 — Medium — Edge time synchronization has no owner (§10.3, §17.4 clock drift)
- **File + subtask patched:** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, new **P6.31**.
- **Evidence:** §10.3 requires clock-quality/synchronization status on time-dependent records; §17.4 lists clock drift as a threat with "time-quality monitoring, bounded command expiry, synchronization checks". P6.15 step 2 verifies commands "against the edge clock with the recorded clock-quality check" — but no subtask records that check, defines the sync source (NTP or equivalent), the tolerance, or the monitoring. P4.21 tests clock skew for commands centrally; the edge side is unowned.
- **Patch applied:** P6.31 — edge time-sync and clock-quality monitoring: sync source, tolerance, quality status surfaced in health, behaviour when sync is lost (commands held, not trusted).

### I3-09 — Medium — Edge decommissioning/unenrollment has no implementation subtask
- **File + subtask patched:** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, new **P6.32**.
- **Evidence:** P6.06 implements enrollment (identity, certificates, revocation handling) but nothing implements offboarding: revoke the identity server-side, wipe keys/buffers/audit material on the edge, confirm removal. P6.26 step 1 lists "decommission" only as a documentation topic. An edge that cannot be cleanly unenrolled leaves a live identity and buffered data behind.
- **Patch applied:** P6.32 — edge decommissioning/unenrollment: revocation, local wipe, confirmation record, test with a decommissioned identity attempting reconnect.

### I3-10 — Medium — P6.24 exit-gate evidence pack omits conformance, drill, and security-clearance evidence
- **File + subtask patched:** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, new **P6.33**.
- **Evidence:** §23 Stage 6 exit gate is the read-only lab run; §22.3 additionally requires edge offline/degraded modes tested. P6.24 step 4's evidence pack lists telemetry reconciliation, quality flags, audit trail, health status, no-write attestation — but not the conformance results (P6.19), the failure-mode drill results (P6.28), or security-findings clearance (P6.23). The gate pack as specified cannot evidence the §22.3 offline/degraded item.
- **Patch applied:** P6.33 — exit-gate evidence-pack completeness: the P6.24 pack must cite P6.19/P6.28/P6.23 outputs by version/run ID, or record each as explicitly missing.

### I3-11 — Low — Secure-boot position (§17.3) is unrecorded for edge hosts
- **File + subtask patched:** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, new **P6.34**.
- **Evidence:** §17.3 requires "secure boot/update policy for edge hosts where customer platform supports it". P6.20 covers signed updates thoroughly but never records the secure-boot position per gateway profile. The control is conditional ("where supported"), so a position record — supported, or unsupported with fallback + risk — is the correct scope, not an implementation.
- **Patch applied:** P6.34 — secure-boot position record per gateway profile.

### I3-12 — Low — P6.30 lacks the doc-verification step P4.32/P5.30 have
- **File + subtask patched:** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, **P6.30** (in-place, step 5 added).
- **Evidence:** P4.32 step 3 and P5.30 step 5 both run `python tools/verify_docs.py` and `python tools/validate_agent.py` and record output before claiming the gate. P6.30 (retrospective + v0.4.0 release-gate record) has no equivalent step, although P6.26 step 4 runs `verify_docs.py` only after doc changes. Inconsistent gate discipline across phases.
- **Patch applied:** in-place addition of step 5 to P6.30 (run both tools, record output, fix failures before claiming the gate); CHECKLIST.md "Documentation complete" item unchanged (it already requires `verify_docs.py`).

### I3-13 — Low — P5.30 exit demo does not explicitly require inspecting a rejected command
- **File + subtask patched:** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, **P5.30** (in-place, step 1 extended).
- **Evidence:** §23 Stage 5 exit gate: "an operator can run a full simulated VPP scenario and inspect every scheduling and dispatch decision". P5.30 step 1 says "inspects every scheduling and dispatch decision" but the demonstration record (step 2) never names a rejected command; P5.06/P5.09 build rejection views, but the demo could pass without exercising one.
- **Patch applied:** in-place extension of P5.30 step 1: the demo must include at least one rejected command inspected with its failing constraint and evidence link.

## (b) Issues / errors / contradictions (coverage angle)

- **I3-C1 (Medium, same as I3-10):** P6.24's acceptance says "every CHECKLIST.md item has evidence or is explicitly failed", but the P6 CHECKLIST "Read-only end-to-end" item lists only the five pack contents — the checklist and the subtask agree with each other and both omit conformance/drill/security evidence. Fixed via P6.33, not by editing P6.24 (the pack contents are now defined by the new subtask).
- **I3-C2 (Low):** P4.27 says "Collect the six suite outputs (P4.20–P4.25) plus the combined suite (P4.26)" and its acceptance says "All six rejection categories pass" — the combined suite is a seventh suite, not a seventh category. Wording is defensible on a careful read; no patch (noted here so it is not mistaken for a miscount).
- **I3-C3 (Low):** §22.3 items "backup/restore and upgrade/rollback are tested", "plugin permission boundaries are tested", "simulator benchmark results are reproducible" are not owned in P4–P6 — correctly so: backup/restore is P10.11, plugin boundaries are Phase 7, reproducibility is P5.19. Verified owned elsewhere; not gaps for these phases.

## (c) Suggestions (not patched, or deferred)

- **I3-S1 (Low, deferred):** Adapter lifecycle management (start/stop/restart/version per adapter, §18.1 "local protocol adapter management") is covered only as health status in P6.17. Deferred, not patched: P6.17 + P6.20 cover status and updates; a full lifecycle is arguably Phase 7 plugin-lifecycle territory, and adding it here would expand Stage 6 scope. Revisit if the D5-selected adapter needs runtime restart semantics.
- **I3-S2 (Low):** P5.12 requires a keyboard-only approval workflow test but no screen-reader check. §20.2 does not require one; do not add it as a gate — record as a future accessibility item for the console hardening pass.
- **I3-S3 (Low):** P6.10–P6.11 exercise OpenEMS with simulated PV + battery only; EVSE/flexible-load paths through OpenEMS are not exercised. Acceptable if D5's asset mix says so — P6.01 must name the mix; if the mix includes EVSE/load, P6.11's lab asset set should be extended at that point.

## (d) Stale research

None stale by date: all three RESEARCH.md files are dated 2026-11-07, the report's research cut-off. What exists is correctly-labelled unverified material, not hidden staleness:

- **P5 RESEARCH.md:** React 19.3.1, TypeScript 7.0, Vite 8.x rest on secondary sources, labelled as such with open task 1 (re-verify against official release pages at scaffold time, P5.01). TanStack versions are an explicit ASSUMPTION (open task 2). `pyhelics` binding version for HELICS 3.7.0 unconfirmed (open task 3). Severity: **Low** — honestly flagged; no patch, but P5.01 must not be marked done without the re-check.
- **P4 RESEARCH.md:** OpenDSS 11.0.0.1 and GridLAB-D v5.2 license texts are explicit ASSUMPTIONS, with open verification tasks for the D4 audit. pandapower 3.5.6 (7 Oct 2026) and PyPSA 1.4.0 (9 Oct 2026) are primary-verified. Severity: **Low**.
- **P6 RESEARCH.md:** OCPP 2.1 Edition 2 dates conflict across secondary sources (open task 2, do not cite a single date); OpenADR 3.1 existence unverified (open task 1). The R6-C3 note correctly forces D5 to name the OCPP *edition*, not just the version. Severity: **Low**.
- **No patch** for any of these: the RESEARCH files already carry the open tasks. The risk is a future implementer treating the secondary-source versions as pinned — mitigated by P5.01 step 2 / P6.12 step 4 as written.

## (e) Severity counts (Iteration 3)

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 2 (I3-01, I3-02) |
| Medium | 8 (I3-03, I3-04, I3-05, I3-06, I3-07, I3-08, I3-09, I3-10) |
| Low | 5 (I3-11, I3-12, I3-13, I3-14-group, I3-S1) + 2 wording notes (I3-C2, I3-C3 as checked-not-gaps) |

## (f) Patch log — applied vs deferred

**Applied (new subtasks, all unchecked `- [ ]`, full template: objective / trace / inputs / steps / deliverable / acceptance / owner / dependencies / verification / risks):**

| File | New subtask(s) |
|---|---|
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.33 exhaustive transition-matrix test; P4.34 unbalanced three-phase deferral/trigger; P4.35 feasibility runtime measurement; P4.36 audit-events read API (verify-first); P4.37 constrain/return consumption contract (§12.5 step 8 return loop — P4.07 produces RETURN outcomes and no subtask in any phase consumes them; verified by grep across all 11 ledgers) |
| `ledger/phase-05-operator-console-simulator/ledger_phase_5.md` | P5.31 alarm-contract ownership; P5.32 tenant-context display + switch guard; P5.33 live-update ordering/dedup |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | P6.31 edge time sync + clock-quality monitoring; P6.32 edge decommissioning/unenrollment; P6.33 exit-gate evidence-pack completeness; P6.34 secure-boot position record |

**Applied (in-place fixes):** P6.30 step 5 (doc-tool verification); P5.30 step 1 (rejected-command inspection in the demo); trace-summary rows extended for the new subtasks (P4, P5); P6 subtask-index table extended and count updated; handover subtask dependency ranges widened (P4.32 → P4.01–P4.37; P5.30 → P5.01–P5.33; P6.30 → P6.01–P6.34); CHECKLIST.md entries added (G4.33–G4.37, G5.31–G5.33, P6 exit-gate items for time sync, decommissioning, evidence pack, secure boot) with sign-off rows updated.

**Deferred:** I3-S1 adapter lifecycle management (scope belongs with Phase 7 plugin lifecycle; revisit on D5); I3-S2 screen-reader check (not required by §20.2); I3-S3 OpenEMS EVSE/load lab coverage (depends on the D5 asset mix, P6.01).

## What was re-checked and found covered (so "zero issues" is not claimed by omission)

- Stage 4: all six deliverables present (provider P4.01, AC power-flow path P4.05, voltage/thermal/transformer P4.06, lifecycle+audit P4.10–P4.14/P4.30, human approval + signed commands P4.13/P4.28, sim-only adapter P4.16); exit gate covered by P4.20–P4.27 (all six rejection categories + combined/adversarial + evidence pack).
- Stage 5: all 12 §20.1 workspaces present (P5.02–P5.09); simulator adapters P5.14–P5.17 incl. HELICS role stated correctly; failure injection P5.21–P5.22; benchmarks P5.23; replay reproducibility P5.18–P5.19; exit demo P5.30.
- Stage 6: all six deliverables present (edge agent P6.05, mTLS P6.07, OpenEMS P6.10–P6.11, buffering P6.09, selected adapter P6.12–P6.13, conformance P6.19); exit gate P6.24 with command path disabled by default P6.14.
- §2.1 release ladder consistent in all three phase headers (v0.3.0 sim/shadow; v0.4.0 read-only/test-lab); §2.3 conformance posture held (P6.19 claim boundary; no "certified" claim from parsing alone).
- §22.3 items owned elsewhere were verified, not assumed missing: backup/restore P10.11, plugin boundaries Phase 7, incident response P9.23.
