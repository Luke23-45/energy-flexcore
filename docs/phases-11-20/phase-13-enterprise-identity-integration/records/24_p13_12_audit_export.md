# P13.12 — Audit export service (signed export packs, SIEM-friendly JSONL, scoped integrity statement)

| Field | Value |
|---|---|
| Subtask | P13.12 — signed export packs over `flexcore-audit` records, SIEM-friendly JSONL format, date-range plus tenant scoping, integrity statement inheriting the Phase 11 audit-chain upgrade |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN — no export has ever been produced, no pack has ever been signed, no SIEM has ever consumed a line. Every evidence cell is EMPTY/UNRUN. |
| Sources | `crates/flexcore-audit/src/` (`record.rs` — the §10.1 field set; `chain.rs` — the P11.15 keyed chain, consumed never redefined); `crates/flexcore-storage/src/audit_read.rs` (the SELECT-only read port this builder composes — its two-method shape is pinned by its source-text test and is NOT modified); `crates/adapters/postgres/migrations/0008_audit_records_p1_12/0019_audit_chain_p11_15` (append-only storage + chain columns); Phase 11 record `../../phase-11-operational-robustness/records/31_p11_15_audit_chain_upgrade.md` (the chain upgrade this service INHERITS — citation contract, plan §5) |
| Files touched | `apps/control-plane/src/audit_export.rs` (new), `apps/control-plane/src/lib.rs` (module declaration, marked P13.12) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Pack builder: `AuditExportBuilder::build_pack` — walks the `AuditEventReader` port in bounded pages (`EXPORT_PAGE_SIZE = 200`, keyset cursor via `after_audit_id`, the port's recorded convention) under the caller's `TenantContext` over an INCLUSIVE date range, capped at `MAX_RECORDS_PER_PACK = 100_000` | `apps/control-plane/src/audit_export.rs` | Written-unrun; verified only by in-module tests (complete, UNRUN) |
| SIEM-friendly JSONL: one JSON object per line, snake_case §10.1 field set end to end (audit_id, tenant_id, actor, action, object type/id, before/after references, decision outcome + reason reference, evidence reference, the distinct P0.09 event/ingest times, correlation id, correction linkage), deterministic field order so two exports of the same rows byte-match | same | Written-unrun |
| Tenant scoping BY CONSTRUCTION: the pack walks the port under the caller's `TenantContext`; the port filters by tenant on top of RLS (0008) and platform-level rows (`tenant_id IS NULL`) are unreachable through it — the builder applies no widening filter and exposes no cross-tenant mode | same | Written-unrun; a test pins cross-tenant isolation through the fake |
| Export-scope digest chain: genesis = zero digest; each JSONL line's digest = `HMAC-SHA256(export_key, head_hex + "." + line)`; the manifest head commits to the pack's completeness and order (the crypto is the P13.09 signing module's vector-anchored implementation, reused — no second hand-written copy) | same | Written-unrun |
| Manifest + signature: pack id, tenant scope, inclusive range, record count, `complete` flag (a cap-hit pack is delivered AS incomplete — never silently truncated), generated-at, schema version (`flexcore.audit.export.v1`), head digest, source-chain statement; `HMAC-SHA256(export_key, canonical_manifest_bytes)` | same | Written-unrun; receiver-side `verify_pack_signature` / `verify_pack_body_chain` pair with it |
| Integrity statement INHERITING the Phase 11 chain (never redefining it): the manifest carries `SOURCE_CHAIN_STATEMENT` verbatim | same | The statement text pins the gap (§2) |

## 2. The integrity statement — inheritance, and the gap stated verbatim

The Phase 11 audit-chain upgrade (record 31) is the record-level tamper
evidence: a keyed HMAC-SHA-256 linear chain (`hmac-sha256-chain-v1`,
per-tenant namespace, anchor export, INSERT-only storage role). This
service consumes that authority and adds a SEPARATE, coarser guarantee:
the pack's own completeness/order integrity (that the pack is exactly what
the exporter emitted).

The honest gap, stated in every manifest verbatim
(`SOURCE_CHAIN_STATEMENT`): **"record-level tamper evidence is the
phase-11 keyed audit chain's domain (hmac-sha256-chain-v1, per-tenant
namespace, anchor export); this pack does NOT verify that chain — the read
port does not serve the chain columns — and the pack's digest chain
attests only the pack's own completeness and order."**

Why the gap exists: the `AuditEventReader` port (P4.36) predates the 0019
chain columns and serves exactly two SELECT methods whose shape its
source-text test pins (no export/dump method may be added). The 0019 chain
columns (`chain_sequence`, `chain_prev_head`, `chain_digest`,
`chain_algorithm`) are therefore invisible to this builder. A pack CANNOT
recompute the P11.15 chain without them, so a pack never claims chain
verification. This is a stated limitation, not a missing feature to
quietly imply.

## 3. Export discipline (the append-only trail, exported)

| Rule | Application |
|---|---|
| Export is read-only, always | The builder touches the read port only; no write port, no mutation path, no correction-in-place — the trail's append-only rule (P1.10 three layers + the P4.36 SELECT-only read contract) is untouched |
| Export is a full-fidelity range capture | The action/actor/object filters are deliberately NOT exposed: a selective slice is a QUERY (the `/api/v1/audit-events` resource serves it); an export is the range. No selective-export API is added |
| Export authorization | Producing an export is an operator action per the P9.10 §10.A design (step-up-approved where the deployment requires it, recorded in `audit_records`) — the composition step (§5) owns the route/gate; no export endpoint exists in this batch |
| No unbounded query | The port's listing convention (bounded pages, keyset cursor) is honored; the pack cap is the second bound |
| Export key management | The export signing key is DEPLOYMENT-supplied material under the P11.16 secrets posture (record 32 — consumed, never re-decided); this module never generates, loads, persists, or rotates one (the P11.15 §2 key-management honesty, applied) |

## 4. ASSUMPTION / UNRUN / EMPTY register

1. **UNRUN** — the builder, the JSONL rendering, the digest chain, the
   signature, the verifiers, every test. No pack exists anywhere:
   `record_count` of every pack that would exist is hypothetical; the
   actual count is EMPTY.
2. **ASSUMPTION** — the JSONL field rendering (snake_case, canonical
   order) is this batch's registered format; a SIEM parser's expectations
   are not tested against any real SIEM (no SIEM exists in this program).
3. **ASSUMPTION** — the manifest's canonical bytes are
   `serde_json::to_vec` of the manifest struct (field order = declaration
   order); a receiver must use the same rendering — pinned by the
   round-trip tests, recorded here for the human reviewer.
4. **RECORDED GAP** — §2's chain-column gap; the follow-up (§5) is the
   ONLY path to pack-level chain verification.
5. **EMPTY** — every pack id, every head digest, every signature, every
   SIEM consumption record.

## 5. Wiring (recorded composition steps)

1. Extend the Postgres audit read path to serve the 0019 chain columns
   (a read-side change with a recorded decision record — the port shape's
   two-method test is pinned in flexcore-storage, so the extension is a
   reviewed change to THAT crate, not this one); then the manifest gains a
   chain-coverage statement (first/last chain sequence, anchor reference)
   and packs can carry chain-verification materials. Until then §2's
   statement stands verbatim.
2. The export ROUTE (operator-triggered, tenant-scoped, step-up-gated per
   the deployment) and its delivery mechanism (download vs push — the
   residency constraint of record `26_p13_14_jurisdiction_registry.md`
   applies to where a pack may travel) are composition decisions this
   batch does not make; no endpoint exists.
3. The export key composition (`with`-builder on the store, like the
   P11.15 `with_audit_chain_key` precedent) lands with the human build's
   key-management sign-off.

## 6. Handoff notes

1. **Orchestrator**: tick P13.12 with this record's citation; implemented
   (written-unrun), never tested or verified.
2. **Phase 19 (SOC 2 track)**: this service produces the audit-export
   EVIDENCE packs the attestation track collects (plan §5) — the records
   are the format contract; the evidence itself comes from real, human-run
   exports, none of which exist.
3. **Wave 2 (P13.19)**: negative tests (tampered pack, wrong key,
   cross-tenant isolation) can extend this module's test surface; the
   fakes are in-module.
