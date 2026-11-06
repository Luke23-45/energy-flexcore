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

- [ ] **Rust edge agent exists and runs in the lab** — Evidence: `apps/edge-agent/` builds (`cargo build` output), runs as a systemd service or container on the lab gateway (x86-64 and ARM64 where the profile requires both), version recorded. (P6.05, P6.20)
- [ ] **Outbound mTLS enrollment works** — Evidence: enrollment run record showing a unique edge identity bound to one tenant/site, mutual authentication succeeding, and plaintext/wrong-CA/expired-cert connections rejected. (P6.06, P6.07)
- [ ] **Selected OpenEMS integration works (read-only)** — Evidence: lab run record with pinned OpenEMS version, EdgeConfig, channel-mapping table, and value-for-value telemetry reconciliation. (P6.10, P6.11)
- [ ] **Telemetry buffering and recovery demonstrated** — Evidence: fault-injection record (channel outage, edge restart, outage mid-drain, overflow) with record counts reconciled and gap events where loss occurred. (P6.09, P6.28)
- [ ] **One protocol adapter selected with a real pilot partner** — Evidence: D5 ADR naming the protocol, asset mix, and pilot partner; adapter crate implementing the read path. (P6.01, P6.12, P6.13)
- [ ] **Adapter conformance tests exist and run in CI** — Evidence: CI run of the conformance suite with protocol version and suite version recorded; claim-boundary statement in adapter docs stating what passing does **not** certify. (P6.19, P6.27)

## Exit gate

- [ ] **Read-only end-to-end site integration works in a lab** — Evidence: P6.24 exit-gate run book + evidence pack (telemetry reconciliation, quality flags, audit trail, health status, no-write attestation by network capture), witnessed and reproducible from the pack alone.
- [ ] **Command path disabled by default** — Evidence: default-configured lab edge rejects an issued command with an audit record; no plugin/adapter/optimizer can enable the path (P6.14 test record).
- [ ] **If the command path was enabled for a test environment, the approval is attached** — Evidence: written approval naming approver(s), scope, site, and expiry; bounded test record per P6.15–P6.16. If it was never enabled, this item is marked N/A with the statement "command path never enabled in this phase".
- [ ] **D5 closed (or recorded OPEN with owner/venue/date and adapter work blocked)** — Evidence: ADR or dated OPEN record in `.agent/DECISIONS.md`. (P6.01)
- [ ] **D6 cyber/OT boundary review complete for the pilot site** — Evidence: signed, dated, site-specific review listing all five §17.2 zones and permitted/prohibited flows. (P6.03)
- [ ] **D4 license audit covers the Phase 6 stack** — Evidence: audit table; zero unknown-license dependencies; every flagged item has a named action. (P6.02)
- [ ] **Security tests pass with zero open high/critical findings** — Evidence: §19.2(7) security-test report (replay, forgery, malicious telemetry, auth bypass, secrets exposure). (P6.23)
- [ ] **Multi-tenant/site isolation demonstrated** — Evidence: isolation suite including the modified-client case; denials audited. (P6.21)
- [ ] **CI gates green on the new code** — Evidence: CI run showing `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, dependency scanning, SBOM generation. (P6.27)
- [ ] **Operator console shows edge/adapter health and command-path state** — Evidence: console walkthrough record; proposed vs executed states visually distinct. (P6.25)
- [ ] **Documentation complete (edge operations, adapter, protocol guides)** — Evidence: docs under `docs/operations/`, `docs/protocols/`, `docs/security/`; `python tools/verify_docs.py` passes. (P6.26)
- [ ] **Failure-mode drill complete** — Evidence: drill report with per-fault detection/recovery/loss; zero silent-loss or duplicate-command faults open. (P6.28)
- [ ] **Handover to Phase 7 written** — Evidence: handover note listing seams, non-seams, and the `site-edge` profile input. (P6.29)
- [ ] **Retrospective and v0.4.0 release-gate record written** — Evidence: retrospective + open-items register + §22.3 position statement containing the "not certified / not field-control" limitation in plain language. (P6.30)
- [ ] **Edge time sync and clock quality recorded** — Evidence: sync source + tolerance per profile; quality states visible in health/console; offset beyond tolerance blocks command acceptance. (P6.31)
- [ ] **Edge decommissioning works** — Evidence: decommissioned identity cannot reconnect; local keys/buffers wiped; confirmation record exists. (P6.32)
- [ ] **Exit-gate evidence pack is complete** — Evidence: pack cites conformance run (P6.19), drill report (P6.28), and security clearance (P6.23) by run ID/version, or records each as explicitly missing. (P6.33)
- [ ] **Secure-boot position recorded per gateway profile** — Evidence: per-profile record (supported / not enabled / unsupported with fallback + risk). (P6.34)

## What this gate does NOT establish

- It does not establish certified protocol conformance, safety qualification,
  or field-control approval. v0.4.0 remains read-only/test-lab by default;
  operator-approved control is the v1.0.0 default after Stage 9
  qualification (§21.4, §22.3).
- It does not establish capability for any site, protocol, or asset mix
  other than the one named in the D5 ADR and exercised in the lab run.
