# Implementation plan — the two HIGH gaps (HEAD `74d56f4`)

**Status:** PLAN ONLY. No code written, nothing installed, built, or run.
**Scope:** exactly the two HIGH gaps from the `d6eeddc` review, re-verified still present at HEAD `74d56f4` on 2026-11-18:
- **Gap A — Postgres `TelemetryReader` has no implementation.** Only `FakeTelemetryReader` exists; `apps/control-plane/src/main.rs` passes `None`; the `/telemetry/query`, `/history`, `/current-state`, `/quarantine` routes fail closed.
- **Gap B — Source authentication is a recorded STUB.** `authenticate_source` checks only self-reported health + an optional trusted-source-id string; `SOURCE_AUTH_STATUS` says so in plain text.

**Goal:** patches that are robust and state of the art — not just gap-closing. Each patch below states the design decision, why it is the state-of-the-art choice (with sources), the exact files to touch, the acceptance criteria, the tests to write, and what it deliberately does NOT do.

**Loop rules that still bind the implementer** (from `ledger/IMPLEMENTATION_PROMPT.md`): do not install, build, or run tests — write complete test files with real assertions; Krishna builds, runs, and patches. Max 2 subagents at a time, batches of 5–8 subtasks, no shared files between subagents, coordinator merges. Tick ledger boxes only as `implemented, awaiting human build/test`. Never claim tested/verified. D1–D6 are not decided by these patches.

**Order:** Patch A first (smaller, unblocks a whole API surface, no new crypto), then Patch B. They share no files except `apps/worker/src/ingestion.rs` documentation and the ledger — sequence them, don't parallelise across the boundary.

---

## Patch A — Postgres `TelemetryReader`

### A.0 What exists today (verified in-tree)

- Trait `TelemetryReader` — `crates/flexcore-ports/src/telemetry_read.rs:421`, three methods: `query(ctx, TelemetryReadQuery) -> TelemetryPage`, `latest_state(ctx, asset_id) -> Vec<NormalizedTelemetry>`, `list_quarantine(ctx) -> Vec<QuarantinedTelemetry>`. Sync by house rule (no future in, no future out).
- `TelemetryReadQuery` — asset, `from`/`until` (`EventTime`), optional quantities, optional quality, `limit: u32` (zero rejected), optional `TelemetryCursor`.
- Table `telemetry` (migration `0010_telemetry_storage_p2_06.up.sql`): PK `(asset_id, event_time, source_sequence, quantity)`; indexes `telemetry_asset_time_idx`, `telemetry_tenant_asset_time_idx`, `telemetry_asset_quantity_time_idx`; `GRANT SELECT ON telemetry TO flexcore_query`, `GRANT INSERT, SELECT TO flexcore_ingest`; RLS policy `tenant_isolation`. Quarantine table `telemetry_quarantine` with tenant index.
- `PgUnitOfWork` (`crates/adapters/postgres/src/unit_of_work.rs:86`): one native transaction behind `Arc<AsyncMutex<Option<Tx>>>`; `tx`/`runtime` are private fields. `state.rs:139-146` records the intended landing spot: the implementation must land **on `PgUnitOfWork`** so reads run inside the request transaction with RLS applied.
- Routes already fail closed correctly (`routes_phase2.rs:445-448` returns opaque infrastructure error when the reader is `None`; `map_read_error` never forwards storage text). That behaviour must be preserved, not worked around.

### A.1 Design decision (the one the implementer must resolve first)

**Tension to resolve, not paper over:** `AppState.telemetry` is a long-lived `Option<Arc<dyn TelemetryReader>>`, but `PgUnitOfWork` is a per-request, transaction-bound object. Two candidate shapes:

1. **Recommended — pool-held reader, fresh read transaction per call.** Implement `PgTelemetryReader { pool, runtime}` in the postgres adapter crate. Each trait call opens its own transaction, sets the tenant context with `SELECT set_config('app.tenant_id', $1, true)` (transaction-local), runs the read, commits (read-only). Sync trait surface preserved by `block_on` through the adapter's runtime, exactly as `PgUnitOfWork::commit` already does.
2. **Alternative — per-request reader built from the `PgUnitOfWork`.** Faithful to the `state.rs` note, but requires changing `AppState`/`routes_phase2.rs` so handlers receive a reader derived from the request's unit of work. More invasive; only choose it if the coordinator judges the pool-held shape violates the P1.10 unit-of-work rules — and if so, record the deviation and update the `state.rs` note in the same patch.

Either way the reader lives in `crates/adapters/postgres` (dependency rule: domain/ports must never depend on sqlx). Never implement it in `flexcore-ports` or the control plane.

### A.2 State-of-the-art read design (from the research)

- **Tenant isolation in depth, not in one layer.** The API layer passes a `TenantContext` built only from authenticated claims; the SQL adds an explicit `tenant_id = $1` predicate; RLS is the backstop. RLS must be `FORCE ROW LEVEL SECURITY` so even the table owner cannot bypass it, and the app role (`flexcore_query`, SELECT-only) must have neither superuser nor `BYPASSRLS`. Test RLS as the application role, never as the migration owner — an owner-role test passes vacuously.
- **Keyset pagination, never OFFSET.** `query` is already specified keyset-paginated. Sort key `(event_time, source_sequence, quantity)` — unique by the PK, so no boundary row is ever skipped or repeated. Cursor = opaque base64url encoding of the full-precision sort key (never a millisecond-truncated timestamp: `timestamptz` is microsecond-precise; truncating it silently skips rows). Malformed cursor → 400, never silent fallback to page one. Index must match the ORDER BY tuple exactly, including direction.
- **Snapshot consistency for multi-statement reads.** `latest_state` is one statement (`DISTINCT ON (quantity) … ORDER BY quantity, event_time DESC, source_sequence DESC` — the canonical Postgres idiom for latest-per-group). If any read ever needs two statements that must describe the same moment, run it in `REPEATABLE READ`; under default Read Committed, two selects can see different row versions.
- **Reads are read-only.** Use a read-only transaction; the reader must never write, never `SET` session-level state (only `SET LOCAL` / `set_config(..., true)`), and must never leak the tenant setting into the next pooled request.

### A.3 Files to touch (Patch A)

| File | Change |
|---|---|
| `crates/adapters/postgres/src/telemetry_reader.rs` (new) | `PgTelemetryReader` implementing all three trait methods; row-mapping from `telemetry` / `telemetry_quarantine` to `NormalizedTelemetry` / `QuarantinedTelemetry`; cursor encode/decode; `TelemetryReadError` mapping (validation errors stay validation; storage failures stay opaque infrastructure errors) |
| `crates/adapters/postgres/src/lib.rs` (or `mod.rs`) | Export the reader; register it in `tools/check_dependency_rule.py`'s member/edge lists if the checker requires it |
| `apps/control-plane/src/main.rs` | Wire `Some(Arc::new(PgTelemetryReader::new(pool, runtime)))` in place of `None`; delete the P2.06-adjacent gap comment only when the wiring lands |
| `apps/control-plane/src/state.rs` | Update the field docs (remove the "does not implement yet" note) |
| `crates/adapters/postgres/migrations/0026_telemetry_read_indexes_*.sql` (new, **only if** `EXPLAIN ANALYSE` shows a gap) | Composite index matching the query ORDER BY, e.g. `(tenant_id, asset_id, event_time, source_sequence, quantity)`; up/down pair; verify the existing three indexes first and do not duplicate them |
| `crates/adapters/postgres/tests/telemetry_reader_*.rs` (new, several files) | See A.5 — written, not run |
| `ledger/phase-02-asset-telemetry/ledger_phase_2.md`, `CHECKLIST.md`, `HANDOVER.md` | Tick P2.06-adjacent criteria only as `implemented, awaiting human build/test`; record the A.1 design decision and the index evidence |

### A.4 Acceptance criteria (each must be testable)

1. All four routes return real data for the owning tenant and 404/empty (never another tenant's rows) for anyone else — verified against a real Postgres, as the `flexcore_query` role.
2. Keyset pagination: walking a 250-row range in pages of 50 returns every row exactly once, in ascending sort-key order, even when rows are inserted mid-walk.
3. `latest_state` returns exactly one row per quantity present, the latest `event_time` (ties: higher `source_sequence`); a late-arriving record with a newer `event_time` updates it; one with an older `event_time` does not regress it; a quantity with no record is absent (API derives MISSING, never zero).
4. `list_quarantine` returns only the caller's tenant rows, in quarantine order, with raw payload + reason + source identity + timestamps.
5. RLS negative test: a query run as the app role with no tenant set returns zero rows; a query with tenant B's context returns zero of tenant A's rows even if the SQL predicate is deliberately removed in the test double (RLS as backstop).
6. Cursor round-trip preserves microsecond precision; a truncated/malformed cursor is a 400, not page one.
7. `EXPLAIN ANALYSE` on the query path shows an index scan on the sort key (no sequential scan, no sort node) at 10k+ rows per asset.
8. No `.unwrap()` in production code; no storage error text reaches any API response; `cargo clippy` + `fmt --check` + dependency-rule checker clean (Krishna runs these).
9. The `schedule_store.rs` lesson is applied: the reader must not access `PgUnitOfWork`'s private `tx`/`runtime` fields from a sibling module — if the chosen shape needs them, add an explicit accessor or constructor on `PgUnitOfWork` itself instead of reaching across the module boundary.

### A.5 Tests to write (complete files, real assertions, no stubs)

- `telemetry_reader_query.rs` — range + quantity + quality filters, inverted range / zero limit rejected, pagination walk, mid-walk insert stability, cursor precision + malformed cursor.
- `telemetry_reader_latest_state.rs` — per-quantity latest, tie-break, late-arrival update/no-regress, absent quantity.
- `telemetry_reader_quarantine.rs` — tenant scoping, ordering, payload/reason fidelity.
- `telemetry_reader_rls.rs` — runs as the `flexcore_query` role: no-tenant → zero rows; cross-tenant → zero rows; owner-role bypass attempt fails (documents `FORCE RLS`).
- `telemetry_reader_errors.rs` — storage failure → opaque infrastructure error; no Postgres text in the response body.
- **Fix the silent-skip trap while here:** these tests are DB-gated. A plain `cargo test` without Postgres must not go green while the adapter layer never runs — gate them so CI fails loudly when `FLEXCORE_TEST_DATABASE_URL` is unset (fail, don't skip), matching the existing HIGH-adjacent finding that DB-gated tests silently skip.

### A.6 Explicitly out of scope for Patch A

TimescaleDB, read replicas, caching, a new query language, changing the trait, changing the table schema (except the conditional index migration), touching the ingestion/write path.

---

## Patch B — Cryptographic source authentication

### B.0 What exists today (verified in-tree)

- `authenticate_source` (`apps/worker/src/ingestion.rs:473`) — health check + optional `trusted_source_id` string equality. No cryptography anywhere in the worker/ports ingestion path (grep for hmac/signature/mTLS/client-cert: zero hits).
- `SOURCE_AUTH_STATUS` (`ingestion.rs:159`) — the STUB constant, quoted in the review.
- `TelemetrySource` port (`crates/flexcore-ports/src/telemetry_source.rs:834`) — provider declaration, `subscribe`, `decode`, `source_id`, `health`. Exactly one provider in Phase 2: the simulator (P2.14). `decode` turns one opaque raw frame into canonical records.
- Transport: the worker connects with `async_nats::connect(&config.nats_url)` (`apps/worker/src/main.rs:190`); the NATS adapter has no TLS configuration at all (zero `tls` hits in `crates/adapters/nats/src`).
- Identity groundwork already in the tree: migration `0018_edge_device_registry_p6_06` creates `edge_device_registry` and explicitly records "NO key material, NO certificate" — the registry exists, the key columns do not. The edge-agent crate already plans `rustls 0.23` + `rustls-pemfile` for an outbound mTLS channel (P6.07), flagged as an open D4 verification item; no key-generation crate exists anywhere (no `rcgen`, no `openssl`).

### B.1 Threat model (state it in the patch, then design to it)

1. **Spoofing** — an unauthenticated sender injects telemetry as a real source.
2. **Tampering** — a frame is altered in transit (including by a compromised broker or a stolen broker credential).
3. **Replay** — a legitimately signed frame is re-sent later to mask current conditions.
4. **Compromise blast radius** — one leaked credential must not expose the fleet.
5. Explicitly **not** in scope: device hardware compromise (no TPM/HSM in this phase), PKI operations at fleet scale, command-path authorisation (that's D6 / Stage 6 territory).

The governing principle, from the current industrial-IoT practice: **TLS protects the hop, not the message.** Integrity must travel *with the message*, so a forged message fails verification, a tampered message fails its digest, and a replayed message fails sequence checks — even if the broker or a broker credential is compromised.

### B.2 Design decision — message-level signed envelopes first, transport mTLS second

**Recommended order (and why):** implement **Ed25519 signed envelopes at the message level first**, then harden the NATS transport. Three reasons:

1. **It works over the transport that exists today.** The worker currently connects to NATS with no TLS at all; message-level signing is transport-independent and survives a compromised broker, whereas mTLS alone authenticates only the hop and does nothing once an attacker holds valid broker credentials.
2. **No PKI has to exist first.** There is no CA, no certificate issuance, no key store in the tree. Per-device Ed25519 keypairs are generated on the device, the private key never leaves it, only the public key is registered — the same primitive trusted across SSH and modern PKI, fast enough for modest gateway CPUs (RFC 8032).
3. **It is testable without operations.** Envelope verification is pure cryptography + database lookups; it needs no certificates, no broker reconfiguration, no fleet rollout to verify.

**Transport hardening is not dropped — it is Patch B, step 2** (below), and the plan must say so in the code comments, so nobody mistakes step 1 for the finished job. The industry baseline for the transport layer is TLS 1.2+ (prefer 1.3), mutual TLS as the production gold standard, per-device X.509 certificates, and per-subject authorisation so a device can publish only to its own namespace. NATS itself supports exactly this: TLS client certificates, NKeys (Ed25519 JWTs), auth callouts, per-user subject permissions, and JetStream encryption at rest (AES / ChaCha20-Poly1305).

### B.3 Envelope design (normative for the implementer)

- **Algorithm:** Ed25519 (RFC 8032), `ed25519-dalek` (pure Rust, no OpenSSL dependency — keeps the D4 license audit simple; add it to the audit table, do not silently add a crypto crate).
- **Envelope fields:** `key_id`, `source_id`, `tenant_id`, `sequence` (per-source monotonic `u64`), `event_time`, `payload_digest` (SHA-256 of the canonical frame bytes), `signature` (64 bytes over the canonical signing bytes).
- **Canonical signing bytes:** domain-separated — `b"flexcore-telemetry-v1"` + length-prefixed fields in fixed order. Never sign raw JSON (key order / whitespace ambiguity is a signature-malleability hole). Use one canonical serialisation, implemented once, tested with byte-exact vectors.
- **Verification (new Stage 1 in the ingestion pipeline, before `decode`):**
1. Look up the source's registered public key by (`tenant_id`, `source_id`, `key_id`) — **never accept a self-asserted key**. Unknown source / unknown key → reject + quarantine.
2. Verify the Ed25519 signature over the canonical bytes. Failure → reject + quarantine, counted, never silently stored.
3. Verify the SHA-256 payload digest. Failure → same.
4. Replay checks: `sequence` must be greater than the last accepted sequence for that source (persisted watermark); a duplicate `sequence` with identical bytes is an acknowledged duplicate, not a re-store; a duplicate with different bytes is a conflict → quarantine. `event_time` must fall inside a configured acceptance window (default ±5 minutes, the industrial-IoT norm for unsynchronised clocks; configuration, not a constant).
5. Only then does the frame reach `decode` and the existing stages 2–7. A failed record is quarantined with raw payload + reason + source identity + timestamps, per the P2.07 pipeline rule — never silently dropped, never stored as GOOD.
- **Key lifecycle:** registration binds a public key to (`tenant_id`, `source_id`, site) out of band (enrollment flow, reusing the `0018` edge-registry pattern — add key columns by migration, never store private keys). Rotation: `key_id` versioning with overlapping validity; revocation: a revoked key verifies as rejected from the moment of revocation, with an audit record. Compromise of one key affects one source only — no fleet-wide shared secrets, ever.
- **Simulator:** the simulator provider signs with an ephemeral dev key generated at startup in tests and in local dev only. Production configuration must refuse unsigned frames and refuse dev keys — enforce with a CI gate and a startup fail-fast check, not a comment.
- **Keep `trusted_source_id`:** it stays as a defence-in-depth allowlist, explicitly documented as *not* authentication. Do not remove it and do not let any code path treat it as proof of identity.

### B.4 Files to touch (Patch B)

| File | Change |
|---|---|
| `crates/flexcore-ports/src/telemetry_source.rs` (or a new `source_auth.rs` in `flexcore-ports`) | `SignedEnvelope` type, canonical signing-bytes construction, `SourceAuthenticator` trait (verify-only; key lookup is a port, not an implementation detail) |
| `crates/flexcore-ports/src/telemetry_source.rs` | `TelemetrySource` gains an optional signing identity (default: unsigned — the simulator overrides it); `decode` contract unchanged |
| `crates/adapters/postgres/migrations/0026_source_keys_*.sql` (new; next free number after `0025`) | `source_signing_keys` table: (`tenant_id`, `source_id`, `key_id`, `public_key` bytea, `valid_from`, `valid_until`, `revoked_at`, `created_at`); RLS tenant policy; least-privilege grants; up/down pair. Check the number at implementation time — do not hard-code it if a later migration lands first |
| `crates/adapters/postgres/src/source_keys.rs` (new) | Key lookup + sequence-watermark read/write inside the ingestion transaction (atomic with the store, so a crash cannot accept a frame without recording its sequence) |
| `apps/worker/src/ingestion.rs` | New verification stage before `decode`; replace the `SOURCE_AUTH_STATUS` STUB constant with the real status text; `authenticate_source` becomes verification + allowlist, and its name/docs must stop implying the old behaviour |
| `apps/sim-runner/src/` (simulator provider) | Sign outgoing frames with the dev key; expose a `signing` test hook that generates an ephemeral keypair |
| `crates/adapters/nats/src/` + `apps/worker/src/main.rs` + NATS server config | **Step 2 — transport:** TLS 1.3 to the broker; per-client credentials (NKeys or client certificates); per-user publish/subject permissions (telemetry subjects only for sources); JetStream encryption at rest. If the deployment has no PKI yet, ship the config + code with TLS *required* in production profiles and document the CA bootstrap as a deployment prerequisite — do not ship a "TLS optional" default |
| `Cargo.toml` (workspace) | Add `ed25519-dalek` + `sha2` (pinned, `default-features = false`); record both in the P6.02/D4 license-audit table |
| `crates/edge-agent/` | Note only: the P6.07 rustls mTLS channel is the device-side counterpart of step 2 — do not re-implement it here; cross-reference it |
| `ledger/phase-02-asset-telemetry/ledger_phase_2.md`, `CHECKLIST.md`, `HANDOVER.md` | Tick P2.10 step 6 only as `implemented, awaiting human build/test`; record the envelope version string and the key-rotation policy |

### B.5 Acceptance criteria (each must be testable)

1. An unsigned frame in production mode is rejected + quarantined, never stored; the same frame in dev/test mode with the dev key is accepted only when the dev-key flag is explicitly enabled.
2. A frame signed with the wrong key, a tampered payload (one byte flipped), or a wrong digest is rejected + quarantined.
3. A replayed frame (same sequence, same bytes) is detected as a duplicate and not re-stored; the same sequence with different bytes is quarantined as a conflict.
4. A frame with `sequence` older than the watermark is rejected; gaps in sequence are accepted and audited (devices restart; gaps are not proof of attack).
5. A frame outside the ±5-minute acceptance window is rejected or quarantined (config decides; the choice is recorded, not left implicit).
6. A revoked key's frames are rejected from revocation onward, including frames signed before revocation but delivered after it.
7. A frame from source A replayed with source B's identity fails (the signature is bound to the registered key for that source/tenant).
8. Key rotation: a source with two valid `key_id`s (old + new) verifies under both during overlap, and under only the new one after the old expires.
9. No private key material appears in the repo, in logs, in test fixtures, or in any committed config. Test keys are generated ephemerally per test run.
10. Transport step: with TLS required, a plaintext NATS connection is refused; a client without a valid credential cannot publish to any telemetry subject.
11. This patch does **not** close D6 (cyber/OT boundary review) — record that explicitly in the HANDOVER. D6 stays OPEN until its gate.

### B.6 Tests to write (complete files, real assertions, no stubs)

- `source_auth_envelope.rs` — canonical-bytes vectors (byte-exact, including a known-answer vector generated once and frozen), sign/verify round-trip, tamper-one-byte, wrong-key, wrong-tenant/source binding.
- `source_auth_replay.rs` — duplicate, conflict, old sequence, gap acceptance, window edge cases (exactly at the boundary, one second over).
- `source_auth_keys.rs` — registration, rotation overlap, expiry, revocation, unknown key/source.
- `source_auth_pipeline.rs` — end-to-end through the worker pipeline with a real Postgres + NATS (DB-gated, fail-loudly-if-unset per A.5): signed simulator frame stored; unsigned/tampered/replayed frame quarantined with the right reason and counted, never stored as GOOD.
- `source_auth_transport.rs` — plaintext refused, unauthenticated publish refused, wrong-subject publish refused (requires the step-2 broker config; write it in the same patch, gated on the test broker).
- Negative test that must exist: a test asserting the old behaviour is gone — `trusted_source_id` alone, with no valid signature, does **not** authenticate a frame.

### B.7 Explicitly out of scope for Patch B

Hardware-backed keys (TPM/HSM), fleet PKI operations and certificate issuance, command-path authorisation, the P6.07 edge-agent mTLS implementation itself (cross-referenced, not built here), any change to the telemetry schema beyond the key table, closing D6.

---

## Shared verification checklist (Krishna's build/test round — the implementer writes it into each HANDOVER)

```sh
cargo generate-lockfile
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
python3 tools/check_dependency_rule.py
# gated suite (Postgres + NATS up, per the Compose smoke setup in the P1/P2 HANDOVERs)
FLEXCORE_TEST_DATABASE_URL=... FLEXCORE_TEST_NATS_URL=... cargo test --workspace
```

Treat any failure as a patch round, not a surprise. The ledger ticks stay `implemented, awaiting human build/test` until this round passes.

## Sources

- PostgreSQL RLS with Rust/sqlx, transaction-local tenant context, `FORCE ROW LEVEL SECURITY`, testing as the application role — https://www.mehdi.cz/blog/sql-postgresql-rust-row-security-tenant-context
- Read Committed vs Repeatable Read snapshots for multi-statement reads — https://www.mehdi.cz/blog/sql-postgresql-rust-row-snapshots
- RLS as defence-in-depth with `SET LOCAL`, least-privilege roles — https://dev.to/amorizz/why-rust-postgres-rls-mp2
- Keyset pagination: sort-key uniqueness, index matching ORDER BY, microsecond-precision cursor trap — https://dev.to/ahmed_mahmoud360/why-my-pagination-got-slower-every-page-field-notes-on-offset-keyset-cursors-and-stable-sort-keys-jpi and https://www.atharva.page/post/rust/rust-web-pagination/
- mTLS / X.509 device identity, TLS 1.3, per-device credentials, least-privilege topics — https://blog.michaelsam94.com/mqtt-tls-authentication-iot/ and https://blog.4geeks.io/how-to-build-a-secure-and-scalable-iot-platform/
- Signed envelopes: Ed25519 (RFC 8032), sequence + timestamp + digest, replay defence, TLS protects the hop not the message — https://www.merobix.com/blog/mqtt-security-best-practices and https://www.merobix.com/blog/scada-telemetry-data-integrity
- NATS security: TLS client certificates, NKeys (Ed25519 JWTs), per-user subject permissions, JetStream encryption at rest — https://docs.nats.io/concepts/security
- NIST SP 800-82 Rev. 3 (OT security) threat framing, as cited in the Merobix pieces above.
