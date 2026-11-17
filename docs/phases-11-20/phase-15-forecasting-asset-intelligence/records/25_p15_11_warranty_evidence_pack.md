# P15.11 — Warranty evidence-pack generator

| Field | Value |
|---|---|
| Subtask | P15.11 — counters (P15.07) → timestamped, digest-chained claim-SUPPORT pack export; supports human warranty claims, NEVER asserts them; export carries the artifact-signing provenance record shape (P8.32 pattern) |
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11) |
| Status | WRITTEN-UNRUN: the generator and its tests are written, never compiled, never executed. No pack was exported, no claim was supported, no digest chain was verified in operation. |
| Sources | The P8.32 pattern: `crates/flexcore-research/src/artifact_signing.rs` (the versioned provenance record binding artifact ref, digest, digest algorithm, signer key REFERENCE, with provenance LINKED TO THE RECORD; the fail-closed verifier posture; the labelled test-grade FNV-1a 64 digest; the scheme-prefix key-reference rule; the P7.05 no-echo rule). MIRRORED BY CITATION — no flexcore-research dependency (the domain crate's only dependency is flexcore-contracts; the mirror is the `artifact_signing.rs` alignment posture). |
| Files touched | `crates/flexcore-domain/src/warranty_evidence.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). `crates/flexcore-research/` NOT touched. |

## 1. What landed (deliverable class: pure export generator)

| Piece | Where | Honest status |
|---|---|---|
| `EvidencePack` — schema id `flexcore-warranty-evidence-pack-v1`; caller-supplied pack id (validated, bounded) and generation timestamp (this module NEVER reads a clock); entries in chain order; chain-head digest; sealing provenance record; the claim-support and digest-grade statements carried INSIDE the pack | `warranty_evidence.rs` | Implemented; UNRUN |
| `EvidenceEntry` — `seq`, fixed-vocabulary `key`, rendered `value`, cumulative chain `digest_hex` | same | Implemented; UNRUN |
| The digest chain — seeded from (schema id, pack id, generation time), each step digesting the previous chain hex + the canonical entry `{seq}|{key}|{value}`; entries generated from the P15.07 ledger, P15.10 terms, and P15.10 headroom report, key-sorted for byte-identical determinism | same | Implemented; UNRUN |
| `fnv1a64_test` — the TEST-GRADE FNV-1a 64 digest (the repo's P4.28/P6.14/P7.14 pattern, mirrored from P8.32), pinned to the standard test vectors in tests; `EVIDENCE_DIGEST_GRADE_STATEMENT` records that it is NOT a security primitive and that a production SHA-256 (or stronger) pack digest is a DEPLOYMENT REQUIREMENT (pre-registered as `EVIDENCE_DIGEST_ALGORITHM_SHA256` so the deployment wiring needs no schema change) | same | Implemented; UNRUN |
| `EvidenceProvenanceRecord` — the P8.32 record shape, mirrored: `flexcore-warranty-evidence-provenance-v1`, artifact ref (pack id), chain digest, digest algorithm (closed vocabulary), scheme-prefixed signer key REFERENCE (key MATERIAL structurally impossible — there is no field a key could hide in), generation time, and the source refs (ledger + warranty identities) LINKED TO THE RECORD | same | Implemented; UNRUN |
| The refusal rule — a signer key reference without one of the `key-store:` / `env:` / `file:` schemes is refused as likely inline key material, and the offered value is NEVER echoed back (the P7.05 secret-reference rule, mirrored; pinned by test) | same | Implemented; UNRUN |
| `verify_chain` — re-digests the chain from the entries and compares (tamper-EVIDENCE for review, not tamper-PROOF for release); tests pin determinism, tamper detection (entry tamper + head tamper), and key-sorted entry order | same | Written complete; UNRUN |
| `CLAIM_SUPPORT_STATEMENT` — "This pack SUPPORTS a human warranty claim; it does NOT assert one …" — carried as an entry inside every pack, so the boundary travels with the export | same | Implemented; UNRUN |
| Provenance visibility — every counter, histogram, exposure, dwell, and SoH observation is exported WITH its merged provenance label; an UNKNOWN-provenance counter is visible as UNKNOWN and the ledger's `counters_creditable` verdict travels with it (never silently presented as known data) | same | Implemented; UNRUN |
| Tests: FNV-1a vectors, determinism, timestamp sensitivity, chain verification and tamper cases, entry ordering, in-pack statements, provenance-record linkage, inline-key refusal without echo, pack-id validation, unknown-provenance visibility | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. The claim boundary (the decisive line)

The pack is evidence-FOR-A-HUMAN. No automated claim, no warranty verdict,
no compensation action, and no dispatch action is produced or authorized by
its existence (ADR-010, conventions §6.3). The headroom values inside the
pack are rendered RECORDS; the recipient evaluates the evidence, FlexCore
records it.

## 3. What this module is NOT

1. **Not a signature.** No PKI, no release keys, no signature algorithm; the
   test-grade chain is tamper-EVIDENCE for review. Production signing is a
   DEPLOYMENT REQUIREMENT (the P8.32 decision-point posture, mirrored).
2. **Not a research-crate coupling.** The P8.32 shape is mirrored BY
   CITATION; `flexcore-research` is untouched and no dependency edge is
   added.
3. **Not a clock reader.** The generation timestamp is caller-supplied —
   the injected-clock discipline holds (no `SystemTime::now()` anywhere).

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The exact pack schema a warranty counterparty would accept (field names, formats) — UNVERIFIED; the pack is a platform-defined export shape | `EVIDENCE_PACK_SCHEMA_ID` |
| 2 | The production digest/signing wiring (SHA-256 verifier, key store) — DEPLOYMENT REQUIREMENT, not implemented | `EVIDENCE_DIGEST_GRADE_STATEMENT` |

## 5. Verification (permitted tools)

Outputs quoted verbatim in `26_phase_15_wave1_agent_b_close_out.md`. No
build, no test run (conventions §1.3).
