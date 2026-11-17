# P11.15 — Production-grade audit chain upgrade

| Field | Value |
|---|---|
| Subtask | P11.15 — the keyed HMAC-SHA-256 audit chain (P11-E-1 DEFAULT), per-tenant chain namespace, periodic anchor-export design, INSERT-only DB role on audit tables, per-tenant chain verification [S-88] |
| Wave / agent | Phase 11 wave 2, Agent A |
| Status | WRITTEN-UNRUN throughout: the chain is implemented code that has never been compiled, keyed, deployed, or exercised (the first human build/test is the gate; conventions §1). The keyed chain is a DEPLOYMENT REQUIREMENT (conventions §6.6) — key management is a HUMAN-GATED deployment concern and this batch does not close it, fake it, or claim it. |
| Sources | [S-88] (immutable/hash-chained audit logs as the industry baseline — no FlexCore-specific security claim attaches to it); FIPS 180-4 (SHA-256) and RFC 2104 (HMAC) / RFC 4231 (test vectors 1, 2, 7 — fetched from rfc-editor.org 2026-11-14 and transcribed into the unit tests verbatim) |
| E-decision | **P11-E-1 registered OPEN, never closed by this batch.** This implementation is the plan §4 DEFAULT (option a: keyed HMAC-SHA-256 linear chain with per-tenant chain namespace and periodic anchor export), implemented because the subtask says so (conventions §4: "Agents never close an E-decision. They implement the recommended default only when a subtask explicitly says so, and record the dependence."). What closes P11-E-1: the human owner's sign-off on the provider and its key-management plan. |
| Files touched | `crates/flexcore-audit/src/chain.rs` (new), `crates/flexcore-audit/src/lib.rs` (additive module line + table row), `crates/edge-agent/src/audit_proof.rs` (additive keyed section + module-header honesty update + 4 new error variants; the FNV-1a test-grade chain is byte-identical), `crates/adapters/postgres/src/audit.rs` (chain columns, append-with-chain, verification read path), `crates/adapters/postgres/src/store.rs` (chain-key configuration, verification method, sink path), `crates/adapters/postgres/src/unit_of_work.rs` (chain-key field + keyed sink path), `crates/adapters/postgres/migrations/0019_audit_chain_p11_15.up.sql` + `.down.sql` (new) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Platform-side keyed chain: std-only SHA-256 (FIPS 180-4), HMAC-SHA-256 (RFC 2104), `ChainKey` (deployment-supplied material, best-effort volatile zeroization, no Debug/Clone), per-tenant namespace derivation (`chain_key_for_namespace`), genesis/step over the canonical `AuditRecord` encoding, per-tenant walk verification, `TenantChainAnchor` export/verify with the continuation seed (anchors chain across export windows), named `ChainError` set | `crates/flexcore-audit/src/chain.rs` | Implemented code; anchored to FIPS 180-4 / RFC 4231 test vectors in the unit tests, verified only at the first human `cargo test` |
| Edge-side keyed chain: chain VERSION 2 (`hmac-sha256-chain-v1`), `EdgeChainKey`, keyed step folding the COMPLETE record (including the kind — the FNV chain omits it; that difference is recorded and the FNV chain stays byte-identical per LR-203), keyed anchor + continuation seed + keyed export bundle + keyed verdict text with the DEPLOYMENT REQUIREMENT wording | `crates/edge-agent/src/audit_proof.rs` | Additive; the existing test-grade FNV-1a 64 chain is untouched and remains labeled TEST-GRADE — never a release signature |
| Keyed append path: head state read under `FOR UPDATE`, digest over (previous head \|\| canonical record bytes) under the TENANT-NAMESPACE key, row INSERTed with its chain columns, head state advanced — all inside the caller's transaction (atomic with the evidence row and the use case's state change) | `crates/adapters/postgres/src/audit.rs`, `store.rs`, `unit_of_work.rs` | Written; no key configured = chain columns NULL (the honest un-keyed state) |
| Migration 0019: 4 nullable columns on `audit_records` (chain_sequence, chain_prev_head, chain_digest, chain_algorithm) + `audit_chain_state` head-pointer table; expand-only (no constraint scan, no index, no destructive ALTER — the P11.18 policy); INSERT-only grant set on audit_records UNCHANGED (table-level INSERT/SELECT from 0008 ride along; the UPDATE grant on `audit_chain_state` is the recorded bookkeeping exception, with the trigger + grants on the evidence table untouched) | `migrations/0019_audit_chain_p11_15.up/down.sql` | Written; never applied |
| Per-tenant chain verification: store-level `verify_tenant_audit_chain` (transaction-scoped tenant context, explicit filter on top of RLS), row decode back into the record type, named `TenantChainVerification` report with the UNCHAINED-row count reported (never silently skipped) | `audit.rs`, `store.rs` | Written; running it against a live chain is a human-gated operation |

## 2. Key management — the DEPLOYMENT REQUIREMENT gate (recorded, not faked)

- Key generation, distribution, rotation, storage, and the operator
  procedure are HUMAN-GATED deployment concerns. This batch implements the
  consumption seam only: `FlexCoreStore::with_audit_chain_key(key)` (and
  `audit_chain_configured()` for composition-root diagnostics). Nothing
  generates, loads, persists, or rotates a key.
- Until an operator provisions reviewed keys and signs off, the production
  chain is NOT running; rows written without a key carry NULL chain
  columns and are verifiable only through the TEST-GRADE FNV-1a chain
  (LR-203), which is never claimed as a release signature anywhere.
- The periodic anchor-export CADENCE is likewise a deployment decision —
  the anchor types and export/verify functions exist; the scheduler is
  deliberately absent.

## 3. ASSUMPTION / UNRUN / UNSIGNED register

1. **ASSUMPTION — hand-written crypto.** The program prefers std-only
   crypto primitives where feasible, and this wave's file-ownership rules
   forbid new internal dependency edges and manifest edits, so SHA-256 and
   HMAC are hand-written (std-only, no new crate, no manifest change).
   Correctness is anchored to published vectors (FIPS 180-4 "abc"/empty;
   RFC 4231 test cases 1, 2, and 7 — the 131-byte block-size-crossing key
   case), asserted in BOTH copies' test modules, and verified only at the
   first human test run. An independent review of this module's crypto is
   part of the DEPLOYMENT REQUIREMENT gate — no production keying without
   it.
2. **ASSUMPTION — duplication.** The edge-agent cannot depend on
   `flexcore-audit` without a new internal dependency edge (outside this
   wave's ownership), so the crypto is duplicated in
   `audit_proof.rs` with the rationale recorded at the section header.
   Consolidation is a recorded follow-up for a wave that owns both
   manifests and the dependency-rule checker's edge table.
3. **UNRUN** — every function above, the migration, and every test.
4. **UNSIGNED** — no key material exists, no anchor has been exported, no
   verification verdict has been recorded.
5. **RECORDED LIMITATION** — the platform chain digests the correction
   LINKAGE id only: the correction reason is not a persisted column of
   `audit_records` (P1.12 §4.7), and a digest over an unpersisted field
   could never be recomputed from the stored rows. The reason stays
   enforced at the record-construction layer and travels in the record
   type.
6. **Tamper-EVIDENCE, not tamper-PROOF** — a keyed chain resists
   recomputation by an adversary WITHOUT the key; key compromise (or a
   superuser who can rewrite rows AND the anchor store) defeats detection.
   No certification, conformance, or security-qualification claim is made
   or implied.

## 4. Handoff notes

1. **Orchestrator**: tick P11.15 only with this record's citation; P11-E-1
   stays OPEN (the human sign-off on provider + key-management plan closes
   it).
2. **First human build**: the vector tests (FIPS 180-4, RFC 4231 TC1/TC2/
   TC7) are the correctness gate for the hand-written crypto; any vector
   mismatch is a build-blocking finding, not a tolerance issue.
3. **Operator**: wiring `with_audit_chain_key` into the composition roots
   is a composition-root edit (outside this wave's ownership) and must
   happen only after the key-management sign-off; until then the chain
   columns stay NULL by design.
