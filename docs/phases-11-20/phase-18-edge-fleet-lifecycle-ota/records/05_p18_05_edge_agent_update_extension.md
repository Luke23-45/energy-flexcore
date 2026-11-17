# Record 05 — P18.05: Edge-agent update module extension (the real verifier behind the P9.34 seam)

Date: 2026-11-17. Subtask: P18.05 (`ledger_phase_18.md` §P18.05). Status:
**IMPLEMENTED-UNRUN, ADDITIVE** — real code behind the seam; nothing is
wired into a build here; execution is the first human build/test.

| Field | Value |
|---|---|
| Files | `crates/edge-agent/src/update_session.rs` (NEW); `crates/edge-agent/src/update_bundle.rs` (NEW, record 02); `crates/edge-agent/src/update_verification.rs` (ONE additive visibility edit); `crates/edge-agent/src/lib.rs` (two additive module entries); `crates/edge-agent/Cargo.toml` (one additive dependency); `crates/flexcore-audit/src/chain.rs` (one additive visibility edit) |
| Seam | The P9.34 `UpdateGate` / `UpdateVerifier` / `NoUpdateVerifier` surface — preserved |

## 1. The fail-closed default is preserved — the plain statement

**With nothing configured, the agent refuses every update, exactly as
shipped.** The default verifier of the P9.34 gate remains
`NoUpdateVerifier` (algorithm `none`; refuses EVERYTHING with the
recorded reason). Nothing in this phase changes the default composition,
the gate's refusal order, the verify-before-any-write structure, the
append-only register, or the never-silent-downgrade rule. The new
[`Sha256UpdateVerifier`] is an INJECTED OPTION: the composition root
decides between the fail-closed default and the production verifier —
the code contains no path that silently upgrades the default.

## 2. The real verifier — consumed, not duplicated

`Sha256UpdateVerifier` (in `update_session.rs`) implements the P9.34
`UpdateVerifier` port with algorithm name `sha-256` (the closed
vocabulary's production name). The digest is **CONSUMED from the P11.15
std-only SHA-256 implementation** (`flexcore-audit/src/chain.rs`,
`sha256` / `hex_encode_digest`) — the workspace's existing crypto
approach, reused per program conventions §6.6 and the loop-ban rule
against new crypto-crate additions. Two minimal additive edits make the
consumption possible; both are visibility-only, no behavior change:

- `flexcore-audit/src/chain.rs`: `pub(crate) fn sha256` → `pub fn
  sha256`; `pub(crate) fn hmac_sha256` → `pub fn hmac_sha256` (the
  FIPS 180-4 / RFC 2104 implementations and their published-vector tests
  are untouched);
- `crates/edge-agent/src/update_verification.rs`: `fn crc32` →
  `pub(crate) fn crc32` (the P18.02 counter framing reuses it).

The dependency edge `flexcore-edge-agent → flexcore-audit` is a
composition→core edge (the normal direction; checker-legal). The
SHA-256 implementation remains WRITTEN-UNRUN, anchored to its published
FIPS 180-4 vectors, verified only at the first human `cargo test`. An
additional in-module test pins the FIPS 180-4 `"abc"` and empty-string
vectors through the verifier itself. **Wiring the production verifier
under reviewed key management remains a DEPLOYMENT REQUIREMENT**
(P10.21 §5 carries over unchanged).

## 3. The extended gate — composition, order, audit

`ExtendedUpdateGate` composes the P18.02 v2 checks AROUND the untouched
P9.34 gate. The recorded refusal order:

1. manifest present → `UPDATE_UNSIGNED_BUNDLE`;
2. bundle within the total-size cap → `UPDATE_BUNDLE_TOO_LARGE`;
3. v2 structural validation (base P9.34 validation inside) →
   `UPDATE_MANIFEST_INVALID`;
4. applicability to THIS device (identity from the edge's own
   configuration, mandatory non-empty — a device that cannot state its
   identity refuses everything) → `UPDATE_NOT_APPLICABLE`;
5. not expired (injected clock) → `UPDATE_MANIFEST_EXPIRED`;
6. sequence strictly ahead of the anti-rollback counter →
   `UPDATE_SEQUENCE_NOT_AHEAD` (fail-closed on an unavailable counter);
7. **the P9.34 gate, verbatim** — its six-step refusal order, its audit
   on every verdict (`EDGE_UPDATE_VERIFIED` /
   `EDGE_UPDATE_VERIFICATION_REFUSED`), and its no-write structure;
   refusals surface as `UPDATE_BASE_VERIFICATION_REFUSED` carrying the
   stable P9.34 code;
8. every payload digest matches (the SAME verifier instance) →
   `UPDATE_PAYLOAD_DIGEST_MISMATCH` / `UPDATE_PAYLOAD_VERIFIER_REFUSED`
   / `UPDATE_PAYLOAD_BOUNDS_OUTSIDE`;
9. the anti-rollback counter advances DURABLY — an I/O failure refuses
   the verdict (a pass without the recorded sequence is never issued).

Extended-level verdicts are audited (`EDGE_FLEET_UPDATE_VERIFIED` /
`EDGE_FLEET_UPDATE_REFUSED` / `EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED`):
one verification, two honest audit layers.

## 4. Sequence checks and the update-progress state machine

- Sequence checks: `ExtendedUpdateManifest::sequence_refuses` +
  `AntiRollbackCounter` (record 02 §2; the semantics statement and its
  UNVERIFIED label are pinned in code).
- The update-progress state machine: `UpdateSessionStateMachine` +
  `UpdateProgressState` + the closed failure-state vocabulary (record
  04) — manifest validation and sequence checks feed the
  `Verifying → VerifiedPendingInstall` / `Verifying → UpdateFailed`
  edges.

## 5. In-module tests (WRITTEN-UNRUN)

The FIPS-vector verifier test; the happy-path and auto-rollback state
machine walks; the explicit-failure-state pins; the
commands-in-flight blocking test (P18.15 consumed by citation); the
cancel-legality matrix; the extended-gate full pass with durable counter
advance; the same-manifest-replay refusal; the metadata-failure battery
(unsigned / not-applicable / expired / zero-sequence / payload-digest /
payload-bounds); **the fail-closed-preserved test** (the same
composition with `NoUpdateVerifier` refuses with
`UPDATE_VERIFIER_REFUSED`, the counter does not advance, and BOTH audit
layers fired); the mandatory-device-identity test; the allowlist test;
the honesty-statement pins.

## 6. Honest status

Implemented-unrun; no update produced, signed, staged, or applied; no
fleet; no production key; the ledger tick reads "implemented, awaiting
human build/test".
