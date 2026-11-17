# P11 wave 2 Agent A — close-out record (P11.15–P11.19)

| Field | Value |
|---|---|
| Wave / agent | Phase 11 wave 2, Agent A |
| Subtasks in this batch | P11.15, P11.16, P11.17, P11.18, P11.19 (security / data integrity) |
| Status | WRITTEN-UNRUN throughout. No build, no test run, no install, no docker command, no scanner run was executed (conventions §1). The three permitted verification tools were run at the end; their verbatim outputs are quoted in §3. |

## 1. Deliverables in this batch (all WRITTEN, never run)

| Subtask | Record | Code / config delivered |
|---|---|---|
| P11.15 | `31_p11_15_audit_chain_upgrade.md` | `crates/flexcore-audit/src/chain.rs` (new: std-only SHA-256 + HMAC-SHA-256 keyed chain, per-tenant namespace, anchor export/verify with continuation seed, named errors, FIPS 180-4 / RFC 4231 vector tests), `crates/flexcore-audit/src/lib.rs` (additive module row + line), `crates/edge-agent/src/audit_proof.rs` (additive keyed chain v2 — FNV-1a chain byte-identical and still labeled TEST-GRADE per LR-203), `crates/adapters/postgres/src/audit.rs` (chain columns, keyed append under row lock, per-tenant verification read path), `store.rs` (`with_audit_chain_key`, `verify_tenant_audit_chain`, keyed sink paths), `unit_of_work.rs` (chain-key field, keyed unit-of-work sink), `migrations/0019_audit_chain_p11_15.up/down.sql` (expand-only). P11-E-1 registered OPEN with the DEFAULT implemented per conventions §4; key management recorded as the DEPLOYMENT REQUIREMENT gate, never faked. |
| P11.16 | `32_p11_16_secrets_management.md` | `deploy/compose/docker-compose.yml` (top-level `secrets:` file-per-host, `POSTGRES_PASSWORD_FILE`, entrypoint shims composing the DB URL from the mounted secret — no credential in the file), `deploy/compose/secrets/README.md` (new: procedure + 0600 + production posture), `deploy/compose/README.md` (credentials section), `.gitignore` (additive ignore), the SQLx pool credential-refresh design written at the `store.rs` pool seam. Vault DEFAULT (dynamic Postgres creds TTL 1h, NATS creds, PKI later) [S-74][S-75] registered as a RECOMMENDATION — nothing deployed. |
| P11.17 | `33_p11_17_secret_scanning.md` | `.gitleaks.toml` (new), `.pre-commit-config.yaml` (new, gitleaks pinned v8.30.1), `.github/workflows/ci.yml` (additive comment on the EXISTING P1.27 secret-scan step; behavior unchanged). NO scan was run; no findings exist; all marked UNRUN. |
| P11.18 | `34_p11_18_migration_policy_lock_audit.md` | `crates/adapters/postgres/MIGRATION_POLICY.md` (new — placed outside `migrations/` because sqlx `migrate!` owns that directory) + the static per-migration lock-risk audit table (0001–0019): two MEDIUM findings (0009 validated CHECK scan; 0014 unique-constraint scan) and one HIGH (0012's PK swap on populated `telemetry`), remediations scheduled as runbook items, nothing retroactive. |
| P11.19 | `35_p11_19_rls_pooling_reconciliation.md` | The systematic grep audit: ZERO session-level `SET`, ZERO pool hooks planting session state, `set_config(..., true)` everywhere (transaction-scoped), RLS policies fail closed on unset context. Verdict: no code fix required. The PgBouncer transaction-mode compatibility plan written (prepared-statement hazard flagged ASSUMPTION/UNVERIFIED; migration/maintenance paths excluded from pooling). |

Test files written complete and NEVER RUN (human gate): the Rust unit
tests inside `chain.rs` (FIPS 180-4 SHA-256 vectors; RFC 4231 HMAC
test cases 1, 2, 7; chain/namespace/anchor/continuation behavior), the
keyed-chain tests inside `audit_proof.rs` (RFC 4231 TC2 for the
duplicated crypto; tamper/kind/continuation gates), and the adapter
mapping tests inside `audit.rs`.

## 2. Source checks performed (documentation reads, not installs)

- RFC 4231 (HMAC-SHA-256 test vectors) fetched from rfc-editor.org on
  2026-11-14; test cases 1, 2, and 7 transcribed verbatim into the test
  modules (case 7 covers the >block-size key path).
- gitleaks release index checked 2026-11-14: latest tag `v8.30.1` —
  pinned in `.pre-commit-config.yaml`, UNPINNED-REVERIFY at the first
  human run.
- No crate/package was added anywhere: all P11.15 crypto is std-only by
  design (the recorded tradeoff in record 31 §3), so no manifest, no
  Cargo.lock, and no dependency-rule edge changed in this batch.

## 3. Permitted-tool outputs (verbatim)

`python tools/verify_docs.py`:

```text
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
```

`python tools/validate_agent.py`:

```text
skills: 5  roles: 5
OK: skills, roles, and README index are valid
```

`python -I tools/check_dependency_rule.py` (the character rendered as
`ù` is the same console mojibake of the tool's separator glyph that
Agent B recorded in wave 1; the ASCII payload reads OK for all 20
workspace members):

```text
OK ù dependency rule holds for 20 workspace members.
```

## 4. Deviations from the plan (all recorded, none silent)

1. **Hand-written SHA-256/HMAC instead of a crypto crate.** The
   conventions prefer std-only crypto where feasible and this wave's
   ownership rules forbid manifest edits and new internal edges; the
   crypto is anchored to published vectors and gated on an independent
   human review (record 31 §3.1). `ring` was considered (already in the
   edge tree via rustls) but rejected: it would have added an
   infrastructure dependency to the core `flexcore-audit` crate.
2. **Crypto duplicated across the two crates** (edge-agent /
   flexcore-audit) rather than shared — no new dependency edge was
   permitted; consolidation recorded as a follow-up (record 31 §3.2).
3. **The platform chain digests the correction LINKAGE id, not the
   correction reason** — the reason is not a persisted column of
   `audit_records` (P1.12 §4.7); a digest over an unpersisted field would
   be unverifiable from the stored rows (record 31 §3.5).
4. **`audit_chain_state` carries an UPDATE grant for `flexcore_app`** —
   the one deliberate grant change: it is the head-pointer bookkeeping
   table, NOT an evidence table; `audit_records` keeps its INSERT/SELECT
   grant set and its append-only trigger untouched (recorded in the
   migration header and record 31 §1).
5. **ci.yml edited additively (comment only)** — the brief anticipated
   either new CI files or the existing shape; the existing shape already
   had the gitleaks job, so the deliverable became the checked-in
   CONFIG plus a comment. The step's behavior is unchanged.
6. **The migration policy document lives at
   `crates/adapters/postgres/MIGRATION_POLICY.md`**, not inside
   `migrations/` — sqlx `migrate!` treats that directory's contents as
   versioned SQL files; a non-SQL file there would break the embedded
   runner.
7. **`FlexCoreStore::verify_tenant_audit_chain` uses `codes::REJECTED`
   for the un-configured-key refusal** (no `CONFIG` code exists in the
   adapter's error vocabulary; the message names the DEPLOYMENT
   REQUIREMENT explicitly).
8. **The chain-verification report types are re-exported through
   `store.rs`** (`pub use crate::audit::{...}`) rather than moving their
   module to `pub` in `lib.rs` — the adapter's `lib.rs` was not in this
   wave's ownership set, and the store method's public interface needs a
   nameable return type. The `audit` module's statements stay
   crate-internal (the P4.36 pattern unchanged).

## 5. Handoff notes to Agent B / the orchestrator

1. **Orchestrator — ledger**: tick P11.15–P11.19 with records 31–35 as
   the citations after verification (working agreement; this batch does
   not touch the ledger).
2. **P11-E-1 remains OPEN**: the human sign-off on the provider and its
   key-management plan closes it; record 31 registers the dependence on
   the DEFAULT. P11-E-2/P11-E-3 are unaffected.
3. **Agent B (P11.20–P11.22 close)**: the phase-close consistency audit
   should check (a) that record 31's P11-E-1 entry and the plan §4
   register agree, and (b) that no record in 31–35 claims a run, a
   deployment, a signature, or a security qualification — this batch
   wrote none.
4. **Composition roots**: wiring `with_audit_chain_key` into the control
   plane / worker composition is a future composition-root edit gated on
   the key-management sign-off; `audit_chain_configured()` exists for
   diagnostics. The Compose app services need NO env change for P11.16
   (the shim composes the URL), but the two secret files must exist
   before the first compose start (secrets/README procedure).
5. **First human build checklist (this batch)**: the RFC 4231/FIPS vector
   tests in `chain.rs` and `audit_proof.rs` are the crypto correctness
   gate; the keyed append path's SQL assumes migration 0019 (additive
   columns + `audit_chain_state`); the ci.yml comment's gitleaks-action
   config-discovery ASSUMPTION is confirmed at the first scan run.
6. **Verification-read path note**: `verify_tenant_audit_chain` reports
   UNCHAINED rows separately (never silently skipped) and returns a named
   `Failed { at_sequence, finding }` outcome rather than a bare boolean —
   an operator reads the finding text directly.
