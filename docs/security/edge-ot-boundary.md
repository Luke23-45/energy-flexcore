# Edge OT Boundary Note (Pilot Site)

| Field | Value |
|---|---|
| Document class | SITE-SPECIFIC operations note derived from the D6 cyber/OT boundary review (P6.03) |
| Site | **`pilot-site-A` = PLACEHOLDER.** No real site exists (`01_d5_disposition.md` §4). Every site-specific value below is a placeholder or a RECOMMENDATION until the named security engineer's review is signed against a real site description |
| D6 status | **OPEN — review PREPARED, sign-off NOT SIGNED** (`03_d6_boundary_review.md`). No pilot-site connection work proceeds until D6 closes; this note describes the recommended boundary positions and the BUILT code's structural properties, and nothing here constitutes the signed review |
| Governing reference | NIST SP 800-82 Rev. 3 (September 2023) — the current final baseline; Rev. 4 (initial public draft, 21 September 2026) is a DRAFT and is not cited as a requirement (RESEARCH.md §6) |
| Report trace | §17.1–§17.5, §18.2, §3.3, §16.2, §21.2–§21.4; `ledger/phase-06-edge-protocol/03_d6_boundary_review.md` |
| What this note is NOT | Not a certification, accreditation, or approval of any kind; not a substitute for the signed D6 record; no safety or protection-grade claim |

## 1. Zones and the edge position (recommended; site confirmation pending)

The report's five-zone model (§17.2): Enterprise IT, Central application, Customer integration/DMZ, OT/site control, Field/device. Recommended edge position: the FlexCore edge agent runs as a separate daemon **in the site's OT DMZ**, talking to the site EMS (OpenEMS), which talks to device gateways and assets (§3.3). Whether the site's actual DMZ matches, and whether the EMS sits inside the OT zone with the edge as the only crossing point, are D6 reviewer questions Z2/Z3 (`03_d6_boundary_review.md` §3.1) — unanswered.

## 2. Connectivity: outbound-only (structural no-inbound)

| Property | Status |
|---|---|
| Connection pattern | The edge dials OUT to the central platform (or a customer-controlled broker/gateway); the central platform never dials into the site; there is no inbound internet access into OT (§3.3, §18.2). RECOMMENDATION pending D6; the code is built to exactly this shape |
| Structural guarantee | The edge-agent crate contains NO listen/bind/accept surface: the ONLY socket operation is an outbound `TcpStream::connect_timeout` in the uplink transport (`06_07_identity_channel.md` §4). Enforced by a lexical scan test over every source file, refusing `TcpListener`, `UdpSocket`, `::bind(`, `.listen(`, `.accept(`, and related tokens |
| Deployment shape | The systemd unit ships no `Listen*` directive; the network allowlist is exactly (a) the control-plane endpoint and (b) the site-EMS endpoint from the adapter configuration — anything else is absent by construction (P6.20 §2) |
| Prohibited flows | No direct database/NATS/plugin network access from any field-device network (§17.2); no inbound central connection; no listener socket on the edge |
| Lab confirmation | Packet capture / host port scan — **UNRUN** (P6.07 verification method; recorded as open verification work) |

## 3. mTLS profile (client-only, from the P6.06/P6.07 record)

The values below are the **DEFAULT/RECOMMENDATION baseline** P6.07 pins while D6 is OPEN (`06_07_identity_channel.md` §2). A signed D6 record changes these as CONFIGURATION (`FLEXCORE_EDGE_UPLINK_TLS_MINIMUM`), never as structural code changes. The site-specific TLS minimum "must come from the reviewer, not from code defaults" (`03_d6_boundary_review.md` C2).

| Parameter | Pinned value (DEFAULT unless noted) |
|---|---|
| Mutual authentication | mTLS in BOTH directions on the edge↔central channel: the edge verifies the central certificate (root CA pinned FROM the credential record — never the ambient system store; an empty/unparsable root store is a named fail-closed error), and the central platform verifies the edge certificate issued at enrollment (P6.06) |
| Minimum protocol version | **TLS 1.3** (rustls TLS13 only). A `tls12-floor` option exists — opt-in via configuration, only per a site D6 record, never the default |
| Cipher suites | The rustls `ring` provider defaults, recorded not customized (TLS 1.3: AES-128-GCM, AES-256-GCM, CHACHA20-POLY1305; TLS 1.2 ECDHE suites only if the floor is opted in). **No suite list is invented**; a per-site allowlist belongs to the signed D6 record |
| Scheme | `wss://` or `https://` REQUIRED; `ws://`, `http://`, scheme-less REFUSED |
| Client identity | Device certificate from the P6.06 identity; key material from the deployment's key source (§18.3), loaded per session, never logged, never stored in the registry |
| Direction | Client-only from the edge: the edge initiates every connection; there is no server/listener anywhere in the agent |
| Reconnect | Exponential ×2 backoff from 1 000 ms to a 60 000 ms cap, deterministic (no jitter — a recorded DEFAULT); keepalive = fresh re-handshake after an idle window, never silent half-open reuse |
| OpenEMS hop | The site-EMS REST hop is a SEPARATE channel and currently `http://` only (open item — see `docs/protocols/openems-adapter.md` §7). OpenEMS's native Edge↔Backend authentication is an API key over WebSocket — adapter-internal business; FlexCore's own channel is never downgraded to it (RESEARCH.md R6-C1) |

Enrollment identity rules behind the channel: each edge holds a unique identity bound to exactly ONE tenant and the approved site/asset set (§16.2), stored server-side and enforced on every session; expired or revoked credentials are rejected; expiry is operator-visible at least one rotation window before it (P6.06).

## 4. D6 review status

| Field | Value (from `03_d6_boundary_review.md`) |
|---|---|
| Status | **OPEN — recorded, not closed** |
| Review | **PREPARED**: the boundary-question checklist (zones Z1–Z3, flows F1–F5, channel/identity controls C1–C7, command-path boundary P1–P4) is written for the reviewer |
| Sign-off | **NOT SIGNED** — reviewer NOT ASSIGNED, review date NOT SET, verdict NOT RECORDED |
| Consequence | No pilot-site connection work proceeds; the mTLS policy parameters above remain RECOMMENDATION-level until then |
| Owner | Security (a named security engineer, assigned at the D6 gate meeting — never invented) |

## 5. Command path: disabled by default + the 4-condition gate

The edge command path ships **disabled by default in code, in config, and in every shipped profile** (P6.14). With the flag off, a command arriving at the edge is rejected (not queued, not forwarded) and the rejection is audited. Enabling requires ALL FOUR conditions, fail-closed and all-or-nothing, with deterministic named refusal reasons:

| # | Condition |
|---|---|
| a | Explicit signed per-site configuration change |
| b | Signed enablement record + matching test-lab token + out-of-band key, TIME-BOUNDED (auto-disables at expiry; the transition is audited) |
| c | Target adapter on the site command allowlist (checked at evaluation AND per command) |
| d | The site's D6 review record exists |

Reinforcing facts an operator should rely on:

- Even when enabled, edge verification (P6.15: signature; expiry against a clock whose quality must be `Synchronized` — any other quality fails closed, §17.4/§17.5 rule 5; monotonic sequence with a restart-persisted checkpoint; scope/allowlist) gates every command before forwarding (P6.16, §14.4 per-asset fallback — no universal "set power to zero").
- No plugin, adapter, or optimizer can enable the path (§5.7): the enablement state is computed once by a private constructor and re-derived read-only thereafter — there is no setter, and a shared reference exposes reads only.
- Even enabled, the path creates NO inbound listener: response-direction frames arrive ON the outbound session the edge dialled (§3.3).
- The console can only display the gate state and record a request; it can never flip the gate (§20.2).

## 6. Clock rule: hold on drift

The edge never trusts expiry checks on an untrusted clock. Sync source, tolerance, and clock-quality states are P6.31 scope (edge time synchronization — the seam is wired, monitoring NOT implemented); the built rule today is: ANY clock quality other than `Synchronized` fails the command expiry check CLOSED — commands are rejected, never accepted on a drifting or unknown clock, and the rejection is audited (P6.15; §17.4 clock-drift threat; §14.3). The systemd unit starts the agent only after `time-sync.target`.

## 7. Tenant isolation and blast radius (from the P6.18/P6.21 record)

| A compromised edge CAN reach | A compromised edge CANNOT reach |
|---|---|
| Its OWN bound identity's site telemetry, via its own allowlisted EMS adapter, within the approved asset scope of its single (tenant, site) binding (§16.2) | Any other tenant or site: the enrollment registry binds one identity to ONE (tenant, site); the server scopes every edge-session request to that binding (`resolve_edge_session_scope` — no request-carried field can widen it); a cross-tenant identity is INVISIBLE to the resolver |
| Its OWN local state: telemetry buffer, audit buffer, credential store — all local to the edge host | RLS-protected central tables beyond its scope (migration 0018: RLS ENABLE + FORCE with the shared `tenant_isolation` policy; runtime role INSERT+SELECT only; append-only UPDATE/DELETE guards) |
| Its own allowlisted command path WITHIN the P6.14 gate + P6.17 policy — test environment only, time-bounded, gate+policy ADMISSION only (edge verification P6.15 and forwarding P6.16 complete the pipeline; a receipt is not an issuance, §14.1) | Any other edge's identity: no two edges share an identity (partial unique index on `(device_id, tenant_id) WHERE status = 'ACTIVE'`), and a single-use provisioning token cannot mint a second identity |
| — | Plugin surfaces: there is NO plugin host in Phase 6 (WASM host is v0.5.0 / Stage 7); nothing to escape into |

Isolation is enforced server-side too — never by the edge's goodwill: every cross-tenant/site attempt fails and is audited (actor, object, timestamp), and the app role does not own tenant tables (the §16.2/[R25] RLS-bypass warning; the app-role ownership review across all migrations is a recorded UNRUN human gate input).

## 8. Secrets handling

| Rule | Implementation status |
|---|---|
| No secrets in config, source, logs, or audit lines (§17.3) | Enforced by design: FlexCore generates, stores, ships, and distributes NO key material — enrollment keys, the enablement key, and signing keys are deployment-provisioned out-of-band (secret manager / protected volume). Config carries only non-secret references |
| Redaction | Secret-bearing types (`ProvisioningToken`, `EnablementKey`, `CommandSigningKey`, `PemBody`, `DeviceCredentialRecord`, `EdgeConfig`) implement redacted `Debug` — a planted secret value must be unrecoverable from any Debug/Display rendering (the `expose()` accessor is the one named exception, for composition-root use) |
| OpenEMS defaults | The OpenEMS Getting Started defaults (`admin`/`admin`, `Metadata.Dummy` accept-any-password, `ws://`) must never be the FlexCore enrollment or pilot model (RESEARCH.md R6-C2) |
| Secrets-exposure testing | Written as tests (lexical source scan + redaction runtime checks) — **UNRUN**; no "zero findings" claim is made (P6.23) |

## 9. Open items carried into the D6 review

| # | Open item | Owner / gate |
|---|---|---|
| 1 | Site description, permitted/prohibited flow list, connection pattern choice (broker / DMZ / unidirectional gateway / jump host) | D6 reviewer, against the real site |
| 2 | Signed TLS minimum + cipher allowlist for the edge mTLS channel | D6 reviewer (C2) |
| 3 | Real CA/issuance service, OCSP/CRL revocation adapter, provisioning-token service | Deployment composition (P6.06 seams; shipped defaults deny everything) |
| 4 | Encryption at rest for the edge journals (§18.3) | Deployment requirement until a license-audited crypto dependency closes it (P6.09/P6.18 records) |
| 5 | TLS on the OpenEMS REST hop | Recorded open item (P6.10/P6.11) |
| 6 | Uplink wire format (the encoder seam refuses until implemented) | Wire-format owner (P6.07/P6.08 records) |
| 7 | App-role/RLS ownership review across all migrations | Security engineer (§16.2/[R25] gate) |
