# Audit Chain Upgrade Note — pre-upgrade evidence and verification (FIM V-9)

| Field | Value |
|---|---|
| Document class | Verification/migration NOTE for the Postgres `audit_records` chain upgrade — FIM item V-9 (`docs/releases/final-implementation-before-publication.md` §3, "V-9"; subtask FIM-V09.01) |
| Status | **WRITTEN-UNRUN (FIM wave 8).** Written 2026-11-21 by the FIM wave-8 agent under the owner's 2026-11-19 implementation authorization. No build, no test execution, no database was touched by the implementing agent; every code path cited below is exercised only at the human V-1 gated re-run. Nothing here is a certification, an accreditation, a security approval, or a permission for live grid control |
| Upgrade state | The adapter's keyed chain is **already wired**: `crates/adapters/postgres/src/audit.rs` composes the audit-crate HMAC-SHA-256 construction (`flexcore_audit::chain`) under migration 0019 whenever the deployment configured a chain key. The V-9 finding premise ("the postgres adapter's `audit_records` chain is still test-grade FNV-1a, `audit.rs:33`") is **stale against the code it cites**: the line it points at is the documented TEST-GRADE fallback COMMENT describing the un-keyed state, not a code path. No FNV-1a digest was ever written into an `audit_records` chain column by any code path in this repository |
| Remaining work this note closes | The migration/verification record: how pre-upgrade evidence verifies across the upgrade boundary — old evidence keeps its recorded digest algorithm (no silent re-hash), and a verifier tells the generations apart by the recorded algorithm field alone |
| Governing records | LR-203 ("FNV-1a-64 audit chain and keyed-checksum signature are tamper-EVIDENCE-grade, not cryptographic proof" — `ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.29, from `23_incident_response.md` §12 R-P9.23-8); the P11.15 record `docs/phases-11-20/phase-11-operational-robustness/records/31_p11_15_audit_chain_upgrade.md`; the P11-E-1 register (`records/54_p11_e1_audit_crypto_register.md`) |
| What this note is NOT | Not a claim that any chain has ever been keyed, computed, or verified against a live database; not a key-management procedure; not a substitute for the P11.15 DEPLOYMENT REQUIREMENT gate (P11-E-1 stays OPEN) |

## 1. What the upgrade actually was (and was not)

The P11.15 upgrade replaced the TEST-GRADE FNV-1a 64 chain (LR-203) with a
keyed HMAC-SHA-256 linear chain as the production construction, and the
postgres adapter composes it — it never redefined it:

| Fact | Evidence (current file:line) |
|---|---|
| The chain construction is the audit crate's, consumed not redefined | `crates/flexcore-audit/src/chain.rs:83` — `pub const AUDIT_CHAIN_ALGORITHM: &str = "hmac-sha256-chain-v1"`; the adapter imports `chain_step`, `genesis_head`, `hex_encode_digest`, `parse_digest_hex`, `ChainKey`, `TenantChainEntry`, `AUDIT_CHAIN_ALGORITHM` (`crates/adapters/postgres/src/audit.rs:60-63`) |
| The keyed append composes the HMAC-SHA-256 chain inside the caller's transaction | `crates/adapters/postgres/src/audit.rs:243` (`append_audit_record_with_chain`): head state read under the `audit_chain_state` row lock, digest = `chain_step` over (previous head \|\| canonical record bytes) at `audit.rs:276`, record + chain columns INSERTed together (`audit.rs:282`), head advanced |
| The keyed path is selected ONLY when the deployment configured a key | `crates/adapters/postgres/src/store.rs:368-373` (store-level sink) and `crates/adapters/postgres/src/unit_of_work.rs:1129-1133` (unit-of-work sink): `Some(key)` → `append_audit_record_with_chain` under `key.chain_key_for_namespace(record.tenant_id)`; `None` → `insert_audit_record(..., None)` — the honest un-keyed state |
| The key is consumed, never created | `FlexCoreStore::with_audit_chain_key` (`crates/adapters/postgres/src/store.rs:139`); key generation, distribution, rotation, and storage remain the human-gated DEPLOYMENT REQUIREMENT (P11.15; P11-E-1 OPEN) |
| The FNV-1a chain lives on the EDGE, outside the database | `crates/edge-agent/src/audit_proof.rs:86` — `pub const AUDIT_CHAIN_ALGORITHM: &str = "fnv1a64-chain"`, labeled TEST-GRADE per LR-203. No postgres migration, adapter, or store code computes or stores an FNV digest; the `fnv1a64:` strings in `crates/adapters/postgres/tests/` are the P4.31 model-content-hash labels of feasibility records, a different mechanism entirely |
| No schema change belongs to this note | Migration 0019 (`crates/adapters/postgres/migrations/0019_audit_chain_p11_15.up.sql`) already carries the expand-only chain columns — `chain_sequence`, `chain_prev_head`, `chain_digest`, `chain_algorithm`, all NULL-able (0019 up.sql lines 43-47) — plus the `audit_chain_state` head table. **FIM wave 8 added no migration** (the highest existing migration number remains 0026); there is nothing to expand: the upgrade is a code-side composition over a schema that has existed since 0019 |

## 2. The recorded algorithm field is the discriminator

Every chain-bearing datum in this repository records the algorithm that
produced it. A verifier NEVER guesses: it reads the recorded field and
refuses what it cannot recompute.

| Datum | Where the algorithm is recorded |
|---|---|
| A keyed `audit_records` row | The row's own `chain_algorithm` column — bound as `AUDIT_CHAIN_ALGORITHM` ("hmac-sha256-chain-v1") in the same INSERT as the digest (`crates/adapters/postgres/src/audit.rs:149`); a construction change is a detectable format change, never a silent one (0019 up.sql lines 34-36) |
| The per-namespace head state | `audit_chain_state.chain_algorithm` (`0019_audit_chain_p11_15.up.sql` lines 65-72), advanced with the head by `advance_chain_state` (`crates/adapters/postgres/src/audit.rs`) |
| An exported anchor | `TenantChainAnchor.algorithm` (`crates/flexcore-audit/src/chain.rs`), set to `AUDIT_CHAIN_ALGORITHM` at export |
| An edge FNV proof | The edge chain's own recorded label `fnv1a64-chain` (`crates/edge-agent/src/audit_proof.rs:86`), carried by the edge-side proof/anchor surface it was always carried by — untouched, byte-identical, still labeled TEST-GRADE (LR-203) |

The cross-generation refusal is mechanical: `verify_against_anchor`
compares the anchor's recorded algorithm against the verifier's and fails
with a named `ChainError::AnchorAlgorithmMismatch`
(`crates/flexcore-audit/src/chain.rs:756`, raised at `chain.rs:633`)
instead of re-hashing under a different construction. That refusal IS the
mechanism that keeps the generations apart — a "hmac-sha256-chain-v1"
verifier can never silently absorb FNV evidence, and the edge verifier
never silently absorbs HMAC evidence.

## 3. The evidence generations and how each verifies

| Generation | Where it lives | Recorded algorithm | How it verifies | Re-hashed? |
|---|---|---|---|---|
| Rows written BEFORE migration 0019 | `audit_records` with NULL chain columns (the columns did not exist) | none recorded | Outside the keyed chain by construction: the verification read selects `chain_sequence IS NOT NULL` (`audit.rs:349`) and the NULL-population is COUNTED, never skipped — reported as `unchained_records` (`audit.rs:328`, `audit.rs:358`). They verify, if at all, under whatever mechanism was in force when they were written | **No** |
| Rows written after 0019 while NO chain key was configured | `audit_records` with NULL chain columns ("the honest un-keyed state" — `audit.rs` module header) | NULL | Same as above: outside the keyed chain, counted as `unchained_records` | **No** |
| Rows appended while a chain key WAS configured | `audit_records` chain columns + `audit_chain_state` head | `hmac-sha256-chain-v1` | `FlexCoreStore::verify_tenant_audit_chain` (`store.rs`) → `verify_tenant_chain_rows` (`audit.rs:502`): recompute every digest from genesis under the tenant-namespace key; every mismatch is a named `ChainError` finding | **No** |
| Edge-side audit chains (any era) | The edge agent's own audit proof/buffer surface — never postgres rows | `fnv1a64-chain` | The edge's own verifier, under its own recorded label, exactly as before the upgrade; stays labeled TEST-GRADE per LR-203 and is never a release signature | **No** |

The invariant this note pins: **no migration, no backfill, and no code
path re-hashes, re-labels, or folds pre-upgrade evidence into the keyed
chain.** The keyed chain starts at its own genesis; pre-upgrade rows stay
outside it and are reported. Old chains keep their recorded digest
algorithm — the FNV chain is not "upgraded in place", because rewriting
evidence would be indistinguishable from tampering with it.

## 4. Verification procedure for a tenant with a mixed history

WRITTEN-UNRUN, human-gated (the V-1 re-run and then operator sign-off;
key provisioning is the P11.15 DEPLOYMENT REQUIREMENT — P11-E-1 OPEN):

1. Provision the chain key through the deployment's reviewed key-management
   process and hand it to `FlexCoreStore::with_audit_chain_key`. Until that
   human gate closes, the keyed chain is NOT running and new rows keep the
   NULL (un-keyed) state — that is the recorded honest default, not a fault.
2. Run `FlexCoreStore::verify_tenant_audit_chain(ctx)` and read the report:
   - `outcome == Verified` with `verified_records = N`: all N keyed rows
     recomputed cleanly from genesis and the head matches.
   - `unchained_records > 0`: those rows predate the keyed chain (or were
     written un-keyed). They are OUTSIDE the keyed chain — report them with
     the count, and verify them, if required, under the mechanism in force
     when they were written (for edge-covered evidence, the edge FNV proof
     under its `fnv1a64-chain` label). NEVER fold them into the HMAC chain
     retroactively and never backfill their chain columns.
   - `outcome == Failed { at_sequence, finding }`: a named security finding
     for an operator (keyed tamper-EVIDENCE), never a retried read.
3. Anchor exports: verify windows with `verify_against_anchor`; an anchor
   whose recorded algorithm is not `hmac-sha256-chain-v1` is refused with
   `AnchorAlgorithmMismatch` — that is the correct handling of pre-upgrade
   evidence, not an error to work around.
4. Restated from the P11.15 record: this is keyed tamper-EVIDENCE under
   reviewed key management — never tamper-PROOF, never a certification,
   never a release signature.

## 5. Recorded limits

| Limit | Consequence |
|---|---|
| Everything cited here is WRITTEN-UNRUN | The keyed append, the verification read path, and the anchor mechanics have never been executed against a live database; the human V-1 gated re-run (`docs/releases/final-implementation-before-publication.md` §3) is the proving gate. This note records code structure, not observed behavior |
| Key management is unsolved (P11-E-1 OPEN) | A keyed chain does not exist until an operator provisions reviewed keys; every row written meanwhile carries NULL chain columns and lands in the `unchained_records` count |
| NULL chain columns are ambiguous between "pre-0019" and "post-0019 un-keyed" | The database cannot distinguish the two generations after the fact; both are reported identically as `unchained_records`. The distinction lives in deployment records (when 0019 was applied; when a key was configured), not in the rows |
| The edge FNV chain stays TEST-GRADE (LR-203) | It is tamper-EVIDENCE-grade, not cryptographic proof, and is never a release signature; its upgrade (keyed chain version 2 on the edge) is the P11.15 edge record's concern, outside this note's scope |
| Supervisory orchestration evidence only | Nothing here touches dispatch authority, control states, or the simulation-only ceiling (the entire 0.x line) |
