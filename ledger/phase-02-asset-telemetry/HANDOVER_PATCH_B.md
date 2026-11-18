# HANDOVER — Patch B: Cryptographic source authentication (HIGH gap 2 of 2)

- **Date:** 2026-11-18
- **Repo HEAD:** `74d56f4` (uncommitted work on top, including Patch A; nothing staged, committed, or pushed)
- **Status:** Implemented, awaiting human build/test. Nothing in this patch was built, run, or verified by execution — no `cargo` command of any kind was run, by the hard rules of this task. Krishna builds, runs, and patches.
- **Companion:** Patch A handover is `HANDOVER_PATCH_A.md` in the same directory.

## What this patch is

Source authentication was a recorded STUB: `authenticate_source` checked only self-reported health + an optional `trusted_source_id` string, and `SOURCE_AUTH_STATUS` said so. This patch replaces it with message-level Ed25519 signed envelopes (verified before decode, with key registry, replay/window checks, and a persisted per-source sequence watermark), keeps `trusted_source_id` as a defence-in-depth allowlist only, and hardens the NATS transport as step 2 of the same patch (TLS required in production profiles, per-user credentials, JetStream encryption at rest). No private key material is stored anywhere; test/dev keys are ephemeral per run/process. D6 (command-path authorisation) stays OPEN — this patch does not close it.

## Files changed

### Production code (Worker B1)

| File | Change |
|---|---|
| `crates/flexcore-ports/src/source_auth.rs` (NEW) | `SignedEnvelope`, `canonical_signing_bytes` (byte-exact layout documented), `SourceAuthError` + stable `quarantine_reason()` strings, `VerifiedEnvelope`, `SourceAuthenticator` trait (`authenticate(ctx, payload)`), `SourceAuthMode` (`Enforced`/`DevKey`), constants `ENVELOPE_DOMAIN = b"flexcore-telemetry-v1"`, `ENVELOPE_VERSION = 1`, `DEFAULT_AUTH_WINDOW_SECS = 300`. Module docs state the normative JSON wire format and the deliberate decision that the JSON codec lives in exactly two places (simulator producer + adapter consumer) because ports cannot depend on serde/base64. Zero `.unwrap()`; no infra identifiers (verified by grep: no `sqlx`/`nats`/`axum`/`postgres`/`tokio`). |
| `crates/flexcore-ports/src/lib.rs` | Export of the new module. |
| `crates/adapters/postgres/src/source_keys.rs` (NEW) | `PgSourceAuthenticator` (envelope parse, Ed25519 verify, SHA-256 digest, ±window check, watermark read; dev mode verifies `key_id == "dev"` against in-memory dev keys; enforced mode rejects `key_id == "dev"` as unknown; storage failures fail closed as `UnknownSourceOrKey`) + inherent `PgUnitOfWork` methods `lookup_source_key` / `read_source_sequence` / `advance_source_sequence` (advisory-xact-lock, `Advanced`/`Stale`, no UPDATE anywhere). Zero `.unwrap()`. |
| `crates/adapters/postgres/src/lib.rs` | Export of the new module. |
| `crates/adapters/postgres/migrations/0026_source_signing_keys_p2_07.up.sql` + `.down.sql` (NEW) | `source_signing_keys` (tenant_id, source_id, key_id, term_seq, public_key bytea, status CHECK IN ('ACTIVE','REVOKED','EXPIRED'), valid_from/until_unix_millis, enrolled_at_unix_millis, enrolled_by; PK (tenant_id, source_id, key_id, term_seq)) and `source_sequence_events` (tenant_id, source_id, sequence, recorded_at_unix_millis; PK (tenant_id, source_id, sequence)). Both copy migration 0018's pattern: RLS ENABLE + FORCE, tenant-isolation policy on `flexcore.tenant_id`, grants SELECT + INSERT to `flexcore_app` only, UPDATE/DELETE-refusing triggers. Public keys only, never private keys. **Note:** both tables carry a foreign key to `tenants` (not in the brief's table summary — the brief omitted it; B2's tests seed the tenant row accordingly). Down migration drops both tables. |
| `apps/worker/src/ingestion.rs` | `SOURCE_AUTH_STATUS` STUB text replaced with real Ed25519-envelope status; new verification Stage 1 in `process_frame` after `authenticate_source`, before `decode` (reuses the existing `frames_rejected_auth` counter; quarantines with `e.quarantine_reason()`); `process_batch` gains `verified: Option<&VerifiedEnvelope>` and advances the watermark in-transaction, returning a new `IngestionPipelineError::Replay` variant that `process_frame` turns into a replay quarantine without retry; `IngestionTask::new` gains final `authenticator: Arc<dyn SourceAuthenticator>`. |
| `apps/worker/src/config.rs` + `apps/worker/src/main.rs` | New env vars `FLEXCORE_SOURCE_AUTH_MODE` (default `"enforced"`; `"enforced"` \| `"dev-key"`; anything else fail-fast) and `FLEXCORE_SOURCE_AUTH_WINDOW_SECS` (default 300; u64; 0 fail-fast). Enforced mode: plaintext `nats://` URL fail-fast; connects via verified `ConnectOptions::new().require_tls(true)`. Dev-key mode: ephemeral keypair generated once in `run()` (never persisted), `tracing::warn!` that it must never be used in production, `SigningKey` into the simulator, verifying-key bytes into `PgSourceAuthenticator`; plaintext NATS allowed in dev-key mode (recorded in a comment). |
| `crates/flexcore-simulator/src/source.rs` | `signing_key: Option<SigningKey>`, `envelope_sequence: AtomicU64`, `with_signing_key`; `pull_frame` wraps frames in envelopes when a key is present (sequence starts at 1; `event_time_unix_millis` = the frame's `event_time_unix_ms` — all samples share one frame event time, so no scenario-clock fallback was needed; SHA-256 digest; `key_id = "dev"`; serde_json + base64 STANDARD + private hex helper, no hex crate). No key → behaviour unchanged. Production region (lines 1–621) has zero `.unwrap()` (all 37 hits are in the pre-existing `#[cfg(test)] mod tests`, line 622+). |
| `deploy/compose/nats-server.conf` (NEW) | TLS `cert_file`/`key_file`/`ca_file`/`timeout: 2` from mounted secrets; **no** `verify: true` (mTLS for edge agents is the separate P6.07 rustls channel, cross-referenced, not re-implemented; user/password is the client auth here); JetStream `key: ${JETSTREAM_ENCRYPTION_KEY}`; authorization users — worker: publish `flexcore.v1.>` + subscribe `flexcore.v1.>`/`_INBOX.>`; source: publish `flexcore.v1.telemetry.>` only, subscribe deny `>`; control-plane: publish `flexcore.v1.>` + subscribe `_INBOX.>` only. |
| `deploy/compose/.env.example` (NEW) | Generation commands only, no real values. |
| `deploy/compose/docker-compose.yml` | NATS service now runs `nats:2.15.2` with `command: ["-c", "/etc/nats/nats-server.conf"]` (the bare `--jetstream`-only invocation is gone); env-substituted `JETSTREAM_ENCRYPTION_KEY` + three `NATS_*_PASSWORD` secrets (fail-fast `${VAR:?message}`); mounts for the server config (`:ro`) and `./secrets/nats-certs` (`:ro`). Both `FLEXCORE_NATS_URL` values are `tls://nats:4222`; worker service sets `FLEXCORE_SOURCE_AUTH_MODE: dev-key`. |
| `deploy/compose/README.md` | Patch B section with the VERIFY-BUILD list. |
| `Cargo.toml` (workspace) | New pins: `ed25519-dalek = { version = "3", features = ["rand_core"] }`, `sha2 = "0.11"`, `base64 = "0.23"`, `rand = "0.9"`. Versions verified by the coordinator against crates.io/lib.rs on 2026-11-18: ed25519-dalek 3.0.0 (Jul 6, 2026), base64 0.23.1 (Aug 4, 2026), sha2 0.11.x. Regular deps: simulator → ed25519-dalek, sha2, base64; postgres adapter → ed25519-dalek, sha2, base64; worker → ed25519-dalek, rand. Dev-deps: postgres adapter → all four; worker → all four (sha2 + base64 added on the coordinator's follow-up — B2's pipeline test imports both). |
| `.agent/PROVENANCE.md` | License-audit section added: ed25519-dalek 3 / BSD-3-Clause; sha2 0.11 / MIT OR Apache-2.0; base64 0.23 / MIT AND Apache-2.0; rand 0.9 / MIT AND Apache-2.0 (all verified from docs.rs/lib.rs 2026-11-18). |
| `apps/worker/tests/fault_suite.rs`, `apps/worker/tests/security_phase2.rs` | `IngestionTask::new` call sites updated to pass a locally-defined `PassThroughAuthenticator` (permissive double: `was_enveloped: false`, payload = raw bytes). B1 also had to update all 12 direct `process_batch` calls in these files to pass `None` (the brief said update only the `new` sites, but the signature change forced it; no test logic changed). |

### Tests (Worker B2 — 36 tests, all fail-loud gated, keys ephemeral per run)

| File | Tests |
|---|---|
| `crates/adapters/postgres/tests/source_auth_envelope.rs` (12) | Constants; canonical-bytes byte-exact vectors incl. one frozen known-answer vector (commented never-change-without-a-version-bump); Ed25519 sign/verify round-trip over canonical bytes; every `SourceAuthError` → exact quarantine reason; correctly signed envelope authenticates; flip-one-payload-byte → `BadDigest`; flip-one-signature-byte → `BadSignature`; unregistered key → `BadSignature`; tenant-A envelope under tenant-B context → `UnknownSourceOrKey`; source-A key for source B → `UnknownSourceOrKey`; `key_id == "dev"` rejected in Enforced mode; malformed envelopes (bad JSON, JSON array, wrong version, short digest/signature hex, bad base64, missing signature). 4 pure (no DB), 8 DB-gated. |
| `crates/adapters/postgres/tests/source_auth_replay.rs` (7) | Duplicate rejected only after the watermark advances (asserts the first `authenticate` does NOT advance the watermark — `read_source_sequence` is `None` after it — then advances via `advance_source_sequence` and asserts the replay rejection, contract documented in a comment); same-sequence-different-bytes rejected + watermark unchanged; sequence below watermark rejected; gap accepted; window edges ±300 s accepted; ±301 s rejected (`OutsideTimeWindow`). |
| `crates/adapters/postgres/tests/source_auth_keys.rs` (6) | Registration readable with right fields; two active keys for one source both authenticate; expired key unknown + future expiry accepts; revocation by higher term rejects frames signed before it (delivered after); unregistered key/source unknown; lookup returns `None`, not an error, for an unregistered key. |
| `apps/worker/tests/source_auth_pipeline.rs` (8) | Signed frame stored as GOOD; unsigned frame rejected + quarantined; tampered frame quarantined with `BadDigest`; bad-signature frame quarantined; replayed frame quarantined as replay, never re-stored; **`trusted_source_id` without a signature does NOT authenticate** (the old-behaviour-is-gone negative test); simulator signed frame verifies against its dev key; simulator signed frame stores GOOD through the real pipeline. DB-gated, no NATS needed (outbox only). |
| `apps/worker/tests/source_auth_transport.rs` (3) | Plaintext connects to the documented broker but a TLS-required client fails closed; client without credentials cannot publish to any telemetry subject; wrong-subject JetStream publish refused. See deviation 5 — these assert what the documented broker actually does, with VERIFY-BUILD comments. |

### Coordinator merge fixes (mine, not the workers')

1. **`apps/worker/tests/ingestion.rs` reconciled.** Neither worker owned this file, but B1's `IngestionTask::new` / `process_batch` signature changes broke it: 2× `IngestionTask::new` (now pass a local `PassThroughAuthenticator`, copied from B1's fault_suite pattern), 23× `process_batch` (now pass `None`), and the stale STUB assertion at line 629 (now asserts the Ed25519 status text; test renamed to `source_auth_failure_rejects_the_source_and_records_the_envelope_status`, header comment updated).
2. **`apps/worker/tests/security_phase2.rs` STUB test reconciled.** B1 updated only the `new` call sites; the test `source_authentication_matches_the_recorded_phase2_stub_status` still asserted `SOURCE_AUTH_STATUS.contains("STUB")` and would have failed. Rewritten to `source_authentication_matches_the_recorded_envelope_status`, asserting the Ed25519 status text and its mode limitations; all 8 stale STUB references in the file updated (header, section header, doc comments, log comment). The `authenticate_source` accept/reject assertions are unchanged — that behaviour is intact.
3. **`crates/adapters/postgres/src/unit_of_work.rs`: `pub(crate)` on `tx` and `runtime`.** Pre-existing latent compile blocker, NOT introduced by either patch: the fields had no visibility modifier (private to `unit_of_work.rs`), yet 131 `self.tx` / `self.runtime` accesses in 15 sibling modules (`corrections.rs`, `envelope_store.rs`, `telemetry.rs`, `schedule_store.rs`, `source_keys.rs`, …) sit in `impl PgUnitOfWork` blocks that cannot see private fields (E0616). The d6eeddc review flagged only `schedule_store.rs`; the problem is crate-wide. The fix is two words of visibility, no behaviour change. Recorded here so it is not mistaken for patch work — and so the build round does not misattribute 131 errors to these patches. The broader compile state of the tree remains unverified.

## Design decisions (recorded once)

- **Message-level signing first, transport second.** TLS protects the hop, not the message; signing works over the transport that exists today, survives a compromised broker, needs no PKI first, and is testable without operations. Transport hardening is step 2 of this same patch, not dropped.
- **Enforced by default.** `FLEXCORE_SOURCE_AUTH_MODE` defaults to `enforced`; dev-key mode requires an explicit env value; unknown values fail fast; a zero window fails fast. The dev keypair is generated once per process start and never persisted.
- **Watermark is per (tenant, source), not per key.** A rotated key must not reset the sequence space — otherwise old sequences could replay under the new key. The watermark is an append-only event log (`source_sequence_events`, PK (tenant, source, sequence)) read as `MAX(sequence)`; advancing takes a per-source advisory transaction lock, re-checks MAX, and inserts — atomic with the batch store inside the ingestion transaction.
- **Verify reads, advance writes — split across two transactions.** `authenticate` (verify) runs in its own short UoW and only READS the watermark; the ADVANCE runs inside the batch UoW. The in-batch re-check closes the TOCTOU race between two concurrent frames with the same sequence: the second frame's advance returns `Stale`, its batch rolls back, and it is quarantined as replay.
- **Replayed frames are quarantined, not silently acknowledged.** The pipeline rule is "failed records are quarantined, never silently dropped". The storage-layer dedup (P2.09) still acknowledges exact-duplicate records inside a batch, but at the auth stage a replayed frame is a failed record with a replay reason.
- **Revoked/expired/unknown are indistinguishable** (`UnknownSourceOrKey` for all three) — the lookup returns `None` when the current term is not acceptable. No oracle for attackers.
- **Wire codec in exactly two places.** Ports cannot depend on serde/base64 and contracts cannot depend on base64, so the JSON codec is implemented in the simulator (producer) and the postgres adapter (consumer), both implementing the normative field list documented in `source_auth.rs` and both using the shared `canonical_signing_bytes` for the security-critical exactness. No third copy.
- **`trusted_source_id` stays as a defence-in-depth allowlist**, documented as NOT authentication. It is not removed.
- **Subject namespace is the repo's, not the brief's.** The brief said `telemetry.>`; the repo's real namespace is `flexcore.v1.telemetry.>` / `flexcore.v1.>` (topology.rs), so the NATS config uses the repo namespace.
- **No mTLS on the NATS client listener in this patch.** User/password authorization is the client auth; no client certificates are wired anywhere here. The P6.07 edge-agent rustls mTLS channel is the device-side counterpart — cross-referenced, not re-implemented.

## Per-acceptance-criterion status

| # | Criterion (from the brief) | Status |
|---|---|---|
| 1 | Unsigned frame in production mode rejected + quarantined, never stored; dev key accepted only when the dev-key flag is explicitly enabled | **Implemented, deferred to human build** — enforced mode rejects (parse fails or unknown key); dev-key mode is explicit-env only. Tested (pipeline tests 2, 6, 7–8). Not run. |
| 2 | Wrong key / one-byte tampered payload / wrong digest rejected + quarantined | **Implemented, deferred to human build** — tested (envelope tests: wrong key, flip payload byte → `BadDigest`, flip signature byte → `BadSignature`; pipeline tests 3–4). Not run. |
| 3 | Replayed frame (same sequence, same bytes) detected as duplicate, not re-stored; same sequence different bytes quarantined as conflict | **Implemented, deferred to human build** — replay → `ReplayOrStaleSequence` + quarantine; conflict → same rejection with the watermark unchanged. Tested (replay tests 1–2, pipeline test 5). Not run. |
| 4 | Sequence older than watermark rejected; gaps accepted and audited | **Implemented, deferred to human build** — tested (replay tests 3–4). "Audited": gaps are accepted through the normal path; there is no separate gap-audit record — the watermark-advance insert is the audit trail. Recorded as a wording note, not a gap. |
| 5 | Frame outside ±5-minute window rejected or quarantined (config decides; record the choice) | **Implemented, deferred to human build — CHOICE: rejected + quarantined** (`OutsideTimeWindow` → `1-receive` quarantine). Window is configuration (`FLEXCORE_SOURCE_AUTH_WINDOW_SECS`), not a constant. Tested at ±300/±301 s. Not run. |
| 6 | Revoked key's frames rejected from revocation onward, including frames signed before revocation but delivered after | **Implemented, deferred to human build** — tested (keys test 4). Not run. |
| 7 | Source A's frame replayed with source B's identity fails (signature bound to that source/tenant's registered key) | **Implemented, deferred to human build** — tested (envelope tests: wrong-tenant, wrong-source). Not run. |
| 8 | Rotation: two valid key_ids verify under both during overlap, only the new one after old expiry | **Implemented, deferred to human build** — tested (keys test 2: both active keys authenticate; keys test 3: expiry). Not run. |
| 9 | No private key material in repo, logs, fixtures, or committed config; test keys ephemeral per run | **Met by inspection** — grep-verified: no key material in the new files; dev keypair is runtime-generated; `.env.example` holds generation commands only. Not build-verified. |
| 10 | Transport: plaintext NATS connection refused, client without valid credential cannot publish to any telemetry subject | **PARTIALLY met — see deviation 5.** The server config + compose wiring landed (TLS listener, auth users, `tls://` URLs); the transport *tests* assert what the documented broker does today, with VERIFY-BUILD comments on what broker hardening would be needed before the refusal readings can be tested end-to-end. The refusal behaviour itself is config, not code, and was not executed. |
| 11 | This patch does NOT close D6 | **Met** — recorded here and in the ledger note. D6 stays OPEN. |

## Deviations from the brief (with reasons)

1. **`process_batch` call sites in fault_suite.rs / security_phase2.rs** — the brief said update only the `IngestionTask::new` sites, but the signature change broke the 12 existing direct `process_batch` calls; B1 updated them mechanically to pass `None`. No test logic changed. (The same reconciliation was needed in `ingestion.rs`, done by the coordinator — see merge fixes.)
2. **`apps/worker/tests/ingestion.rs` was not in either worker's file list** — reconciled by the coordinator (merge fix 1).
3. **Enforced + simulator cannot store together.** The simulator signs with `key_id == "dev"`, which Enforced mode rejects as `UnknownSourceOrKey`. So the Enforced end-to-end pipeline tests use hand-built envelopes under a DB-registered `key-1`, and the simulator tests use DevKey mode. Stated in the pipeline file's header.
4. **Simulator frames need a wide window + 2026-06 partition.** The simulator runs on the scenario clock (June 2026), far outside the default 300 s window at wall-clock time. Simulator pipeline tests use a 400-day window on the DevKey authenticator and create the `telemetry_y2026m06` partition. Documented in the file header.
5. **Transport tests assert the documented broker, not the brief's premise.** `deploy/compose/docker-compose.yml` (as B2 read it) ran `nats:2.15.2` with `--jetstream` only — no TLS, no auth — so "plaintext refused" and "no valid credentials" as authentication refusals did not hold against that broker. B1 has since landed the hardened compose config (TLS listener, auth users, `tls://` URLs), but B2's tests were written against the pre-hardening read and assert what that broker does (plaintext connects; a `tls://` client fails closed against the plaintext listener; JetStream publishes to telemetry/wrong subjects fail for "no matching stream"), each carrying a VERIFY-BUILD comment stating what broker hardening would be needed before the auth-refusal readings can be tested. **Build-round action:** re-read the transport tests against the now-hardened compose config; test 1's plaintext half likely needs inverting once the TLS-only broker is the test broker.
6. **ed25519-dalek 3 + rand 0.9** instead of the anticipated 2.x — the docs.rs `SigningKey::generate(&mut OsRng)` example pairs with rand 0.9; 2.x pairs with rand 0.8's rand_core 0.6, so 2.x would have mismatched the prescribed call. Versions verified by the coordinator (see "Files changed", Cargo.toml row).
7. **TLS 1.3 minimum not pinned server-side** — no verified NATS server config key exists (the `tls_min_version` key found belongs to Nomad, not NATS); enforced client-side via `require_tls(true)` + `tls://` URL only. VERIFY-BUILD.
8. **JetStream encryption key spelling unresolved** — the guide says `key:`, the config reference says `encryption_key`; B1 used the guide's `key:` form. `nats-server -t` must confirm which 2.15.2 accepts. VERIFY-BUILD.
9. **Window-0 rejected** — `FLEXCORE_SOURCE_AUTH_WINDOW_SECS=0` fails fast (a zero window would reject everything); the brief did not define invalid.
10. **`.expect()` used in two places** (`canonical_signing_bytes` length check, simulator serialization), matching pre-existing code style. The brief bans `.unwrap()` specifically; `expect()` with a message is not a bare unwrap, but it is a panic path — recorded here for the build round to judge. Zero `.unwrap()` in all new production code (verified by grep).
11. **Migration 0026 has a foreign key to `tenants`** — not in the brief's table summary. B2's tests seed the tenant row accordingly.
12. **Revocation lookup returns `None`, not a REVOKED row** — B1's `lookup_source_key` returns `None` when the current term is not acceptable (deliberate indistinguishability). B2 corrected its first draft to assert `Ok(None)` after revocation. The brief's requirement (frames rejected after revocation) is still asserted.

## VERIFY-BUILD items (human build/first-run must confirm)

- All four new pins at lockfile generation (no Cargo.lock exists): ed25519-dalek 3, sha2 0.11, base64 0.23, rand 0.9. Versions exist on crates.io (verified by the coordinator), but trait pairing (ed25519-dalek 3.x + rand 0.9) and feature resolution (`rand_core` feature; note: ed25519-dalek 3.0.0 removed the `std` feature, so B1's comment that "default features (std, alloc, zeroize) stay on" is stale for 3.x — the pin itself only adds `rand_core` to defaults, so this is a comment fix, not a build risk) are lockfile-time checks.
- `Signature::from_bytes` was NOT separately verified on docs.rs by B1.
- NATS TLS 1.3 minimum (no verified server config key) and JetStream key spelling (`key:` vs `encryption_key`) — confirm with `nats-server -t` against 2.15.2.
- First-run cert provisioning under `./secrets/nats-certs/` is required before the stack starts (the server fails fast without the certs, by design). No cert-path env vars exist yet; private-CA root / client-cert wiring not done.
- Transport tests vs the now-hardened compose config (deviation 5) — re-read and adjust test 1's plaintext half.
- Window-edge tolerance: the ±300 s accept tests read "now" immediately before signing; the authenticator reads its own clock milliseconds later, so an exactly-at-the-edge value is a few ms past the boundary when checked. The semantics is inclusive and B1 implements it inclusively, but a strict-millisecond implementation would flake there (noted in the replay file header).
- **Pre-existing latent compile blocker, fixed by the coordinator (merge fix 3):** `PgUnitOfWork.tx`/`runtime` visibility — now `pub(crate)`. The broader compile state of the tree remains unverified.

## Exact commands for Krishna's build/test round

```bash
cargo generate-lockfile
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
python3 tools/check_dependency_rule.py
```

Then the gated suite with the test database and NATS set:

```bash
FLEXCORE_TEST_DATABASE_URL=postgres://<test-user>@<host>:<port>/<db> \
FLEXCORE_TEST_NATS_URL=nats://<host>:<port> \
cargo test --workspace
```

Treat any failure as a patch round. The new tests fail loudly (panic naming `FLEXCORE_TEST_DATABASE_URL` / `FLEXCORE_TEST_NATS_URL`) when the variables are unset or empty — they never silently skip. The pure tests (canonical-bytes vectors, error→reason mapping, envelope constants) need no DB and are exempt from the gate.

**First-run note for the compose stack:** generate the NATS certs and the `JETSTREAM_ENCRYPTION_KEY` + `NATS_*_PASSWORD` values per `deploy/compose/.env.example` before `docker compose up` — the NATS server fails fast without its certs, by design. The worker service in compose runs with `FLEXCORE_SOURCE_AUTH_MODE: dev-key` (local dev); any production profile must set `enforced` (the default when the variable is absent) and use `tls://` NATS URLs.

Read-only checks already run by the coordinator on this patch (no build): `python3 tools/verify_docs.py` OK, `python3 tools/validate_agent.py` OK, `python3 tools/check_dependency_rule.py` OK ("dependency rule holds for 29 workspace members").

## Ledger ticks

**No new boxes were ticked by this patch.** The P2.10 ("Source-auth status is recorded honestly (stub vs real)") and P2.22 ("Unauthenticated/wrong-tenant source tests pass — implemented per the recorded P2.10 stub status") boxes were already ticked — the first honestly (the stub WAS honestly labelled), the second against the stub. This patch changes what those boxes mean: the status is now real, and the tests now pin the real mechanism. Dated notes to that effect were added under both sections in `ledger/phase-02-asset-telemetry/ledger_phase_2.md`; their status text is unchanged because nothing is build-verified yet. **D6 stays OPEN** — no decision was made or closed by this patch. D1–D5 likewise untouched.

## Blockers / decisions still needed

- **None blocking the build round.** The VERIFY-BUILD list above is the build-round checklist.
- **Open question for Krishna (not a blocker):** is user/password client auth sufficient for the NATS broker long-term, or should the P6.07-style mTLS client certificates be wired to the broker listener as well? This patch deliberately did not add `verify: true`; adding it is a deployment decision, not a code patch.
- **Transport test reconciliation** (deviation 5) is a build-round edit, not a blocker.

## Assumptions (labelled)

- The existing `flexcore.v1.>` subject namespace (from topology.rs) is the correct one for the NATS authorization rules (assumption from reading the tree, not from a running broker).
- `pg_advisory_xact_lock(hashtext(...))` serializes concurrent watermark advances per source correctly (standard Postgres behaviour; untested here).
- The `expect()` calls in `canonical_signing_bytes` and the simulator serialization are acceptable per pre-existing code style (recorded as deviation 10 for the build round to judge).
- The dev-key `key_id == "dev"` reservation is acceptable as a convention (it is rejected outright in Enforced mode).

## Risks

- **Unverified until built**: every line of both patches is written, not built. The handover's claims are inspection claims only.
- **Two-transaction verify/advance**: correct by design (in-batch re-check), but the race it guards has not been exercised — no concurrent same-sequence test exists yet. A build-round addition worth considering.
- **Transport tests vs hardened broker** (deviation 5): if the tests are run unchanged against the TLS-only broker, test 1's plaintext half will fail — not because the code is wrong, but because the test was written against the pre-hardening broker. Re-read before running.
- **Cert provisioning is manual**: the compose stack fails fast without certs under `./secrets/nats-certs/`; first run needs the operator to generate them.
