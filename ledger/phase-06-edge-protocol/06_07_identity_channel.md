# P6.06 + P6.07 — Edge identity/enrollment/certificate lifecycle and the outbound mTLS channel

Date: 2026-11-13 (continuation of the 2026-11-12 wave-2a loop; the loop
bans in `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` still hold: nothing
installed, nothing built, no tests run). Status: **IMPLEMENTED — awaiting
human build/test**. Report trace: §18.1, §18.2, §18.3, §17.3, §17.4, §16.2,
§3.3. Ledger: P6.06, P6.07.

Inputs consumed: P6.05 crate (`crates/edge-agent`), the P6.03 D6 boundary
review (OPEN — its checklist question C2 is answered below as
RECOMMENDATION/DEFAULT), P6.02 license audit (rustls flagged UNVERIFIED),
the P4.36/P5.31 additive-route rule, and the Phase 1 tenancy/RLS/auth
conventions (P1.12/P1.17).

---

## 1. Per-subtask coverage

### P6.06 — Edge identity, enrollment, and certificate lifecycle

| Ledger step | Status | Evidence |
|---|---|---|
| 1. Enrollment protocol (key pair, CSR/flow, site certificate) | **SEAM implemented; the CA is NOT implemented** — recorded honestly in §3 below | edge side: `crates/edge-agent/src/enrollment.rs` (`EnrollmentClient` port, `TokenEnrollmentAdapter`, `ProvisioningToken`, `ScriptedCredentialIssuer` test fake, `DenyAllCredentialIssuer` shipped default); control-plane side: `apps/control-plane/src/routes_edge.rs` (`POST /api/v1/edge/enroll`), `dto_edge.rs`, persistence `crates/flexcore-storage/src/edge_enrollment.rs` + `crates/adapters/postgres/src/edge_enrollment.rs` + migration 0018 |
| 2. Identity bound to exactly one tenant + approved site; binding stored server-side and enforced | **Implemented** (record shape + registry binding + route checks); per-session enforcement is P6.21's isolation work — recorded | `DeviceBinding`, `EdgeDeviceIdentityRow` (site FK, tenant PK), route binding/scope/approval matrix; RLS + explicit tenant filter on every statement |
| 3. Expiry tracking, renewal before expiry, revocation handling, operator-visible health | **Expiry: implemented (clock-based, audited).** Renewal: the rotation path (EXPIRED → new term, ROTATED event). Revocation: the SEAM only (§3) | `CredentialExpiryMonitor` + `EnrollmentOrchestrator::assess_expiry` (audit kinds `EDGE_CREDENTIAL_EXPIRY_WARNING` / `EDGE_CREDENTIAL_EXPIRED`); route rotation; `RevocationChecker` port + `UnconfiguredRevocationChecker` (fails closed) |
| 4. Negative tests (duplicate, expired, revoked, wrong tenant — all fail closed) | **Written, UNRUN** | `crates/edge-agent/tests/enrollment_flow.rs`; `apps/control-plane/tests/api_edge.rs`; `crates/adapters/postgres/tests/edge_enrollment.rs` (env-gated) |

Acceptance-criteria honesty: "no two edges share an identity" is enforced
at the DATABASE level (migration 0018's partial unique index on
`(device_id, tenant_id) WHERE status='ACTIVE'` + the route's 409 mapping);
"expiry visible at least one rotation window before expiry" is the
`CredentialExpiryMonitor` rotation-window rule audited as
`EDGE_CREDENTIAL_EXPIRY_WARNING`. Both are CODE-LEVEL claims only — the
verification method (run the negative tests; lab revocation-rejection
check) has NOT run (loop bans), so nothing here is "verified".

### P6.07 — Outbound mTLS channel

| Ledger step | Status | Evidence |
|---|---|---|
| 1. Edge dials out; central never dials in | **Implemented as a structural property** (§4) | `crates/edge-agent/src/uplink_mtls.rs` (`RustlsUplinkTransport::dial` is the ONLY socket operation: outbound `TcpStream::connect_timeout`); `tests/no_inbound_listener.rs` lexically refuses listener surfaces in the whole crate |
| 2. mTLS both directions | **Config-composition implemented; NO TLS ever executed** (loop bans) | client certificate from the P6.06 identity, root CA pinned FROM the credential record (never the ambient system store), server name pinned from config — `assemble_material` + `build_client_config` |
| 3. TLS versions/cipher policy pinned per D6; plaintext rejected | **DEFAULT policy pinned (§2); D6 still OPEN** — the reviewer's signed values may change configuration, not structure | `MtlsUplinkConfig::validate` refuses `ws://`/`http://`/scheme-less; `TlsMinimum` (DEFAULT tls13; `tls12-floor` opt-in) |
| 4. Reconnect/backoff/keepalive; channel-down vs site-down | **Implemented** | `MtlsUplinkChannel` + `BackoffSchedule` (deterministic exponential ×2, cap; no jitter — recorded decision); `ChannelStatus` is the "channel down" signal, distinct from the runner's EMS "site down" states; keepalive = fresh re-handshake after an idle window, never silent half-open reuse |

At-least-once honesty: `forward` returns `Ok` ONLY after the transport's
acknowledgement (`ACK\n`, bounded read); `NACK`/EOF/garbage → `Err` and the
batch stays in the P6.09 buffer, which owns redelivery. The channel itself
buffers NOTHING. The telemetry wire PAYLOAD is deliberately not invented:
`BatchPayloadEncoder`'s shipped default (`NoBatchPayloadEncoder`) refuses —
P6.08/P6.09 own the read path and its wire format; the channel cannot be
composed into fabricating one.

## 2. TLS policy (the 03_d6_boundary_review.md checklist question C2)

**Status label: DEFAULT/RECOMMENDATION.** The P6.03 record explicitly says
the TLS minimum is "site-specific; must come from the reviewer, not from
code defaults" — D6 is OPEN and no security sign-off exists. The values
below are the recommended baseline P6.07 pins while D6 is OPEN; a signed D6
record changes these as CONFIGURATION (`FLEXCORE_EDGE_UPLINK_TLS_MINIMUM`),
never as structural code changes.

| Parameter | Pinned value | Label |
|---|---|---|
| Minimum protocol version | **TLS 1.3** (`rustls::version::TLS13` only) | **DEFAULT** |
| TLS 1.2 floor option | `tls12-floor` (1.2 + 1.3 offered) — opt-in via `FLEXCORE_EDGE_UPLINK_TLS_MINIMUM`, only per a site D6 record | opt-in (never default) |
| Cipher suites | The rustls `ring` provider DEFAULTS, recorded not customized: TLS 1.3 — TLS13_AES_128_GCM_SHA256, TLS13_AES_256_GCM_SHA384, TLS13_CHACHA20_POLY1305_SHA256; TLS 1.2 (only if the floor is opted in) — the provider's ECDHE suites. **No suite list is invented in this repo**; a per-site allowlist belongs to the signed D6 record | recorded (rustls defaults) |
| Endpoint scheme | `wss://` or `https://` REQUIRED; `ws://`, `http://`, scheme-less REFUSED (ledger P6.07 step 3) | DEFAULT |
| Server trust | Root CA pinned from the P6.06 credential record (`root_ca_pem`); an empty/unparsable root store is a named fail-closed error — the edge never trusts an unpinned platform | DEFAULT |
| Client identity | Device certificate from the P6.06 identity; key material from the deployment's key source (§18.3), loaded per session, never logged, never stored in the registry | DEFAULT |
| Reconnect/backoff | Exponential ×2 from `FLEXCORE_EDGE_UPLINK_BACKOFF_INITIAL_MS` (default 1 000) to `FLEXCORE_EDGE_UPLINK_BACKOFF_MAX_MS` (default 60 000), deterministic (no jitter — supervisory traffic, deterministic tests; jitter is an acceptable later refinement) | DEFAULT |
| Keepalive | Idle session past `FLEXCORE_EDGE_UPLINK_KEEPALIVE_MS` (default 30 000) is re-ESTABLISHED (fresh handshake) — no silent half-open reuse | DEFAULT |

## 3. The enrollment-honesty record (what is a DEPLOYMENT REQUIREMENT)

Nothing in this wave may look like a working PKI. The table below is the
verbatim boundary; code comments carry the same table.

| Concern | Implemented here | DEPLOYMENT REQUIREMENT (not implemented) |
|---|---|---|
| Key-pair generation (§18.3, hardware-backed where available) | NOTHING — no key-generation code exists anywhere in this crate (no rcgen, no openssl; the hard ban on production key generation is structural) | the site provisions the key pair at install time |
| Certificate issuance / the CA | The `CredentialIssuer` seam + `DenyAllCredentialIssuer` (refuses everything) + `POST /api/v1/edge/enroll` storing the identity RECORD only | a real CA/issuance service per the (OPEN) D6 review |
| Certificates themselves | The record's `certificate_pem` stays `None`; every 201 response says `certificate_issued: false` + the note | issued device certificates with real validity terms |
| OCSP/CRL revocation checking | The `RevocationChecker` seam + `UnconfiguredRevocationChecker` (fails closed — an un-established status is never "not revoked") | an OCSP/CRL adapter composed per deployment |
| Protected credential storage (§18.3) | `FilesystemCredentialStore`: versioned record format, 0600 owner-only on Unix, load-time permission re-check, write-to-temp+rename, key MATERIAL never stored (only a reference) | the protected volume; non-Unix permission enforcement (documented limitation — the store never stores key material regardless) |
| Single-use provisioning tokens | The seam + single-use semantics enforced by the issuer side (scripted issuer in tests; `StaticEnrollmentTokens` demonstrates the contract) | the real token-issuing/validation service (`ProvisioningTokenValidator` seam; shipped default denies all) |

Consequence for the mTLS channel (honest): with the shipped seam NO edge
can obtain a client certificate, so NO real channel session can be
established until a deployment composes an issuer. The channel code is
complete and fail-closed; the missing piece is a deployment input, recorded
here, not hidden in a stub.

## 4. The structural no-inbound guarantee

- The edge-agent crate contains NO listen/bind/accept surface. The ONLY
  socket operation is the outbound `TcpStream::connect_timeout` in
  `RustlsUplinkTransport::dial` (§3.3: the edge dials out or fails).
- Enforced by `crates/edge-agent/tests/no_inbound_listener.rs`: a lexical
  scan over every `.rs` file under `crates/edge-agent/src/` (including
  `main.rs`) refusing `TcpListener`, `UdpSocket`, `::bind(`, `.bind(`,
  `.listen(`, `listen(`, `.accept(`.
- Consistent with the P6.20 deployment profile (no `Listen*` directive, no
  listening socket in the binary).
- Lab confirmation (packet capture / host port scan — the ledger P6.07
  verification method) is NOT RUN (loop bans); recorded as open
  verification work.

## 5. New DIRECT dependencies — D4 verification note (required by ENTRY_NOTE boundary 3 and P6.02)

| Dependency | Version | License (source) | Note |
|---|---|---|---|
| `rustls` | `"0.23"` caret, `default-features = false`, features `ring`,`std`,`tls12`,`logging` — **UNPINNED-REVERIFY** (no in-repo source records a rustls version pin; RESEARCH.md has none) | MIT/Apache-2.0/ISC per crates.io metadata — **UNVERIFIED in this repo** (the P6.02 audit table flags rustls exactly this way) | First direct TLS dependency. `ring` provider chosen (DEFAULT: avoids the aws-lc-rs C build toolchain on edge targets); `default-features = false` guarantees exactly ONE provider feature (enabling both `aws_lc_rs` + `ring` is a documented rustls runtime panic hazard) |
| `rustls-pemfile` | `"2"` caret — **UNPINNED-REVERIFY** | MIT/Apache-2.0/ISC family per crates.io metadata — **UNVERIFIED in this repo** | PEM → DER for the pinned root CA, device certificate, and device key; the standard rustls pairing |

Named action (owner: legal + engineering, D4): the first human
`cargo deny check licenses` against the first lockfile must (a) verify both
licenses against the primary sources, (b) resolve the exact versions, and
(c) diff the transitive set — the P6.02 verification method applies
unchanged; any unlisted dependency fails the check. **No other dependency
was added** (no serde_json in the edge crate — the credential store uses a
documented line format; no rcgen anywhere).

Dependency-rule check: `python -I tools/check_dependency_rule.py` —
output recorded in the report below (edge-agent remains COMPOSITION; both
new deps are infrastructure, allowed there; no core-crate edges added).

## 6. Decisions (all DEFAULT / recorded)

| # | Decision | Alternatives rejected |
|---|---|---|
| D-P6.06-1 | The enrollment endpoint is an ADDITIVE path `/api/v1/edge/enroll` under the recorded P4.36/P5.31 rule (the 16 frozen §15.2 candidates govern dispatch-state mutation; this route mutates none; precedents: `/api/v1/audit-events`, `/api/v1/alarms`, `/api/v1/live`) | extending the frozen list (a documented revision — unnecessary; the list's purpose is the dispatch prohibition); a `/api/v1/sites/{id}/enroll` sub-resource (conflates site registry writes with provisioning) |
| D-P6.06-2 | Enrollment auth = provisioning-token validator SEAM, deny-all by default (NOT a role-gated caller; the edge is not a user) | reusing `IdentityResolver`/dev-auth (a user-identity model for a device credential would be the §18.2 failure mode — an edge trusted merely because it belongs to a customer site) |
| D-P6.06-3 | The device registry is append-only per identity TERM (`enrollment_seq`), current status = highest sequence; flexcore_app gets INSERT+SELECT only on both tables | an updatable status column (would need UPDATE grants and re-opens the 0008-style immutability question); a status event-sourcing without a current-row read (slower, no benefit here) |
| D-P6.06-4 | The credential store uses a documented line format (`flexcore-edge-credential-v1`) instead of adding serde_json to the edge crate | adding serde_json now (the P6.09 disk buffer owns wire serialization; wave-2b owns that Cargo.toml change — coordination recorded in HANDOVER.md) |
| D-P6.06-5 | Expiry monitoring is a pure clock-based assessment + audit pairing (`CredentialExpiryMonitor` + orchestrator), rotation window from config (default 7 days, max 30) | hardcoding a window; treating `None` expiry as "valid forever" (explicitly refused — `ExpiryStatus::NoExpiryRecorded`) |
| D-P6.07-1 | The TLS build lives in the transport FACTORY (`UplinkTransportFactory::build(material, tls_minimum)`), keeping the channel's reconnect state machine rustls-agnostic and fake-testable | building rustls config inside `compose` (would force real PEM parsing into every state-machine test — impossible without real key material, which the loop bans forbid generating) |
| D-P6.07-2 | Sync transport (std TCP + rustls `StreamOwned`) — the `UplinkChannel` port is sync by the P6.05 house rule; the adapter owns the blocking shape | tokio-rustls (a second async dependency for a sync port the runner drives tick-wise) |
| D-P6.07-3 | Deterministic backoff without jitter | jittered backoff (nicer fleet-wide de-synchronization, but the supervisory 0.x fleet is small and determinism keeps the state machine exactly testable — recorded as an acceptable later refinement) |
| D-P6.07-4 | The telemetry payload encoder is a seam with a refusing default | inventing a wire format in P6.07 (P6.08/P6.09 own the read path; a fabricated shape would be a hallucinated contract) |
| D-P6.07-5 | The composition root (`main.rs`) is NOT rewired this wave — the channel composes via `MtlsUplinkChannel::compose` with explicit dependencies; wiring it into the binary happens with the P6.08 read path (which supplies the encoder) | a half-wired composition root running with the refusing encoder (a startup-time guaranteed-degraded edge would be honest but useless; deferring keeps the diff surgical) |

## 7. Ownership deviations and coordination notes

- `crates/flexcore-storage/src/edge_enrollment.rs` is NEW (one additive
  `lib.rs` line): the storage trait must live where the Postgres adapter
  and the control plane can both see it — the P4.30 `FeasibilityCheckStore`
  home pattern. The wave's ownership list named the postgres impl + tests
  but not the flexcore-storage module; the addition is recorded here
  rather than silently absorbed.
- The enrollment store is NOT in the `UnitOfWork` request bundle: it is a
  provisioning surface whose Postgres implementation opens its OWN
  tenant-scoped unit of work per call (recorded in `state.rs`). This
  deviates from the per-request one-transaction rule deliberately — the
  registry write and its journal event still commit together inside that
  unit (§15.5 discipline applied at the enrollment boundary).
- `apps/control-plane/src/state.rs` gains two additive fields + two builder
  methods (both fail-closed defaults; no existing constructor signature
  changed).
- Concurrent wave-2b (P6.08/P6.09) owns the payload encoder composition,
  the disk buffer, and any serde_json addition in the edge crate; the
  P6.07 encoder seam is the single integration point.

## 8. Honest limitations (complete list)

1. Nothing was built, run, or installed (loop bans). Every test named
   above is UNRUN; the ledger ticks read "implemented, awaiting human
   build/test".
2. NO TLS handshake has ever been executed; the rustls-touching paths are
   UNRUN code against the recorded 0.23 API (the caret pin's UNPINNED-
   REVERIFY rule also covers API drift at resolve time).
3. No CA, no certificate issuance, no OCSP/CRL client, no real
   provisioning-token service exists (§3 — deployment requirements).
4. The lab verification methods of both subtasks (negative-test run,
   revocation-rejection in the lab, plaintext/wrong-CA/expired-cert
   connection attempts, no-listener host confirmation) are OPEN.
5. On non-Unix platforms the credential store's owner-only permission
   enforcement is a deployment requirement (documented in code and §3).
6. The Postgres implementation follows the sibling-module private-field
   pattern of the existing stores (flagged VERIFY-BUILD there as well);
   the human build pass covers it identically.
7. `FlexUuid`-shaped journal event ids come from a process-local counter +
   wall instant — unique within a process, and the real PK
   `(event_id, tenant_id)` refuses any collision; a deployment may replace
   the id derivation with the pool-backed UUIDv7 generator (the recorded
   D-S1 policy) when the route joins the main composition.
