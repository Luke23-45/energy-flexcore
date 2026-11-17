# Record 03 — P18.03: Metadata repository implementation plan (roles, keys, rotation, expiry)

Date: 2026-11-17. Subtask: P18.03 (`ledger_phase_18.md` §P18.03). Status:
**PLAN RECORD ONLY — no repository is stood up, no key exists, nothing
runs.** This record fixes the role/key plan the P18-E-1 DEFAULT (b)
requires to be "designed-in" (ADR-1801, record 01), so that migrating to
a full TUF-style repository is an additive operational step, never a
format break.

| Field | Value |
|---|---|
| Sources | [S-161] (TUF specification v1.0.37) — role semantics summarized from the gap analysis and phase context; **UNVERIFIED against the spec text until the §6 register is cleared**; P10.21 (`ledger/phase-10-v1-release-hardening/21_signed_artifacts.md`) — the key-ceremony discipline |
| Scope | DESIGN. No service exists, no metadata is published, no key material exists anywhere |

## 1. The role plan (TUF-shaped; [S-161])

| Role | Key posture | Signs | FlexCore shape under the DEFAULT (b) |
|---|---|---|---|
| root | **OFFLINE** — the root private key never touches an online system; it exists only in the key ceremony | The root metadata (the trust-anchor set: which keys are trusted for which role) | The edge's trusted signer-reference set (the `UpdateGate` constructor's `trusted_signer_references`) is the deployment's instantiation of this trust anchor; role-qualified from day one (`key-store:flexcore-release-root`, `...-targets`, ...) |
| targets | Online signing key under the deployment's key store | Target metadata: the per-device-class artifact digests and the DELEGATIONS | The v2 manifest's `signer_key_reference` names the targets-role key; per-device-class delegations are expressed as one targets key per device class (§2) |
| snapshot | Online signing key | Consistency: binds WHICH targets version is current | Under the DEFAULT (b) carried by the platform-side evidence chain (the update metadata registry row the orchestrator reads); designed-in, not yet operated |
| timestamp | Online signing key, short expiry | Freshness: proves the metadata set is recent | Under the DEFAULT (b) carried by the v2 manifest's MANDATORY `expires_at_unix_millis` field + the device's injected clock; designed-in, not yet operated |

**The honest residual (ADR-1801 §4, restated):** until the full
repository is operated, freshness and multi-target consistency are
carried by the manifest expiry and the platform-side records — weaker
than a live timestamp/snapshot pair against a malicious-replay adversary
with repository write access; recorded, not quantified, and the reason
the full repository is a fleet-threshold-gated commitment, not a
forever-optional nice-to-have.

## 2. Per-device-class target delegations

Delegation boundary (designed-in; the operation is the threshold-gated
step):

- One targets-role key per device class (the P18.12 registry's
  `device_class` vocabulary — consumed by citation, record 25).
- A device-class key compromise compromises ONLY that class's updates;
  the root rotates the class key without touching other classes — the
  key-compromise recovery path that option (c) of P18-E-1 lacked.
- The applicability `vendor`+`device_class` match on the device side
  mirrors the delegation: a manifest signed by the wrong class key
  cannot be applicable to the wrong class (defense in depth — the
  signature check and the applicability check are independent).

## 3. N+1 root rotation ([S-161]; UNVERIFIED against the spec text)

The rotation pattern (designed-in; a rehearsal is an UNRUN human
exercise):

1. The new root metadata is signed by BOTH the old and the new root
   keys (N+1: N old signers plus 1 new); the device accepts the new
   trust anchor only when it verifies against the OLD anchor it already
   holds (never a blind trust swap).
2. The following root metadata is signed by the NEW key only; the old
   key is removed. Devices that missed the overlap window re-seed from
   the deployment's anchor distribution, never by skipping verification.
3. Rotation is a HUMAN CEREMONY (P10.21 interplay): key generation,
   cross-signing, and publication are gated, signed, and recorded steps
   — no agent performs any of them.

## 4. Expiry discipline

- The v2 manifest's `expires_at_unix_millis` is MANDATORY; an expired
  manifest is refused (`UPDATE_MANIFEST_EXPIRED`) against the device's
  INJECTED clock (the P6.31 clock-quality discipline applies: expiry
  checks are trusted only on a synchronized clock — the edge refuses
  rather than guessing when the clock is unsynchronized).
- Repository-role metadata (when the full repository is operated)
  carries per-role expiry windows recorded as deployment configuration;
  the publishing cadence is an operator commitment, never code.

## 5. Key ceremonies — every one a human gate (P10.21 interplay)

| Ceremony | Status |
|---|---|
| Root key generation + offline storage | HUMAN GATE — NOT STARTED; no key material exists (binding statement, P10.21 §5 carried forward) |
| Per-device-class targets keys | HUMAN GATE — NOT STARTED |
| Snapshot/timestamp keys (threshold-gated) | HUMAN GATE — NOT STARTED |
| Root rotation rehearsal | HUMAN GATE — NOT STARTED (UNRUN) |
| Test-grade signing | The FNV-1a-64 family stays LOUDLY LABELED test-grade (`fnv1a64-test`); never a release signature; never used in any ceremony |

**No production signing key material exists anywhere in this
repository's history** — the P10.21 binding statement carried forward
unchanged.

## 6. UNVERIFIED register

| # | Item | Source | Status |
|---|---|---|---|
| U-1 | The four role semantics and their division of trust | [S-161] | UNVERIFIED against the spec text until the repository implementation reads the spec |
| U-2 | The N+1 root rotation procedure (cross-signing/overlap details) | [S-161] | UNVERIFIED against the spec text; the implemented-in-design steps are the safer reading |
| U-3 | Snapshot/timestamp consistency semantics | [S-161] | UNVERIFIED; carried by platform-side records under the DEFAULT |

## 7. Honest status

No repository, no metadata, no keys, no rotation, no expiry publisher.
The plan is the designed-in structure the P18-E-1 DEFAULT requires; the
operational step is the human owner's fleet-threshold decision (P18-E-1
stays OPEN, closer: human owner).
