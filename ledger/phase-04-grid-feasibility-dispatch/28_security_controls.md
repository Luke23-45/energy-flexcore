# Decision record — P4.28 Security controls for dispatch (§17.3–§17.4)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, or executed** — no `cargo build/check/test/clippy/fmt`, no test
runs, no secret scan beyond the static checks named in §5. Loop bans hold
(AGENTS.md working agreement; the loop's build/test/install bans).

- **Report trace:** §17.3, §17.4, §17.5 (invariant 8), §16.2, §14.3 field 15, §19.2 layer 7.
- **Closes:** the P4.28 re-verification open item recorded in
  `14_16_lifecycle_completion.md` §4 ("no production signing identity, no
  mTLS, no replay-protected transport — P4.28's scope") and the audit-wiring
  open item recorded in `23_25_rejection_suites_b.md` (the cross-tenant
  denial's `AuditSink` append). Both are addressed here at the
  simulation-layer seam level (§3, §4 below); production wiring remains
  P4.30's, recorded — not silently assumed.

## 1. What already existed (verified by reading, not assumed)

| Ledger point | Existing surface | Where |
|---|---|---|
| Signed commands | `SimulationKeyring` (dev-labelled; `sim-sig:<key_id>:<command_id>` traceability reference); the adapter signs at issue and records the reference as the ISSUED journal evidence (§14.3 field 15); the envelope carries `signature_reference` + `transport_identity` (`sim://dispatch-adapter/...`) | P4.16, `crates/flexcore-application/src/dispatch_simulator.rs` |
| Missing-signature refusal | `check_issue_refusals` → `IssueRefusal::Unsigned` → `DispatchAdapterError::UnsignedCommandRefused`; the unsigned/unsequenced cases are additionally unrepresentable through the P1.06 constructor | P4.16 (same file); P4.14 `CommandRecordFields` |
| Sequence | `sequence ≥ 1` required at creation; the pure `out_of_sequence_rejection` verdict (incoming ≤ last recorded → reject, first-of-channel never out of sequence) | P4.14, `command_record.rs` |
| Replay/duplicates | exact-replay dedup, conflicting-duplicate rejection, redelivery idempotency (P4.24); forged-signature + replayed-sequence adversarial cases (P4.26) | `tests/rejection_duplicate.rs`, `tests/rejection_combined.rs` |
| Expiry | `IssueRefusal::Expired` at issue; the P4.15 sweep; expiry at every transition attempt | P4.15/P4.16 |
| Tenant isolation | the tenant-scoped `CommandGateStore::load_command` seam (`None` for a foreign command — existence never leaks, P0.16 rule 5); cross-tenant issue/approve denial tests | P4.16 `tests/dispatch_simulator.rs`, P4.23 `tests/rejection_unauthorized.rs` |
| Secrets | no secret material anywhere: the keyring carries key identifiers only; test keys are clearly-labelled non-production simulation ids | P4.16 |

## 2. The genuine gaps P4.28 closed (and how)

1. **The signature did not bind the payload.** `sim-sig:<key>:<command>` is
   traceability only: a tampered payload under the same command ID verified
   identically, so the ledger's "tampered payload → signature MUST fail"
   case was not provable. **Closed** with the payload-bound signature layer
   (D-P4.28-1 below).
2. **No verification primitive existed.** The adapter signs and records;
   nothing recomputed-and-compared anywhere. **Closed** with
   `verify_dispatch_payload` and the named failure surface
   (missing/malformed/foreign-key/tampered), plus the
   `VerifyingDispatchTransport` receive-point decorator (D-P4.28-2).
3. **No distinguishable replay-vs-out-of-order verdict at the receive
   point.** `out_of_sequence_rejection` is the pure creation-side rule and
   does not classify. **Closed** with `SequenceGuard` (D-P4.28-3).
4. **The cross-tenant denial was refused but not auditable as a record.**
   The P4.23 decision record explicitly left the audit append open.
   **Closed** at the seam level with `TenantDenialAuditRecord` +
   `record_tenant_denial` + `SecurityAuditSink` (D-P4.28-4); the P4.30
   audit persistence consumes these records.
5. **The secrets/deployment-requirements note did not exist.** **Recorded**
   (§4 below) — not implemented, per the ledger's own wording.

## 3. Recorded decisions (labelled DEFAULT, reviewable)

| ID | Decision |
|---|---|
| D-P4.28-1 | **The P4.28 signature is the payload-bound extension of the P4.16 keyring pattern** — `sim-sig:<key_id>:<command_id>:fnv1a64:<16 hex>`, over the canonical binding (command, asset, sequence, quantity, exact `f64` bit pattern, transport identity). The digest is `fnv1a64`, the repo's existing NON-cryptographic integrity-checksum pattern (model content hashes). It is an integrity CHECKSUM, not a security primitive; no cryptographic property is claimed; the layer inherits the keyring's dev/SIMULATION labelling and MUST be replaced by a Stage 6/9-qualified signing identity before any non-simulation use. DEFAULT. |
| D-P4.28-2 | **The receive-point verification pattern is a transport decorator** (`VerifyingDispatchTransport`): verify the envelope's payload-bound signature, then admit its sequence, and only then hand it to the inner transport; every refusal is an application-layer NACK with a `dispatch-security:`-prefixed reason (mapping to the frozen ISSUED → FAILED trigger), and a refused envelope NEVER reaches the inner transport nor advances the sequence window. A legacy (non-payload-bound) keyring reference is REFUSED — it cannot prove the envelope untampered. Wiring this decorator into the composition root, and moving the P4.16 adapter's issue path to payload-bound signatures (which would change the ISSUED evidence format and the P4.16 suite's pinned assertions), is composition-root/Stage-6 wiring — recorded, not done here. DEFAULT. |
| D-P4.28-3 | **The sequence guard is strictly monotonic per asset+channel** with distinguishable verdicts: `unsequenced` (0), `replayed_sequence` (equal to the last accepted), `out_of_order_sequence` (lower). A refusal never updates the guard state — a rejected sequence cannot move the monotonic window. DEFAULT. |
| D-P4.28-4 | **The cross-tenant denial record is built by the platform from its own records** for the denied caller's attempt (caller tenant, owner tenant, command, asset, kind read/approve/issue, stable reference, instant), never returned to the caller — the caller still sees only the named `NotFound` shape (existence never leaks, P0.16 rule 5). The `SecurityAuditSink` port is declared in flexcore-application because the crate cannot depend on flexcore-audit (the dependency rule); this mirrors the recorded `FallbackAuditSink` seam pattern. The P4.30 audit persistence consumes the records; the append here is the seam, not the production store. DEFAULT. |
| D-P4.28-5 | **mTLS is NOT implemented.** The §14.3 field-15 authenticated transport identity remains the recorded `sim://dispatch-adapter/<key_id>` simulation stand-in; real transport security (mTLS, certificate handling) is a Stage 6 deployment matter. CONTEXT.md D6 stays OPEN: no OT boundary is crossed; these are code-level controls in the simulation layer only. Recorded. |

## 4. Secrets and deployment requirements (ledger P4.28 step 4 — recorded, NOT implemented)

- **No secrets exist in this work.** The keyring layers hold key
  IDENTIFIERS only (structural test: `SimulationSigningKey` has exactly one
  public field, `key_id: String`; the P4.28 module contains no key-material
  identifiers). All test keys are generated in-test with clearly-labelled
  non-production ids (`sim-key-p428-nonproduction-test`); no committed
  credential exists.
- **Recorded as later-stage DEPLOYMENT REQUIREMENTS (not implemented):**
  short-lived credentials, signing-key/certificate rotation, key custody
  and revocation, and the secret store that would hold them. The Phase 1
  identity work in this slice provides NO secret-store port — recorded,
  not assumed. These requirements bind Stage 6 (protocol adapters) and any
  non-simulation use; they change nothing in the 0.x simulation line,
  which stays simulation-only (§21.4).
- **No-secrets rule:** nothing secret-shaped may enter command payloads,
  logs, audit texts, or fixtures. The suite pins this structurally
  (marker scan over the payload JSON, signature references, transport
  identities, refusal texts, and audit texts).

## 5. Secret-scan status — STATIC ONLY (honest limitation)

- **What was done (static, in-repo only):** the structural source scans in
  the test suite (`the_keyring_layers_hold_no_key_material`,
  `payloads_signatures_transports_and_audit_texts_hold_no_credential_shaped_material`)
  and this record's review. No credential exists in the touched files.
- **What was NOT done:** `gitleaks` and `cargo deny` were NOT RUN — the
  loop bans any tool installation/execution. **The scan output the ledger's
  exit check asks to file ("scan output filed") is therefore pending the
  human gate**: run the chosen scanner over the working tree at build/test
  time and file its output here or in the phase evidence pack (P4.27).

## 6. Gaps and limitations (recorded, none silently filled)

- **Nothing was built or run** (§0). All tests below are pre-registered
  UNRUN expectations; the human build/test pass is the gate. "Determinism"
  is an in-suite double-run assertion, not an executed re-run.
- **The adapter's issue path still signs with the legacy keyring
  reference** (D-P4.28-2 wiring note). The P4.28 verification layer is
  exercised at its own public surface and through the decorator; it is not
  yet inside the adapter's issue flow.
- **`VerifyingDispatchTransport` covers the command direction only.** The
  `observe` (telemetry) direction delegates unchanged; telemetry
  authenticity/tenant scoping are the telemetry seams' recorded obligations
  (P0.16 rule 5; §10.4 quality states), not re-decided here.
- **`SecurityAuditSink` is a seam, not persistence.** The production
  append (PostgreSQL audit trail, §16.2) is P4.30's.
- **mTLS, real PKI, rotation, secret store:** not implemented — see §4 and
  D-P4.28-5. No certification, safety, or transport-security claim is made
  anywhere; FlexCore remains a supervisory orchestration layer (§14.5).
- **Research cut-off:** no external sources consulted; 7 November 2026
  unchanged.

## 7. File/test index

| File | Content |
|---|---|
| `crates/flexcore-application/src/dispatch_security.rs` | P4.28 module: payload binding + fnv1a64 digest, `sign_dispatch_payload`/`verify_dispatch_payload` + `DispatchSecurityError`, `PayloadBoundVerifier` + `DispatchEnvelopeVerifier`, `SequenceGuard`/`SequenceRefusal`/`channel_key`, `VerifyingDispatchTransport`, `TenantDenialAuditRecord`/`record_tenant_denial`/`TenantDenialKind`/`SecurityAuditSink`/`SecurityAuditError`, the two recorded statements |
| `crates/flexcore-application/src/lib.rs` | Additive module declaration + re-exports only (no existing line edited) |
| `crates/flexcore-application/tests/dispatch_security.rs` | 14 tests: tampered payload (5 attack vectors), missing signature (both layers), wrong key, non-payload-bound (legacy) reference refused, altered sequence, guard replay/out-of-order/unsequenced + no-advance-on-refusal, verifying-transport control + tampered/missing/wrong-key/unsequenced/malformed/replayed/out-of-order refusals with never-reaches-the-inner-transport oracles, cross-tenant read/approve/issue denial + audit records, no-secrets marker scans, keyring structural scan, two-run determinism |

**`dispatch_simulator.rs` was NOT edited** — every P4.28 addition composes
over its existing public API. No manifest changed; the dependency-rule
check is unaffected (flexcore-application gained no new crate edge).
