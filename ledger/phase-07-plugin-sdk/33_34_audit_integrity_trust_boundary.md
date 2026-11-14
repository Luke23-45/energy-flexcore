# P7.33 + P7.34 — Audit-integrity proof and the plugin-host trust boundary

Date: 2026-11-13. Subtasks: P7.33, P7.34 of Phase 7. Owner role: Security
engineer (Rust). Report trace: P7.33 — §17.3, §17.5, §22.3, §5.1/§5.7;
P7.34 — §17.3, §18.2, §5.4 Class B, §8.3. Status: **IMPLEMENTED (the parts
implementable without a TLS stack, a PKI, or the platform's durable store)
— awaiting human build/test**. The loop bans hold: nothing installed,
nothing built, no tests run; every claim below is code-level, not a run
result.

Inputs consumed: `apps/plugin-host/src/{broker,enforcement,health,host,limits}.rs`
(the P7.10–P7.13/P7.20 audit surfaces), the kernel crate (READ-ONLY:
`crates/flexcore-kernel/src/{lifecycle,signing,manifest,recomposition}.rs`
in-module `AuditSink` ports and the P7.16 recomposition gate),
`crates/flexcore-storage/src/audit_read.rs` (P4.36 SELECT-only read
contract), `crates/adapters/postgres/src/audit.rs` + migration
`0008_audit_records.up.sql` (the append-only store reality),
`ledger/phase-04-grid-feasibility-dispatch/28_security_controls.md`
(D-P4.28-1: the FNV-1a 64 TEST-GRADE checksum pattern),
`crates/edge-agent/src/{uplink_mtls,enrollment}.rs` + `ledger/phase-06-
edge-protocol/06_07_identity_channel.md` (the mTLS/identity precedent,
mirrored by citation), `ledger/phase-07-plugin-sdk/{10_11,12_13,14_15_16,
19_20_36}.md`, `.agent/DECISIONS.md` (nothing here decides D6 or any open
gate).

---

## 1. P7.33 — what was implemented

Ledger P7.33 step 1 requires the audit-integrity MECHANISM to be recorded
and to actually detect alteration. The mechanism chosen: a **hash chain
over the audit sequence, per plugin and per host session**, with an
externally retained anchor.

| Artifact | Content |
|---|---|
| `apps/plugin-host/src/audit_chain.rs` | NEW: `fnv1a64` (TEST-GRADE), `AuditChainRecord` (canonical encoding, version-tagged `flexcore-audit-chain-v1`, length-prefixed fields), `ChainedAuditSink` (the `BrokerAuditSink` decorator that chains every host-issued event), `first_broken_link` + `verify_with_anchors` (the verifiers), `ChainAnchor`/`ChainBreak`/`ChainTruncation`/`ChainVerification`, and the `AUDIT_INTEGRITY_BOUNDARY` statement |
| `apps/plugin-host/tests/audit_integrity_p7_33.rs` | NEW: chain builds; honest chain verifies (with and without anchors); single-record mutation detected with the FIRST BROKEN LINK named (middle + last record); reordering detected (the verifier walks stored order, never re-sorts); middle deletion detected as a sequence gap; truncation detected against the anchor (with the honest no-anchor limit asserted); audit-or-refuse preserved (a failed inner write leaves the chain empty); empty session id refused; boundary statement asserted as documentation; canonical encoding deterministic + mutation-sensitive |
| `apps/plugin-host/src/lib.rs` | EDITED (additive): `pub mod audit_chain;` + the module-table row |

Mechanics: `digest(n) = FNV1a64(canonical_encoding(session, chain_key,
sequence, prev_digest, body))`; `prev_digest(1) = GENESIS_DIGEST (0)`;
`prev_digest(n) = digest(n-1)`; chain key = the plugin id, or `host` for
events that name no plugin. The writer (`ChainedAuditSink`) forwards each
event to the inner (durable-side) sink FIRST and chains only a successful
write — audit-or-refuse preserved, the local chain never leads the
durable store. A verifier detects: any mutated field (digest self-check),
insertion/reordering (prev-digest or sequence discontinuity), deletion
(sequence gap), and truncation (against the `ChainAnchor` — the head
digest + count the platform retains after each flush, mirroring the way
the platform's own store anchors evidence).

### The digest honesty (the signing.rs / D-P4.28-1 pattern)

The digest is **FNV-1a 64 — TEST-GRADE**. It is the repo's existing
NON-cryptographic integrity-checksum pattern (D-P4.28-1: "an integrity
CHECKSUM, not a security primitive; no cryptographic property is
claimed"). No new dependency and no key material was introduced (the task
bans both). **Production hash = DEPLOYMENT REQUIREMENT**: before any
non-simulation use, the chain digest MUST be replaced by a Stage
6/9-qualified cryptographic hash (e.g. SHA-256) preserving the chaining
structure; the canonical encoding carries the `flexcore-audit-chain-v1`
version tag so that swap is a deliberate, detectable format change. No
"secure" property is claimed anywhere in code or docs.

### The boundary statement (P7.33's honesty core)

Recorded verbatim as `AUDIT_INTEGRITY_BOUNDARY` (asserted by the tests):

> Hash-chaining proves tamper-EVIDENCE, not tamper-PROOF: an attacker who
> controls the host process can rebuild the whole chain. Authoritative
> audit integrity lives in the platform's append-only audit store
> (PostgreSQL `audit_records`, migration 0008: INSERT/SELECT-only grants,
> the unconditional append-only trigger, and tenant-isolation RLS; the
> SELECT-only read contract `flexcore-storage/src/audit_read.rs`). The
> plugin-path hash chain is the LOCAL evidence layer ... The kernel's
> in-module AuditSink ports (lifecycle, signing) do not chain — recorded
> seam; this chain covers host-issued records. TEST-GRADE digest:
> FNV-1a 64; production hash = DEPLOYMENT REQUIREMENT.

The Phase 4 storage reality cited (reuse-by-citation, not reimplementation):
migration 0008 enforces append-only in three layers (INSERT/SELECT-only
grants to `flexcore_app`; a BEFORE UPDATE/DELETE trigger that raises
unconditionally, including for the table owner; the AuditSink port
exposing append only), with `tenant_isolation` RLS and corrections as NEW
rows (`corrected_audit_id`), never in-place edits; the read side is the
SELECT-only P4.36 contract (`audit_read.rs` — exactly two read methods,
source-text-pinned).

### Decisions

| ID | Decision | Status |
|---|---|---|
| D-P7.33-1 | Hash chain (not an append-only store) at the host boundary: the host cannot own a store; a chain over its own records + a platform-retained anchor gives detection of mutation/insert/reorder/delete/truncate with std-only code | DEFAULT |
| D-P7.33-2 | FNV-1a 64 TEST-GRADE digest, version-tagged canonical encoding; production hash = DEPLOYMENT REQUIREMENT (no crypto dep, no keys) | DEFAULT (the signing.rs honesty pattern) |
| D-P7.33-3 | Chain key = plugin id, host-level events (empty plugin id, e.g. operator-initiated revocations) chain under `host`; one chain space per host session id (blank session refused at composition) | DEFAULT |
| D-P7.33-4 | Writer order: durable inner sink first, chain second; a failed inner write refuses and chains nothing (audit-or-refuse, never a chain that leads the store) | DEFAULT |
| D-P7.33-5 | Truncation is detectable ONLY against the external anchor; the verifier asserts this limit in the tests rather than hiding it | DEFAULT |

## 2. P7.34 — what was implemented

| Artifact | Content |
|---|---|
| `apps/plugin-host/src/transport_trust.rs` | NEW: `HostServiceIdentity` + `HostIdentityStore` + `assemble_trust_material` (the P6.06 enrollment pattern mirrored — fail-closed on identity-missing, identity-incomplete, and CA-pin-missing), `MtlsTrustConfig` (pinned CA reference, pinned server name, plaintext refusal, `TlsMinimum` mirror of `uplink_mtls.rs`, short-lived session TTL, recorded `rotation_owner`, `expiry_monitor_enabled`, `RevocationChecking`), `HostTrustMaterial` (REFERENCES only — no key material, no PKI), `TRUST_POSTURE` (the 6-layer boundary table), `COMPROMISED_HOST_BOUND`, `never_granted_bound()` (re-export of the broker's set — ONE set, not a copy), `TRANSPORT_WIRING_SEAM`, `TrustError` with stable names |
| `apps/plugin-host/tests/transport_trust_p7_34.rs` | NEW: identity-missing / certificate-reference-missing / key-reference-missing → fail-closed (`TRUST_IDENTITY_MISSING`/`TRUST_IDENTITY_INCOMPLETE`); CA-reference-missing → fail-closed (`TRUST_CA_PIN_MISSING`, with the no-system-store-fallback rule); plaintext/scheme-less endpoints refused; rotation/expiry/revocation posture REQUIRED (each refusal named); honest composition assembles references-only material; trust-posture table + compromised-host bound + transport seam asserted as documentation; the never-granted set restated BEHAVIORALLY through the broker (self-grant of `dispatch.authorize` refused `PERMISSION_NEVER_GRANTABLE`; check refused; default-deny for an ungranted capability; both audited denials land tamper-evident in the P7.33 chain) |
| `apps/plugin-host/src/lib.rs` | EDITED (additive): `pub mod transport_trust;` + the module-table row |

### The trust posture (restated)

The host executes untrusted code and is **NOT in the TCB for authorization
decisions**: the kernel/enforcement are policy, the PLATFORM is the
authority. A compromised host can **deny service and expose its own
inputs**; it cannot grant itself capabilities (the never-granted set —
`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` — is
re-refused at grant AND check time regardless of what any manifest,
profile, or host call says), cannot raise the control-authority mode, and
cannot forge authoritative audit (P7.33's boundary statement). Every
result it returns is untrusted input the platform validates (schema, §9.2
provenance, tenant scope) before the protected path sees it; a proposal
gains no authority from having been proposed (§13.4, ADR-010). The full
table is `TRUST_POSTURE` in the module.

### The transport is a RECORDED seam

The gRPC-with-mTLS transport itself is config-shaped only. **rustls is
NOT added to plugin-host in this wave**; `TRANSPORT_WIRING_SEAM` records
the exact composition-time additions — `rustls` with
`default-features = false` and exactly one crypto-provider feature (the
`ring` provider, avoiding the aws-lc-rs C toolchain, per the edge-agent
precedent), `rustls-pemfile` for PEM parsing, plus the chosen gRPC stack
for the P7.20 seam — each through the D4 audit-note discipline, pins
UNPINNED-REVERIFY until the first human lockfile. No TLS handshake, no
socket, no PKI exists in this crate. Identity MATERIAL (certificates,
keys) is a DEPLOYMENT REQUIREMENT: the module carries references only,
and a config that cannot verify revocation, monitor expiry, or name a
rotation owner REFUSES to compose (the config-shaped enforcement of the
ledger's rotation/revocation posture; session-time refusal and in-flight
drain of a revoked identity remain the platform-side gate over the same
seam).

### Decisions

| ID | Decision | Status |
|---|---|---|
| D-P7.34-1 | Service identity = the P6.06 pattern mirrored (references only; no PKI, no key generation, no key material in this crate — the `enrollment.rs` honesty pattern taken further: no issuer seam at all) | DEFAULT |
| D-P7.34-2 | Fail-closed rules carried over from `uplink_mtls.rs`: pinned CA from the deployment config (never the ambient system store), pinned server name, plaintext/scheme-less refusal, TLS 1.3 default with opt-in `tls12-floor` (D6 stays OPEN — a signed D6 record changes configuration, not structure) | DEFAULT (mirrored) |
| D-P7.34-3 | Rotation owner / expiry monitor / revocation checking are REQUIRED validated configuration, not code defaults; the session-time revocation gate is the platform's, over the recorded transport seam | DEFAULT |
| D-P7.34-4 | `never_granted_bound()` re-exports `broker::NEVER_GRANTED_PERMISSIONS` instead of restating a copy — the compromised-host bound and the enforcement bound are ONE set | DEFAULT |
| D-P7.34-5 | The mTLS transport is not implemented; the dependency/feature list is recorded in `TRANSPORT_WIRING_SEAM` for the composition owner (rustls NOT added to plugin-host this wave) | RECORDED SEAM |

## 3. The boundary statement (recorded exactly)

See §1 — the statement is recorded verbatim in
`apps/plugin-host/src/audit_chain.rs::AUDIT_INTEGRITY_BOUNDARY` and
asserted phrase-by-phrase by
`apps/plugin-host/tests/audit_integrity_p7_33.rs::the_boundary_statement_
is_asserted_as_documentation`, with the same statement carried in the
test file's header doc-comment.

## 4. Cross-task closure notes

- **The never-granted set** is restated (not copied) as the
  compromised-host bound (D-P7.34-4), and pinned behaviorally through the
  broker in the P7.34 tests — the same set `12_13_enforcement_negatives.md`
  tests at the enforcement gate.
- **P7.37 cross-reference:** the audit row of `TRUST_POSTURE` names the
  authoritative store, supporting the §17.5 invariant map's audit rows.
- **P7.39 cross-reference:** tenant scoping of the audit read path is the
  migration-0008 RLS + explicit-filter pattern (`audit_read.rs` header);
  data-level tenant filtering on plugin results remains P7.39's proof.
- **P7.16 closure support:** a profile switch cannot drop audit coverage
  through the host vocabulary — no permission id touches audit (the
  closed `ALLOWED_PERMISSIONS` has no audit-disable id), and the
  kernel-side recomposition gate (P7.16, DEFAULT decision D-3: EVERY step
  gated) is the second layer.

## 5. Recorded seams (NOT implemented — on purpose; kernel read-only)

| Seam | Why it stays open | Owner / closure path |
|---|---|---|
| Kernel `lifecycle.rs` / `signing.rs` in-module `AuditSink` ports do NOT chain | The kernel crate was READ-ONLY this wave; the third AuditSink (kernel-local, distinct from `flexcore-audit::AuditSink` and `edge-agent::ports::AuditSink` — the naming note in `07_08_lifecycle_registry.md`) feeds the host's audit path but builds no hash chain | Kernel owner: wrap the kernel-local sink records in the same chain discipline (or route them through the host chain at composition) |
| P7.33 step 3 (host-kill survival drill) | Requires the durable store over the transport seam; the in-memory inner sink cannot survive a process death by design, and this wave does not fabricate a store | Composition owner + control plane: run the drill against the real append-only store once the P7.20 gRPC seam is wired |
| P7.33 step 4 (profile-switch audit-coverage refusal; retention per §16.3) | The coverage refusal lives in the kernel's recomposition gate (kernel read-only); retention is the platform store's §16.3 setting | Kernel owner (recomposition); platform owner (retention config) |
| P7.34 ledger step 3 (compromised-host result suite at the PLATFORM) | The result-validation gate is the control plane's, over the P7.20 seam; this wave enforces the posture on the host side and records it | Control-plane owner: forged/oversized/wrong-tenant results rejected + host reported failed |
| P7.34 ledger steps 1/4 at the wire (short-lived certs, rotation, session revocation, drain) | No TLS stack in this crate (D-P7.34-5); the config refuses to compose without the posture recorded | Composition owner, with D6 reviewer sign-off (D6 OPEN) |
| mTLS on the P7.20 health/telemetry traffic | Recorded in `19_20_36_config_health_path.md` §5 and in `health.rs`; the trust boundary applies to the WHOLE host↔platform channel (the ledger's "happy-path-only mTLS" risk is answered by one config for the one channel) | Composition owner |

## 6. Acceptance-criteria position (honest)

- P7.33: "Tampering is detected in every test case" — implemented as
  mutation/insert/reorder/delete/truncate detections (test-side,
  WRITTEN-UNRUN); "host-kill loses no completed-decision event" — SEAM
  (§5); "a switch that drops audit coverage is refused" — enforced by
  vocabulary absence + recorded kernel seam (§4/§5); "retention is
  recorded" — platform store setting, recorded as a seam.
- P7.34: "mTLS with rotation/revocation is implemented and tested" —
  NOT met at the wire by design (recorded seam, D-P7.34-5); met at the
  config/identity boundary (fail-closed construction, REQUIRED posture);
  "every compromised-host result is rejected" — platform-side gate,
  recorded seam; "a revoked identity gets no session" — config-shaped
  enforcement plus the platform-side session gate, recorded seam.

Nothing above is "verified" or "tested" in the run sense (AGENTS.md rule
7); the verification methods of both subtasks (independent re-run of the
tamper suite; run the compromised-host suite; inspect the rotation log)
require the human build/test gate first.
