# HANDOVER — Phase 6: Edge and Protocol Integration

Status: **CLOSED** — 34/34 subtasks implemented (P6.01–P6.34); the §23
Stage 6 exit gate is **NOT HELD** (prepared, not demonstrated — see
`30_retrospective_release_gate.md`). Loop bans held throughout: nothing
installed, nothing built, no tests run, no CI run — all suites WRITTEN-UNRUN.

## Blockers carried (human gate inputs)

1. Stages 1–5 gates NOT HELD (demonstrations/runs/sign-offs pending) — P6.04
   records this honestly.
2. D5 (protocol/asset mix) and D6 (cyber/OT) OPEN — recorded, not decided;
   work proceeds against the RECOMMENDED default (OpenEMS first).
3. OpenEMS REST API usage UNVERIFIED (never executed); pandapower/helics
   pins from Phase 5 carried.
4. Secret/dependency scans (P6.02) are the human's.

## Wave status

| Wave | Subtasks | Status |
|---|---|---|
| 1a | P6.01, P6.03, P6.02, P6.04 | DONE — decision records written: `01_d5_disposition.md` (D5 STILL OPEN, OpenEMS-first RECOMMENDATION, `pilot-site-A` PLACEHOLDER), `03_d6_boundary_review.md` (D6 STILL OPEN, review PREPARED, outbound-only + test-lab defaults RECOMMENDATION, sign-off NOT SIGNED), `02_license_audit.md` (static; OpenEMS license + rustls license UNVERIFIED → D4 items; cargo-deny/gitleaks NOT RUN — human inputs), `04_readiness_check.md` (Stages 1–5 artifacts exist, UNBUILT/UNRUN; all gates NOT HELD; verdict PREPARED, not proven). Ledger P6.01–P6.04 ticked "implemented, awaiting human build/test"; verify_docs.py + validate_agent.py → OK (exit 0) |
| 1b | P6.05, P6.12, P6.20 | DONE — NEW composition crate `crates/edge-agent/**` (P6.05: config with command-path flag DEFAULT FALSE, ports EmsSource/UplinkChannel/LocalBuffer/AuditSink with fail-closed stubs + in-memory impls, supervision runner with offline-first degraded-mode state machine, WallClock, binary composition root; placement deviation `crates/edge-agent` vs report §8.1 `apps/edge-agent` RECORDED), adapter port + ACL + registry (P6.12: `EmsAdapter` discover/read/capabilities/health with NO dispatch method, mapping-table ACL — units/quality/time/signs all explicit, unknown → named error, no default quality, `openems-rest` PLANNED P6.10/P6.13 + `generic-rest` RESERVED), deployment profile (P6.20: `deploy/edge/flexcore-edge-agent.service` written NOT installed, signed-update design — no PKI implemented, resource bounds configuration not measured — `20_deployment_profile.md`). Decision records: `05_12_20_edge_foundation.md`, `20_deployment_profile.md`. Workspace member + check_dependency_rule.py additive edits; python -I check_dependency_rule.py → OK; verify_docs.py + validate_agent.py → OK. Tests written, NOT RUN (loop bans); `cargo build/fmt/clippy/test` UNRUN — human build pass pending |
| 2a | P6.06, P6.07 | DONE — P6.06: NEW `crates/edge-agent/src/enrollment.rs` (`EnrollmentClient`/`CredentialIssuer`/`CredentialStore`/`RevocationChecker` ports; `TokenEnrollmentAdapter`; `DenyAllCredentialIssuer` + `UnconfiguredRevocationChecker` FAIL-CLOSED shipped defaults; `ProvisioningToken`/`PemBody` redacted secrets; `FilesystemCredentialStore` — versioned line format `flexcore-edge-credential-v1`, 0600 owner-only on Unix + load-time re-check, key MATERIAL never stored (reference only); `CredentialExpiryMonitor` rotation-window assessments; `EnrollmentOrchestrator` with audited EDGE_ENROLLMENT_COMPLETED/REJECTED + expiry events) + `src/test_support.rs` (temp-dir credential-store harness); control-plane NEW `routes_edge.rs` + `dto_edge.rs` (POST `/api/v1/edge/enroll` — ADDITIVE path per the recorded P4.36/P5.31 rule; provisioning-token validator seam, DENY-ALL default; fail-closed matrix: scope-mismatch/site-unapproved/REVOKED → 403, duplicate ACTIVE → 409 `EDGE_IDENTITY_EXISTS`, EXPIRED → rotation seq+1 + ROTATED event; REJECTED attempts journaled §17.3) + `state.rs` additive fields/builders (enrollment store + token validator, both fail-closed defaults) + openapi/routes/lib wiring + `tests/api_edge.rs`; persistence: NEW `crates/flexcore-storage/src/edge_enrollment.rs` trait (ownership deviation RECORDED — decision doc §7) + migration 0018 (append-only `edge_device_registry` per identity TERM, partial unique index = ONE ACTIVE term per device/tenant — the no-two-edges-share-an-identity rule at the DB level; append-only `edge_enrollment_journal`; RLS ENABLE+FORCE, flexcore_app INSERT/SELECT, guard triggers) + postgres `src/edge_enrollment.rs` impl (own tenant-scoped unit of work per call) + env-gated `tests/edge_enrollment.rs`. P6.07: NEW `crates/edge-agent/src/uplink_mtls.rs` (`MtlsUplinkChannel` implementing the P6.05 `UplinkChannel` port over rustls — CLIENT-only, edge-initiated; STRUCTURAL no-inbound guarantee + `tests/no_inbound_listener.rs` lexical scan over the whole crate src; TLS minimum DEFAULT TLS 1.3 with opt-in `tls12-floor` — the D6 checklist C2 answer, cipher suites = rustls ring-provider defaults RECORDED not invented; plaintext ws:///http:///scheme-less REFUSED; root CA pinned FROM the P6.06 credential record, fail-closed on empty; deterministic exponential backoff ×2 with cap, keepalive = fresh re-handshake after the idle window, `ChannelStatus` = "channel down" distinct from the runner's "site down"; at-least-once: Ok ONLY after the bounded `ACK` read; wire-payload encoder seam REFUSES — P6.08/P6.09 own the format) + `tests/uplink_mtls_channel.rs` (fail-closed composition, reconnect/keepalive state machine on a fake transport). FIRST DIRECT DEPS: `rustls = "0.23"` (ring provider, default-features off) + `rustls-pemfile = "2"` — BOTH UNPINNED-REVERIFY caret ranges with the D4 note (decision doc §5; license re-verify = the human's first cargo-deny run). Decision record: `06_07_identity_channel.md` (per-subtask coverage, the enrollment-honesty table — NO CA/key-generation/OCSP implemented, all DEPLOYMENT REQUIREMENTS — TLS policy table, DEFAULT decisions, honest limitations; `main.rs` composition root deliberately NOT rewired — the encoder seam is the P6.08/P6.09 integration point, matching wave 2b's note). Tests written NOT RUN; cargo build/fmt/clippy/test UNRUN — human build pass pending |
| 2b | P6.08, P6.09 | DONE — NEW `crates/edge-agent/src/ingest.rs` (P6.08 read path: `AdapterIngestSource` = EmsAdapter.read → P6.12 ACL → canonical `EdgeTelemetrySample` → `EmsSource`; per-poll error accounting `IngestStats` — quality passthrough only, never upgraded; event_time from source / ingest_time edge-stamped, distinct; bounded edge dedup window `(source identity, event_time)` — the P4.24 fixture-(a)/(b) rules cited, exact replay skipped+counted, conflicting refused), `src/adapters/simulated.rs` (`SimulatedEmsSource`, registry id `simulated-test`, NEW `AdapterStatus::TestOnly` — deterministic scenario-fixture-driven, in-process, NEVER a real EMS, `source_id` carries the id), `src/buffer_file.rs` (P6.09 `FileLocalBuffer`: append-only CRC-32 length-prefixed journal, std-only wire encoding decoded through the validated sample constructor, truncate-and-audit corrupt-tail recovery, audited drop-OLDEST overflow with hard counter + latched health flag, `ingest_time`-based max-age retention with audited counter, temp-file+rename commit compaction — at-least-once: drained records stay on disk until `LocalBuffer::commit_forwarded` (new defaulted port method called by the runner after a confirmed forward); crash-window redeliveries are the platform's P4.24-style ingest's duplicates, cited), additive `ports.rs`/`runner.rs`/`config.rs` (`FLEXCORE_EDGE_DEDUP_WINDOW`, `FLEXCORE_EDGE_BUFFER_MAX_AGE_MS`, `FLEXCORE_EDGE_OVERFLOW_POLICY` — only `drop-oldest-audited` accepted)/`lib.rs`/`registry.rs`/`adapters/mod.rs` edits. Tests: `tests/ingest_p6_08.rs` (read-path integration through the runner: order, units, quality passthrough, times, dedup, ACL-failure accounting, forced-unreachable hard audit, outage+recovery), `tests/buffer_p6_09.rs` (outage→crash→restart redelivery in order, commit empties journal, backpressure-first + drop-oldest-last with latched counters, forced-disconnect drill, file/memory port parity), plus unit tests in all three new modules — ALL UNRUN (loop bans). Decision record: `08_09_ingest_buffer.md` (includes the at-least-once/dedup honesty record, the overflow policy, DEFAULT-labelled decisions, honest limitations: encryption at rest NOT implemented, counters not persisted, corruption truncates from the first bad frame, `main.rs` composition still fail-closed — wiring the read path + durable buffer + P6.07 channel into the binary root is coordinator/P6.10 territory) |
| 3a | P6.10, P6.11 | DONE — `adapters/openems.rs` (EdgeConfig discovery, versioned `openems-mapping-profile.v1.json` with unmapped/ambiguous register, GET-only HTTP/1.1 client, A1–A8 UNVERIFIED register) + e2e read-path test; registry `openems-rest` AVAILABLE (DEFAULT flip done in the coordinator pass with the pinned test updated); `10_11_openems.md` |
| 3b | P6.13, P6.14, P6.17 | DONE — protocol-agnostic conformance harness (`conformance.rs`, 9 closed §19.2 cases); command path disabled by default (`command_path.rs`: 4-condition fail-closed gate, signed time-bounded enablement, structural no-plugin-enable, audited); `policy.rs` + `health.rs` (closed vocabularies); `13_14_17_adapter_command_policy.md` |
| 4a | P6.17, P6.18, P6.21 | DONE (P6.17 in 3b) — `audit_buffer.rs` (CRC-32 journal, in-order at-least-once upload, drop-OLDEST audited, gap-event rule); tenant isolation (`EdgeSessionScope` server-side + isolation suites both sides); `18_21_audit_offline_isolation.md` |
| 4b | P6.19, P6.23, P6.22 | DONE — `flexcore-edge-adapter-conformance-suite-v1` (11 cases + bounded claim boundary, NO certification claims); §19.2(7) attack tests (both sides, distinct reasons, EMPTY findings); load harness + EMPTY performance tables; CI job additive; `19_23_22_conformance_security_performance.md` |
| 5a | P6.25, P6.31, P6.32 | DONE — `SitesEdgeStatus.tsx` (display-only command-path state; no enable endpoint exists); `clock_quality.rs` (Unknown/Synchronized/Degraded/Unsynchronized, hold-on-drift fail-closed, feeds the P6.15 verifier seam); decommissioning (revocation + wipe + upload-then-wipe-or-gap-event, `POST /api/v1/edge/decommission`); `25_31_32_console_clock_decommission.md` |
| 5b | P6.27, P6.28, P6.26 | DONE — CI gates + `deny.toml` (unlicensed dep fails by construction; green run UNRUN); drill procedure + EMPTY per-fault report (12-row reuse-by-citation); docs `docs/operations/edge-agent-operations.md`, `docs/protocols/openems-adapter.md`, `docs/security/edge-ot-boundary.md` describing the BUILT system; `26_27_28_docs_ci_drill.md` |
| 6a | P6.33, P6.34, P6.29 (+ P6.24, added by coordinator — missing from the wave plan) | DONE — exit-gate lab-run book PREPARED-UNRUN (`24_exit_gate_lab_run_record.md`); Phase 7 seam handover with real trait names + explicit non-seams + protected-kernel boundary (`29_phase7_seam_handover.md`); evidence pack with the explicitly-missing rule (`33_evidence_pack_completeness.md`); per-profile secure-boot positions PENDING PILOT HARDWARE SPEC (`34_secure_boot_position.md`) |
| 7 | P6.30 (coordinator) | DONE — `30_retrospective_release_gate.md` (retrospective, 12-row open-items register with owners, §22.3 v0.4.0 position NOT MET-prepared, plain-language not-certified/not-field-control statement); CHECKLIST.md 24/24 ticked with honest statuses; verify_docs/validate_agent/check_dependency_rule all OK (exit 0) |

## What was implemented

`crates/edge-agent` (17th workspace member): `config.rs`, `ports.rs`
(EmsSource / FORWARD-ONLY UplinkChannel / LocalBuffer / AuditSink),
`clock.rs`, `clock_quality.rs` (P6.31), `enrollment.rs` (P6.06 + P6.32
decommission), `uplink_mtls.rs` (P6.07, TLS 1.3 floor, structural
no-inbound), `ingest.rs` (P6.08 read path + dedup), `buffer_file.rs`
(P6.09 CRC-32 journal), `audit_buffer.rs` (P6.18), `adapters/{port, acl,
registry, openems, simulated}.rs` (P6.12/P6.10), `command_path.rs`
(P6.14), `command_verify.rs` (P6.15), `command_forward.rs` (P6.16),
`policy.rs` + `health.rs` (P6.17), `conformance.rs` (P6.13), `runner.rs`,
`main.rs`, `test_support.rs`; `config/openems-mapping-profile.v1.json`;
`deploy/edge/flexcore-edge-agent.service` (written NOT installed).
Control-plane: `routes_edge.rs`/`dto_edge.rs` (enroll, status, decommission
+ `EdgeSessionScope`), migration `0018`. Console: `SitesEdgeStatus.tsx` +
`api/edgeStatus.ts`. Repo-wide: `deny.toml`, CI additive jobs, three docs
guides. Tests: 21 edge-agent integration test files + control-plane API
tests + 1 vitest file — ALL WRITTEN-UNRUN. 21 decision records in this
directory.

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

**Build/test (the human's gate pass — nothing here has been compiled):**
`cargo build --workspace` → fix any compile findings (known-risk items:
the P6.14 composition wiring in `main.rs`/`config.rs` recorded in
`13_14_17_adapter_command_policy.md` §7, and the runner wiring of the
clock monitor recorded in `25_31_32_console_clock_decommission.md`) →
`cargo clippy --workspace --all-targets -- -D warnings` → `cargo test
--workspace` → `cd console && npm install && npm run test` → CI green run.
Then the lab inputs: the P6.24 exit-gate run (`24_exit_gate_lab_run_record.md`),
the conformance suite run, the P6.28 drill, the security-test run, the
secret/dependency scans, and the D6 sign-off.

**Blockers (human gate inputs, all UNRUN/NOT SIGNED/OPEN):** Stages 1–5
gates NOT HELD (recorded exception); D5 OPEN (pilot partner); D6 OPEN;
D4 re-verify (rustls, OpenEMS license); wire format O-4; TLS-on-OpenEMS-hop
O-5. Full register: `30_retrospective_release_gate.md` §2.

**Decisions still needed:** D5/D6 (NOT decidable by agents); pilot-site
load profile (P6.22); secure-boot hardware determinations (P6.34).

**Assumptions (labelled in the records):** OpenEMS REST API shape A1–A8
(UNVERIFIED); `pilot-site-A` PLACEHOLDER; rustls caret ranges
UNPINNED-REVERIFY.

**Known risks:** the entire crate is uncompiled — the first build may
surface integration defects (the coordinator pass fixed the two found so
far: the `PointMapping` import and the registry PLANNED pin); the uplink
wire format refuses, so no telemetry leaves the agent until O-4 closes;
console panel embedding (O-7) and authz action-id swap points (O-8) are
recorded owner gaps.
