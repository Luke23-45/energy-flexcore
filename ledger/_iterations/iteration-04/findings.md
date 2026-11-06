# Iteration 4 — Traceability / contradictions

- **Reviewer:** Iterations 3+4 reviewer (P4–P6 scope)
- **Date:** 2026-11-07
- **Scope:** `ledger/phase-04-grid-feasibility-dispatch/`, `ledger/phase-05-operator-console-simulator/`, `ledger/phase-06-edge-protocol/` (with read-only checks across all 11 phase ledgers where a cross-phase claim was made)
- **Grounding:** report `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (§3.4, §8.1, §10.2–§10.4, §12.3–§12.6, §14.1–§14.5, §15.2–§15.3, §16.2, §17.2–§17.5, §18.1–§18.3, §19.2, §20.1–§20.2, §21.4, §23); `.agent/DECISIONS.md`; `AGENTS.md`.
- **Method:** (1) script-extracted every `§N[.M]` reference in all 11 ledgers and checked it against the report's real headings; (2) script-extracted every `P<N>.<MM>` reference in P4–P6 and checked it against the *defined* subtask headings (not mere mentions) of the target phase; (3) compared every subtask title across phases for duplicates; (4) hand-verified the §14.1 transition diagram against P4.10's enumeration, the §15.3 event list against P4.30/P4.07/P4.11/P4.15, the §17.5 invariant list against P4.29, and the §20.1 workspace list against the P5 trace summary; (5) grepped control-authority, solver-class, units/sign, and certification language across P4–P6.
- **Verdict:** traceability is strong — zero dangling §-references in any phase, zero dangling P-references in P4–P6, zero duplicate subtask titles. The failures are subtler: wrong *ranges* in trace summaries and cross-phase sections, two overclaimed traces (§19.2 layer 10 HIL, §3.4 step 5), one self-contradictory sentence, one console-vs-gate contradiction on enabling the command path, and one title overclaim. All are fixed in place below.

## (a) Missing subtasks

None. Traceability/contradiction work turns up no missing subtask; the missing-subtask findings belong to Iteration 3. Two trace gaps are closed by in-place fixes, not new subtasks (I4-08, I4-18).

## (b) Issues / errors / contradictions

### I4-01 — Medium — P5 trace summary mis-maps §20.1 workspaces to subtasks (off by one, three rows); P4 trace summary mis-maps §14.3
- **Files patched (in place):** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md` (Trace summary); `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` (Trace summary).
- **Evidence:** §20.1 lists 12 workspaces. P5's table said: workspaces 1–4 → P5.01–P5.04 (but P5.01 is the scaffold, not a workspace; workspaces 1–4 are P5.02–P5.04); workspaces 5–7 (Grid, Schedules, Dispatch) → P5.05–P5.07 (actually P5.04 Grid, P5.05 Schedules, P5.06 Dispatch; P5.07 is Market/Plugins/Admin); workspaces 8–12 → P5.08–P5.09 (P5.08 is Simulation Lab (9) and P5.09 is Audit (11); items 8, 10, 12 live in P5.07). P4's table said §14.3 command invariants → P4.14–P4.16, but P4.15 is expiry/cancel/supersede (§14.1–§14.2) and P4.16 is the dispatch adapter (§14.1); only P4.14 is §14.3. The §14.1–§14.2 row (P4.10–P4.13) likewise omitted P4.15–P4.16.
- **Fix applied:** P5 rows corrected to P5.02–P5.04 / P5.04–P5.06 / P5.07–P5.09; P4 rows corrected to §14.1–§14.2 → P4.10–P4.13, P4.15–P4.16 and §14.3 → P4.14.

### I4-02 — Medium — P4 "Cross-phase references" cites wrong P5 ranges
- **File patched (in place):** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, Cross-phase references section.
- **Evidence:** it said "dispatch lifecycle states and audit trail (P5.13–P5.15)" — P5.13 is simulation honesty, P5.14 asset-simulator adapter, P5.15 grid adapter; the dispatch lifecycle/audit consumers are P5.06 (Dispatch workspace), P5.09 (Audit), P5.13 (honesty labelling). It said "feasibility evidence display (P5.11)" — P5.11 is design principles; feasibility display is P5.04 (Grid workspace). It said "simulator integration (P5.16–P5.20)" — P5.16 is HELICS, P5.17 scoping; the adapters are P5.14–P5.15 and replay/library P5.18–P5.20.
- **Fix applied:** ranges corrected to (P5.06, P5.09, P5.13), (P5.04), and (P5.14–P5.15 adapters; P5.18–P5.20 replay/library).

### I4-03 — Low — P5 "Cross-phase references" overstates the HELICS handover
- **File patched (in place):** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, Cross-phase references section.
- **Evidence:** "HELICS federation definition (P5.14–P5.16)" — P5.14 is the asset simulator, P5.15 the grid adapter; only P5.16 is HELICS.
- **Fix applied:** corrected to (P5.16).

### I4-04 — Medium — P6.24 trace cites §19.2 layer 10 (HIL/commissioning) for a lab run
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.24 Report trace.
- **Evidence:** P6.24 trace was "§23 Stage 6 (exit gate); §3.4; §19.2(4)(10)". §19.2 layer 10 is "HIL / commissioning tests: only after simulation, integration, and cyber reviews pass" — a Stage 9 activity. The Stage 6 exit gate is a lab run, not HIL; citing layer 10 overclaims the gate and contradicts P5.17/P6.30, which explicitly defer HIL to Stage 9.
- **Fix applied:** trace reduced to "§23 Stage 6 (exit gate); §3.4; §19.2(4)".

### I4-05 — Low — P6.11 trace overclaims §3.4 steps 2–5
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.11 Report trace.
- **Evidence:** §3.4 step 5 is "Asset and portfolio services calculate available flexibility" — a flexibility calculation, not part of the read-only telemetry path. Steps 2–4 (connect, telemetry arrives, validate/normalize/store) are the read path.
- **Fix applied:** trace corrected to "§3.4 (steps 2–4)".

### I4-06 — Medium — P4.15 contradicts P4.10: "EXPIRED is terminal except for FALLBACK"
- **File patched (in place):** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, P4.15 step 1.
- **Evidence:** P4.10 correctly lists the terminal states as COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK (matching the §14.1 diagram, which has EXPIRED → FALLBACK). P4.15 step 1 said "EXPIRED is terminal except for FALLBACK" — a state with an outgoing transition is not terminal, and the sentence contradicts P4.10's terminal list and P4.18's FALLBACK-entry rule.
- **Fix applied:** reworded to "EXPIRED has one outgoing transition, to FALLBACK where the policy requires it (§14.1); it is not terminal."

### I4-07 — Low — P4.03 trace mislabels §12.6 as "(grid model version)"
- **File patched (in place):** `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md`, P4.03 Report trace.
- **Evidence:** §12.6 is the optimization result contract; "grid model version" is one field inside its return list, not the section's subject. The trace is not dangling (the heading exists) but misleads a reader about what §12.6 covers.
- **Fix applied:** trace now reads "§12.6 (result contract carries grid-model version)".

### I4-08 — Low — P5.14 step 4 cites the wrong rejection subtask for simulator dropout
- **File patched (in place):** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, P5.14 step 4.
- **Evidence:** a simulator asset dropout produces stale/missing telemetry and triggers re-scheduling and fail-closed rejection. The step cited "(P3.24, P4.22)" — P4.22 is the infeasible-command rejection suite (network-limit violations), the wrong gate for a dropout. The correct refs are P3.24 (rolling-horizon re-scheduling) and P4.08 (fail-closed) / P4.21 (stale-command rejection).
- **Fix applied:** citation corrected to "(P3.24, P4.08, P4.21)".

### I4-09 — Low — P6.28 fault drill overlaps P4.31/P5.21 with no non-duplication cross-reference
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.28 (step 4 added).
- **Evidence:** P6.28 drills dropped/reordered/duplicated messages, clock skew, stale telemetry, unavailable assets, partial fleet response — the same fault set as P4.31 and P5.21. P5.21 step 4 explicitly cross-references P4.31 ("reuse, do not duplicate"); P6.28 had no equivalent, so the same platform suites could be re-run (and drift) at the edge layer. The overlap is layered testing, not duplication — the missing piece is the boundary statement.
- **Fix applied:** step 4 added to P6.28: reuse P4.31/P5.21 suites where they already cover a fault; this drill adds only the edge/adapter layer.

### I4-10 — Medium — P6.25 step 4 contradicts P6.14: console "enabling the test-env command path"
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.25 step 4.
- **Evidence:** P6.14 requires the command path to be enabled only by "an explicit, signed, per-site configuration change" under a written gate with named approvers and expiry, and states "no plugin, adapter, or optimizer can enable the path" (§5.7/§21.4: mode is not selectable by software). P6.25 step 4 listed "enabling the test-env command path" as a console high-impact action requiring confirmation — implying a console action can flip it, which weakens the P6.14 gate into a UI confirmation.
- **Fix applied:** step 4 reworded: the console must never enable the path directly; it may only display the P6.14 gate state and record a request for the signed configuration change, which takes effect only through the P6.14 procedure.

### I4-11 — Low — P6.23 title overclaims "plugin escape" (no plugin exists in Phase 6)
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.23 title + step 5 note.
- **Evidence:** title: "Security testing: replay, forgery, malicious telemetry, plugin escape". Steps 1–5 test forgery/replay, malicious telemetry, auth bypass, secrets exposure — never plugin escape. §19.2 layer 7 does include "plugin privilege escalation", but there is no plugin host in Phase 6 (WASM host is v0.5.0 / Stage 7); the title promises a test the phase cannot run.
- **Fix applied:** retitled to "Security testing: replay, forgery, malicious telemetry, secrets exposure" with a note that plugin-escape testing belongs to Stage 7 (P7, WASM host) and is not claimed here. The subtask index row for P6.23 was updated to match.

### I4-12 — Medium — P6.07's "pin TLS versions/cipher policy per the D6 review" has no D6 source: P6.03 never records a TLS floor
- **File patched (in place):** `ledger/phase-06-edge-protocol/ledger_phase_6.md`, P6.03 step 3 (addition).
- **Evidence:** P6.07 step 3 requires pinning "TLS versions/cipher policy per the D6 review". P6.03's steps record zones, flows, connection pattern, certificates/firewall/segmentation — but never a TLS minimum version or cipher policy. The input P6.07 depends on is never produced, so its acceptance criterion ("Pin TLS versions/cipher policy") is unverifiable as written. The report itself names no TLS version, so this is a ledger-internal producer/consumer gap, not a report contradiction.
- **Fix applied:** P6.03 step 3 extended: the D6 review must record the TLS minimum version and cipher policy for the mTLS channel, consumed by P6.07.

### I4-13 — Low — P5.27 lists all four control-authority modes without the Phase 5 restriction
- **File patched (in place):** `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`, P5.27 step 1.
- **Evidence:** P5.27 step 1 says "Read the mode (Simulation / read-only/shadow / operator-approved / bounded automation) from deployment + per-site configuration". P5.02 step 3 and P5.13 restrict this phase to Simulation / read-only/shadow. P5.27 step 4 already says no promotion is in scope, so the tension is small — but a reader of P5.27 alone could conclude operator-approved mode is configurable in Phase 5.
- **Fix applied:** step 1 now notes "in this phase the configured mode is Simulation or read-only/shadow only (P5.02); the other modes are listed only as the configuration vocabulary".

## (c) Suggestions

- **I4-S1 (Low):** P5.26 traces "§17.2–§17.4" — §17.2 (security zones) is only loosely relevant to console RBAC; a tighter trace would be §17.3–§17.4 + §16.2. Not patched: the trace resolves and the extra section is harmless. Leave for the phase lead.
- **I4-S2 (Low):** P4.27's "six suite outputs plus the combined suite" phrasing (see Iteration 3, I3-C2) is also a mild traceability wrinkle — "six rejection categories" vs seven suites. Not patched; noted in both iterations so it is not re-raised as new.
- **I4-S3 (Low):** the P6 ledger's subtask-index table duplicates the heading traces (good for audit) but will drift again if a trace is edited in only one place. Suggest a future lint: index traces must equal heading traces. Not implemented here (tooling, out of scope).

## (d) Stale research

None. Research staleness is Iteration 3's topic; for traceability purposes the only research-linked finding is I4-12 (P6.07's TLS input), which is a producer/consumer gap in the ledger, not stale research. P6 RESEARCH.md's open tasks (OpenADR 3.1, OCPP Edition 2 dates) are correctly labelled unverified and are already gated by P6.01/P6.12/P6.19 as written.

## (e) Severity counts (Iteration 4)

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 0 |
| Medium | 6 (I4-01, I4-02, I4-04, I4-06, I4-10, I4-12) |
| Low | 7 (I4-03, I4-05, I4-07, I4-08, I4-09, I4-11, I4-13) |

## (f) Patch log — applied vs deferred

**Applied (all in-place edits, no new subtask IDs — traceability fixes correct existing text):**

| File | Fix |
|---|---|
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | Trace summary: §14.1–§14.2 → P4.10–P4.13, P4.15–P4.16; §14.3 → P4.14 (I4-01). Cross-phase references: P5 ranges corrected (I4-02). P4.03 trace relabelled (I4-07). P4.15 step 1 reworded — EXPIRED not terminal (I4-06). |
| `ledger/phase-05-operator-console-simulator/ledger_phase_5.md` | Trace summary: three workspace rows corrected (I4-01). Cross-phase references: HELICS → P5.16 (I4-03). P5.14 step 4 citation → P3.24, P4.08, P4.21 (I4-08). P5.27 step 1 Phase 5 mode restriction noted (I4-13). |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | P6.24 trace: §19.2(4)(10) → §19.2(4) (I4-04). P6.11 trace: §3.4 steps 2–5 → 2–4 (I4-05). P6.25 step 4: console cannot enable command path (I4-10). P6.23 retitled, index row updated (I4-11). P6.03 step 3: TLS minimum version + cipher policy recorded for P6.07 (I4-12). P6.28 step 4: reuse P4.31/P5.21 suites (I4-09). |

**Deferred:** I4-S1 (trace tightening, harmless as written); I4-S2 (already logged in Iteration 3); I4-S3 (index/heading drift lint — tooling, out of scope for this review).

## What was re-checked and found clean (with method, so the clean verdicts are auditable)

- **§-trace resolution (script):** every `§N[.M]` in all 11 phase ledgers resolves to a real report heading. Zero dangling references in any phase, including P4–P6.
- **Cross-phase ID resolution (script, defined headings only):** every `P<N>.<MM>` mentioned in P4–P6 resolves to a defined subtask heading in its target phase. Zero dangling IDs. (An earlier naive check against mere mentions was re-run against defined headings to avoid false negatives.)
- **Duplicate subtasks (script):** no duplicate subtask titles across all 11 phases (first-60-chars normalised comparison). Fault-injection overlap P4.31/P5.21/P6.28 is layered (platform / simulator-runner / edge), not duplicated; P5.21 already cross-references P4.31, and P6.28 now does too (I4-09).
- **§14.1 transition diagram vs P4.10 (hand check):** all 14 states present; every diagram transition enumerated (PROPOSED→VALIDATED/REJECTED; VALIDATED→FEASIBLE/REJECTED/SUPERSEDED; FEASIBLE→APPROVED/REJECTED/SUPERSEDED; APPROVED→ISSUED/EXPIRED/SUPERSEDED/CANCELLED; ISSUED→ACKNOWLEDGED/FAILED/EXPIRED/SUPERSEDED/CANCELLED; ACKNOWLEDGED→TRACKING/FAILED/EXPIRED/SUPERSEDED; TRACKING→COMPLETED/FAILED/EXPIRED/SUPERSEDED; FAILED/EXPIRED→FALLBACK); terminal set correct.
- **§15.3 events vs P4 (hand check):** every event P4.30/P4.07/P4.11/P4.15 claims to emit (`DispatchProposed/Approved/Rejected/Issued/Acknowledged/TrackingUpdated/Completed/Failed/Expired/Cancelled/Superseded/FallbackActivated`, `FeasibilityCheckCompleted`, `ScheduleValidated`) exists verbatim in §15.3.
- **§17.5 invariants vs P4.29 (hand check):** all 10 invariants mapped, 1–9 with tests, 10 as a documented boundary note. Correct.
- **§20.1 vs P5 (hand check):** all 12 workspaces have an owning subtask (P5.02–P5.09); only the trace-summary mapping was wrong (I4-01), not the coverage.
- **Solver classes (grep):** no HiGHS-as-SOCP, no Clarabel-as-MISOCP, no AC-power-flow-labelled-AC-OPF, no IPOPT-global claim in P4–P6. P4.09 Tier 2 SOCP + Clarabel boundary matches §12.3 exactly. HELICS role stated correctly in P5.16 ("coordinates federates; does not replace their physics models", §12.4/§19.1).
- **Control authority (grep):** consistent everywhere — Simulation in 0.x, operator-approved v1.0.0 default, bounded automation post-1.0 per site/asset class, plugin never selects mode. P6 header "Simulation-only until this phase's exit gate is met" is consistent with §2.1 v0.4.0 ("Read-only or test-lab control by default"): read-only begins after the gate.
- **Units/sign (grep):** MW/MVAr/MWh, `P > 0` export, SOC fraction in [0,1], six quality states — consistent across P4–P6 and with §10.2/§10.4. P4.04's "reject kW/kWh at the boundary" is consistent with §10.2 (kW allowed only as UI display, never in persisted/API contracts); no P5 subtask claims kW display, so no contradiction. SOC vs SoC spelling follows §10.2 ("SOC") throughout; §14.2's "SoC" variant is the report's own inconsistency, not the ledger's.
- **Certification language (grep):** no "certified/conformant/compliant" claim in P4–P6 except the bounded P6.19 claim-boundary statement and its grep-verification step. Clean.
- **P6 subtask index vs headings (hand check):** all 30 rows match their heading traces; updated for P6.23's retitle.
- **§8.1 paths cited (hand check):** `web/console/`, `apps/sim-runner`, `plugins/builtin/asset-simulator`, `flexcore-grid-api`, `apps/edge-agent`, `crates/adapters/<protocol>/`, `docs/operations|protocols|security` all exist verbatim in the §8.1 tree.
- **P6.14 vs P6.24 (hand check):** the bounded test-env command test in P6.24 step 3 ("if and only if the test-env gate was explicitly approved") is consistent with the §23 Stage 6 exit gate wording. Clean.
