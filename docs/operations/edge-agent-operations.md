# Edge Agent Operations Guide

| Field | Value |
|---|---|
| Scope | Operating the FlexCore edge agent (`flexcore-edge-agent`, `crates/edge-agent`) at a site running a site EMS (OpenEMS) — install, enroll, monitor, update, recover, decommission |
| Control status | **Read-only / test-lab by default** (report §2.1 v0.4.0, §21.4). The command path is **disabled by default**; field write authority begins at v1.0.0 with operator approval. FlexCore is a supervisory orchestration layer — no protection-grade, hard-real-time, or sub-cycle control (§14.5, §17.5). |
| Build status | **WRITTEN, NOT BUILT AND NOT RUN.** The code this guide describes is written but unbuilt; every lab run, install, and upgrade path below is **UNRUN** until the human build/test pass executes it (AGENTS.md rule 7; `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`). Nothing here is evidence of a run. |
| Sources | Decision records `ledger/phase-06-edge-protocol/{05_12_20,06_07,08_09,10_11,13_14_17,15_16,18_21,19_23_22,20}_*.md`; report §18.1–§18.3, §21.2–§21.4, §17.2–§17.5, §14.1–§14.4 |
| Units | MW, MVAr, MVA, MWh; `P > 0` = export at the point of connection (§10.2); SOC a fraction in [0,1] at the API boundary |

## 1. What the edge agent is (and is not)

| The edge agent IS | The edge agent IS NOT |
|---|---|
| A separate Rust daemon in the site's OT DMZ with an independent lifecycle (§18.1, §8.3) | A library inside the control plane |
| A read-only telemetry source: it reads site-EMS channels, normalizes them to the canonical model (§10.2–§10.4), and forwards them over an outbound mTLS channel (§18.1, §3.3) | A writer to the site EMS: the OpenEMS adapter's entire outbound surface is GET-only (§10_11 record §3) |
| A bounded local buffer + local audit buffer with store-and-forward recovery (P6.09/P6.18) | A protection layer or fast safety controller: the local site EMS remains authoritative for fast safety behaviour (§17.5 invariant 10, §21.2) |
| Structurally inbound-free: the ONLY socket operation is an outbound connect; there is no listener (§3.3; enforced by a lexical scan test) | Reachable from the internet or dialable by the central platform (§18.2) |

## 2. Install

**Status: the deployment profile is RECORDED; nothing is built, packaged, or installed.** Source: `ledger/phase-06-edge-protocol/20_deployment_profile.md` (P6.20). The systemd unit is versioned in-tree at `deploy/edge/flexcore-edge-agent.service` — **written, NOT installed** anywhere.

| Step | Action | Status / note |
|---|---|---|
| 1 | Build the release binary: `cargo build --release -p flexcore-edge-agent` under the exact toolchain pin (`rust-toolchain.toml`, Rust 1.99.1) | **UNRUN** — human gate input (P6.20 §5) |
| 2 | Place the binary at `/usr/local/bin/edge-agent`; the unit at `/usr/lib/systemd/system/flexcore-edge-agent.service` | Site adaptation happens at deployment time under the D6 review (P6.03) |
| 3 | Create `/etc/flexcore/edge/env` with the required `FLEXCORE_EDGE_*` variables (see §2.1) | The shipped environment file **deliberately omits** the command-path flag: it defaults to off in code and config (P6.14) |
| 4 | Create the state root `/var/lib/flexcore-edge` (0700, `StateDirectory`): `buffer/` (telemetry journal), `audit/` (audit journal) | The agent's only legitimate write surface (P6.20 §3) |
| 5 | Start the service; confirm fail-fast with a valid env file and refusal to start without one (exit 2, named error) | **UNRUN** — P6.20 §8, task 2 |
| 6 | Confirm NO listening socket exists on the edge host (`ss -tlnp`) | **UNRUN** — the structural guarantee is a code-level lexical scan, not a lab capture |

Hardening in the unit (documented directives, not verified by execution): dedicated non-login user `flexcore-edge` (no shell), empty capability bounding set, `ProtectSystem=strict`, `MemoryMax=256M` / `MemoryHigh=192M` (a conservative STARTING BOUND, not a measured requirement), `TasksMax=64`, `Restart=on-failure` with `RestartSec=5s`, `Wants=time-sync.target` (start only after time sync; clock quality is never assumed, §17.4).

### 2.1 Required configuration surface (selection)

| Variable | Meaning | Default / constraint |
|---|---|---|
| `FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT` | Central platform endpoint for the outbound mTLS channel | must be `wss://` or `https://`; `ws://`/`http://`/scheme-less are named refusals (P6.07) |
| `FLEXCORE_EDGE_OPENEMS_BASE_URL` | Site-EMS REST base URL for the OpenEMS adapter | **only `http://` composes today** — TLS on this hop is a recorded open item (§6.2) |
| `FLEXCORE_EDGE_OPENEMS_MAPPING_PROFILE` | Mapping-profile file | defaults to the shipped `openems-mapping-profile.v1.json` (version-checked; a different version is a named refusal) |
| `FLEXCORE_EDGE_BUFFER_CAPACITY` / `FLEXCORE_EDGE_BUFFER_MAX_AGE_MS` / `FLEXCORE_EDGE_OVERFLOW_POLICY` | Buffer bound (default 10 000 records; age default 1 h, cap 24 h; policy accepts exactly one value, `drop-oldest-audited`) | unknown policy values refuse startup (P6.09) |
| `FLEXCORE_EDGE_POLL_INTERVAL_MS` | Telemetry poll cadence (default 5 000 ms — supervisory minutes-scale, never sub-second, §3.5) | |
| `FLEXCORE_EDGE_COMMAND_PATH` | Command-path flag | **absent/disabled by default**; `enabled` alone enables nothing — see §7 |
| `FLEXCORE_EDGE_SITE_POLICY_PATH` | Site-policy document (`flexcore-site-policy-v1`) | parse failure is a fail-fast startup error (P6.17) |
| `FLEXCORE_EDGE_UPLINK_TLS_MINIMUM` | TLS floor for the uplink | DEFAULT `tls13`; `tls12-floor` is opt-in only per a site D6 record (P6.07 §2) |

Config precedence: environment file → unit environment → binary defaults. There are **no default credentials anywhere**; the endpoint may embed credentials and is redacted from all log renderings (`EdgeConfig: Debug` redaction — a secret is never recoverable from Debug output).

## 3. Enroll

**Status: the enrollment seam and control-plane route are implemented; the CA/issuance service is NOT — it is a deployment requirement.** Source: `ledger/phase-06-edge-protocol/06_07_identity_channel.md` (P6.06/P6.07).

| Step | Action | Status |
|---|---|---|
| 1 | Site provisions the edge key pair at install time (hardware-backed storage where available, §18.3). FlexCore generates, stores, ships, and distributes NO key material — this is a deployment input | **DEPLOYMENT REQUIREMENT — not implemented** |
| 2 | Operator issues a single-use provisioning token via the deployment's token service | **DEPLOYMENT REQUIREMENT** — the `ProvisioningTokenValidator` seam's shipped default denies everything |
| 3 | Edge submits enrollment to `POST /api/v1/edge/enroll` with the token; the server binds the identity to exactly ONE tenant and the approved site/asset set (§16.2), checked against the server-side approved scope | Route implemented; negative matrix below is written, **UNRUN** |
| 4 | Server stores the identity record (migration 0018; partial unique index forbids two ACTIVE identities per `(device_id, tenant_id)`); the record's `certificate_issued` stays `false` until a real CA is composed | Issuance is the seam's `DenyAllCredentialIssuer` default — no edge can obtain a client certificate until a deployment composes an issuer |
| 5 | Edge stores the credential record (0600 owner-only on Unix; key MATERIAL never stored, only a reference); expiry is monitored (`EDGE_CREDENTIAL_EXPIRY_WARNING` / `EDGE_CREDENTIAL_EXPIRED`) and surfaced in health at least one rotation window before expiry | Expiry tracking implemented (clock-based, audited); renewal = the rotation path (EXPIRED → new term, ROTATED event) |

**Fail-closed enrollment matrix** (each case must be refused and audited — all **UNRUN**, written as tests):

| Attempt | Required outcome |
|---|---|
| Duplicate identity (same `(device_id, tenant)`, still ACTIVE) | 409, journaled REJECTED |
| Expired provisioning token | 401; no scope assigned; nothing journaled as an enrollment |
| Revoked identity re-enrolling | 403 + audited REJECTED naming the revocation |
| Wrong-tenant token / foreign site within the tenant | 403 + journaled REJECTED with actor, object, timestamp, correlation id |
| Unauthenticated / garbage bearer | 401; nothing journaled |
| Malformed device identity or tenant/site UUIDs | 422; no store write |

Revocation checking (OCSP/CRL) is the `RevocationChecker` seam with a fail-closed default (`UnconfiguredRevocationChecker`: an un-established status is never "not revoked"); composing a real OCSP/CRL adapter is a deployment requirement.

## 4. Monitor

**Health vocabulary (verbatim from `ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md` §3.2, P6.17):** the closed status set is `HEALTHY`, `DEGRADED_<reason>`, `FAILING_<reason>`, with reasons from the closed set `UNVERIFIED`, `UNRESPONSIVE`, `STALE_DATA`, `ERROR_RATE`, `POLICY_BLOCK`, `CREDENTIAL_EXPIRY`. Extending either set is a reviewed change. Derivation rules (first match wins; FAILING outranks DEGRADED):

| # | Observation | Status |
|---|---|---|
| 1 | Never observed | `DEGRADED_UNVERIFIED` |
| 2 | Last responsive check failed, or no observation within the unresponsive window | `FAILING_UNRESPONSIVE` |
| 3 | Credential expired | `FAILING_CREDENTIAL_EXPIRY` |
| 4 | Policy block noted | `DEGRADED_POLICY_BLOCK` |
| 5 | Credential inside the rotation window | `DEGRADED_CREDENTIAL_EXPIRY` |
| 6 | Data older than the freshness limit (or no data ever seen) | `DEGRADED_STALE_DATA` |
| 7 | Error count at/above the window maximum | `DEGRADED_ERROR_RATE` |
| 8 | Only otherwise | `HEALTHY` |

Health is **never inferred from the absence of errors**, and "unknown" is `DEGRADED_UNVERIFIED`, never healthy. While an adapter is `FAILING_*`, ingesting from it and any command targeting it are blocked with an audited refusal; `DEGRADED_*` still flows (data arrives flagged). A degraded edge clock means expiry checks fail closed (any clock quality other than `Synchronized` rejects the command — P6.15, §17.4).

**Console surfacing (P6.25 — status: in progress, UNRUN at the time of writing):** the Sites workspace must show edge connectivity, certificate expiry, adapter health, last-seen timestamps, buffer depth, prominent data freshness (measured / estimated / stale / simulated visually distinct, §20.2), and the command-path state (disabled / test-env enabled with approver + expiry). The console can only DISPLAY the gate state and record a request for the signed per-site change — it can never enable the command path (§20.2; the enablement state is structurally immutable from any shared reference). Until P6.25 lands, the data comes from the code surfaces (`HealthSnapshot`, the receiver's enablement read) — a fresh-engineer walkthrough against the lab has not happened.

**Channel-down vs site-down:** the mTLS channel reports `ChannelStatus` distinctly from the runner's EMS-side states — an operator must be able to tell a broken uplink from a dead site feed (P6.07). Uplink transport acknowledgement (`forward` returns `Ok` only after the channel's ack) is at-least-once; the buffer owns redelivery.

## 5. Update

**Status: DESIGN ONLY — no PKI, no signing keys, no verification code exists.** Source: `ledger/phase-06-edge-protocol/20_deployment_profile.md` §6 (P6.20).

| Design rule | Content |
|---|---|
| Verify signature BEFORE swap | The bundle manifest (version, checksums, expiry) is verified against the platform release key before any file is touched; a bad/missing signature refuses the bundle entirely |
| No self-upgrade | Activation is performed by the package layer or a dedicated out-of-process updater, never by the agent itself (§9.3) |
| Staged rollout | Per site and per cohort with a hold window; a failing cohort freezes the rollout (§21.4) |
| Health-gated rollback | The previous version is retained on disk until the new version passes its health check; a failed check triggers automatic rollback + audit |
| Audit per update | Applied / rolled back / refused are each audit events; the running version is operator-visible |
| Downgrade | Explicit operator-signed action only, audited |

**What does NOT exist (honest):** no release key pair was generated, no signature format chosen, no verification code written, nothing signed, and no install/upgrade/rollback/downgrade test has run on any architecture (x86-64 and ARM64 are both UNRUN). This must exist before ANY edge receives an update.

## 6. Recover

**Status: buffer and audit-buffer code written, UNRUN; the lab recovery drills are human gate inputs.** Sources: `08_09_ingest_buffer.md` (P6.09) and `18_21_audit_offline_isolation.md` (P6.18).

### 6.1 Telemetry buffer (P6.09)

| Property | Behaviour |
|---|---|
| Durability | Append-only journal (`FLEXCORE-EDGE-TELEMETRY-JOURNAL`), CRC-32 length-prefixed records, `sync_data` per record; survives edge restarts |
| Overflow policy | **Audited drop-of-OLDEST** — hard counter, latched health flag, loud log; never a silent overwrite. Runner backpressure (pause polling at capacity) is the first line; drop-oldest engages only when the bound is hit anyway |
| Age limit | Records older than `FLEXCORE_EDGE_BUFFER_MAX_AGE_MS` (measured on `ingest_time`) are dropped at drain/reopen with an audited counter |
| Outage recovery | On reconnect the drain resumes IN ORDER; a second outage mid-drain loses nothing (drained records stay on disk until the confirmed-forward commit) |
| Crash semantics | A crash in the drain→forward→commit window produces a RE-DELIVERY (at-least-once by design); downstream dedup is the platform ingest layer's job (P2.09/P2.10), not re-invented at the edge |
| Corrupt tail | **Truncate-and-audit**: the first malformed frame (bad length / CRC mismatch / undecodable payload) ends the replay, the file is truncated at the last good record, the loss is counted (`corrupt_tail_truncated`) and logged — never guessed past. A wrong magic/version is a hard open error |
| Gap events | Every drop path increments a named counter and logs loudly; the affected intervals are the platform gap detector's `MISSING` markers downstream (the edge emits no gap markers itself) |

### 6.2 Audit buffer and offline behaviour (P6.18)

The audit buffer mirrors the same mechanics (bounded, disk-first, restart-safe, in-order upload with a fail-closed in-order commit — a batch that does not start at the journal front is refused and removes nothing). The offline behaviour statement, in force while the central channel is down:

1. The local site EMS remains authoritative for fast safety behaviour (§21.2, §17.5).
2. The edge issues NO new commands while disconnected — the test-env path included. Response-direction frames travel on the OUTBOUND session, which is down; nothing can arrive, be admitted, or be queued (there is no command queue).
3. Telemetry buffering follows §6.1.
4. A central outage does not disable site-local protective controls or force an unsafe device state (§21.2).

All audit events generated during an outage upload on reconnect in order; overflow is reported (audited drop-of-oldest + reconciliation identity), never silent.

### 6.3 Recovery operator checklist

| Situation | Check |
|---|---|
| Channel restored after an outage | Event-count reconciliation: `appended_total == committed + pending + dropped` (telemetry and audit buffers); drain completes in order; no duplicate stored values downstream |
| Edge restarted | Journals replay; in-memory audited counters reset (each loss event was audited when it happened); health returns via `DEGRADED_UNVERIFIED` until the first observation |
| Journal corruption reported | Truncated at the last good record; `corrupt_tail_truncated` counted; downstream intervals `MISSING`, never fabricated |

## 7. Command path (test-env only) — disabled by default

The command receiver ships **disabled by default in code, in config, and in every shipped profile**. With the flag off, a command arriving at the edge is rejected (not queued, not forwarded) and the rejection is audited. Enabling requires ALL FOUR conditions of the P6.14 gate (all-or-nothing, fail-closed, deterministic refusal reasons) — a written procedure with named approvers, never a config convenience:

| # | Condition |
|---|---|
| a | Explicit signed per-site configuration change (`FLEXCORE_EDGE_COMMAND_PATH=enabled` plus the enablement inputs) |
| b | Signed enablement record + matching test-lab token + out-of-band key (time-bounded: the enablement auto-expires at `valid_until`) |
| c | Target adapter on the site command allowlist (checked at evaluation AND per command) |
| d | The site's D6 review record exists |

Even when enabled, edge-side verification (P6.15: signature, expiry with clock-quality fail-closed, sequence with restart-persisted checkpoint, scope/allowlist) and forwarding only to allowlisted adapters with per-asset fallback (P6.16, §14.4 — no universal "set power to zero") apply. No plugin, adapter, or optimizer can enable the path (§5.7 — the enablement state has no setter). Acknowledgement is not physical delivery (§14.1).

## 8. Decommission

**Status: the procedure below is the P6.32 ledger definition; the implementation is NOT built — P6.32 is open.** Source: `ledger/phase-06-edge-protocol/ledger_phase_6.md` §P6.32.

| Step | Action |
|---|---|
| 1 | Operator-initiated, authorized (RBAC + step-up where required), with a confirmation record naming the edge, tenant, site, actor, and time |
| 2 | Server-side revocation of the edge identity/certificate; the central platform rejects any later session from that identity (revocation is checked per session — a cached certificate cannot reconnect) |
| 3 | Edge-side wipe: keys/certificates, the telemetry buffer (P6.09), and un-uploaded audit records (P6.18) |
| 4 | **Un-uploaded audit rule:** upload-then-wipe, or record the loss as a gap event — **never silently drop** |
| 5 | Post-wipe verification (no recoverable key or buffered record) and a reconnect-after-decommission rejection test, both audited |

## 9. Known limitations (stated plainly)

| Limitation | Status |
|---|---|
| Encryption at rest for the telemetry and audit journals (§18.3) | **NOT implemented — DEPLOYMENT REQUIREMENT.** Until closed with a license-audited crypto dependency, the journals hold records in the clear on the edge host; the deployment must treat `/var/lib/flexcore-edge` as in-scope for OS-level site protection (protected volume / permissions / full-disk encryption per the site's OT boundary review) |
| TLS on the OpenEMS REST hop | **OPEN ITEM — only `http://` composes today**; a `https://` base URL is a named refusal. The FlexCore edge↔central uplink is unaffected (mTLS, P6.07) — the two channels must not be conflated (RESEARCH.md R6-C1) |
| Uplink wire format | The `BatchPayloadEncoder` seam's shipped default REFUSES encoding: the read path and the durable buffer are real, but **nothing is forwarded to the central platform** until the wire-format owner implements the encoder and composes the P6.06 credential material into the mTLS channel. The startup log states exactly this fail-closed posture |
| Enrollment / updates PKI | No CA, no signing keys, no verification code — both are deployment requirements (§§3, 5) |
| Everything executable | **UNRUN** — no build, no lab run, no install. A procedure in this guide is a written procedure, not a run one |

## 10. Verification status

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the phase record `ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md`) |
| Install / enroll / upgrade / recovery lab executions | **UNRUN** — human gate inputs (P6.20 §8; P6.06 negative tests; P6.09/P6.18 lab drills; P6.28 drill) |
| Section references | All `§N` references resolve to report headings; every protocol claim cites its specification version in `docs/protocols/openems-adapter.md` |
