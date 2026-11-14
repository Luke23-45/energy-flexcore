# P6.29 — Handover to Phase 7 (plugin SDK): edge seams, non-seams, and the hard boundary

Date: 2026-11-13. Subtask: P6.29, Phase 6 ledger (`ledger_phase_6.md` §P6.29).
Owner role: architect (per the ledger). Report trace: §5.2; §9.1; §23 Stage 7;
§5.7; §5.8.

**Honesty status:** every trait/struct/enum name below was verified against
the actual source files in `crates/edge-agent/src/` on 2026-11-13 (names
exist verbatim in the named module). The crate's overall version state is
**WRITTEN, NOT BUILT, NOT RUN** — every Phase 6 decision record carries
"implemented, awaiting human build/test", and that is the state this handover
carries: no seam below is a *verified-by-execution* contract yet. The
toolchain is Rust 1.99.1 stable, exact-pinned (`rust-toolchain.toml`,
P1.02). This record invents no contract that is not in the source, and
lists explicitly what does NOT exist (§2).

Purpose: Phase 7 (P7.01/P7.04 — readiness check and capability-seam
inventory) starts from this note. The Phase 7 ledger already names this
dependency: "If P6.29 was not delivered, P7.04 must fail" (`ledger/
phase-07-plugin-sdk/ledger_phase_7.md`, P7.01 inputs).

## 1. THE SEAM INVENTORY (real contracts only)

Owner crate throughout: `flexcore-edge-agent` (source root
`crates/edge-agent/src/` — note the recorded placement deviation from report
§8.1's `apps/edge-agent`, decision record `05_12_20_edge_foundation.md` §2).
Where a contract type comes from another crate it is named.

### 1.1 Adapter read-path seams (P6.05/P6.08/P6.10/P6.12/P6.13)

| Seam (exact names) | Owner module | Version / state | § trace | Notes |
|---|---|---|---|---|
| `EmsAdapter` (trait: `discover`, `read`, `capabilities`, `health` — NO dispatch/write method, absence is structural) | `adapters/port.rs` | written, UNBUILT | §9.1; §6.4; §2.3; P6.12 | the adapter port; adapters depend on it, never the reverse (§8.2) |
| `AdapterCapabilities`, `ChannelDescriptor`, `AdapterHealth` (enum), `AdapterError`, `PointKey`, `EmsNativeReading` | `adapters/port.rs` | written, UNBUILT | §9.1; §10.2–§10.4; P6.12 | capability honesty is a REQUIRED declaration (P0.07/P0.08/P0.09); `AdapterCapabilities::clock_quality` is declared, never assumed (A8) |
| `AntiCorruptionLayer`, `PointMapping` (structs); `NativeUnit`, `NativeTimestamp`, `NativeSign`, `AclError` (enums) | `adapters/acl.rs` | written, UNBUILT | §6.4; §10.2–§10.4; P6.12 | the ONLY path from EMS-native to canonical; points/units/quality/time mapped-or-refused; quality mapping table frozen (GOOD/BAD/STALE/SUBSTITUTED→ESTIMATED/OUT_OF_RANGE·UNSYNCHRONIZED·UNRELIABLE→SUSPECT/COMMUNICATION_LOSS→STALE); native MISSING refused (gap markers are platform-derived, P2.04 D-T6) |
| `OpenEmsRestAdapter`, `OpenemsRestTransport` (trait, GET-only), `TcpHttpGetTransport`, `OpenemsMappingProfile`, `ProfileMapping`, `ProfileUnmapped`, `ChannelMappingStatus`, `ChannelMappingEntry` | `adapters/openems.rs` | written, UNBUILT; REST API shape UNVERIFIED (A1–A8 register) | §2.3; §18.1; §8.1; P6.10 | registry id `openems-rest` (`OPENEMS_REST_ADAPTER_ID`); versioned mapping profile `openems-mapping-profile.v1.json` (`PROFILE_ID`, `SUPPORTED_PROFILE_VERSION = 1`); freshness via `stale_after_millis` |
| `SimulatedEmsSource`, `SimControl`, `SimulatedFixture` (registry id `simulated-test`, `SIMULATED_ADAPTER_ID`, `AdapterStatus::TestOnly`) | `adapters/simulated.rs` | written, UNBUILT | §20.2; P6.08 | TEST-ONLY; never a real EMS; production composition must not wire it |
| `AdapterRegistry` (allowlist: `openems-rest` = `AdapterStatus::Implemented` via `with_openems_available()`; `generic-rest` = `AdapterStatus::Reserved`, always refused; `with_default_allowlist()` still carries the legacy `Planned` entry — recorded one-line follow-up), `AdapterStatus`, `RegistryError` | `adapters/registry.rs` | written, UNBUILT | §9.3; P6.12 | extending the allowlist is a reviewed change (mapping table + P6.02 audit note) |
| `AdapterIngestSource`, `EdgeDedupWindow`, `DedupKey`, `AdmitOutcome`, `IngestStats` | `ingest.rs` | written, UNBUILT | §18.1; §10.3–§10.4; §6.3; P6.08 | dedup identity = (source identity, `event_time`); per-poll error accounting counters; `flexcore:` source namespace refused |
| `EdgeTelemetrySample` (+ `EdgeTelemetrySampleParts`, `EdgeSampleError`), `EdgeError` | `ports.rs` | written, UNBUILT | §10.1–§10.4; §10.3 | the canonical edge-side output vocabulary; `ingest >= event` enforced by the validated constructor; types from `flexcore-ports`/`flexcore-contracts` (P0.07/P0.09) |

### 1.2 Transport and buffering seams (P6.07/P6.09/P6.18)

| Seam (exact names) | Owner module | Version / state | § trace | Notes |
|---|---|---|---|---|
| `UplinkChannel` (trait: `is_connected`, `forward` — **FORWARD-ONLY, no receive method**, documented in the trait) | `ports.rs` | written, UNBUILT | §3.3; §18.1–§18.2; §15.4; P6.07 | outbound-only; `FailClosedUplink` is the shipped fail-closed impl |
| `MtlsUplinkChannel` (+ `compose`), `MtlsUplinkConfig`, `TlsMinimum` (DEFAULT tls13; `tls12-floor` opt-in), `BackoffSchedule` (deterministic ×2, no jitter), `ChannelStatus`, `RustlsUplinkTransport`, `RustlsUplinkTransportFactory`, `UplinkTransport`/`UplinkTransportFactory` (traits), `KeyMaterialSource`, `FileKeyMaterialSource`, `ChannelMaterial`, `assemble_material`, `build_client_config` | `uplink_mtls.rs` | written, UNBUILT; **NO TLS handshake ever executed** | §3.3; §17.3; §17.4; P6.07 | rustls/rustls-pemfile UNPINNED-REVERIFY (D4); root CA pinned from the credential record; endpoint scheme `wss://`/`https://` REQUIRED |
| `BatchPayloadEncoder` (trait) + `NoBatchPayloadEncoder` (shipped default — **REFUSES encoding**) | `uplink_mtls.rs` | seam with refusing default; **the wire format is an OPEN item** | §15.1 (ADR-008); P6.07 D-4 | the uplink wire format is deliberately NOT invented; the owner implements this seam — Phase 7 must not assume a wire format exists (see §2) |
| `LocalBuffer` (trait: `len`, `capacity`, `append`, `drain`, `prepend`, `commit_forwarded`), `LocalBufferExt`; `InMemoryLocalBuffer` (volatile, test); `FileLocalBuffer` (durable journal) | `ports.rs`; `buffer_file.rs` | written, UNBUILT; **encryption at rest NOT implemented (DEPLOYMENT REQUIREMENT)** | §18.1; §18.3; §21.2; §15.4; P6.09 | journal `FLEXCORE-EDGE-TELEMETRY-JOURNAL` v0x01, CRC-32 records, audited drop-of-oldest (`drop-oldest-audited` is the only accepted policy), truncate-and-audit recovery, at-least-once commit |
| `AuditSink` (trait), `EdgeAuditEvent`, `EdgeAuditKind` (CLOSED vocabulary — command-path events currently ride `EDGE_STARTUP` with the dedicated `EDGE_COMMAND_*` name leading the detail; the dedicated variants are the recorded follow-up), `InMemoryAuditSink` | `ports.rs` | written, UNBUILT | §17.3; §6.5; §21.2; P6.05/P6.18 | |
| `FileAuditBuffer` (+ `BufferedAuditKind`, `BufferedAuditRecord`, `AuditUploadError`, `FileAuditBufferError`) | `audit_buffer.rs` | written, UNBUILT; encryption at rest NOT implemented (DEPLOYMENT REQUIREMENT) | §18.1; §18.3; §21.2; P6.18 | durable offline audit; in-order, fail-closed upload commit (`OutOfOrderCommit`); reconciliation identity `appended_total == committed + pending + dropped`; NO age-based drop for audit records (deployment `RetentionPolicy` decision, P0.16 §4) |

### 1.3 Command-path seams (P6.14/P6.15/P6.16/P6.17) — PROTECTED KERNEL BEHAVIOUR (see §3)

| Seam (exact names) | Owner module | Version / state | § trace | Notes |
|---|---|---|---|---|
| `CommandPathRuntime`, `CommandReceiver` (+ `CommandPathGateInputs`, `SignedEnablementRecord`, `EnablementKey`, `GateRefusal`, `CommandPathEnablement`, `CommandPathError`, `EdgeCommand`, `CommandRefusal`, `CommandRejectionAuditRecord`, `CommandReceipt`, `GatedCommandPipeline`), record format `flexcore-edge-command-enablement-v1` (`ENABLEMENT_RECORD_FORMAT`) | `command_path.rs` | written, UNBUILT | §14.1; §21.4; §17.5; §5.7; §23 Stage 6; P6.14 | the 4-condition gate (config flag → signed record + token → site command allowlist → D6 record reference); enablement is a PRIVATE field computed once by `CommandPathRuntime::evaluate` (the only constructor; no setter, no `&mut self` path) and re-derived read-time with expiry — no plugin can flip it (§3 below) |
| `CommandVerifier` (+ `verify(..., clock_quality: ClockQuality)` — the clock-quality PARAMETER seam), `CommandSigningKey`, `IncomingCommand`, `canonical_command_binding`, `sign_command`, `CommandAuthorization`, `AssetCommandScope`, `CommandVerificationRejection` (`signature`/`expiry`/`sequence`/`duplicate`/`allowlist`/`scope`), `VerifiedCommand`; `SequenceCheckpointStore` (trait) + `FileSequenceCheckpoint` (durable restart-replay defence) | `command_verify.rs` | written, UNBUILT; signature digest is keyed FNV-1a (a keyed CHECKSUM, NOT a cryptographic MAC — cryptographic upgrade is a DEPLOYMENT REQUIREMENT) | §14.3; §17.3–§17.5; §18.1; P6.15 | `ClockQuality` is the `flexcore_contracts::time_model` three-value vocabulary; any quality ≠ `Synchronized` fails the expiry check closed (§17.4) |
| `CommandForwarder` (port, defined IN `command_forward.rs` — in-module, NOT `ports.rs`) + `FailClosedCommandForwarder` (production impl: every call fails with a named transport error); `CommandForwardingEngine`, `SiteCommandAllowlist`, `ForwardRequest`, `ForwardOutcome`, `ForwardError`, `ForwardRecord`, `TrackingObservation`, `FallbackActivation`, `FallbackVariant` (closed 4-variant set; the universal "set power to zero" fallback is STRUCTURALLY ABSENT), `FallbackTrigger`, `FallbackPolicyConfig`/`FallbackPolicyEntry`/`OperatorReview` (format `flexcore-edge-fallback-policy-v1`; operator-review field REQUIRED to load, **NOT SIGNED** in this loop) | `command_forward.rs` | written, UNBUILT | §14.4; §14.1; §21.2; P6.16 | reachable ONLY behind gate + verifier + policy re-check; the read path's `EmsAdapter` has no write method and cannot reach this port |
| `CommandPolicyGate` (trait, defined in `command_path.rs`) + `SitePolicyEngine` (+ `SitePolicyConfig`, `AdapterPolicy`, `RateLimit`, `OperatingWindow`, `PolicyViolation`, `PolicyError`; format `flexcore-site-policy-v1`) | `policy.rs` | written, UNBUILT | §18.1; §18.3; §21.3; P6.17 | per-adapter enablement, rate limits, operating windows; a command path without an independent policy gate does not exist (fail-closed by composition) |

### 1.4 Health, clock, enrollment, config seams (P6.06/P6.17/P6.31/P6.32/P6.05)

| Seam (exact names) | Owner module | Version / state | § trace | Notes |
|---|---|---|---|---|
| `AdapterHealthMonitor`, `HealthSnapshot`, `AdapterHealthRecord`, `HealthState` (closed: `HEALTHY`, `DEGRADED_<reason>`, `FAILING_<reason>`), `HealthReason` (closed set: `UNVERIFIED`, `UNRESPONSIVE`, `STALE_DATA`, `ERROR_RATE`, `POLICY_BLOCK`, `CREDENTIAL_EXPIRY`), `AdapterObservation`, `HealthLimits`, `BlockedAction` | `health.rs` | written, UNBUILT; P6.25 console surfacing consumes it (display-only) | §18.3; §21.3; P6.17 | health is never inferred from the absence of errors; only `FAILING_*` blocks dependent actions |
| `ClockQualityMonitor`, `ClockQualityState` (closed: `UNKNOWN`/`SYNCHRONIZED`/`DEGRADED`/`UNSYNCHRONIZED`; `is_expiry_trusted`, `for_p6_15_verifier`), `TimeSyncProfile`, `SyncSourceConfig`, `TimeSamplePair`, `ClockQualityError`, `audited_expiry_check` | `clock_quality.rs` | owned by **ledger P6.31** (its own decision record is authoritative for its status); the P6.15 consumer contract (fail-closed on any non-`Synchronized` state) is recorded in `15_16_command_verify_forward.md` §2.2 | §10.3; §17.4; §14.3; P6.31 | the PRODUCER of the clock-quality parameter the verifier consumes; lab tolerance DEFAULT 500 ms, pilot DEFAULT 100 ms (recorded DEFAULTs) |
| Enrollment: `EnrollmentClient`, `CredentialIssuer` (+ `DenyAllCredentialIssuer` shipped default — refuses everything), `ScriptedCredentialIssuer` (test), `TokenEnrollmentAdapter`, `CredentialStore` + `FilesystemCredentialStore` (format `flexcore-edge-credential-v1`; key MATERIAL never stored), `RevocationChecker` + `UnconfiguredRevocationChecker` (fails closed), `ProvisioningToken`, `PemBody`, `DeviceBinding`, `DeviceCredentialRecord`, `CredentialExpiryMonitor`, `ExpiryStatus`, `RevocationStatus`, `EnrollmentOrchestrator`; decommission: `DecommissionConfirmation`, `DecommissionGapLedger`, `DecommissionWipe`, `AuditSettlement` | `enrollment.rs` | written, UNBUILT; the CA/issuance/revocation service and token-issuing service are **DEPLOYMENT REQUIREMENTS** (no PKI exists) | §18.1; §18.3; §17.3; §16.2; P6.06; P6.32 | identity bound to exactly one tenant + approved site; server-side registry migration 0018 (RLS ENABLE+FORCE); decommission owned by ledger P6.32 (its record is authoritative) |
| `EdgeConfig` (the agent configuration struct; endpoint redacted in `Debug`; `command_path_enabled` DEFAULT FALSE) + the env-var surface (`FLEXCORE_EDGE_*` constants in `config.rs`) | `config.rs` | written, UNBUILT | §18.3; §21.4; P6.05 | the shipped environment file deliberately omits the command-path flag (P6.20) |
| `EdgeRunner` (+ `RunnerParams`, `TickReport`), `WallClock` (the P1.07 `Clock` port) | `runner.rs`; `clock.rs` | written, UNBUILT | §18.1; §3.5; P6.05 | the supervision loop: poll → buffer → forward, degraded-mode state machine, audited backpressure |
| Conformance: `ConformanceHarness`, `ConformanceCase` (closed 9-case set), `ConformanceReport`, `CaseOutcome`, `CaseRecord`, `ConformanceLimits` | `conformance.rs` | written, UNBUILT; suite id `flexcore-edge-adapter-conformance-suite-v1` (P6.19) | §19.2(3)/(6); P6.13/P6.19 | protocol-agnostic; not, and never quotable as, protocol conformance/certification (§2.3) |

## 2. THE NON-SEAMS (explicit — Phase 7 must define these; none exists)

| Non-seam | What actually exists | What Phase 7 owns |
|---|---|---|
| WIT interfaces for adapters | **NONE.** No `wit/` package, no WIT world, no Component Model surface exists anywhere in the edge crate. Plugins are out-of-process or WASM only (report §5.4; AGENTS.md §8; ADR-006 — no arbitrary Rust `dylib` loading; Phase 6 PLAN §9 "no plugin loader"). The edge seams above are RUST TRAITS consumed in-process by the edge composition root — they are not yet plugin-reachable interfaces | Define versioned WIT worlds for the adapter/telemetry contract families (§5.4 Class B), translating these Rust traits — and record where a Rust trait deliberately does NOT translate (e.g. `&mut self` stateful methods) |
| Plugin manifest fields for edge deployment | **NONE.** No manifest schema exists (Phase 7 P7.05 owns it). The edge's current "configuration surface" is the `FLEXCORE_EDGE_*` environment set + the versioned line-format documents (`openems-mapping-profile.v1.json`, `flexcore-site-policy-v1`, `flexcore-edge-fallback-policy-v1`, `flexcore-edge-command-enablement-v1`) — deployment config, not plugin metadata | Manifest `runtime kind`/entrypoint/permissions semantics for an edge-deployed plugin, incl. how a plugin declares (or is denied) edge-adjacent capabilities |
| The uplink wire format | **OPEN item.** `BatchPayloadEncoder` refuses (`NoBatchPayloadEncoder`); nothing is forwarded until the owner implements the encoder and composes the P6.06 credential material into `MtlsUplinkChannel::compose` (recorded in `10_11_openems.md` §8.3 and `06_07_identity_channel.md`) | If the wire format touches plugin-visible payloads, Phase 7 inherits this open item; it must not assume one exists |
| Plugin-visible policy configuration | `SitePolicyEngine` + `flexcore-site-policy-v1` are edge-local composition inputs. No plugin-facing policy API, capability vocabulary (`telemetry.read`, `schedule.propose`, …), or capability broker exists | The §5.7 capability vocabulary and the broker that enforces it (Phase 7 P7.11–P7.13) |
| Any plugin capability surface for the command path | **NONE — deliberately.** There is no plugin host at the edge, no plugin runtime, no hook, no callback, no extension point into the command path. The only plugin-relevant fact is negative: the enablement state is structurally immutable from any shared reference (`13_14_17_adapter_command_policy.md` §2.3) | Phase 7 must keep this negative: see §3 |

## 3. THE HARD BOUNDARY (not negotiable in Phase 7)

1. **The edge command path and its P6.14 enablement gate are PROTECTED
   KERNEL BEHAVIOUR — never a plugin capability** (report §5.7: a plugin can
   never select the control-authority mode; §5.1/ADR-005: authorization,
   tenant isolation, audit, command lifecycle, and final dispatch validation
   stay in the protected core; the edge ENFORCES them, never delegates or
   replaces them).
2. Plugins may never bypass authorization, tenant isolation, audit, command
   lifecycle, or final dispatch validation. Concretely for Phase 7:
   - no plugin capability may map to "enable / re-scope / extend the expiry
     of the P6.14 gate" — the gate's four conditions and its signed,
     time-bounded record are the ONLY enablement path;
   - no plugin may reach `CommandForwarder` (the port is reachable only
     behind gate → verifier → policy re-check, and `FailClosedCommandForwarder`
     is the only production impl);
   - the never-granted capability set Phase 7's PLAN already names
     (`dispatch.authorize`, `secret.read`, `network.any`, direct
     device-protocol access) implicitly includes the P6.14 gate;
   - `CommandAuthorizer`/`DispatchGateway` remain protected core (§9.1) —
     unchanged by everything above.
3. The edge agent must not grow a plugin loader in Phase 6 — and Phase 7's
   edge story inherits the P6.20 profile rule: plugins get no shell, no
   arbitrary process execution (§18.3).

## 4. The `site-edge` deployment profile (the Phase 7 composition input)

Report §5.8 defines `site-edge` verbatim as: **"Edge agent, selected
protocol adapters, local buffering, outbound identity, limited command
receiver."** Phase 6's concrete composition input for it is the P6.20
deployment profile (`20_deployment_profile.md`): single static Rust binary
under systemd, Linux x86-64/ARM64 (OCI image an alternative), outbound-only
network posture with the explicit allowlist (control-plane endpoint + site
EMS endpoint), dedicated non-login system user, writable state confined to
`/var/lib/flexcore-edge`, `MemoryMax`/`TasksMax` bounds, signed-update
DESIGN (no PKI implemented — DEPLOYMENT REQUIREMENT), and the shipped env
file deliberately omitting the command-path flag.

Phase 7 composes `site-edge` from the seams in §1 within this profile; a
profile is an explicit, reviewable composition that must not silently enable
a control-capable plugin (§5.8) — which at the edge means the §3 boundary
applies to every bundle composition decision.

## 5. Seam table for the Phase 7 PLAN.md (lift-ready)

| Seam | Contract (exact type) | Provider(s) (Phase 6 state) | Consumer | Version state | Owner module |
|---|---|---|---|---|---|
| Site EMS adapter | `EmsAdapter` | `OpenEmsRestAdapter` (`openems-rest`); `SimulatedEmsSource` (`simulated-test`, TEST-ONLY) | `AdapterIngestSource` → `EdgeRunner` | written, UNBUILT | `crates/edge-agent/src/adapters/port.rs` |
| Anti-corruption | `AntiCorruptionLayer` + `PointMapping` | the ACL itself (frozen mapping tables) | `AdapterIngestSource` | written, UNBUILT | `crates/edge-agent/src/adapters/acl.rs` |
| Edge telemetry source | `EmsSource` | `AdapterIngestSource`; `FailClosedEmsSource` (fail-closed default) | `EdgeRunner` | written, UNBUILT | `crates/edge-agent/src/ports.rs` + `ingest.rs` |
| Uplink | `UplinkChannel` (forward-only) | `MtlsUplinkChannel`; `FailClosedUplink` (fail-closed default) | `EdgeRunner` | written, UNBUILT; TLS never executed | `crates/edge-agent/src/ports.rs` + `uplink_mtls.rs` |
| Uplink wire format | `BatchPayloadEncoder` | `NoBatchPayloadEncoder` (REFUSES) | `MtlsUplinkChannel` | OPEN item | `crates/edge-agent/src/uplink_mtls.rs` |
| Local buffer | `LocalBuffer` | `FileLocalBuffer` (durable); `InMemoryLocalBuffer` (test) | `EdgeRunner` | written, UNBUILT | `crates/edge-agent/src/ports.rs` + `buffer_file.rs` |
| Edge audit | `AuditSink` | `FileAuditBuffer` (durable); `InMemoryAuditSink` (test) | composition root; command path; isolation guard | written, UNBUILT | `crates/edge-agent/src/ports.rs` + `audit_buffer.rs` |
| Command gate | `CommandPathRuntime` + `CommandReceiver` (4-condition gate) | the edge agent itself | `GatedCommandPipeline` | written, UNBUILT; PROTECTED KERNEL | `crates/edge-agent/src/command_path.rs` |
| Command verification | `CommandVerifier` (+ `ClockQuality` parameter) | the edge agent itself | `GatedCommandPipeline` | written, UNBUILT; PROTECTED KERNEL | `crates/edge-agent/src/command_verify.rs` |
| Command forwarding | `CommandForwarder` | `FailClosedCommandForwarder` only (+ scripted test double) | `CommandForwardingEngine` | written, UNBUILT; PROTECTED KERNEL | `crates/edge-agent/src/command_forward.rs` |
| Site policy | `CommandPolicyGate` | `SitePolicyEngine` | `CommandReceiver` + `CommandForwardingEngine` | written, UNBUILT | `crates/edge-agent/src/policy.rs` |
| Health | `HealthSnapshot` (closed vocabulary) | `AdapterHealthMonitor` | P6.25 console (display-only) | written, UNBUILT | `crates/edge-agent/src/health.rs` |
| Clock quality | `ClockQualityState` → `ClockQuality` (contracts) | `ClockQualityMonitor` (P6.31) | `CommandVerifier::verify` | P6.31 owns the producer status | `crates/edge-agent/src/clock_quality.rs` |
| Enrollment/identity | `EnrollmentClient`, `CredentialIssuer`, `CredentialStore`, `RevocationChecker` | `TokenEnrollmentAdapter`, `DenyAllCredentialIssuer`, `FilesystemCredentialStore`, `UnconfiguredRevocationChecker` | composition root; server route `POST /api/v1/edge/enroll` | written, UNBUILT; CA/token service = DEPLOYMENT REQUIREMENT | `crates/edge-agent/src/enrollment.rs` |
| Agent config | `EdgeConfig` + `FLEXCORE_EDGE_*` surface | `EdgeConfig::from_env` | composition root | written, UNBUILT | `crates/edge-agent/src/config.rs` |
| Conformance harness | `ConformanceHarness` / `ConformanceReport` | the suite `flexcore-edge-adapter-conformance-suite-v1` | Phase 7 plugin conformance (cross-referenced, not duplicated — P7 PLAN §3) | written, UNBUILT | `crates/edge-agent/src/conformance.rs` |

**Standing caveat for Phase 7:** the crate has NOT been built or run; P7.01
verifies each seam "exists in code" from this table — the honest per-row
version state is "written, awaiting the human build/test pass", and any
compile-defect discovery at that pass is a Phase 7 readiness gap to record,
not a seam that silently existed.

## 6. Honest limitations

1. Every name was verified present in the source; no behaviour is verified
   (nothing was executed). This note carries no pass/fail claims.
2. P6.31 (clock-quality producer) and P6.32 (decommissioning) are concurrent
   ledger items — this note lists their SEAMS from the source and defers
   status to their own decision records; it does not tick or claim them.
3. This note adds no code and owns exactly one new ledger file plus the
   P6.29 ledger tick.
