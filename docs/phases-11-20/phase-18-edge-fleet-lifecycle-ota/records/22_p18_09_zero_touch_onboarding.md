# P18.09 — Zero-touch onboarding design: FDO-pattern late binding, rendezvous seam, claim flow (manual path stays default and fallback)

| Field | Value |
|---|---|
| Subtask | P18.09 — Zero-touch onboarding design [S-165][S-166] |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN). Design module + claim-flow types written; no device claimed, no voucher exists, no rendezvous service is operated |
| Sources | [S-165] FIDO Device Onboard overview, [S-166] IETF T2TRG security-setup draft (gap analysis §11); seam: `crates/edge-agent/src/enrollment_ceremony.rs` (P9.33), P18.08 (`records/21_p18_08_identity_lifecycle.md`) |
| Files touched | `crates/edge-agent/src/zero_touch.rs` (new), `crates/edge-agent/src/lib.rs` (additive module declaration only) |

## 1. The honest default (stated first, by design)

**The manual provisioning path (P9.33) stays the default AND the fallback.**
This is structural, not aspirational: the SHIPPED rendezvous implementation
(`NoRendezvousConfigured`) refuses EVERY lookup with the named
`ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED` error, so a deployment composed with
this module and nothing else performs zero zero-touch claims and behaves
exactly as the P9.33 manual path does. Zero-touch is an additive,
separately-argued pattern that is structurally OFF until a deployment
implements the rendezvous port.

## 2. The FDO pattern as an analog (UNVERIFIED against the FDO spec text)

Every FDO-specific detail below is **UNVERIFIED** — [S-165] is a secondary
overview; the FDO 1.1 specification text is not held by this program. This
module is an analog of the PATTERN, never FDO conformance.

| FDO concept | FlexCore analog | Key rule |
|---|---|---|
| Ownership voucher | `OwnershipVoucherEntry` in an append-only, tenant-scoped register (`OwnershipVoucherRegister`; events with the closed `VoucherEventKind` vocabulary, latest-event-wins projection, closed transition machine) | A device WITHOUT a voucher entry can never be zero-touch claimed (`ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE`); the manual path is the fallback for exactly that device. A voucher is RECORDED once; re-binding creates a NEW voucher identity, never an edit |
| Rendezvous | `RendezvousSeam` port — resolves a factory-provisioned device to its owner's onboarding endpoint (`RendezvousDirective`) | The platform never runs discovery; a device the rendezvous cannot resolve is refused, never guessed to an owner |
| Late binding | Ownership decided at claim time from voucher + rendezvous agreement (`confirm_ownership` refuses a scope disagreement by name) | The rendezvous scope must equal the voucher scope (`ZERO_TOUCH_SCOPE_MISMATCH`) |
| Factory credential | The IDevID reference (P18.08 mapping); the claim compares the PRESENTED reference against the voucher's recorded reference | Reference-equality ONLY — no hardware anchor is verified (P18-E-2); `VoucherAnchorPosture` records what provisioning claimed (`RecordedUnverified` / `RecordedAbsent` / `NotRecorded`) without any platform claim that an anchor exists |

## 3. The claim flow (closed state machine; issues nothing)

```text
AwaitingDevice --VerifyVoucher------> VoucherVerified
VoucherVerified --ResolveRendezvous-> RendezvousDirected
RendezvousDirected --ConfirmOwner--> OwnershipConfirmed
OwnershipConfirmed --HandToCeremony-> CeremonyHandoff   [terminal]
(any state)      --Refuse-----------> Refused             [terminal, named reason]
```

The load-bearing rule: **`CeremonyHandoff` is the flow's last state and it
ISSUES NOTHING.** The handoff validates the platform-authority token
record's shape early (issuer named, not already expired) and ends there —
the P9.33 ceremony then runs UNCHANGED (its own full token validation at
`begin_request`, the nonce challenge, and THE platform-side approval gate
as `CeremonyApproval`, with `SelfApprovalRefused` semantics untouched).
Zero-touch removes the on-site operator-token step; it NEVER removes the
approval gate.

Named refusals (every one auditable): `ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED`
(the default), `ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE` (manual-path fallback),
`ZERO_TOUCH_DEVICE_REVOKED` (P18.08 propagation: an invalidated voucher can
never be claimed), `ZERO_TOUCH_IDEVID_REFERENCE_MISMATCH`,
`ZERO_TOUCH_VOUCHER_NOT_CLAIMABLE`, `ZERO_TOUCH_SCOPE_MISMATCH`,
`ZERO_TOUCH_TRANSITION_INVALID`, `ZERO_TOUCH_CLAIM_INPUT_INVALID`,
`ZERO_TOUCH_VOUCHER_EVENT_INVALID` / `ZERO_TOUCH_VOUCHER_TRANSITION_INVALID`
(register-side).

## 4. What would make zero-touch real (design dependencies, all human-gated)

1. A rendezvous implementation (a deployment component; NOT built here —
   it would be a new service surface with its own D6 review).
2. A verifiable factory IDevID on the devices (PENDING PILOT HARDWARE
   SPEC — without a verified anchor the claim is reference-equality only,
   and the record says so).
3. An ownership-voucher store implementation behind the tenant-scoped
   registry adapter (the journal/registry surface; the in-memory register
   here is the projection contract and the test double).
4. The human owner's decision that the deployment uses zero-touch at all
   (the DEFAULT remains the manual path).

## 5. Compliance mapping (mapping, never a certification claim)

| Deliverable aspect | Mapped requirement | Basis |
|---|---|---|
| No unauthenticated device joins a fleet (voucher + scope agreement + the unchanged approval gate) | IEC 62443-4-2 CR 2.1 (authorization enforcement) [S-163]; IEC 62443-4-1 SAR 3.4 (process) [S-164] | As cited by the phase-18 plan; clause-content interpretation **UNVERIFIED** — mapping records where evidence lives, never conformance |

## 6. Honesty register

1. **WRITTEN-UNRUN** — never built, never executed; the in-module tests are
   complete and UNRUN (human gate).
2. **No fleet** — no device is claimed, onboarded, or enrolled; no voucher
   exists; the only exercised surfaces are in-memory test doubles.
3. **UNVERIFIED** — every FDO-specific detail [S-165]; the T2TRG survey
   specifics [S-166]; the 62443 clause-content interpretations.
4. **No CA, no issuance** — the handoff token comes from the SAME platform
   identity authority the manual path uses; no new credential path exists.
