<!-- P9.14 — Lab integration test, read-only end-to-end (decision record). EXECUTION TEMPLATE ONLY: every run/result cell EMPTY, status UNRUN. No lab environment exists; no pilot configuration exists (P9.01 freeze UNSIGNED, P9.04/P9.05 templates); the command path was never attempted anywhere because no environment in which to attempt it exists. -->
# P9.14 — Lab Integration Test — Read-Only End-to-End (Execution Template)

Date: 2026-11-14. Subtask: P9.14 (`ledger_phase_9.md` §P9.14).
Owner role: Integration engineering (with platform engineering).
Report trace: §23 Stage 9; §19.2 (test layers, in particular (3) contract
tests, (4) integration tests, (6) fault injection, (9) shadow-mode
evaluation); §23 Stage 6 exit gate (re-verified here for the pilot
configuration); §21.4 (read-only/shadow ceiling).
Inputs: P9.02–P9.06 (all TEMPLATES/OPEN — no site, no adapters, no
network design, D6 OPEN), P9.09 (no PKI exists in the 0.x line — P6.34
carried status), P9.10, P9.13 (plan template only), a lab environment
matching the pilot adapter/protocol versions (**does not exist — §4**),
the Stage 6 exit-gate records (`ledger/phase-06-edge-protocol/
24_exit_gate_lab_run_record.md` — the Stage 6 lab run is UNRUN).

## 1. Honest status — the plain statement

**No lab integration test has been run, and none can be run: no lab
environment exists.** There is no lab host, no control-plane deployment,
no OpenEMS (or other EMS) instance, no edge-agent build under test, no
CA/certificates, no pilot grid model, and no pilot configuration — the
inputs that name them (P9.01 freeze, P9.04 adapter inventory, P9.05
network design, P9.03 model version) are all UNSIGNED/EMPTY templates.
The Stage 6 exit-gate lab run this test re-verifies was itself never run
(`24_exit_gate_lab_run_record.md`: PREPARED/UNRUN). This record delivers
what an agent can deliver under ENTRY_NOTE boundary 6:

- the test-case list **derived from the ACTUAL code paths** (§3 — every
  case cites the file that implements the behaviour it exercises);
- the environment requirements — what must exist and currently does not
  (§4, all absent);
- the run record as an **EMPTY template** with pass/fail semantics
  defined BEFORE the run (§6 — the semantics are fixed here so they
  cannot be fitted to results afterwards);
- the §19.2(6) fault-injection subset mapped to named code seams (§3
  part C, aligned with the P8.24 fault vocabulary by citation);
- the honest claim boundary: what a passing lab test would and would not
  establish (§7 — a lab is not a site; §2.3 conformance wording).

Nothing was executed in this loop (loop bans): no cargo command, no
build, no test run, no network connection. Every result cell in §6 is
EMPTY on purpose and stays EMPTY until the human execution fills it from
actual run output.

## 2. The read-only ceiling — the rule the whole test enforces

The lab configuration under test is the §21.4 read-only/shadow posture:

1. Telemetry flows end-to-end: device → adapter → edge → control plane →
   operator console (§19.2(4) integration layer).
2. The command path is **DISABLED by default** in the lab configuration;
   the test ATTEMPTS a command and confirms it is rejected BEFORE issue,
   with the rejection audited (ledger step 3). A command accidentally
   enabled in the lab and treated as evidence the field path works is the
   ledger's named failure mode — §6's pass semantics therefore make any
   observed command REACHING an adapter an automatic FAIL of the whole
   run, not a partial result.
3. Proposed actions in shadow are compared, never issued (§19.2(9));
   acknowledgement is not physical delivery (§14.1) and no command
   lifecycle state beyond the refusal is reachable in this configuration.

This re-verifies the §23 Stage 6 exit gate ("read-only end-to-end site
integration works; the command path stays disabled unless explicitly
approved for a test environment") for the PILOT configuration — the
Stage 6 gate itself was NOT HELD (recorded exception,
`01_pilot_scope_freeze.md` §2); a passing P9.14 run upgrades the evidence
for the pilot configuration only, never the Stage 8-wide exception.

## 3. Test cases derived from the actual code paths

Every case below names the file(s) implementing the behaviour it
exercises, verified by reading on 2026-11-14. "Expected" cells are DESIGN
statements from the cited code — they are NOT results and not claims
that the behaviour works until the human run says so.

### Part A — telemetry end-to-end (ledger step 2; §19.2(4))

| Case | Code path exercised (cited) | Expected (design-derived, UNRUN) | Verifies at each boundary |
|---|---|---|---|
| L1 | `crates/edge-agent/src/adapters/openems.rs` → `src/ingest.rs` (`AdapterIngestSource::new`/`admit`) → `src/uplink_mtls.rs` (outbound mTLS channel) → control-plane ingest → operator console | A sample generated at the EMS fixture reaches the operator console with identity, units, and quality intact | **device→adapter**: fixture shape per `openems-mapping-profile.v1.json`; **edge**: dedup admission (`EdgeDedupWindow`); **transport**: mTLS, scheme `wss://`/`https://` enforced (`MtlsUplinkConfig::validate`); **console**: display shows the delivered sample |
| L2 | `adapters/openems.rs` ACL mapping (frozen table; `OPENEMS_STALE_CODE` → canonical `STALE`) | Each telemetry value carries EXACTLY ONE §10.4 quality state; degraded never upgraded to GOOD | adapter→edge mapping; edge→console passthrough (suite case `contract_quality_passthrough_no_upgrade` in `crates/edge-agent/tests/conformance_suite_p6_19.rs`) |
| L3 | `openems.rs` unit/sign mapping incl. the DECLARED reactive-power sign per device profile (P9.04 §7; profile `assumptions` array is UNVERIFIED — A-register rows A1–A8, `37_adapter_conformance_pilot.md` §5) | Values arrive in canonical units (MW/MVAr/MWh, SOC [0,1], §10.2) with declared signs | adapter mapping; console display (suite case `contract_units_and_acl_conformance`) |
| L4 | `ingest.rs` `EdgeDedupWindow` / `AdmitOutcome` | A duplicated sample within the window is admitted as duplicate (recorded), never silently re-ingested as fresh; out-of-window handled per config | edge ingest boundary |
| L5 | `adapters/openems.rs` stale stamping: readings older than `stale_after_millis` stamped STALE at read time against the edge clock | Stale reading carries STALE end-to-end; freshness limit honoured | adapter→edge→console |
| L6 | `uplink_mtls.rs` + `buffer_file.rs` (`FileLocalBuffer`) + `audit_buffer.rs` (`FileAuditBuffer`) | Uplink down → telemetry buffered in order (bounded, at-least-once); restored → in-order upload; drops audited; no new commands while disconnected | edge offline behaviour (§18.1) |
| L7 | `adapters/registry.rs` + `adapters/acl.rs` | Only registry-admitted, allowlisted adapters deliver; an unregistered adapter source is refused (`adapter_acl_and_registry.rs` register) | edge admission boundary |

### Part B — command path disabled by default (ledger step 3)

| Case | Code path exercised (cited) | Expected (design-derived, UNRUN) |
|---|---|---|
| L8 | `crates/flexcore-domain/src/command_state.rs` (closed transition table; no `PROPOSED → ISSUED` shortcut) + control-plane gates (`crates/flexcore-application/src/command_gates.rs`, `dispatch_security.rs`) | A command attempt in the lab default configuration is refused BEFORE `ISSUED`; the refusal is recorded on the command record and in the audit trail (`crates/flexcore-audit`) |
| L9 | `crates/edge-agent/src/command_path.rs` (`CommandPathRuntime`; enablement computed once by a private constructor, re-derived read-only via `enablement_at`; NO setter exists) | The edge's command-path enablement is OFF in the lab default config; inspection shows no enablement surface that configuration alone can flip without the P6.14 four-condition gate (incl. the signed site D6 record — D6 OPEN ⇒ unflippable today) |
| L10 | `command_path.rs` + `command_forward.rs` (`CommandForwardingEngine::forward_verified` — the ONLY route to an adapter requires a `VerifiedCommand`) | Even a gate-admitted command cannot reach an adapter without passing `SitePolicyEngine::admit_command` (`policy.rs`) and the adapter allowlist; in the disabled lab config, zero adapter reach |

### Part C — §19.2(6) fault-injection subset applicable to the lab (ledger step 4)

The subset the ledger names, mapped to the fault vocabulary recorded by
P8.24 (`ledger/phase-08-advanced-coordination/24_failure_drill.md` F1–F8)
and to the lab-representable code seams. The P8.24 record owns the
research-module drill; THIS record exercises the fault set on the
integration path (edge + control plane). Where P8.24 recorded a coverage
limitation (C-1 transport-level reorder; C-2 process kills), the same
limitation applies here and is inherited BY CITATION, not re-solved.

| Case | Fault (P8.24 vocabulary) | Code seam exercised (cited) | Expected fail-closed behaviour (design-derived, UNRUN) |
|---|---|---|---|
| F11 | Dropped message (F1) | `ConformanceCase` closed set: `truncated`, `malformed` (`crates/edge-agent/src/conformance.rs`; harness driven per adapter) | Refused with a named error; nothing partially applied; the dropped sample is MISSING/absent, never a silent wrong value |
| F12 | Duplicated message (F1) | `ConformanceCase::duplicate_delivery`; ingest dedup L4 | Duplicate visible and handled per the dedup window; no double-count as fresh |
| F13 | Reordered message (F1; P8.24 C-1 inherited) | Transport-level at the lab composition root; `conformance.rs` `out_of_order` case | Refused/recorded per the seam; never silently accepted as in-order |
| F14 | Stale telemetry (F4) | `openems.rs` freshness stamping; suite case `stale_data` | STALE carried end-to-end; no upgrade; downstream consumers see the degraded state |
| F15 | Unavailable asset (F5) | Adapter/registry: source for a non-admitted asset refused (`adapters/registry.rs`); control plane: unknown asset context refused per §17.5 invariant 1 | Refusal recorded; no fabricated values for the absent asset |
| F16 | Solver timeout (F6) | Control-plane scheduling/feasibility path (`crates/flexcore-application/src/command_gates.rs`; P8.24 M-V seam rule: a failed gate is a REFUSAL, never a pass) | Timeout produces a recorded failure state; no schedule/feasibility evidence is invented |
| F17 | Clock skew (F3) | `clock_quality.rs` (`ClockQualityMonitor::expiry_check_allowed`; ANY quality other than `Synchronized` fails the command-expiry check CLOSED, rejection audited) | With a skewed/unknown clock, expiry checks fail CLOSED; commands rejected |
| F18 | Hostile telemetry / unmapped channel (F4 shape) | `ConformanceCase::unmapped_point`; ACL/registry (`adapter_acl_and_registry.rs`) | Unmapped channel classified per the classify-once register (P6.12), never silently mapped |

The full 9-case harness set (`valid`, `malformed`, `truncated`,
`out_of_order`, `duplicate_delivery`, `clock_skew_future`,
`clock_skew_past`, `stale_data`, `unmapped_point`) is exercised via the
suite (`suite_full_p6_13_fault_injection_set` and its
`_openems` variant — `conformance_suite_p6_19.rs`); the P9.31 attack
vocabulary (forged/replayed/expired/out-of-sequence/conflicting-
idempotency) is exercised by `crates/edge-agent/tests/
pilot_rejection_proof_p9_31.rs` as part B/C cross-coverage — its matrix
is WRITTEN-UNRUN and must pass its human build/test before this lab run
cites it.

### Part D — version/seed recording (ledger step 5)

Every case run records: edge-agent git SHA, control-plane version, EMS
version (re-pinned at execution — never assumed from RESEARCH.md's
2026.11.0 reference pin), mapping-profile version, grid-model version
(P9.03), harness/config identifiers, and any seed. The record template
(§6) has the columns; a run without them is not repeatable and fails
acceptance criterion (d).

## 4. Environment requirements — what must exist (NONE OF IT DOES)

| # | Requirement | Purpose | Status at writing |
|---|---|---|---|
| E1 | Lab host(s) able to run the control plane (PostgreSQL, NATS, API, console) | §19.2(4) integration environment | **DOES NOT EXIST** |
| E2 | A site-EMS instance at the PINNED pilot version (OpenEMS or as the filled P9.04 dictates) with a fixture/firmware shape matching the pilot | Adapter under test at pilot versions | **DOES NOT EXIST — no version pinned** |
| E3 | An edge-agent build (pinned git SHA) with the shipped `openems-mapping-profile.v1.json` (or a pilot variant created additively per P9.37 §3) | The edge under test | **NOT BUILT — loop bans; no SHA exists** |
| E4 | mTLS material: root CA, central certificate, edge credential issued via the enrollment flow (P9.33) | Secure uplink | **DOES NOT EXIST — no PKI in 0.x (P6.34 carried status)** |
| E5 | A network segment mirroring the P9.05 zones/allowlist (or an explicit, recorded simplification) | Test the documented-flows-only rule | **DOES NOT EXIST** |
| E6 | The frozen pilot grid model version (P9.03) for any feasibility-gated case | Deterministic model version | **DOES NOT EXIST — P9.03 template** |
| E7 | The fault-injection harness wired to the adapter under test (`ConformanceHarness`) | Part C | Present in code (WRITTEN-UNRUN); needs the built binary + EMS to drive |
| E8 | Version/seed recording procedure + log retention | Repeatable run | Procedure = §6 template; retention per P9.10 (unwritten) |
| E9 | A security witness for the command-path cases (B) | Trustworthy negative evidence | **NOT NAMED** |

Every row is an EMPTY cell in the §6 configuration record until filled at
execution. Standing up E1–E6 is the human/organizational gate; no agent
in this loop can create any of it.

## 5. Execution procedure (the human steps, in order)

1. Close the §3 input gates: signed P9.01 freeze; FILLED P9.04/P9.05;
   P9.03 frozen model; D6 closed for the (lab) deployment record per
   P9.06 §9; P9.13 commissioning plan reviewed; P9.09 secrets/certs
   ready (closes E4).
2. Build + run the WRITTEN-UNRUN suites against the lab build FIRST
   (commands in §8) — a suite failure stops the end-to-end run and is
   dispositioned before continuing.
3. Stand up E1–E6; pin every version; fill the §6 configuration record
   BEFORE the first end-to-end case.
4. Run Part A cases (telemetry), then Part B (command path — the
   security witness observes), then Part C (fault injection) — in that
   order, because Part B's negative evidence is only meaningful once
   Part A shows the healthy path.
5. Record every result from the ACTUAL run output into §6 the same day;
   any fault producing a silent wrong value is an automatic FAIL (§6
   semantics) and a fix-and-re-run, never a note.
6. File the lab report as the P9.14 evidence named in `CHECKLIST.md`
   ("Lab integration passed") only when every case has a result or a
   reasoned not-applicable and every failure has a disposition.

## 6. Run record — EMPTY template (pass/fail semantics fixed BEFORE the run)

**Pass/fail semantics (fixed now, before any run exists — acceptance
criterion semantics are pre-registered so they cannot be fitted to
results):**

- **PASS** = observed behaviour matches the case's expected behaviour,
  evidenced by the named artifact (log/capture/audit extract), with the
  full version/seed record present.
- **FAIL** = any deviation, and SPECIFICALLY: (i) any command reaching an
  adapter or device in any case — automatic run FAIL regardless of which
  case it occurred in; (ii) any fault producing a SILENT WRONG VALUE (a
  value delivered as fresh/GOOD that is not, or a refusal with no audit
  record) — automatic run FAIL; (iii) any case closed without a result
  or a reasoned not-applicable — run FAIL on completeness.
- **NOT-APPLICABLE** requires a written reason tied to the environment
  (e.g. "reorder not representable at this seam — P8.24 C-1 inherited"),
  never "covered by unit tests" — that shortcut is the ledger's named
  failure mode and is rejected in review.
- A failed case is dispositioned (fixed-and-re-run / accepted limitation
  filed to P9.27 / blocks the pilot); a failure without a disposition
  blocks P9.17/P9.28.

**Configuration record (fill BEFORE the run):** every cell EMPTY.

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Edge-agent build (git SHA) | |
| Control-plane version / components (PostgreSQL, NATS, API, console) | |
| EMS instance + version (re-pinned, installed) | |
| Mapping profile (file + version) | |
| Grid-model version (P9.03) | |
| CA/cert references (E4) | |
| Network segment description (E5) | |
| Harness config / seeds | |
| Run dates / executors / security witness (Part B) | **UNSIGNED** |

**Per-case result table:** every result cell EMPTY (UNRUN). One row per
§3 case:

| Case | Result (PASS / FAIL / NOT-APPLICABLE-with-reason) | Evidence artifact (log/capture reference) | Detail (from the actual output) |
|---|---|---|---|
| L1 | (EMPTY — UNRUN) | | |
| L2 | (EMPTY — UNRUN) | | |
| L3 | (EMPTY — UNRUN) | | |
| L4 | (EMPTY — UNRUN) | | |
| L5 | (EMPTY — UNRUN) | | |
| L6 | (EMPTY — UNRUN) | | |
| L7 | (EMPTY — UNRUN) | | |
| L8 | (EMPTY — UNRUN) | | |
| L9 | (EMPTY — UNRUN) | | |
| L10 | (EMPTY — UNRUN) | | |
| F11 | (EMPTY — UNRUN) | | |
| F12 | (EMPTY — UNRUN) | | |
| F13 | (EMPTY — UNRUN) | | |
| F14 | (EMPTY — UNRUN) | | |
| F15 | (EMPTY — UNRUN) | | |
| F16 | (EMPTY — UNRUN) | | |
| F17 | (EMPTY — UNRUN) | | |
| F18 | (EMPTY — UNRUN) | | |

**Failures-and-dispositions table:**

| Failed case | Disposition (fixed-and-re-run / accepted limitation filed to P9.27 / blocks the pilot) | Evidence | Status |
|---|---|---|---|
| (none — exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

## 7. What a passing lab test would and would not establish

Report §2.3, verbatim, governs every word written about a result:
*"v0.1.0 ships simulation adapters only. Implementing an adapter is not a
conformance claim; each production adapter must pass the relevant
conformance or interoperability suite before it is described as certified
or qualified."*

**A passing P9.14 run would establish ONLY:** that the named builds, on
the recorded lab versions and configuration, delivered read-only
telemetry end-to-end with quality/units verified at each boundary; that
the command path was demonstrably disabled in that configuration with
the refusal audited; and that every injected fault produced a recorded,
fail-closed outcome with no silent wrong value — in the LAB, on that
date, with the recorded evidence.

**It would NOT establish** (and none of these may be written about it):

1. **Site behaviour.** A lab is not a site: no customer network, no real
   EMS production configuration, no physical devices, no site staff, no
   real telemetry rates. Field behaviour is P9.13 (commissioning) +
   P9.17 (field acceptance, shadow first).
2. **Any conformance or certification claim** for any protocol or
   adapter (§2.3; AGENTS.md §3 rule 2). The permitted claim for the
   adapter rows is exactly the P6.19 §3 boundary wording: *"passes the
   FlexCore edge adapter conformance suite version 1 against the
   recorded OpenEMS REST API shape"* — no more.
3. **Performance at pilot scale** (§19.2(8) is P9.39's deliverable; lab
   numbers presented as pilot numbers is the ledger's named risk).
4. **Safety or protection-grade properties** (AGENTS.md §3 rules 2–3):
   the lab proves supervisory software behaviour, never grid safety.
5. **Equality of lab and field versions.** A later version/firmware bump
   invalidates the record and forces a re-run (the ledger's named risk:
   "lab adapter version differs from the field version").
6. **The Stage 1–8 exception upgraded.** A P9.14 pass is evidence for
   the pilot configuration; the recorded Stage 6 NOT-HELD exception is
   not retroactively "held".

## 8. Filling commands (for the human executor)

```sh
# (a) Documented-tools gate (before and after filling):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Build + run the edge suites against the LAB build (the log IS the run record):
cargo test -p flexcore-edge-agent --test conformance_suite_p6_19 -- --nocapture
cargo test -p flexcore-edge-agent --test conformance_p6_13 -- --nocapture
cargo test -p flexcore-edge-agent --test openems_p6_10 --test openems_e2e_p6_11 -- --nocapture
cargo test -p flexcore-edge-agent --test security_p6_23 -- --nocapture
cargo test -p flexcore-edge-agent --test pilot_rejection_proof_p9_31 -- --nocapture
#     ...plus the rest of the 21-file Phase 6 register per 37_adapter_conformance_pilot.md §2.2.

# (c) End-to-end lab run: stand up E1–E6, pin versions into §6, then drive
#     the L/F cases against the RUNNING lab deployment (fixture EMS + edge
#     agent + control plane). Record §6 per-case results from the ACTUAL
#     output the same day.

# (d) Claim-phrase check on the filled report (§7 discipline):
grep -n "certified\|conformant\|compliant\|safety" ledger/phase-09-pilot-qualification/14_lab_integration_test.md
#     The only permitted occurrences are inside the quoted §2.3/§7 boundary statements.
```

No cargo/build/test command has been executed in this writing loop
(banned); every run above is a human gate input whose actual output —
never this template — fills §6.

## 9. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged and NO executed-run claim
   exists anywhere in the record.
2. Confirm every §4 status cell and every §6 result cell is EMPTY /
   UNRUN; confirm the configuration record is unfilled.
3. Confirm the §6 pass/fail semantics are present and pre-registered
   (they precede any result cell by construction — the template ships
   empty); confirm the two automatic-FAIL rules (adapter reach; silent
   wrong value) are intact.
4. Traceability check (for a filled future record): every result cites
   an artifact that resolves; every version cell names the INSTALLED
   version; every NOT-APPLICABLE has a written environment reason (never
   "covered by unit tests"); every failure has a disposition.
5. Confirm §7's quotes (report §2.3; the P6.19 claim-boundary wording)
   are verbatim and no forbidden claiming phrase appears outside them
   (§8 (d) command).
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; the lab test has not run.

## 10. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No lab environment exists (E1–E9 all absent) — the §23 Stage 6 re-verification cannot start, and P9.15 (which depends on P9.14) and P9.39 are blocked behind it | §4 |
| Stage 6 lab run UNRUN: the pilot re-verifies an exit gate that was never run once — a P9.14 pass is the FIRST end-to-end evidence of its kind | §2; `24_exit_gate_lab_run_record.md` |
| No PKI exists (0.x): E4 credential issuance depends on the P9.33/P9.34 path being verified by the human build/test first; a lab run without real mTLS would test a degraded posture | §4 E4; P6.34 carried status |
| Lab adapter/EMS version drift vs field version — a bump silently invalidates the record | §7 item 5; ledger risk row |
| Fault injection skipped as "covered by unit tests" — prevented by the NOT-APPLICABLE rule, but the rule must be enforced by the reviewer at the filled record | §6 semantics; ledger risk row |
| A command accidentally enabled in the lab and misread as evidence the field path works — prevented by the automatic-FAIL rule, enforcement is human | §2, §6; ledger risk row |
| P9.04/P9.05/P9.03 are templates — the configuration under test is undefined until they are filled; any lab run before that would test an invented configuration | §3, §5 step 1 |
| A1–A8 OpenEMS REST assumptions remain UNVERIFIED until a lab execution diffs the real Edge (`37_adapter_conformance_pilot.md` §5) | §3 L3 |

Status: **WRITTEN-UNRUN** — a lab integration test cannot be produced by
an agent in this loop; no lab, no pilot configuration, no pinned version,
and no certificate material exists; every result cell is EMPTY and stays
EMPTY until the human execution fills it from actual run output. Nothing
was built, run, or connected, and no pass is claimed.
