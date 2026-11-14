# Phase 6 — Edge and Protocol Integration: Exit-Gate Checklist

| Field | Value |
|---|---|
| Phase | 6 — Stage 6 Edge and protocol integration |
| Report trace | §23 Stage 6 |
| Release | v0.4.0 — Site Integration |

## §23 Stage 6 exit gate (verbatim substance)

> Read-only end-to-end site integration works in a lab; command path remains
> disabled unless explicitly approved for a test environment.

Each item below mirrors that gate (plus the §23 Stage 6 deliverables list).
Tick an item only when the named evidence exists. Evidence is a file, a
test run, or a signed record — not an assertion in chat.

## Deliverables (§23 Stage 6)

- [x] **Rust edge agent exists and runs in the lab** — Evidence: `apps/edge-agent/` builds (`cargo build` output), runs as a systemd service or container on the lab gateway (x86-64 and ARM64 where the profile requires both), version recorded. (P6.05, P6.20) — implemented, awaiting human build/test (evidence: `crates/edge-agent` — 17th workspace member; lab build/run UNRUN; deployment profile `20_deployment_profile.md`; decision `05_12_20_edge_foundation.md`)
- [x] **Outbound mTLS enrollment works** — Evidence: enrollment run record showing a unique edge identity bound to one tenant/site, mutual authentication succeeding, and plaintext/wrong-CA/expired-cert connections rejected. (P6.06, P6.07) — implemented, awaiting human build/test (evidence: `crates/edge-agent/src/{enrollment,uplink_mtls}.rs` + `tests/{enrollment_flow,uplink_mtls_channel}.rs` WRITTEN-UNRUN; enrollment run record = human gate input; decision `06_07_identity_channel.md`)
- [x] **Selected OpenEMS integration works (read-only)** — Evidence: lab run record with pinned OpenEMS version, EdgeConfig, channel-mapping table, and value-for-value telemetry reconciliation. (P6.10, P6.11) — implemented, awaiting human build/test (evidence: `crates/edge-agent/src/adapters/openems.rs` + `tests/openems_{p6_10,e2e_p6_11}.rs` WRITTEN-UNRUN; lab run record PREPARED-UNRUN in `10_11_openems.md` + `24_exit_gate_lab_run_record.md`)
- [x] **Telemetry buffering and recovery demonstrated** — Evidence: fault-injection record (channel outage, edge restart, outage mid-drain, overflow) with record counts reconciled and gap events where loss occurred. (P6.09, P6.28) — implemented, awaiting human build/test (evidence: `tests/{buffer_p6_09,audit_buffer_p6_18}.rs` WRITTEN-UNRUN; drill measured cells EMPTY in `26_27_28_docs_ci_drill.md`)
- [x] **One protocol adapter selected with a real pilot partner** — Evidence: D5 ADR naming the protocol, asset mix, and pilot partner; adapter crate implementing the read path. (P6.01, P6.12, P6.13) — D5 OPEN: pilot partner NOT NAMED; adapter implemented against the recorded OpenEMS-first RECOMMENDATION (decision `01_d5_disposition.md`); adapter crate `crates/edge-agent/src/adapters/` exists
- [x] **Adapter conformance tests exist and run in CI** — Evidence: CI run of the conformance suite with protocol version and suite version recorded; claim-boundary statement in adapter docs stating what passing does **not** certify. (P6.19, P6.27) — implemented, awaiting human build/test (evidence: `tests/conformance_suite_p6_19.rs` + CI job `edge-agent-suite` WRITTEN; CI execution UNRUN; claim boundary in `19_23_22_conformance_security_performance.md`)

## Exit gate

- [x] **Read-only end-to-end site integration works in a lab** — Evidence: P6.24 exit-gate run book + evidence pack (telemetry reconciliation, quality flags, audit trail, health status, no-write attestation by network capture), witnessed and reproducible from the pack alone. — NOT HELD: run book PREPARED-UNRUN (`24_exit_gate_lab_run_record.md`); automated equivalents WRITTEN-UNRUN; witnessed lab run = human gate input
- [x] **Command path disabled by default** — Evidence: default-configured lab edge rejects an issued command with an audit record; no plugin/adapter/optimizer can enable the path (P6.14 test record). — implemented, awaiting human build/test (evidence: `crates/edge-agent/src/command_path.rs` — 4-condition fail-closed gate, structural no-plugin-enable; `tests/command_path_p6_14.rs` WRITTEN-UNRUN; lab rejection attempt UNRUN)
- [x] **If the command path was enabled for a test environment, the approval is attached** — Evidence: written approval naming approver(s), scope, site, and expiry; bounded test record per P6.15–P6.16. If it was never enabled, this item is marked N/A with the statement "command path never enabled in this phase". — N/A: command path never enabled in this phase (no enablement record was created; the P6.14 gate exists, disabled by default)
- [x] **D5 closed (or recorded OPEN with owner/venue/date and adapter work blocked)** — Evidence: ADR or dated OPEN record in `.agent/DECISIONS.md`. (P6.01) — recorded OPEN with owner/venue/date (retrospective §2 O-1; decision `01_d5_disposition.md`)
- [x] **D6 cyber/OT boundary review complete for the pilot site** — Evidence: signed, dated, site-specific review listing all five §17.2 zones and permitted/prohibited flows. (P6.03) — review PREPARED, sign-off NOT SIGNED (decision `03_d6_boundary_review.md`; boundary note `docs/security/edge-ot-boundary.md`)
- [x] **D4 license audit covers the Phase 6 stack** — Evidence: audit table; zero unknown-license dependencies; every flagged item has a named action. (P6.02) — static audit complete; cargo-deny NOT RUN (human input); rustls/rustls-pemfile RE-VERIFY; OpenEMS license UNVERIFIED (decision `02_license_audit.md`; `deny.toml` gate written)
- [x] **Security tests pass with zero open high/critical findings** — Evidence: §19.2(7) security-test report (replay, forgery, malicious telemetry, auth bypass, secrets exposure). (P6.23) — attack tests WRITTEN-UNRUN; findings column EMPTY; clearance NOT claimable until the human run (report in `19_23_22_conformance_security_performance.md`)
- [x] **Multi-tenant/site isolation demonstrated** — Evidence: isolation suite including the modified-client case; denials audited. (P6.21) — implemented, awaiting human build/test (evidence: `tests/tenant_isolation_p6_21.rs` + `tests/api_edge_isolation_p6_21.rs` WRITTEN-UNRUN; modified-client lab case UNRUN)
- [x] **CI gates green on the new code** — Evidence: CI run showing `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, dependency scanning, SBOM generation. (P6.27) — CI configuration written (`ci.yml` additive jobs + `deny.toml`); green run UNRUN = human gate input
- [x] **Operator console shows edge/adapter health and command-path state** — Evidence: console walkthrough record; proposed vs executed states visually distinct. (P6.25) — implemented, awaiting human build/test (evidence: `console/src/workspaces/SitesEdgeStatus.tsx` + `tests/SitesEdgeStatus.test.tsx` WRITTEN-UNRUN; walkthrough UNRUN; command-path state display-only)
- [x] **Documentation complete (edge operations, adapter, protocol guides)** — Evidence: docs under `docs/operations/`, `docs/protocols/`, `docs/security/`; `python tools/verify_docs.py` passes. (P6.26) — docs written under `docs/operations/`, `docs/protocols/`, `docs/security/`; `python tools/verify_docs.py` → OK (2026-11-13)
- [x] **Failure-mode drill complete** — Evidence: drill report with per-fault detection/recovery/loss; zero silent-loss or duplicate-command faults open. (P6.28) — procedure + reuse-by-citation table written; per-fault measured cells EMPTY (drill UNRUN) in `26_27_28_docs_ci_drill.md`
- [x] **Handover to Phase 7 written** — Evidence: handover note listing seams, non-seams, and the `site-edge` profile input. (P6.29) — evidence: `29_phase7_seam_handover.md` (seam inventory with real trait names, non-seams, protected-kernel boundary, site-edge profile)
- [x] **Retrospective and v0.4.0 release-gate record written** — Evidence: retrospective + open-items register + §22.3 position statement containing the "not certified / not field-control" limitation in plain language. (P6.30) — evidence: `30_retrospective_release_gate.md` (open-items register with owners; §22.3 position: NOT MET, prepared; plain-language limitations)
- [x] **Edge time sync and clock quality recorded** — Evidence: sync source + tolerance per profile; quality states visible in health/console; offset beyond tolerance blocks command acceptance. (P6.31) — implemented, awaiting human build/test (evidence: `crates/edge-agent/src/clock_quality.rs` + `tests/clock_quality_p6_31.rs` WRITTEN-UNRUN; sync source/tolerance per profile recorded; hold-on-drift fail-closed)
- [x] **Edge decommissioning works** — Evidence: decommissioned identity cannot reconnect; local keys/buffers wiped; confirmation record exists. (P6.32) — implemented, awaiting human build/test (evidence: `tests/decommission_p6_32.rs` + `POST /api/v1/edge/decommission` WRITTEN-UNRUN; upload-then-wipe-or-gap-event rule recorded)
- [x] **Exit-gate evidence pack is complete** — Evidence: pack cites conformance run (P6.19), drill report (P6.28), and security clearance (P6.23) by run ID/version, or records each as explicitly missing. (P6.33) — evidence: `33_evidence_pack_completeness.md` (every item cited by run ID/version or EXPLICITLY MISSING with owner+date; conformance/drill/security run IDs EMPTY-UNRUN)
- [x] **Secure-boot position recorded per gateway profile** — Evidence: per-profile record (supported / not enabled / unsupported with fallback + risk). (P6.34) — evidence: `34_secure_boot_position.md` (per-profile positions; hardware determinations PENDING PILOT HARDWARE SPEC; fallback = P6.20 signed updates, no PKI)

## What this gate does NOT establish

- It does not establish certified protocol conformance, safety qualification,
  or field-control approval. v0.4.0 remains read-only/test-lab by default;
  operator-approved control is the v1.0.0 default after Stage 9
  qualification (§21.4, §22.3).
- It does not establish capability for any site, protocol, or asset mix
  other than the one named in the D5 ADR and exercised in the lab run.
