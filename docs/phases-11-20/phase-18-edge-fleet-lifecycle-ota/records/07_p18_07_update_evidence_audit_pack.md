# Record 07 — P18.07: Update evidence + audit pack (per-device update records, digest-chained, exportable)

Date: 2026-11-17. Subtask: P18.07 (`ledger_phase_18.md` §P18.07). Status:
**IMPLEMENTED-UNRUN, ADDITIVE** — real code in `crates/flexcore-audit`
(module `update_evidence`, additive entry in
`crates/flexcore-audit/src/lib.rs`); **no update evidence record exists
anywhere — no device, no fleet, no update.**

| Field | Value |
|---|---|
| Files | `crates/flexcore-audit/src/update_evidence.rs` (NEW); `crates/flexcore-audit/src/lib.rs` (additive entry) |
| Consumed, never redefined | The P11.15 keyed HMAC-SHA-256 chain (`crates/flexcore-audit/src/chain.rs`) and the §10.1 `AuditRecord` / `AuditSink` port (P1.10); LR-203 (test-grade FNV-1a chains stay labeled test-grade — untouched here) |
| Ledger line | "Per-device update records into `flexcore-audit` (attempt / verify / install / result / rollback), digest-chained, exportable" |

## 1. The record shapes

`UpdateEvidenceRecord` — one per-device, per-attempt evidence record:
attempt id (the chain key; one chain per update attempt), device id (the
P18.12 registry's identity, consumed by citation), target version, the
closed kind, the closed outcome, a MANDATORY detail (a refusal's stable
code, a rollback's reason and override reference, an install's slot
reference — an evidence record without a reason is not evidence), and
the P0.09 `EventTime`/`IngestTime` pair (the ordering rule mirrored).

The closed kind set is EXACTLY the ledger's five
(`UpdateEvidenceKind::from_code` refuses anything else):

| Code | Kind | §10.1 audit action |
|---|---|---|
| 1 | `Attempt` | `update.attempt` |
| 2 | `Verify` | `update.verify` |
| 3 | `Install` | `update.install` |
| 4 | `Result` | `update.result` |
| 5 | `Rollback` | `update.rollback` |

Outcome vocabulary: `SUCCESS` / `FAILED` / `REFUSED` — never a bare
boolean; the reason rides the detail.

## 2. Digest-chained — the per-attempt chain

- Each attempt's records form a linear HMAC-SHA-256 chain
  (`update_evidence_step` over previous head || canonical bytes, with
  the per-attempt sequence INSIDE the canonical bytes — the head
  commits to content, order, and count).
- Domain-separated from the P11.15 audit chain
  (`UPDATE_EVIDENCE_DOMAIN = "flexcore-update-evidence-v1"`) using the
  SAME std-only primitives (`chain::hmac_sha256`) — consumed, not
  duplicated; a digest from one domain can never be replayed into the
  other.
- The chain is per-attempt so one attempt's evidence can be handed over
  (exported) without the whole tenant chain — a second-level evidence
  commit, never a replacement for the P11.15 chain.
- Append-only by construction: `UpdateEvidenceChain::append` refuses a
  different attempt (`UPDATE_EVIDENCE_ATTEMPT_MISMATCH`), refuses
  sequence disorder, and never edits.
- Empty-chain export is refused (`UPDATE_EVIDENCE_EMPTY_CHAIN_EXPORT`)
  — an empty bundle commits to nothing (the P11.15 empty-export rule,
  mirrored).

## 3. Riding the P11.15 production chain (consumed, never redefined)

`to_audit_record` maps every evidence record onto the §10.1
`AuditRecord` (fixed mapping: action `update.*`, object `("device",
device_id)`, after_ref `device/{id}/version/{v}`, evidence
`update-evidence/attempt/{attempt}/seq/{n}`; actor/tenant/ids supplied
by the composition — never invented here). The mapped record flows
through the EXISTING `AuditSink` port and is chained by the EXISTING
P11.15 functions — an in-module test exercises exactly that consumption
(`chain::chain_step` accepting the mapped record). The P11.15 chain's
construction, namespace discipline, and DEPLOYMENT REQUIREMENT carry
over verbatim: keyed tamper-EVIDENCE under reviewed key management,
never tamper-PROOF, never a release signature.

## 4. Exportable

`UpdateEvidenceExport` (per attempt): chain version, algorithm
(`hmac-sha256-update-evidence-v1`), attempt id, device id, record count,
head digest, exported-at instant (injected clock). `audit_text()`
carries the honesty wording verbatim ("keyed tamper-EVIDENCE under
reviewed key management — the P11.15 DEPLOYMENT REQUIREMENT carries
over; never a release signature"). The transport (operator pack,
offline retention) is the composition's concern.

## 5. Key management honesty

The chain key is CONSUMED from the deployment (the P11.15 key-material
discipline): generation, distribution, rotation, storage, and operator
sign-off are human-gated DEPLOYMENT concerns. This module never
generates, loads, or stores key material; until an operator provisions
reviewed keys, the evidence chain is NOT running.

## 6. In-module tests (WRITTEN-UNRUN)

The five-kind lifecycle chain; honest-replay head reproduction and the
flipped-outcome tamper detection at its sequence; attempt-mixing
refusal; namespace independence (two namespace keys, different heads);
fail-closed record validation (empty attempt/device/detail, P0.09
ordering); the export wording pins; the §10.1 mapping riding
`chain::chain_step`; the statement pins (no device/no fleet/no update;
never tamper-PROOF; never a release signature).

## 7. Honest status

Implemented-unrun; no evidence record exists anywhere; no key material;
no fleet; the test-grade FNV path (LR-203) untouched; the ledger tick
reads "implemented, awaiting human build/test".
