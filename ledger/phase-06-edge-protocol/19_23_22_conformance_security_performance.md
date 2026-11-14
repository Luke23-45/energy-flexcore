# P6.19 + P6.23 + P6.22 — Adapter conformance suite and claim boundary, security testing, performance validation templates: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P6.19, P6.23, P6.22. Owner roles: integration
engineer + product (P6.19), security engineer (P6.23), Rust engineer (P6.22).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the code and tests in this
wave are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every suite case and attack test below is UNRUN until the human
build/test pass executes it. No finding, pass, or measurement is claimed in
this document — the findings and performance tables are deliberately EMPTY.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/edge-agent/tests/conformance_suite_p6_19.rs` | NEW (P6.19) |
| `crates/edge-agent/tests/security_p6_23.rs` | NEW (P6.23, edge side) |
| `crates/edge-agent/tests/perf_harness_p6_22.rs` | NEW (P6.22) |
| `apps/control-plane/tests/api_security_p6_23.rs` | NEW (P6.23, server side) |
| `.github/workflows/ci.yml` | EDITED (one additive job `edge-agent-suite`; no existing job touched) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.19/P6.23/P6.22 ticks only) |
| this file | NEW (decision record) |

The P6.15/P6.16 modules (`command_verify.rs`, `command_forward.rs`) were
PRESENT on disk when this wave was written, so the P6.23 attack tests drive
the FULL pipeline (P6.14 gate → P6.15 verifier → P6.16 forwarder) through
the public API; nothing in those modules (or in `ports.rs`, `config.rs`,
`main.rs`, `adapters/*`, `audit_buffer.rs`, `lib.rs`, or any `Cargo.toml`)
was edited here.

## 1. P6.19 step 1 — the conformance-suite landscape (recorded honestly, per RESEARCH.md)

The ledger requires the applicable suite, its owner, its access terms, and
lab runnability to be recorded BEFORE writing tests. The finding, stated
plainly:

> **NO official protocol conformance suite applies to the OpenEMS REST
> integration.** The FlexCore-side suite below is therefore 100%
> FlexCore-side; there are no "protocol-specific cases from the official
> suite" to run, because none exists for this channel.

Per-protocol landscape, exactly as `RESEARCH.md` (checked 2026-11-07)
supports it and no further:

| Protocol / program | Applies to the OpenEMS REST channel? | Owner | Access terms (per RESEARCH.md) | Runnable in the lab against this adapter? |
|---|---|---|---|---|
| OpenEMS (REST channel reads) | **This is the channel.** RESEARCH.md §1 records the architecture (Natures/Channels, EdgeConfig JSON) and release train, and records NO protocol-conformance or certification program for the REST channel. **UNVERIFIED (labelled honestly):** "the OpenEMS project does not publish a REST-channel conformance certification" is the absence of any such program in the checked sources — the OpenEMS project's own docs were checked 2026-11-07 for architecture and releases, and no suite is recorded there. No suite name or version is invented. | OpenEMS project (community) | N/A — no suite identified | No suite exists to run (as far as the recorded sources go) |
| OpenADR 3.0 certification | No — a DIFFERENT protocol (demand response), a later adapter per §2.3 | OpenADR Alliance | Members-only online test tool; certification license (US $1,250 per the Alliance's certification page — RE-CONFIRM AT PROCUREMENT, RESEARCH.md open task 3); final verification at an Alliance-appointed test house | No (wrong protocol; and membership-gated regardless) |
| OCPP 2.0.1/2.1 certification profiles | No — EV charging; later adapter | Open Charge Alliance | Parts 5/6 certification profiles exist per SECONDARY sources with conflicting dates (RESEARCH.md open task 2 — not recorded as fact) | No (wrong protocol) |
| SunSpec Modbus (IEEE 1547) certification | No — device information models over Modbus; later adapter | SunSpec Alliance | SunSpec Certified via Authorized Test Laboratories | No (wrong protocol) |
| IEEE 2030.5 / CSIP | No — DER/smart-inverter profile; later adapter (§2.3) | IEEE SA / SunSpec (CSIP test procedure) | CSIP conformance test procedure; certification via SunSpec Authorized Test Laboratories | No (wrong protocol) |

**Labels used, per AGENTS.md rule 1:** the OpenADR figures are from the
Alliance's own pages via RESEARCH.md and are still flagged for
re-confirmation at procurement time (RESEARCH.md open task 3); the OCPP
Edition-2 material is secondary-sourced and NOT recorded as fact; the
absence of an OpenEMS REST conformance program is UNVERIFIED-ABSENCE (what
the checked sources support), not a verified negative claim about every
OpenEMS artefact.

Consequence for P6.19 step 2: every case in the FlexCore suite maps to a
REPORT requirement (§19.2(3)/(6)) or is marked FLEXCORE-ONLY — see §2.

## 2. P6.19 steps 2–3 — the FlexCore-side suite and its CI wiring

**Suite identity:** `flexcore-edge-adapter-conformance-suite-v1` (the
versioned id — the ONLY thing a pass claim may name; defined as
`SUITE_ID` in the test file).

**Adapters under test (both driven through the real composition):**

| Adapter id | Registry status | Driven how |
|---|---|---|
| `openems-rest` | Implemented (P6.10) | the REAL `OpenEmsRestAdapter` over an IN-PROCESS GET-only fixture transport serving the RECORDED EdgeConfig/channel JSON shape (the `openems_e2e_p6_11.rs` fixture; UNVERIFIED-ASSUMPTION spellings, confirmed only at the P6.11 lab run). OpenEMS version pin: 2026.11.0 (RESEARCH.md §1) — re-pin at the lab run |
| `simulated-test` | TEST-ONLY (P6.08) | the deterministic in-process scenario fixture |

**Case → requirement map (the P6.19 acceptance artefact; identical to the
table in the test file header):**

| # | Case | Requirement |
|---|---|---|
| 1 | `contract_capability_honesty` | §19.2(3) — capabilities are full REQUIRED declarations, no defaults (P0.07 §3, P0.08 §3, P0.09 §4) |
| 2 | `contract_named_error_refusals` | §19.2(3) — every bad input is a NAMED, non-empty error; nothing partially applied |
| 3 | `contract_quality_passthrough_no_upgrade` | §19.2(3) + §10.4 — degraded source states never map to GOOD; original code retained |
| 4 | `contract_units_and_acl_conformance` | §19.2(3) + §10.2 — canonical units (MW/MVAr/MWh, SOC fraction in [0,1]), explicit sign handling, unit/quantity pairing |
| 5 | `contract_identity_source_id_discipline` | §19.2(3) + P2.04 D-T6 — source identity shape; reserved `flexcore:` namespace refused |
| 6 | `contract_unmapped_channel_register` | §19.2(3) + P6.12 step 2 — every channel classified exactly once; nothing mapped by guessing |
| 7 | `suite_full_p6_13_fault_injection_set` | §19.2(6) — the FULL fault-injection set through the P6.13 harness (`simulated-test`) |
| 8 | `suite_full_p6_13_fault_injection_set_openems` | §19.2(6) — the SAME set through the harness against the `openems-rest` adapter over the recorded REST fixture shape |
| 9 | `flexcore_only_read_only_structural_guarantee` | FLEXCORE-ONLY (§2.1 v0.4.0, §21.4) — the adapter port has NO write surface; the OpenEMS transport exposes GET only; the fixture double records zero write attempts |
| 10 | `flexcore_only_registry_allowlist_conformance` | FLEXCORE-ONLY (§9.3) — every suite adapter id is registry-allowlisted with its honest status; a RESERVED id refuses construction |
| 11 | `suite_record_renders_suite_and_claim_boundary` | FLEXCORE-ONLY (P6.19 steps 3–4) — the run record carries the suite version, the protocol-shape pin, and the bounded claim sentence |

**CI wiring (P6.19 step 3):** one additive job `edge-agent-suite` in
`.github/workflows/ci.yml` runs the conformance suite (with `--nocapture`
so the per-case run record lands verbatim in the job log, together with the
suite version and protocol-shape pin), the P6.23 edge security suite, and
the P6.22 load harness. No existing job was modified.

**RUN RECORD: suite runs in CI — CI execution UNRUN (human gate input).**
The job has never executed; there is no pass/fail record, no green run,
and no per-case outcome table in this document. The first human CI run is
the verification point, and its log is the run record of ledger P6.19
step 3.

## 3. P6.19 step 4 — the claim boundary, in plain language

> Passing this suite entitles FlexCore to claim **exactly** this:
>
> *"passes the FlexCore edge adapter conformance suite version 1 against
> the recorded OpenEMS REST API shape."*
>
> It does **NOT** entitle any claim of OpenADR Alliance, Open Charge
> Alliance (OCA), SunSpec, or IEEE 2030.5 certification or conformance.
> Those require the respective programs' own processes, their own test
> tools, and (for several) verification at their appointed test houses
> (report §2.3: implementing an adapter is not a conformance claim).
> FlexCore is not certified, and no marketing text, README, or release note
> may claim certification, conformance, or compliance of any protocol.

Where the boundary lives: there is NO `crates/edge-agent/README.md` in the
repository, so per the recorded subtask instruction the claim boundary is
stated here (and pinned inside the suite itself by case 11, which refuses
any run record carrying a forbidden certification claim). When P6.26 writes
the adapter guide under `docs/protocols/`, it MUST carry this section
verbatim.

Grep-forbidden claiming phrases in any FlexCore-authored text about this
suite: "certified", "is certified", "OpenADR certified", "conformant to
OpenADR", "SunSpec certified", "2030.5 certified", "OCPP certified" —
except inside this boundary statement itself or the P6.19 decision records
(quoting the boundary).

## 4. P6.23 — security-test REPORT (dated 2026-11-13)

**Status of every case below: WRITTEN-UNRUN.** No case has executed. The
findings column is EMPTY on purpose: no finding is fabricated, and no pass
is claimed. **"Zero open high/critical findings" CANNOT be claimed until
the human runs the suite** and this table is filled from its actual output
(independent re-run by a second engineer is the ledger's verification
method, P6.23).

### 4.1 Command forgery and replay, extended to the full lifecycle (edge side, `tests/security_p6_23.rs`)

Driven through the FULL pipeline (P6.14 gate admission → P6.15 four-check
verifier → P6.16 forwarder); every refusal must be a DISTINCT named reason,
audited (`EDGE_COMMAND_VERIFICATION_REJECTED`, `code=<reason>`), and the
forwarder must record ZERO calls for the attacked command:

| Case | Attack attempted | Required outcome | Status | Findings |
|---|---|---|---|---|
| `control_a_valid_command_passes_all_four_checks_and_forwards_once` | none (CONTROL: a legitimately signed command) | verifies + forwards exactly once | WRITTEN-UNRUN | (empty) |
| `a_forged_signature_is_rejected_as_signature_audited_and_never_forwarded` | signature produced under attacker key material | `signature` rejection + audit + zero forwards | WRITTEN-UNRUN | (empty) |
| `a_wrong_signer_is_rejected_with_the_wrong_signer_detail` | signature reference naming a different key reference | `signature` rejection, detail names the wrong signer | WRITTEN-UNRUN | (empty) |
| `an_unsigned_command_is_rejected_as_signature` | no signature at all | `signature` rejection | WRITTEN-UNRUN | (empty) |
| `a_replayed_valid_command_is_rejected_as_duplicate_and_forwarded_once` | exact re-send of a captured valid command | `duplicate` rejection; forward count stays 1 | WRITTEN-UNRUN | (empty) |
| `an_out_of_sequence_command_is_rejected_as_sequence` | sequence below the last accepted | `sequence` rejection | WRITTEN-UNRUN | (empty) |
| `an_expired_valid_command_is_rejected_as_expiry` | perfectly signed command past `valid_until` | `expiry` rejection | WRITTEN-UNRUN | (empty) |
| `a_degraded_edge_clock_fails_the_expiry_check_closed` | command presented under `Unknown`/`Unsynchronized` clock quality | fail-closed `expiry` rejection (§17.4, §17.5 rule 5) | WRITTEN-UNRUN | (empty) |
| `a_sequence_replay_after_verifier_restart_is_denied_by_the_persisted_checkpoint` | replay after edge restart (the ledger's recorded risk) | persisted checkpoint denies: `duplicate` | WRITTEN-UNRUN | (empty) |
| `a_non_allowlisted_target_is_rejected_as_allowlist_in_the_full_pipeline` | target adapter outside the site allowlist | `allowlist` rejection | WRITTEN-UNRUN | (empty) |
| `a_command_outside_its_asset_scope_is_rejected_as_scope` | value outside the authorization limits | `scope` rejection | WRITTEN-UNRUN | (empty) |

### 4.2 Malicious / incorrect telemetry (edge side, `tests/security_p6_23.rs`, ACL → ingest path with a hostile adapter double)

| Case | Attack attempted | Required outcome | Status | Findings |
|---|---|---|---|---|
| `b_spoofed_foreign_site_point_is_refused_at_the_acl_never_ingested` | compromised adapter serves a reading claiming ANOTHER SITE's point | `PointUnmapped` refusal, counted, zero samples emitted | WRITTEN-UNRUN | (empty) |
| `b_reserved_platform_namespace_is_refused_for_edge_sources` | telemetry under the reserved `flexcore:` namespace / empty identity | construction-time refusal (`ReservedSourceNamespace` / `EmptySourceId`) | WRITTEN-UNRUN | (empty) |
| `b_out_of_range_values_are_refused_never_accepted_as_good` | NaN; SOC 150 % and −5 % presented as GOOD | named ACL refusal, never a GOOD sample | WRITTEN-UNRUN | (empty) |
| `b_out_of_range_quality_code_arrives_suspect_never_good_through_the_poll_path` | source reports `OUT_OF_RANGE` | sample arrives SUSPECT with the original code retained, never GOOD | WRITTEN-UNRUN | (empty) |
| `b_stale_data_replay_is_flagged_or_skipped_never_accepted_as_new_good` | STALE data presented fresh; exact replay; tampered replay | STALE quality (never GOOD); `ExactDuplicate` skip; `ConflictingDuplicate` refusal | WRITTEN-UNRUN | (empty) |
| `b_a_nan_reading_through_the_poll_path_is_counted_and_never_emitted` | NaN through the full poll path | hard poll failure, counted, zero emitted | WRITTEN-UNRUN | (empty) |

### 4.3 Auth bypass at the SERVER (`apps/control-plane/tests/api_security_p6_23.rs`)

Driven against the REAL router (`stage1_router`), exactly as a malicious
client would — the only server-facing edge endpoint in v0.4.0 is
`POST /api/v1/edge/enroll` (`routes_edge.rs` fail-closed matrix):

| Case | Attack attempted | Required outcome | Status | Findings |
|---|---|---|---|---|
| `an_unauthenticated_edge_session_request_is_401_and_never_journaled_as_an_enrollment` | no Authorization header | 401; nothing journaled as an enrollment | WRITTEN-UNRUN | (empty) |
| `a_garbage_bearer_token_is_401` | fabricated bearer value | 401 | WRITTEN-UNRUN | (empty) |
| `a_consumed_single_use_token_replay_is_401` | replay of a consumed provisioning token | 401 (the credential is dead at the seam) | WRITTEN-UNRUN | (empty) |
| `an_expired_token_is_401_and_assigns_no_scope` | expired credential (refused by the validator seam) | 401; no scope; nothing journaled | WRITTEN-UNRUN | (empty) |
| `a_wrong_tenant_token_is_403_and_never_journaled_as_an_enrollment` | tenant-A token, tenant-B binding | 403 | WRITTEN-UNRUN | (empty) |
| `a_foreign_site_within_the_tenant_is_403_and_the_denial_is_audited` | right tenant, invented site | 403 + REJECTED journal event with actor, object, timestamp, correlation id | WRITTEN-UNRUN | (empty) |
| `a_revoked_identity_cannot_re_enroll_and_the_attempt_is_audited` | revoked identity re-enrolls with a fresh token | 403 + audited REJECTED naming the revocation | WRITTEN-UNRUN | (empty) |
| `a_malformed_device_identity_is_422_and_writes_nothing` | blank device id | 422; no store write | WRITTEN-UNRUN | (empty) |
| `a_malformed_tenant_or_site_uuid_is_422` | non-UUID / path-shaped ids | 422 twice; nothing journaled | WRITTEN-UNRUN | (empty) |
| `every_state_decision_refusal_carries_actor_object_and_timestamp` | duplicate-identity conflict | audited REJECTED with actor/object/timestamp/correlation, tenant-scoped | WRITTEN-UNRUN | (empty) |

Honest note on token expiry: expiry/revocation of provisioning tokens is
enforced in the `ProvisioningTokenValidator` seam (the deployment composes
its real token service there; the shipped default denies everything). The
expired-token case models the seam's refusal and proves the ROUTE fails
closed on it; the deployment's actual expiry checking is a DEPLOYMENT
requirement (as recorded in `06_07_identity_channel.md`), not something
this test can prove about a service that does not exist in-repo.

### 4.4 Secrets exposure (edge side, `tests/security_p6_23.rs`)

| Case | Attack / check | Required outcome | Status | Findings |
|---|---|---|---|---|
| `d_no_secret_material_is_embedded_or_logged_in_edge_agent_sources` | lexical scan over `src/**` (≥ 15 files, incl. `main.rs`): no secret-typed CONSTRUCTOR handed an inline string literal in production regions; no hardcoded credential-shaped literals; no `tracing::` line passing `.expose()` or a secret value field | zero violations | WRITTEN-UNRUN | (empty) |
| `d_the_redaction_debug_impls_actually_redact` | runtime: `ProvisioningToken`, `EnablementKey`, `CommandSigningKey`, `PemBody`, `DeviceCredentialRecord`, `EdgeConfig` (endpoint, provisioning token, OpenEMS base URL) — a planted secret value must be unrecoverable from `Debug`/`Display` | redaction holds for every type | WRITTEN-UNRUN | (empty) |
| `d_the_scan_covers_the_binary_entrypoint_and_every_module` | scan-coverage guard (binary entrypoint + P6.15/P6.16 modules scanned) | coverage holds | WRITTEN-UNRUN | (empty) |

### 4.5 Out of scope (stated, not claimed)

**Plugin-escape testing is OUT OF SCOPE for P6.23 and was NOT performed.**
There is no plugin host in Phase 6 — the WASM host is v0.5.0 / Stage 7
(report §5.4, §23; ledger P6.23 step 5). No plugin-escape result is claimed
in either direction, and the §19.2(7) "plugin privilege escalation"
category has no case in this report for exactly that reason. The structural
anti-escalation facts that DO exist in Phase 6 (a plugin can never select
the control-authority mode, §5.7 — the P6.14 enablement is a private,
constructor-only state) are recorded in
`13_14_17_adapter_command_policy.md`, not re-tested here.

## 5. P6.22 — the load harness (templates; NO measured numbers)

`crates/edge-agent/tests/perf_harness_p6_22.rs` is a std-only
(`std::time::Instant`; NO criterion, NO new dependencies) load harness over
the REAL composition — `AdapterIngestSource` (P6.08) → `FileLocalBuffer`
(P6.09 durable journal) → `EdgeRunner.tick` — parameterized by the load
profile (asset count, telemetry rate in samples/asset/poll, buffer depth,
burst factor, steady-poll count). It prints a
`flexcore-edge-perf-record/v1` block with the MEASURED timings and marks
CPU/memory/disk `not_measured_by_this_harness` (an in-process Rust test
cannot measure those; external measurement is a human gate input).

Its assertions are STRUCTURAL ONLY (all samples accounted for; the audited
drop-oldest counter never exceeds what was buffered; the drain completes;
the buffer engagement is OBSERVED) — there is deliberately no timing
threshold, per report §19.4 (no arbitrary universal targets).

**THE MEASURED REPORT (the deliverable) — all cells EMPTY. Nothing has
been measured.** The human runs the harness (and the external CPU/memory/
disk observation) and fills this table from actual output, with the
hardware, versions, and conditions of the run:

| Metric | x86-64 gateway profile | ARM64 gateway profile |
|---|---|---|
| CPU utilisation under the recorded load profile | (EMPTY — not measured) | (EMPTY — not measured) |
| Memory (RSS) under the recorded load profile | (EMPTY — not measured) | (EMPTY — not measured) |
| Disk (journal write rate / growth) | (EMPTY — not measured) | (EMPTY — not measured) |
| Telemetry ingest rate (samples/s, steady state) | (EMPTY — not measured) | (EMPTY — not measured) |
| Forwarding latency p95 / p99 | (EMPTY — not measured) | (EMPTY — not measured) |
| Recovery drain time (post-outage, recorded buffer depth) | (EMPTY — not measured) | (EMPTY — not measured) |
| Buffer-engagement point (load at which the P6.09 buffer policy engages) | (EMPTY — not measured) | (EMPTY — not measured) |

**Load profile — from pilot site — PLACEHOLDER pending pilot input (never
assumed):**

| Profile parameter | Value |
|---|---|
| Asset count | PLACEHOLDER pending pilot input |
| Telemetry rate (samples per asset per interval) | PLACEHOLDER pending pilot input |
| Buffer depth | PLACEHOLDER pending pilot input |
| Burst factor / channel conditions | PLACEHOLDER pending pilot input |

The values in the test file's `LoadProfile::default()` are HARNESS
parameters chosen to exercise the code paths in seconds — they are NOT the
pilot load profile and must not be quoted as one.

**Honesty notes (hard rules for whoever fills the table):**

1. **Lab-laptop-vs-gateway:** any number produced on a laptop/CI runner is
   a lab number, NOT a gateway number. The table above is for the GATEWAY
   profiles; lab runs may be recorded separately and must be labelled as
   such.
2. **No extrapolation:** these measurements do not extrapolate to a
   10,000-asset fleet (report §19.3: do not claim that first tests
   establish capability at utility scale).
3. **Conditions travel with numbers:** every filled cell carries hardware,
   OS, FlexCore/edge-agent version, OpenEMS version (where relevant), and
   the load-profile values in force — a number without its conditions is
   not a record (P6.22 acceptance: "no performance claim appears without
   its measurement conditions").

## 6. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Status |
|---|---|---|
| D-1 | The conformance suite is **100 % FlexCore-side** (`flexcore-edge-adapter-conformance-suite-v1`); no official protocol suite applies to the OpenEMS REST channel (§1). | DEFAULT (recommended and implemented; revisit if the pilot partner identifies an OpenEMS-side test programme — none is recorded in RESEARCH.md) |
| D-2 | The suite runs against the RECORDED OpenEMS REST fixture shape (the P6.10/P6.11 fixture, UNVERIFIED-ASSUMPTION spellings) until the P6.11 lab run confirms the shape; a shape mismatch at the lab run is a fixture/profile edit and a suite re-run, never a reinterpretation. | DEFAULT |
| D-3 | The P6.23 attack tests drive the FULL pipeline (P6.14 → P6.15 → P6.16) because the P6.15/P6.16 modules were present on disk; the same attack cases thereby bind the P6.15/P6.16 integration surface. | DEFAULT (followed from what existed; no alternative considered necessary) |
| D-4 | The expired-provisioning-token case models the validator seam's refusal (401 fail-closed) rather than implementing token expiry in-repo — expiry is a deployment requirement of the composed token service. | DEFAULT |
| D-5 | P6.22 ships the HARNESS and the EMPTY measured tables; measurements are a human gate input on the target hardware. | DEFAULT (forced by the loop bans and by §19.3/§19.4) |

## 7. Human gate inputs (all with EMPTY results until executed)

| # | Gate input | How | Result |
|---|---|---|---|
| 1 | Build + run the four new test files | `cargo test -p flexcore-edge-agent --test conformance_suite_p6_19 -- --nocapture`; `cargo test -p flexcore-edge-agent --test security_p6_23`; `cargo test -p flexcore-edge-agent --test perf_harness_p6_22 -- --nocapture`; `cargo test -p flexcore-control-plane --test api_security_p6_23` | (EMPTY — UNRUN) |
| 2 | First CI run of the `edge-agent-suite` job | push/PR; the job log IS the P6.19 step-3 run record | (EMPTY — CI execution UNRUN) |
| 3 | Fill the P6.23 findings tables from actual output; then (and only then) assess "zero open high/critical findings"; independent re-run by a second engineer per the ledger | run 1's output | (EMPTY — no findings claimed) |
| 4 | Fill the P6.22 measured tables on x86-64 AND ARM64 gateway hardware, with external CPU/memory/disk observation, and the pilot load profile | run the harness + external measurement on the target hardware | (EMPTY — not measured) |
| 5 | Confirm the recorded OpenEMS REST fixture shape at the P6.11 lab run; re-pin the OpenEMS version; re-run the suite if the shape moved | P6.11 lab run | (EMPTY — lab run not executed) |
| 6 | Grep docs/README for claiming phrases per §3 before any release text ships | `grep -rn "certified\|conformant\|compliant" docs/ console/ README.md` | (EMPTY — release text does not exist yet) |

## 8. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (output quoted in the completion report) |
| `cargo build` / `cargo clippy` / `cargo test` (any target) | **NOT RUN — banned in this loop; human gate input 1** |
| CI execution of the new job | **UNRUN — human gate input 2** |
| Security findings | **NONE RECORDED — tables deliberately EMPTY; no "zero findings" claim is made** |
| Performance measurements | **NONE RECORDED — tables deliberately EMPTY** |
| Plugin-escape testing | **OUT OF SCOPE (no plugin host in Phase 6) — NOT tested, NOT claimed** |
