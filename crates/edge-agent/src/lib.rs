// P6.05 — `flexcore-edge-agent`: the edge agent library and composition-root
// design (report §18.1, §8.2, §3.3; ledger P6.05/P6.12).
//
// The edge agent is a separate Rust daemon at the site (in the OT DMZ),
// distinct from the control plane (§3.3, §8.3). Its structural differences
// from the control plane, recorded here because every module below follows
// from them:
//
// - **Offline-tolerant.** The control plane treats its store and broker as
//   always present (fail-fast startup). The edge agent must keep ingesting
//   and buffering when the uplink is down: the supervision loop
//   ([`runner`]) runs DEGRADED, not stopped, and the buffer port
//   ([`ports::LocalBuffer`]) is the degraded-mode spine (P6.09 wires the
//   durable disk buffer; this wave ships the port + a bounded in-memory
//   implementation for tests).
// - **Outbound-only.** The edge initiates the connection to the central
//   platform (§3.3); there is no inbound listener (P6.07 implements the
//   mTLS channel). The [`ports::UplinkChannel`] port is forward-only.
// - **Read-only by default.** v0.4.0 control status is read-only /
//   test-lab (§2.1, §21.4). The command-path flag in
//   [`config::EdgeConfig`] defaults to `false`, and no command-receive
//   path exists in this wave at all — the P6.14 gate implements the
//   disabled-by-default receiver later. Nothing in this crate implies a
//   write path is active.
//
// Module map (the P6.05 step-1 design note, in code):
//
// | Module | Responsibility | Later-wave adapters |
// |---|---|---|
// | [`config`] | Edge deployment configuration, fail-fast parse, command-path flag defaulting FALSE | P6.16 allowlist config |
// | [`ports`] | The edge-local hexagonal ports: `EmsSource`, `UplinkChannel`, `LocalBuffer`, `AuditSink` (+ the canonical edge sample type and fail-closed stubs) | disk buffer (P6.09), mTLS uplink (P6.07) |
// | [`runner`] | Supervision loop: poll → buffer → forward, degraded-mode state machine, graceful shutdown | P6.09 wire-through |
// | [`ingest`] | P6.08: the telemetry read path — `EmsAdapter.read` → ACL mapping → canonical `EdgeTelemetrySample` → `EmsSource`, with per-poll error accounting and the bounded edge dedup window (the P4.24 identity discipline, cited) | OpenEMS composition (P6.10); health surfacing (P6.17) |
// | [`buffer_file`] | P6.09: the DURABLE store-and-forward buffer — append-only CRC-checked journal, truncate-and-audit recovery, audited drop-oldest overflow, age retention, at-least-once commit semantics | encryption at rest (§18.3 — recorded open item); health surfacing (P6.17) |
// | [`enrollment`] | P6.06: `EnrollmentClient`/`CredentialStore`/`RevocationChecker` ports, `TokenEnrollmentAdapter`, filesystem credential store, expiry monitor (certificate issuance + OCSP/CRL are DEPLOYMENT REQUIREMENTS — recorded, not implemented) | the deployment's CA + OCSP/CRL adapters |
// | [`uplink_mtls`] | P6.07: the outbound mTLS `UplinkChannel` over rustls — edge-initiated, no inbound listener (structural), pinned TLS minimum (DEFAULT 1.3), reconnect/backoff/keepalive, ack-confirmed at-least-once | telemetry payload encoder (P6.08/P6.09) |
// | [`adapters`] | The protocol-adapter port (`EmsAdapter`), the anti-corruption layer (EMS-native → canonical), the adapter registry (allowlist), and the `simulated-test` TEST-ONLY scenario fixture (P6.08 — never a real EMS) | `openems-rest` (P6.10/P6.13); `generic-rest` is a RESERVED id, never implemented in this phase |
//
// Dependency direction (§8.2, enforced by `tools/check_dependency_rule.py`,
// which classifies this crate as COMPOSITION): this crate depends on
// `flexcore-contracts` and `flexcore-ports` (the hexagonal inside) plus
// composition-side infrastructure (tokio, time, tracing); future adapter
// crates depend on the [`adapters::port`] port — never the reverse. No
// `flexcore-domain` dependency: the edge carries site/tenant bindings as
// configuration, and the asset-key → registry-`AssetId` mapping exists only
// at registration on the control-plane side (the P2.25 step-2 pattern,
// cited not reinvented).
//
// Status (honest, per AGENTS.md rule 7): the crate is written, NOT built
// and its tests are NOT run — this wave works under the recorded loop bans
// (`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`). Ticks read
// "implemented, awaiting human build/test".

pub mod adapters;
// P6.09 (wave 2b) — the durable journal-backed store-and-forward buffer
// (CRC-checked records, truncate-and-audit recovery, audited drop-oldest
// overflow, age retention, at-least-once commit semantics).
pub mod buffer_file;
pub mod audit_buffer;
pub mod clock;
// P6.14 (wave 2c) — the edge command receiver: DISABLED BY DEFAULT, behind
// the four-condition, all-or-nothing, fail-closed test-env enablement gate
// (signed per-site config change + signed time-bounded enablement record +
// site command allowlist + D6 record reference). No inbound listener in any
// state: commands ride the response side of the outbound channel; the
// P6.17 policy engine is consulted through the `CommandPolicyGate` seam.
pub mod command_path;
pub mod config;
// P6.13 (wave 2c) — the protocol-agnostic adapter conformance harness: a
// reusable test-side driver for the §19.2(6) fault-injection set (the
// P6.19 building block; P6.19 owns the conformance claim boundary).
pub mod conformance;
// P6.16 (wave 3b) — command forwarding ONLY to allowlisted adapters, with
// the per-asset §14.4 fallback policy: the `CommandForwarder` port (defined
// in-module, NOT in ports.rs — the recorded design decision: the read-path
// `EmsAdapter` port keeps its structural no-write guarantee; the command
// surface is the separately-gated one), the fail-closed production stub,
// the separate forward-side allowlist re-check with its own audit event,
// §14.1 issue/ack/tracking recording (acknowledgement is NOT delivery), and
// the closed fallback option set (NO universal "set power to zero"
// fallback) behind a REQUIRED operator-review field (recorded, NOT SIGNED
// in this loop — human gate input).
pub mod command_forward;
// P6.15 (wave 3b) — the four-check edge command verifier, run on every
// gate-admitted command BEFORE any forwarding: (1) payload-bound signature
// against the central platform's key reference, (2) validity window against
// the edge clock WITH the fail-closed clock-quality check (P6.31 supplies
// the status), (3) monotonic per-asset/channel sequence with a persisted
// restart checkpoint (the ledger's replay-after-restart risk), (4) target
// allowlist + approved scope/limits/expiry in the authorization reference
// (§17.5 rule 6). Every rejection carries a DISTINCT named reason
// (signature/expiry/sequence/duplicate/allowlist/scope) and is audited.
pub mod command_verify;
// P6.17 (wave 2c) — per-adapter health monitoring: the closed HEALTHY /
// DEGRADED_<reason> / FAILING_<reason> vocabulary, derived ONLY from
// observed inputs (unknown = DEGRADED_UNVERIFIED, never healthy), with
// last-seen timestamps, the P6.06 certificate-expiry input, blocked
// dependent actions on FAILING_*, and the serializable HealthSnapshot for
// P6.25's console surfacing.
pub mod health;
// P6.08 (wave 2b) — the telemetry ingestion read path (EmsAdapter read →
// ACL → canonical sample → EmsSource) with per-poll error accounting and
// the bounded edge dedup window (P4.24 discipline, cited).
pub mod ingest;
// P6.06 (wave 2a) — edge identity, enrollment, and the certificate
// lifecycle (EnrollmentClient / CredentialStore / RevocationChecker ports,
// the TokenEnrollmentAdapter, the FilesystemCredentialStore, the expiry
// monitor, and the honesty record for what remains a deployment
// requirement). `test_support` is TEST-ONLY tooling (temp-dir credential
// store harness) and is never referenced by the binary.
pub mod enrollment;
pub mod ports;
// P6.17 (wave 2c) — the site-local policy engine: per-adapter enablement,
// command rate limits, and allowed operating windows, enforced at the edge
// (a disconnected edge still enforces something). Violations are reject +
// audit, never silently proceed; the engine implements the command path's
// `CommandPolicyGate` seam (policy → command_path, no cycle).
pub mod policy;
pub mod runner;
// P6.07 (wave 2a) — the outbound mTLS uplink channel over rustls
// (edge-initiated, no inbound listener anywhere in this crate — enforced by
// `tests/no_inbound_listener.rs`).
pub mod uplink_mtls;
// P6.31 (wave 3c) — edge time synchronization and clock-quality monitoring:
// the CLOSED Unknown/Synchronized/Degraded/Unsynchronized state vocabulary
// (never healthy-by-default), the named-allowlisted sync-source
// configuration with per-profile DEFAULT tolerances, offset estimation from
// the bounded §10.3 event_time-vs-ingest_time sample pairs (NO NTP client —
// a new dependency + a new D6 network path, recorded), the hold-on-drift
// rule (expiry checks trusted ONLY on Synchronized; commands rejected/
// held fail-closed and audited otherwise), the P6.15 verifier parameter
// seam, and the additive P6.17 health seam (DEGRADED_CLOCK_DRIFT).
pub mod clock_quality;
// TEST SUPPORT ONLY (P6.06 harness: temp-dir credential store) — public so
// both in-module and integration tests share one harness, exactly like the
// in-memory fakes in `ports`; never referenced by the binary or by any
// production path.
pub mod test_support;
// P9.31 (pilot wave 1) — the forged/replayed/expired/out-of-sequence
// rejection PROOF surface: the attack-case runner over the composed P6.15
// four-check verifier (rejections PROVEN BY EXERCISE, never re-implemented),
// the closed attack-case vocabulary with its expected named rejection codes,
// the fail-loud ErroneouslyAccepted outcome, and the ADDITIVE §14.3
// edge-stand-in control — the restart-surviving command-id binding journal
// (CRC-32 framing, truncate-and-audit) that refuses a conflicting payload
// under an already-used idempotency id before the verifier runs. Nothing
// here forwards, queues, or retains a rejected command.
pub mod pilot_rejection_proof;
// P9.32 (pilot wave 1) — the audit-integrity PROOF surface: the FNV-1a-64
// hash chain over the drained `BufferedAuditRecord`s (content, order, count,
// and sequence range all committed to the head), the platform-retained
// `ChainAnchor` comparison with named per-field mismatches, the
// authority-review export bundle + verified-evidence record, and the honest
// tamper-EVIDENCE-not-tamper-PROOF bound. The durable offline buffer it
// verifies over is the EXISTING P6.18 `audit_buffer` (cited, not forked).
pub mod audit_proof;
// P9.33 (pilot wave 2) — the edge enrollment CEREMONY as code: the closed
// ceremony state machine (Unenrolled → Requested → Challenged → Approved →
// Active → Revoked; every transition named; NO self-approval path — the
// approval is the recorded platform-side human/authority gate), the
// nonce-based challenge with ceremony-message replay protection (INJECTED
// clock; test-grade FNV derivation, CSPRNG is the recorded DEPLOYMENT
// REQUIREMENT), the single-use/short-lived token discipline (spent/expired
// reuse refused), the compromise re-enrollment rule (a revoked identity can
// never authorize its successor), and the append-only identity-lifecycle
// register (CRC-32 framing, truncate-and-audit). The enrollment EXECUTION
// itself is the recorded UNRUN human gate.
pub mod enrollment_ceremony;
// P9.34 (pilot wave 2) — the signed edge-update VERIFICATION and ROLLBACK
// surface: the update manifest (version, digest, signer key REFERENCE,
// minimum-version floor, approval reference), verification BEFORE any write
// (fail-closed `NoUpdateVerifier` default mirroring the kernel P7.14
// signing pattern; the test-grade FNV-1a-64 verifier is loudly labelled and
// the production hash is a DEPLOYMENT REQUIREMENT), the append-only
// update/rollback register, and the never-silent downgrade rule (a rollback
// below the floor is refused unless an explicit recorded override exists).
// NO update has ever been built or applied; secure boot is NOT established
// here (the P6.34 position stands).
pub mod update_verification;
