# P11-E-1 — Audit-chain crypto provider (decision REGISTER record)

| Field | Value |
|---|---|
| Subtask | P11.22 (decision records + phase close) — this record is the formal register entry for P11-E-1, which until now appeared only in the phase plan §4 |
| Wave / agent | Phase 11 wave 2, Agent B |
| Status | **DECISION REGISTERED OPEN — agents never close E-decisions** (program conventions §4). The DEFAULT below is a RECOMMENDATION. Nothing in this record claims a cryptographic property of any shipped code. |
| Sources | [S-88] (immutable/hash-chained audit logs — the industry baseline the gap analysis cites); LR-203 (the recorded test-grade FNV-1a 64 finding); the phase plan §4 P11-E-1 entry (the wording source, unchanged here); the test-grade chain surface `crates/edge-agent/src/audit_proof.rs` (label `fnv1a64-chain`, explicitly non-cryptographic) |
| Files touched | none (a register record) |

## 1. The decision point (P11-E-1)

**Question:** which construction replaces the test-grade FNV-1a 64 chain
(`crates/edge-agent/src/audit_proof.rs`, label `fnv1a64-chain`, LR-203)
in production?

**Options (the plan §4 wording, unchanged):**

| Option | Shape | Recorded trade-off (no security or performance claims) |
|---|---|---|
| (a) Keyed HMAC-SHA-256 linear chain | per-tenant chain namespace, periodic anchor export | the smallest upgrade from the existing linear chain shape; no new service dependency; per-tenant verification natural |
| (b) SHA-256 Merkle tree | periodically anchored roots | a different verification shape (inclusion proofs) with anchoring infrastructure to operate |
| (c) External timestamping / notary service | a third party anchors the chain | a new external dependency per deployment |

## 2. DEFAULT (RECOMMENDATION)

**(a) keyed HMAC-SHA-256 linear chain** with per-tenant chain namespace
and periodic anchor export — the plan's recorded DEFAULT, unchanged here.

**What closes it** (the human owner's action, never an agent's):

1. the human owner's sign-off on the provider and its key-management plan
   (where the HMAC keys live, how they rotate, who holds the anchors);
2. production use remains a **DEPLOYMENT REQUIREMENT** gate (program
   conventions §6.6): the crypto upgrade is a requirement ON DEPLOYMENT,
   and no code-only claim ever satisfies it.

## 3. Register state at phase close

| ID | Question | Status | DEFAULT | Closer |
|---|---|---|---|---|
| P11-E-1 | audit-chain crypto provider | **OPEN** (registered here) | (a) HMAC-SHA-256 linear chain — RECOMMENDATION | human owner (sign-off + key-management plan) |
| P11-E-2 | long-term metrics store | **OPEN** (registered in `records/22_p11_10_metrics_store.md`) | (a) VictoriaMetrics single-node — RECOMMENDATION | operator (footprint/retention acceptance + live provisioning check, UNRUN) |
| P11-E-3 | alert routing channel | **OPEN** (registered in `records/22_p11_10_metrics_store.md`) | (a) generic UNBRANDED webhook — RECOMMENDATION (no PagerDuty/Slack conformance claims anywhere) | operator (the runbook: who is paged, where the receiver lives) |

No E-decision of this phase is closed by an agent. If the P11.15
implementation record (wave 2, Agent A) cites P11-E-1, it records the
DEPENDENCE on this DEFAULT — it does not close the decision.

## 4. Honesty boundary

- The FNV-1a 64 chain stays labeled test-grade (LR-203); it is never
  claimed as tamper evidence or a release signature.
- [S-88] is cited for the industry baseline context only; it makes no
  claim about FlexCore. No construction in this record is asserted to be
  implemented, deployed, or verified — see the P11.15 record for what was
  WRITTEN (that record is Agent A's).
- HMAC-SHA-256 is named as the DEFAULT construction family; the concrete
  key lengths, rotation, and anchoring cadence are the closer's
  key-management plan, not registered here.
