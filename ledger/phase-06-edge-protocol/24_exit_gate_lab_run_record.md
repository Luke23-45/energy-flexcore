# P6.24 — Read-only end-to-end lab integration (the exit-gate run): run book + EMPTY record, status PREPARED/UNRUN

Date: 2026-11-13. Subtask: P6.24, Phase 6 ledger (`ledger_phase_6.md` §P6.24).
Owner roles: integration lead + security reviewer (per the ledger).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the lab run has **NOT
happened** and cannot happen in this loop — nothing was built, executed,
captured, or witnessed. This record is the complete run book plus the record
template with EMPTY result cells, in the same PREPARED/UNRUN pattern as
`10_11_openems.md` §7. **Every measured, captured, or witnessed cell below is
deliberately EMPTY. No figure in this record is measured. The human
executing the lab run fills these cells; nothing here stands in for that
evidence.**

## 0. Gate status: NOT HELD

| Field | Value |
|---|---|
| §23 Stage 6 exit gate | "Read-only end-to-end site integration works in a lab; command path remains disabled unless explicitly approved for a test environment." |
| Gate status | **NOT HELD.** The gate is NOT HELD until a human executes this run book and the evidence pack (§3–§5) is filled from its actual output and independently witnessed. No checkbox in `CHECKLIST.md` is ticked by this record. |
| Blocking open gates | **D5 OPEN** (`01_d5_disposition.md` — no pilot partner named; `pilot-site-A` is PLACEHOLDER) and **D6 OPEN** (`03_d6_boundary_review.md` — review PREPARED, sign-off NOT SIGNED, reviewer NOT ASSIGNED). The ledger P6.03 exit check states P6.24 stays blocked on D6; `01_d5_disposition.md` §5 lists the lab/pilot site connection work as blocked on D5 closed and D6 signed. |
| Automated equivalents | EXIST AS WRITTEN TESTS ONLY, all **UNRUN**: `crates/edge-agent/tests/openems_e2e_p6_11.rs` (P6.11 read path + stale + no-write scan, against a labelled FIXTURE, never a stand-in for a lab Edge) and the FlexCore-side conformance suite `flexcore-edge-adapter-conformance-suite-v1` (`crates/edge-agent/tests/conformance_suite_p6_19.rs`; P6.19 run record UNRUN). Neither substitutes for this run. |

## 1. Prerequisites (all must hold before the run starts)

| # | Prerequisite | Source | Status at writing |
|---|---|---|---|
| P-1 | Edge agent builds per the P6.20 build procedure on the lab host | `20_deployment_profile.md` §5 (procedure RECORDED, NEVER executed) | UNRUN |
| P-2 | A lab OpenEMS Edge instance with the simulated PV + battery asset set per the OpenEMS Getting Started shape | ledger P6.11 inputs | NOT STOOD UP |
| P-3 | OpenEMS version pinned to **the version the lab Edge actually runs** (candidate 2026.11.0 per `RESEARCH.md` §1 — re-verify at the run; never assumed) | `10_11_openems.md` §2 | EMPTY cell (§3.1) |
| P-4 | D6 review signed for the lab topology, or an explicit lab-scope exception recorded by the security owner | `03_d6_boundary_review.md` (sign-off block NOT SIGNED) | NOT SIGNED |
| P-5 | Control plane reachable over the outbound mTLS channel with composed P6.06 credential material (issuer, CA) | `06_07_identity_channel.md` §3 (DEPLOYMENT REQUIREMENTS — no CA exists in-repo) | NOT COMPOSED |
| P-6 | Uplink wire format implemented by the `BatchPayloadEncoder` owner (the shipped `NoBatchPayloadEncoder` REFUSES; nothing is forwarded until then) | `06_07_identity_channel.md` §1; `10_11_openems.md` §8.3 | OPEN item |
| P-7 | Network capture capability arranged (span/mirror or Edge-side), reviewer named | ledger P6.11 verification method | EMPTY cell (§3.5) |
| P-8 | Independent witness named (the ledger verification method: the witness re-runs the run book from the evidence pack alone) | ledger P6.24 verification method | EMPTY cell (§3.6) |

## 2. The run book (the executable procedure)

Steps are executed in order; a deviation at any step is recorded in §3.6,
never absorbed silently.

| Step | Action | What is recorded |
|---|---|---|
| 1 | Stand up the lab OpenEMS Edge with the simulated PV + battery asset set (Getting Started shape: ESS `ess0`, PV inverter, meter) and FlexCore's simulated inputs for it | Exact OpenEMS version (the version actually running), full EdgeConfig dump (stored as a file/commit reference), simulator inputs (profile/meteorological source), Java/JRE version |
| 2 | Build and start the edge agent per `20_deployment_profile.md` §5 with `FLEXCORE_EDGE_OPENEMS_BASE_URL` pointing at the lab Edge (only `http://` composes — TLS on the OpenEMS REST hop is the recorded open item), `FLEXCORE_EDGE_OPENEMS_MAPPING_PROFILE` = the shipped `openems-mapping-profile.v1.json` | Edge-agent git SHA; startup log line (mapping-profile assumptions announced); the mapping-register output |
| 3 | Diff the discovered EdgeConfig against a hand-checked mapping table produced from the OpenEMS documentation (the P6.10 verification method); apply any sign-convention corrections as PROFILE edits, never code changes | Confirmed/corrected profile entries; every unmapped and ambiguous channel with its reason; sign findings |
| 4 | Enroll the edge and establish the outbound mTLS channel per P6.06/P6.07 | Enrollment record reference; confirmation that NO listener socket opened on the edge host (`ss -tlnp` or equivalent) |
| 5 | Run the read path for a defined window (record the window; the §24 slice replay discipline applies — step 10 of report §24: the replay must reproduce the edge read path from its recorded EdgeConfig, mapping version, and buffer/gap events) | Window start/end in UTC; telemetry captured at BOTH ends (Edge-side REST responses + platform-stored canonical records) |
| 6 | Reconcile value-for-value, channel by channel, within stated tolerances (independent reconciliation — not by the run's author) | The §3.3 reconciliation table, filled |
| 7 | Force a stale feed (stop the simulator feed / freeze the channel); confirm `STALE` downstream within the profile's `stale_after_millis` freshness limit and the platform gap detector's `MISSING` derivation for the gap | Time from feed stop to downstream `STALE`; the freshness limit in force; capture references (§3.4) |
| 8 | Attempt an `ISSUED` command against the DEFAULT-configured edge (no P6.14 enablement material present) and confirm rejection + audit (`EDGE_COMMAND_REJECTED`, code `command_path_disabled`) | The rejection audit record reference. **If — and only if — the P6.14 test-env gate was explicitly approved in writing (approver, scope, site, expiry), run the bounded P6.15–P6.16 test and attach the approval; otherwise record "command path never enabled".** |
| 9 | Capture all edge↔Edge traffic for the whole window and review it for write calls: expected HTTP methods toward the Edge are GET only; zero write/setpoint/controller-configuration calls | Capture file reference, duration, reviewer, method inventory (§3.5) |
| 10 | Collect the evidence pack (P6.33 checklist: telemetry reconciliation, quality flags, audit trail, health status, no-write attestation, plus the conformance/drill/security citations) | Pack path: `ledger/phase-06-edge-protocol/33_evidence_pack_completeness.md` |
| 11 | The independent witness re-runs the run book FROM THE EVIDENCE PACK ALONE and confirms the results reproduce | Witness name, date, reproduction outcome (§3.6) |

## 3. Results tables — ALL CELLS EMPTY (fill at the lab run)

### 3.1 Versions and environment

| Item | Value (EMPTY — fill at the lab run) |
|---|---|
| FlexCore edge agent build (git SHA) | |
| OpenEMS Edge version (the version the lab Edge ACTUALLY runs — not assumed) | |
| OpenEMS EdgeConfig dump reference | |
| Java/JRE version on the Edge host | |
| Lab host OS + hardware (per the D6/lab topology) | |
| Lab topology reference (per the D6 review) | |
| Run window (start/end, UTC) | |

### 3.2 Asset set and mapping diff

| Item | Value (EMPTY) |
|---|---|
| Simulated asset set stood up | |
| Hand-checked mapping table reference | |
| Diff result: profile entries confirmed / corrected | |
| Unmapped / ambiguous channels (with reasons) | |
| Sign-convention findings and the profile edits they force | |

### 3.3 Reconciliation table

| Channel | Value at the Edge | Canonical value at the platform | Tolerance | Match? (EMPTY) |
|---|---|---|---|---|
| ess0/ActivePower | | | | |
| ess0/ReactivePower | | | | |
| ess0/Soc | | | | |
| pvInverter/ActivePower | | | | |
| pvInverter/ReactivePower | | | | |
| meter0/ActivePower | | | | |
| meter0/ReactivePower | | | | |
| (any additional lab channel) | | | | |

### 3.4 Stale-feed evidence

| Item | Value (EMPTY) |
|---|---|
| How the feed was stopped | |
| Time from feed stop to downstream `STALE` flag | |
| Freshness limit in force (`stale_after_millis`) | |
| Downstream `MISSING` derivation evidence (platform gap detector) | |
| Capture references (log lines / telemetry records) | |

### 3.5 No-write capture review

| Item | Value (EMPTY) |
|---|---|
| Capture method and location | |
| Capture duration and byte/packet counts | |
| Reviewer who inspected the capture | |
| HTTP methods observed toward the Edge (expected: GET only) | |
| Any write/setpoint/controller-configuration call observed (expected: none) | |

### 3.6 Command-path default rejection and run record

| Item | Value (EMPTY) |
|---|---|
| Command attempted against the default-configured edge: rejection observed + audit reference | |
| P6.14 test-env enablement approval attached (or "command path never enabled in this phase") | |
| Executed by / witnessed by / date | |
| Exit status of run-book steps 1–11 | |
| Deviations from this record | |

## 4. Exit-check mapping (§23 Stage 6 and `CHECKLIST.md`)

| §23 Stage 6 gate / deliverable | Evidence this run must produce | CHECKLIST.md item |
|---|---|---|
| Read-only end-to-end site integration works in a lab | §3.3 reconciliation (filled) + §3.4 stale evidence + §3.5 no-write review; witnessed (§3.6) | "Read-only end-to-end site integration works in a lab" |
| Command path remains disabled unless explicitly approved | §3.6 default-rejection evidence; approval attached only if the P6.14 gate was exercised | "Command path disabled by default" / "approval attached" |
| Rust edge agent runs in the lab | Build + run references (§3.1) | "Rust edge agent exists and runs in the lab" |
| Outbound mTLS enrollment works | Step-4 records | "Outbound mTLS enrollment works" |
| Selected OpenEMS integration works (read-only) | §3.1–§3.3 | "Selected OpenEMS integration works (read-only)" |
| Telemetry buffering and recovery demonstrated | Step-5 window + the P6.28 drill (separate record; the read-only run alone does NOT evidence offline/degraded modes — see `33_evidence_pack_completeness.md` §3) | "Telemetry buffering and recovery demonstrated" |
| Adapter conformance tests exist and run in CI | The P6.19 suite citation (`flexcore-edge-adapter-conformance-suite-v1`; run ID EMPTY/UNRUN until CI executes) | "Adapter conformance tests exist and run in CI" |

The full evidence-pack requirements (including the P6.19/P6.28/P6.23 items
this run alone does not produce) are owned by `33_evidence_pack_completeness.md`.

## 5. Automated equivalents — WRITTEN TESTS, status UNRUN

| Automated equivalent | What it covers in-process | What it does NOT cover | Status |
|---|---|---|---|
| `crates/edge-agent/tests/openems_e2e_p6_11.rs` | Full in-process composition (registry-constructed `OpenEmsRestAdapter` → `AntiCorruptionLayer` → `AdapterIngestSource` → `FileLocalBuffer` → `EdgeRunner.tick`): value-for-value reconciliation at both ends; forced stale feed arrives `STALE` within the freshness limit and never `MISSING`; the module-level lexical no-write scan | A real Edge, a timed window, a network capture, an independent witness | WRITTEN, **UNRUN** (fixture EdgeConfig, labelled FIXTURE, per the UNVERIFIED-ASSUMPTION A1–A8 register in `10_11_openems.md` §2) |
| `flexcore-edge-adapter-conformance-suite-v1` (`tests/conformance_suite_p6_19.rs`) | The 11-case FlexCore-side suite (capability honesty, named refusals, quality no-upgrade, units/ACL, identity, unmapped register, the full P6.13 fault set, structural read-only guarantee, registry allowlist, run-record rendering) | The lab confirmation of the recorded REST fixture shape; CI execution | WRITTEN, **UNRUN** (no CI run, no per-case record exists) |
| `tests/no_inbound_listener.rs` | Crate-wide lexical refusal of listener surfaces (no inbound socket on the edge, structural) | The lab host port-scan confirmation (ledger P6.07 verification) | WRITTEN, **UNRUN** |

## 6. Honest limitations

1. **The gate is NOT HELD.** Nothing in this record executes the run, and no
   cell above is filled. The §23 Stage 6 exit gate is met only when a human
   executes §2, §3 is filled from actual output, and the independent witness
   reproduces the run from the pack alone (ledger P6.24 verification method).
2. **D5 and D6 are OPEN.** The lab run is blocked on both gates per the
   ledger; executing it against `pilot-site-A` (PLACEHOLDER) is impossible
   by definition, and no site name is invented here.
3. **The uplink wire format is an OPEN item** (`BatchPayloadEncoder` refuses):
   until the owner implements it, step 5's platform-side capture cannot
   exist even in principle. This record does not invent a format.
4. **The OpenEMS REST API shape is UNVERIFIED** (A1–A8 register,
   `10_11_openems.md` §2); step 3's diff is the moment those assumptions are
   confirmed or corrected.
5. This record adds no code and touches no source file; it owns exactly one
   new ledger file plus the P6.24 ledger tick.
