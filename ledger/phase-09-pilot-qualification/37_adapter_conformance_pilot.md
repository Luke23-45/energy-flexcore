<!-- P9.37 — Adapter conformance suites executed in the pilot configuration (decision record). EXECUTION TEMPLATE ONLY: every suite-run result cell EMPTY, status UNRUN. No pilot exists; no suite has been executed against any pilot configuration; the A1–A8 register rows remain UNVERIFIED. Passing a suite is protocol-shape conformance against a recorded API shape, NOT product certification (AGENTS.md §3 discipline). -->
# P9.37 — Adapter Conformance Suites Executed in the Pilot Configuration (Execution Template)

Date: 2026-11-14. Subtask: P9.37 (`ledger_phase_9.md` §P9.37).
Owner role: Integration engineering.
Report trace: §23 Stage 9; §2.3; §19.2(3) (contract tests); §5.3.
Inputs: P9.04 integration design (DESIGN TEMPLATE, all adapter/version cells
EMPTY — `04_customer_integration_design.md`); P9.14 lab environment (NOT
EXECUTED); adapter/vendor documentation — **none of which exists yet**.

## 1. Honest status — the plain statement

**No suite has been executed in any pilot configuration, and none is
claimed.** This record delivers what an agent can deliver under ENTRY_NOTE
boundary 6: the DEFINITION of the conformance suites that exist and apply
(§2), the pilot-config-specific parameters the execution needs (§4 — all
EMPTY until P9.01/P9.04 fill them), and the per-suite EXECUTION record as an
EMPTY template (§6) whose fill is the human gate. The ledger's deliverable
is "per-adapter conformance execution report"; no such report exists because
no pilot adapter, version, profile, firmware level, or lab environment
exists. Every result cell below is EMPTY on purpose; nothing was run,
probed, or executed.

The A1–A8 UNVERIFIED API-shape register (§5) stays UNVERIFIED row-by-row
until the suite/lab execution verifies each row against a real Edge. No
verification is claimed, and no row is pre-marked verified.

## 2. The suite inventory — what exists and applies, recorded honestly

### 2.1 What FlexCore has (built, WRITTEN-UNRUN — Phase 6, carried)

| Suite / machinery | Identity | Where | Status |
|---|---|---|---|
| FlexCore edge-adapter conformance suite | `flexcore-edge-adapter-conformance-suite-v1` (the ONLY name a pass claim may cite — `SUITE_ID` in the test file) | `crates/edge-agent/tests/conformance_suite_p6_19.rs` (11 cases, per the P6.19 case→requirement map) | WRITTEN-UNRUN (Phase 6 record `19_23_22_conformance_security_performance.md` §2; CI job `edge-agent-suite` UNRUN) |
| §19.2(6) fault-injection harness (suite building block) | `ConformanceHarness` + `ConformanceCase` (9-case closed set: `valid`, `malformed`, `truncated`, `out_of_order`, `duplicate_delivery`, `clock_skew_future`, `clock_skew_past`, `stale_data`, `unmapped_point`) | `crates/edge-agent/src/conformance.rs` | WRITTEN-UNRUN; protocol-agnostic — driven per adapter by the caller |
| Edge security suite (P6.23) | full-pipeline attack cases (forgery/replay/expiry/sequence/allowlist/scope; hostile-telemetry ACL cases; secrets redaction) | `crates/edge-agent/tests/security_p6_23.rs` | WRITTEN-UNRUN |
| Load harness (P6.22) | `flexcore-edge-perf-record/v1` print; structural assertions only | `crates/edge-agent/tests/perf_harness_p6_22.rs` | WRITTEN-UNRUN; P9.39 owns pilot-scale measurement, NOT this record |
| OpenEMS adapter contract/e2e tests | fixture-shape contract + read-path reconciliation + stale mapping + GET-only scan | `crates/edge-agent/tests/openems_p6_10.rs`, `crates/edge-agent/tests/openems_e2e_p6_11.rs` | WRITTEN-UNRUN |
| Supporting Phase 6 integration suites (the full 21-file register) | see §2.2 | `crates/edge-agent/tests/` | WRITTEN-UNRUN |

**The official-suite finding, carried verbatim from P6.19 §1:** NO official
protocol conformance suite applies to the OpenEMS REST channel as far as the
checked sources go (UNVERIFIED-ABSENCE, checked 2026-11-07). The suite
FlexCore runs is therefore 100 % FlexCore-side. OpenADR 3.0, OCPP 2.0.1/2.1,
SunSpec Modbus, and IEEE 2030.5/CSIP suites exist for OTHER protocols and
are later-adapter obligations (report §2.3; certification is separate, report
[R27]/[R29]) — they do not apply to any pilot adapter until P9.04 §5 names
them (cells EMPTY).

### 2.2 The 21 Phase 6 integration test files (the register this suite executes)

Exactly as present in `crates/edge-agent/tests/` — all WRITTEN-UNRUN, and
all part of what "executed in the pilot configuration" means at the edge
layer:

`adapter_acl_and_registry.rs`, `audit_buffer_p6_18.rs`, `buffer_p6_09.rs`,
`clock_quality_p6_31.rs`, `command_forward_p6_16.rs`,
`command_path_p6_14.rs`, `command_verify_p6_15.rs`, `config_and_runner.rs`,
`conformance_p6_13.rs`, `conformance_suite_p6_19.rs`, `decommission_p6_32.rs`,
`enrollment_flow.rs`, `ingest_p6_08.rs`, `no_inbound_listener.rs`,
`openems_e2e_p6_11.rs`, `openems_p6_10.rs`, `perf_harness_p6_22.rs`,
`policy_health_p6_17.rs`, `security_p6_23.rs`, `tenant_isolation_p6_21.rs`,
`uplink_mtls_channel.rs`.

(The Phase 9 edge-security additions — `pilot_rejection_proof_p9_31.rs`,
`audit_integrity_proof_p9_32.rs`, `enrollment_ceremony_p9_33.rs`,
`update_verification_p9_34.rs` — are P9.31–P9.34's deliverables, not this
record's suite rows; their execution status is owned there.)

## 3. Scope rule — no adapter enters this record without the signed freeze

Identical to P9.04 §3: a suite-execution row in §6 exists only for an
adapter named by the signed P9.01 freeze and designed by the FILLED P9.04
document (adapter version, profile/variant, firmware level recorded against
INSTALLED versions, never datasheets). A row filled without those inputs is
a reviewer rejection (§8, check 3). The shipped OpenEMS mapping profile
`crates/edge-agent/config/openems-mapping-profile.v1.json` is the ONLY
profile that exists; **no pilot-profile variant is created by this record**
— creating one before P9.01/P9.04 supply real site/asset/channel data would
fabricate pilot configuration. If the filled P9.04 dictates a pilot variant
(e.g. corrected sign conventions after the A-register verification), it is
created ADDITIVELY (e.g. `openems-mapping-profile.v1.pilot.json`) and the
existing v1 file is never modified (P6.10 D-10.1: profile edits are
versioned, reviewed changes).

## 4. Pilot-config-specific parameters — ALL EMPTY until P9.01/P9.04 fill them

| Parameter | Required value for the execution | Status |
|---|---|---|
| Adapter endpoints | the pilot Edge's actual base URL(s), composed only over the reviewed transport posture (P6.10 D-10.7: `http://` composes today; TLS-on-OpenEMS-hop is an open item) | `<EMPTY — none exists>` |
| Mapping profile / variant | the profile file id + version the pilot runs (shipped v1, or a pilot variant created additively per §3) | `<EMPTY — v1 is the only file that exists; pilot variant NOT created>` |
| Adapter software version | the edge-agent build (git SHA) the pilot runs | `<EMPTY>` |
| OpenEMS version | the version the pilot Edge ACTUALLY runs (never assumed from RESEARCH.md's 2026.11.0 — re-pinned at execution) | `<EMPTY>` |
| Firmware levels | installed firmware per device/adapter, fleet-confirmed | `<EMPTY>` |
| Expected point behaviours | per-channel expected outcome table: unit, sign (incl. the reactive-power sign DECLARED per device profile — never assumed, P9.04 §7), declared fresh-reading quality, freshness limit, expected register class (MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS) for every discovered channel | `<EMPTY>` |
| Lab environment (P9.14) | the P9.14 lab-first execution environment, then the pilot configuration where the suite supports it | `<EMPTY — P9.14 NOT EXECUTED>` |

## 5. The A1–A8 UNVERIFIED register — carried verbatim, rows UNVERIFIED

From `ledger/phase-06-edge-protocol/10_11_openems.md` §2 (the OpenEMS REST
API was never executed when the adapter was written). **Status stays
UNVERIFIED until a P9.37/P9.14 execution verifies each row against a real
Edge; no row is marked verified here and none may be quoted as verified
elsewhere.**

| # | Assumption | Verify at execution by | Status |
|---|---|---|---|
| A1 | EdgeConfig is fetched at `GET /rest/edge/config` | Diff the real endpoint path (the Edge may instead expose JSON-RPC over WebSocket) | UNVERIFIED |
| A2 | EdgeConfig `components` is an object keyed by component id; each component carries `factoryId`, `alias`, and a keyed `channels` object | Dump the real EdgeConfig and diff the shape | UNVERIFIED |
| A3 | Each channel entry carries an optional `unit` string and human text fields | Diff the real channel metadata keys (`unit` vs `unitName` vs none) | UNVERIFIED |
| A4 | A channel value is fetched at `GET /rest/channel/{component}/{channel}` | Diff the real per-channel read path | UNVERIFIED |
| A5 | The channel-value response is an object with numeric `value` and numeric `timestamp` (Unix epoch ms) | Capture a real response; confirm timestamp unit and field names | UNVERIFIED |
| A6 | REST channel values carry NO quality code; the profile declares the fresh-reading code | Confirm whether OpenEMS exposes any channel validity/quality information; extend the mapping explicitly if it does | UNVERIFIED |
| A7 | Component/channel ids are REST-path-safe (`[A-Za-z0-9._-]`) | Check real id character sets | UNVERIFIED |
| A8 | The Edge's clock-synchronisation status is UNKNOWN | Record the Edge's NTP/synchronisation state | UNVERIFIED |

The profile's own sign declarations (`ess0` ActivePower export-positive,
`meter0` import-positive, reactive injection-positive) are UNVERIFIED-
ASSUMPTIONS inside `openems-mapping-profile.v1.json`'s `assumptions` array;
the execution's sign findings are PROFILE EDITS (versioned, reviewed), never
code changes and never silent flips.

## 6. Per-suite EXECUTION record — EMPTY template (the human gate input)

**EVERY cell EMPTY. No case has run; no pass, fail, or not-applicable
exists. Filling a cell from anything other than the actual run output of the
actual pilot versions is fabrication (AGENTS.md rule 1).**

One table per pilot adapter (a row exists only per §3):

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Adapter id (registry id, e.g. `openems-rest`) | |
| Pilot site id (from the signed P9.01 freeze) | |
| Adapter software version (edge-agent git SHA) | |
| Mapping profile / variant (file + version) | |
| OpenEMS / device firmware version (INSTALLED, fleet-confirmed) | |
| Environment (P9.14 lab, or pilot config — ledger step 1 runs lab FIRST) | |
| Suite id executed (`flexcore-edge-adapter-conformance-suite-v1`) | |
| Suite file(s) executed (from §2.2) | |
| Execution date / executor / witness | |
| Run log reference (file/commit — the log IS the run record, P6.19 §2) | |

Per-case result table (one row per suite case; result ∈ {passed, failed,
not-applicable-with-reason}):

| Case (suite / §2.1 identity) | Requirement | Result | Detail (from the actual output) |
|---|---|---|---|
| `contract_capability_honesty` | §19.2(3) full REQUIRED capability declarations | (EMPTY — UNRUN) | |
| `contract_named_error_refusals` | §19.2(3) named errors, nothing partially applied | (EMPTY — UNRUN) | |
| `contract_quality_passthrough_no_upgrade` | §19.2(3) + §10.4 degraded never → GOOD | (EMPTY — UNRUN) | |
| `contract_units_and_acl_conformance` | §19.2(3) + §10.2 canonical units/signs | (EMPTY — UNRUN) | |
| `contract_identity_source_id_discipline` | §19.2(3) + P2.04 D-T6 identity shape | (EMPTY — UNRUN) | |
| `contract_unmapped_channel_register` | §19.2(3) + P6.12 step 2 classify-once | (EMPTY — UNRUN) | |
| `suite_full_p6_13_fault_injection_set` | §19.2(6) full 9-case set (`simulated-test`) | (EMPTY — UNRUN) | |
| `suite_full_p6_13_fault_injection_set_openems` | §19.2(6) same set via `openems-rest` fixture shape | (EMPTY — UNRUN) | |
| `flexcore_only_read_only_structural_guarantee` | FLEXCORE-ONLY — no write surface, zero write attempts | (EMPTY — UNRUN) | |
| `flexcore_only_registry_allowlist_conformance` | FLEXCORE-ONLY — registry allowlist status honest | (EMPTY — UNRUN) | |
| `suite_record_renders_suite_and_claim_boundary` | FLEXCORE-ONLY — run record carries suite version + pin + bounded claim | (EMPTY — UNRUN) | |

Failures-and-dispositions table (ledger step 3 — a failed case with no
disposition blocks P9.28):

| Failed case | Disposition (fixed-and-re-run / accepted limitation filed to P9.27 / blocks the pilot) | Evidence | Status |
|---|---|---|---|
| (none — the table exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

A1–A8 verification outcome table (filled ONLY from the execution's actual
diffs/captures):

| Register row | Verified? | Finding (diff/capture reference) | Forced profile edit (if any) |
|---|---|---|---|
| A1 … A8 (one row each) | (EMPTY — UNRUN; stays UNVERIFIED until this row is filled) | | |

## 7. What an execution would and would not establish (AGENTS.md §3 discipline, verbatim)

Report §2.3, quoted: *"v0.1.0 ships simulation adapters only. Implementing
an adapter is not a conformance claim; each production adapter must pass the
relevant conformance or interoperability suite before it is described as
certified or qualified."*

AGENTS.md §3, verbatim, and binding on every word of a filled execution
report:

> 2. **No safety or certification claims.** FlexCore is not certified, not a
>    safety case, and not permission for live grid control. Never write text that
>    implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory orchestration
>    layer. It does not perform primary frequency response, synthetic inertia,
>    relay protection, inverter inner-loop control, or sub-cycle control.

A P9.37 execution, if run, would establish ONLY: that the named adapter
build, against the named installed versions and the recorded protocol shape,
honours the FlexCore edge read-path contract under the §19.2(6) fault set
and the P6.19 case map, in the recorded environment, on the recorded date.

It would NOT establish any of the following, and none may be written about
it: certification or conformance of any protocol (the P6.19 §3 claim
boundary applies verbatim — the only permitted claim is *"passes the
FlexCore edge adapter conformance suite version 1 against the recorded
OpenEMS REST API shape"*); safety of any control action (AGENTS.md §3 rule
2); protection-grade or sub-cycle capability (rule 3); correctness of the
pilot site's physical plant, the inverter firmware, or the site itself
(§11.3: passing a suite is not a certification claim for the adapter, the
inverter, or the site); fitness of an adapter version NOT under test — a
later firmware bump invalidates the record and forces a re-run (the ledger's
named risk: a suite run against a different adapter version than the
pilot's). A suite "passed" by skipping failing cases without recording them
is not a pass at all; every case carries a result or a reasoned
not-applicable (ledger acceptance criterion b).

## 8. Reviewer-verification procedure

1. Confirm §1's plain statement is unchanged and no executed-run claim
   exists anywhere in the record.
2. Confirm every §4 parameter cell and every §6 result cell is EMPTY /
   `(EMPTY — UNRUN)`, and every §5 row reads UNVERIFIED.
3. Traceability check (for a filled future record): every executed row
   traces to the signed P9.01 freeze and the FILLED P9.04 design; every
   version cell names the INSTALLED version; every case has a result or a
   reasoned not-applicable; every failure has a disposition; the run log
   reference resolves.
4. Confirm §7's quotes (report §2.3; AGENTS.md §3 rules 2–3; the P6.19
   claim-boundary sentence) are verbatim, and grep the filled report for
   forbidden claiming phrases per §9 command (c).
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no suite has been executed.

## 9. Filling commands (for the human executor)

```sh
# (a) Build + run the conformance suite (lab-first, P9.14; the log IS the run record):
cargo test -p flexcore-edge-agent --test conformance_suite_p6_19 -- --nocapture
cargo test -p flexcore-edge-agent --test conformance_p6_13 -- --nocapture
#     against the pilot/fixture-shape adapter:
cargo test -p flexcore-edge-agent --test openems_p6_10 --test openems_e2e_p6_11 -- --nocapture
#     supporting Phase 6 register files (§2.2) as needed for the executed rows:
cargo test -p flexcore-edge-agent --test security_p6_23 -- --nocapture

# (b) Re-run against the PILOT configuration where the suite supports it
#     (ledger step 1), with the pilot adapter version, profile/variant, and
#     firmware version recorded in §4/§6 BEFORE the run.

# (c) Claim-phrase check on the filled report (§7 discipline):
grep -rn "certified\|conformant\|compliant" docs/ ledger/phase-09-pilot-qualification/37_adapter_conformance_pilot.md
#     The only permitted occurrences are inside the quoted boundary statements.

# (d) The documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

No cargo command has been executed in this writing loop (banned); every run
above is a human gate input, and its actual output — never this template —
fills §6.

## 10. Phase 7 follow-ups

1. **Pilot mapping-profile variant seam:** if the FILLED P9.04 dictates a
   pilot variant of `openems-mapping-profile.v1.json` (sign corrections from
   A-register verification, pilot channel additions), the variant file and
   its envelope-check handling are reviewed additive changes — the shipped
   v1 file is never modified in place (P6.10 D-10.1). Not created here: no
   pilot data exists to fill one.
2. **Per-site transport-timeout configuration:** P6.10 D-10.6 records the
   connect/read timeouts as composition constants; promoting them to
   per-site configuration (needed for a pilot endpoint with real network
   characteristics) remains the recorded follow-up, owned by the Phase 7
   seam owners — never fixed by editing constants for one site.
3. **Suite-version propagation:** the `edge-agent-suite` CI job logs the
   suite id and OpenEMS pin (P6.27 §2.1); when P9.37 executes at pilot
   versions, the pin line must be updated to the ACTUALLY-RUN version (the
   current 2026.11.0 line is a RESEARCH.md reference pin, explicitly marked
   "re-pin at the lab run") — a config edit with its own reviewed change.

## 11. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| Suites never executed (UNRUN): no adapter may be described as conformant for the whole pilot period | §1, §6; P9.04 §5 rule 2 |
| Suite run against a different adapter/firmware version than the pilot's — a version bump silently invalidates the conformance record | ledger P9.37 risk row; §4/§6 |
| A suite "passed" by skipping failing cases without recording them | ledger P9.37 risk row; §6 rules |
| A1–A8 remain UNVERIFIED: adapter behaviour against the real Edge is unconfirmed; every sign/unit/endpoint assumption in the shipped profile is unverified | §5; `openems-mapping-profile.v1.json` assumptions array |
| D5 OPEN — the adapter/protocol set (and therefore the suite set) rests on a RECOMMENDATION, not a decision | P9.04 §2 |
| No lab environment exists yet (P9.14 NOT EXECUTED) — "lab first, then pilot configuration" cannot start | §4; ledger dependencies |
| Conformance wording drift: "certified"/"conformant" appearing about any adapter without a P9.37 citation | §7, §9 (c) |

Status: **WRITTEN-UNRUN** — a suite execution cannot be produced by an
agent in this loop; no pilot configuration, adapter version, firmware
level, or lab environment exists; every result cell is EMPTY and stays
EMPTY until the human execution fills it from actual run output. Nothing
was run, and no pass is claimed.
