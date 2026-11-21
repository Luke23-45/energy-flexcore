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
// | [`adapters`] | The protocol-adapter port (`EmsAdapter`), the anti-corruption layer (EMS-native → canonical), the adapter registry (allowlist), the `simulated-test` TEST-ONLY scenario fixture (P6.08 — never a real EMS), and the edge-to-cloud mapping (`edge_to_cloud`, P14.18) | `openems-rest` (P6.10/P6.13); the six Phase 14 protocol ids registered PLANNED (P14.18, record 82 — `generic-rest` REMOVED) |
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
// P18.08 (phase 18, wave 1, Agent B) — the device identity LIFECYCLE
// formalization: the IDevID/LDevID model mapped onto the P9.33 ceremony
// (the ceremony enrolls/rotates/revokes the LDevID; the factory IDevID is
// only ever verification/attestation input, never a ceremony subject), the
// credential rotation schedule (pure assessment over the injected clock; a
// due rotation is a NORMAL ceremony re-run with a fresh platform-authority
// token — no second token path), and the revocation-propagation plan
// (registry evidence → protocol trust chains → the gates; the kernel P7.38
// plugin-revocation register stays BYTE-IDENTICAL — consumption only, no
// percentage-canary reinterpretation, no un-revoke). [S-165][S-166]; the
// 802.1AR/FDO specifics are UNVERIFIED against the standard texts.
pub mod identity_lifecycle;
// P18.09 (phase 18, wave 1, Agent B) — the ZERO-TOUCH onboarding DESIGN:
// the FDO-pattern analog (ownership-voucher register with an append-only
// event history, the rendezvous SEAM the deployment implements or not) and
// the closed claim-flow state machine whose handoff ISSUES NOTHING — the
// P9.33 ceremony's challenge and platform-side approval gate run unchanged
// after handoff. The shipped rendezvous is the fail-closed
// `NoRendezvousConfigured` (zero-touch structurally OFF): the manual P9.33
// path stays the default AND the fallback. [S-165][S-166]; FDO specifics
// UNVERIFIED. No device is claimed; no voucher exists.
pub mod zero_touch;
// P18.12 (phase 18, wave 1, Agent B) — the FLEET device-state schema types
// keyed to migration `0023_edge_fleet_registry_p18_12` (the 0018 registry
// extension as an append-only state journal): the closed
// firmware-state/cohort-membership/last-seen-health vocabularies, the
// validated journal-row shape, and the BTreeMap current-state projection
// the P18.06 rollout orchestrator CONSUMES (it never redefines a value).
// No fleet exists; no journal row has ever been written.
pub mod fleet_registry;
// P18.11 (phase 18, wave 1, Agent B) — the eSIM/CELLULAR lifecycle DATA
// MODEL only: the closed, deliberately coarse profile-state vocabulary and
// the carrier-AGNOSTIC observation record riding the migration-0023
// ESIM_PROFILE_STATE journal rows (carrier specifics — activation codes,
// SM-DP+ endpoints — are out of scope). [S-167] overview basis; the
// SGP.22/SGP.32 semantics are UNVERIFIED. No SIM exists; no profile state
// was ever recorded.
pub mod esim_lifecycle;
// P18.13 (phase 18, wave 1, Agent B) — the ATTESTATION design: the
// boot-time evidence shape, the verification policy the platform applies
// BEFORE authorizing an update (named, ordered, fail-closed refusals:
// absent evidence → revoked device → device mismatch → stale evidence →
// counter regression → measurement check → anchor requirement), and the
// honest hardware-absent fallback (P18-E-2 registered OPEN, DEFAULT (a)
// hardware-optional, assumption set PENDING PILOT HARDWARE SPEC per the
// P6.34 posture — a claimed anchor is a device REPORT this platform does
// not verify; software-only evidence is consistency-checked
// self-attestation that cannot detect a compromised reporter). [S-166].
// No evidence was ever collected or verified; no attestation ran.
pub mod attestation;
// P18.02 (phase 18, wave 1, Agent A) — the VERSIONED UPDATE-BUNDLE FORMAT
// for the fleet OTA pipeline (ADR-1801, records/01): the
// `flexcore-edge-update-manifest-v2` manifest extending the P9.34
// `UpdateManifest` (composed, not forked) with SUIT-shaped applicability
// metadata and sequence-number anti-rollback [S-162], per-payload
// digests, size caps, and the device-side monotonic anti-rollback counter
// register (append-only CRC-32 framing, truncate-and-audit). The
// strictly-ahead sequence semantics are UNVERIFIED against the TUF/SUIT
// spec texts (the implemented rule is the strictly-safer reading).
// FIM-F12.07 honesty update: the PRODUCING code now exists (see
// `ota_bundle_produce`, WRITTEN-UNRUN) while the fact stands — no bundle
// has ever been produced, signed, staged, or applied, and no key material
// ships (the P18.03 key ceremonies are HUMAN GATES).
pub mod update_bundle;
// P18.04 + P18.05 (phase 18, wave 1, Agent A) — the device update FLOW
// state machine (download → verify → install → health-check → active,
// with explicit failure states and auto-rollback edges; updates BLOCKED
// while commands are ISSUED/TRACKING — the P18.15 rule consumed by
// citation) and the EXTENDED UPDATE GATE behind the P9.34 seam: the
// fail-closed `NoUpdateVerifier` default is PRESERVED verbatim, and the
// real SHA-256 verifier (the P11.15 std-only implementation, consumed
// from flexcore-audit — never duplicated) is an injected OPTION plus the
// v2 manifest checks composed AROUND the untouched P9.34 gate. No update
// has ever been applied; no fleet exists; every hardware determination
// is PENDING PILOT HARDWARE SPEC (P6.34).
pub mod update_session;
// P18.15 (phase 18, wave 2, Agent B) — the UPDATE/COMMAND-LIFECYCLE
// INTERLOCK: updates are BLOCKED while any command on the device is in a
// non-terminal frozen §14.1 lifecycle state (a strict SUPERSET of the
// recorded ISSUED/TRACKING citation — ACKNOWLEDGED/FAILED/EXPIRED also
// block, the drain gap, record 28 §2), fail-closed when the device command
// state is unknown (a device-state check consumed from the command path's
// recorded §14.1 states), the report §5.6 drain-before-update procedure
// (OBSERVER-only — the interlock never transitions a command state), and
// the post-update conformance re-check (the P6.13 harness consumed, never
// reimplemented; a clean §19.2(6) report required after every activation
// before further updates are authorized; a FAILED re-check is terminal
// until a recorded override — the P9.34 RollbackOverride pattern). The
// frozen 14-state/27-edge set is CITED, never forked; the P6.14 command
// receiver and the P18.04 session semantics are untouched (additive
// composition only). No command exists; no update has ever been applied;
// no fleet exists.
pub mod update_command_interlock;
// FIM-F12.01 + FIM-F12.02 (Wave 6, final implementation milestone;
// WRITTEN-UNRUN) — the OTA Ed25519 SIGNING LAYER: the domain-separated
// signature domains (the Patch B `source_auth.rs` discipline),
// device-configured trust anchors, the INJECTED-seed release key (never
// generated at build, never shipped — the P18.03 key ceremonies are HUMAN
// GATES), and the REAL `UpdateVerifier` behind the P9.34 port
// (`Ed25519UpdateSignatureVerifier`, ed25519 over trust anchors; the
// shipped `NoUpdateVerifier` fail-closed default is unchanged, and a
// verifier with zero anchors refuses everything). The metadata roles are
// TUF-SHAPED, not TUF-conformant, and the role semantics are UNVERIFIED
// against the TUF spec text. No bundle has ever been signed.
pub mod ota_signing;
// FIM-F12.01 + FIM-F12.02 (Wave 6, final implementation milestone;
// WRITTEN-UNRUN) — the TUF-SHAPED OTA repository and bundle PRODUCER plus
// the device-side artifact verification: the root/targets/timestamp/
// snapshot metadata roles canonically encoded and Ed25519-signed (a
// FlexCore format — NOT a TUF encoding, no conformance claim, role
// semantics UNVERIFIED), the bundle assembled in memory with per-payload
// signatures, the producer-side counters honoring the EXISTING
// strictly-ahead anti-rollback semantics, and `OtaArtifactVerifier`
// driving the produced bundle through the UNTOUCHED extended gate (the
// P9.34 refusal order, audit, no-write structure, and the durable
// counter advance are the existing code). Signing keys are INJECTED
// seeds; the P18.03 key ceremonies are HUMAN GATES; no key material
// ships. No bundle has ever been produced, signed, staged, or applied.
pub mod ota_bundle_produce;
// FIM-F12.04 (Wave 6, final implementation milestone; WRITTEN-UNRUN) —
// the DEVICE-SIDE A/B slots: the download → verify → install →
// status-report flow per slot composed from the EXISTING P18.04 flow
// machine, the P9.34 register (the source of truth for the active
// version, per UPDATE_SLOT_DECISION), and the P9.34 activation/rollback
// rules. The anti-rollback × A/B rollback resolution is the REGISTER'S
// ACTUAL POLICY, three-way (recorded in the module): a rollback NEVER
// re-presents a manifest and the counter NEVER rewinds; when the failed
// version was never activated (the common A/B case) the register already
// carries the previous version and no register rollback record exists or
// is possible — the slot-level boot rollback completes and is reported;
// when the failed version HAD been activated the register rules run
// verbatim (floor rule + recorded override); and a never-activated
// target (the factory slot on a first-ever update) is the honest
// conflict — recorded and reported as UPDATE_FAILURE_ROLLBACK_FAILED,
// never weakened. The slot write, boot switch, and health evidence are
// DEVICE concerns (PENDING PILOT HARDWARE SPEC, P6.34). No device has
// ever installed anything; no slot exists.
pub mod ota_slots;
// FIM-F15 (Wave 6, final implementation milestone; ADDITIVE, WRITTEN-UNRUN)
// — the FDO-pattern zero-touch onboarding IN-REPO SURFACE (feature F-15;
// record 22_p18_09): the voucher persistence seam lives ADDITIVELY in
// `zero_touch` (the `VoucherStore` trait; the EXISTING register is its
// in-memory DEFAULT — a durable store is the deployment's implementation
// behind the seam), and this module is the claim composition: the
// configured rendezvous resolver implementing the EXISTING
// `RendezvousSeam` (endpoint address/protocol from configuration; the
// network exchange an injected `RendezvousTransport` port — socket-free,
// testable; an unreachable CONFIGURED rendezvous is the distinct
// ZERO_TOUCH_RENDEZVOUS_UNREACHABLE refusal), the opt-in claim service
// composing voucher + rendezvous + fleet/identity surfaces (the handoff
// ISSUES NOTHING — the P9.33 ceremony's challenge and platform-side
// approval gate run unchanged), and the audit trail for every claim
// attempt through the EXISTING audit shapes (dedicated event names LEAD
// the detail text; the EdgeAuditKind vocabulary is not widened). THE
// REFUSE-EVERYTHING DEFAULT IS PRESERVED: zero-touch is opt-in per
// configuration and OFF by default (every claim refuses with
// ZERO_TOUCH_ONBOARDING_DISABLED while off), the shipped
// NoRendezvousConfigured refuses EVERY lookup when unconfigured, and the
// manual P9.33 enrollment path stays the DEFAULT and the FALLBACK. An
// external rendezvous service and a voucher-issuance process are
// HUMAN/INFRA GATES; the claim is reference-equality only (P18-E-2); no
// device is claimed; no voucher exists.
pub mod zero_touch_claim;
