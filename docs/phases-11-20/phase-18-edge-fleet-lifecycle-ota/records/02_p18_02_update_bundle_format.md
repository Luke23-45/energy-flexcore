# Record 02 — P18.02: Update-bundle format specification (`flexcore-edge-update-manifest-v2`)

Date: 2026-11-17. Subtask: P18.02 (`ledger_phase_18.md` §P18.02). Status:
**WRITTEN-UNRUN** — the format types are real additive code
(`crates/edge-agent/src/update_bundle.rs`, module `update_bundle` in
`crates/edge-agent/src/lib.rs`); execution is the first human build/test.
**No bundle has ever been produced, signed, staged, or applied.**

| Field | Value |
|---|---|
| Format id | `flexcore-edge-update-manifest-v2` (constant `UPDATE_BUNDLE_FORMAT_V2`) |
| Extends | The P9.34 `UpdateManifest` v1 (`crates/edge-agent/src/update_verification.rs`) — COMPOSED (the v1 struct is a field of the v2 struct), never forked or reinterpreted |
| Sources | [S-162] (IETF RFC 9019, SUIT) for the applicability and sequence-number anti-rollback concepts; ADR-1801 (record 01) for the architecture |
| Honesty | SUIT-shaped, NOT a SUIT CBOR encoding; no standards-conformance claim; the anti-rollback comparison semantics are UNVERIFIED against the spec texts |

## 1. The field set (v2 = v1 fields + the v2 additions)

| Field | Origin | Rule |
|---|---|---|
| `base.package_id` | v1 | Non-empty ≤ 256 chars |
| `base.version` | v1 | Numeric `M.m.p` core (the P9.34 `compare_versions`) |
| `base.bundle_digest_hex` | v1 | Even-length lowercase hex, 8..=512 digits |
| `base.digest_algorithm` | v1 | The closed `UPDATE_DIGEST_ALGORITHMS` vocabulary (`sha-256` / `fnv1a64-test`) |
| `base.signer_key_reference` | v1 | Scheme-prefixed (`key-store:` / `env:` / `file:`); never key material; the value is not echoed on refusal |
| `base.minimum_version_floor` | v1 | Numeric `M.m.p` core; enforced at install (unchanged) |
| `base.approval_reference` | v1 | Non-empty — an unapproved update is refused (fail-closed, unchanged) |
| `applicability.vendor` | v2 (NEW) | Non-empty ≤ 128 chars; MANDATORY exact match |
| `applicability.device_class` | v2 (NEW) | Non-empty ≤ 128 chars; MANDATORY exact match |
| `applicability.device_ids` | v2 (NEW) | ≤ 4096 ids, each non-empty ≤ 256 chars; EMPTY = all devices of the vendor+class, NON-EMPTY = only listed ids (closed semantics, no third reading) |
| `anti_rollback_sequence` | v2 (NEW) | `u64` ≥ 1; must be STRICTLY ahead of the device's last-seen value |
| `payloads[]` | v2 (NEW) | 1..=64 entries; per entry: unique non-empty name ≤ 128 chars, offset, size ≥ 1 (per-entry cap 256 MiB), lowercase-hex digest; no overlapping byte ranges; total ≤ 1 GiB |
| `expires_at_unix_millis` | v2 (NEW) | MANDATORY positive instant — expiry is required in v2 (the freshness residual until a full TUF-style timestamp role is operated; ADR-1801 §4) |

## 2. Sequence-number anti-rollback (beyond the v1 floor)

The v1 minimum-version floor stays exactly as shipped (it bounds which
VERSIONS install). The v2 sequence number bounds which MANIFESTS may be
presented at all: a manifest whose `anti_rollback_sequence` is not
strictly ahead of the device's monotonic counter
(`AntiRollbackCounter`, an append-only CRC-32-framed register in the
same framing discipline as the P9.34 update register) is REFUSED
(`UPDATE_SEQUENCE_NOT_AHEAD`). The counter advances ONLY on a full
verification pass, durably before the verdict is returned. Consequence
(recorded, fail-safe direction): a manifest whose install later fails
CANNOT be re-presented — a retry requires re-issued metadata with a
higher sequence. Rollback is unaffected: it re-activates a
previously-activated version through the P9.34 register and the recorded
`RollbackOverride` rule — never by re-presenting a manifest.

**UNVERIFIED label:** the strictly-greater-than comparison and the
advance-at-verify point are the implemented, strictly-safer reading of
the TUF/SUIT concepts [S-161][S-162]; the exact standards semantics are
checked against the spec texts at the implementation-time register
(§4) — never asserted from this record.

## 3. Size caps (code maxima)

| Cap | Value | Rule |
|---|---|---|
| `MAX_UPDATE_PAYLOAD_ENTRY_BYTES` | 256 MiB | Per payload entry |
| `MAX_UPDATE_PAYLOAD_TOTAL_BYTES` | 1 GiB | Per bundle (total declared payload size; also the whole-bundle check in the extended gate) |
| `MAX_UPDATE_PAYLOAD_ENTRIES` | 64 | Per manifest |
| `MAX_UPDATE_MANIFEST_BYTES` | 64 KiB | The v1 constant, unchanged |

Deployments may configure TIGHTER caps; the code constants are ceilings
the validation refuses to exceed (never loosened).

## 4. UNVERIFIED register (implementation-time checks)

| # | Item | Source | Status |
|---|---|---|---|
| U-1 | SUIT applicability-metadata concept shape (component-identifier applicability) | [S-162] | The FlexCore field set is SHAPED BY the concept; the SUIT encoding itself is not implemented and no conformance claim is made |
| U-2 | Sequence-number comparison semantics (strictly-greater vs greater-or-equal) and the counter-advance point | [S-161][S-162] | Implemented as the strictly-safer reading; UNVERIFIED against the spec texts |
| U-3 | Payload-digest binding shape (SUIT digests-over-content concept) | [S-162] | FlexCore fields (offset/size/digest); UNVERIFIED as a conceptual alignment only |

## 5. Honest status

The format types, validation, applicability/sequence/expiry checks, and
the `AntiRollbackCounter` register exist as WRITTEN-UNRUN code with
in-module tests pinning the rules above. Nothing was produced, signed,
staged, or applied; the ledger tick reads "implemented, awaiting human
build/test".
