# P18.08 — Device identity lifecycle formalization: IDevID/LDevID mapped onto the P9.33 ceremony, rotation schedule, revocation propagation

| Field | Value |
|---|---|
| Subtask | P18.08 — Device identity lifecycle formalization [S-165][S-166] |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (the WRITTEN-UNRUN class; conventions §2.2). Design module + named types written; nothing built, nothing run, no fleet exists |
| Sources | [S-165] FIDO Device Onboard overview, [S-166] IETF T2TRG security-setup draft (gap analysis §11); seams: `crates/edge-agent/src/enrollment_ceremony.rs` (P9.33), `crates/edge-agent/src/enrollment.rs` (P6.06), `crates/flexcore-kernel/src/revocation.rs` (P7.38, READ-ONLY), `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql` |
| Files touched | `crates/edge-agent/src/identity_lifecycle.rs` (new), `crates/edge-agent/src/lib.rs` (additive module declaration only) |

## 1. The IDevID/LDevID mapping (the deliverable)

The IEEE 802.1AR two-certificate model, as surveyed by the T2TRG security-setup
draft [S-166], maps onto the P9.33 ceremony as follows. **UNVERIFIED**: the
802.1AR standard text itself is not held by this program; the survey is the
only basis, and every 802.1AR-specific claim below inherits that limitation.

| Model element | FlexCore reality | Where it lives |
|---|---|---|
| **IDevID** (factory-installed initial identity) | Manufacturer-issued; FlexCore never generates, issues, rotates, or replaces one. Consumed ONLY as verification/attestation input: the P18.09 zero-touch claim anchor and the P18.13 attestation anchor candidate — both subject to P18-E-2 (PENDING PILOT HARDWARE SPEC, P6.34 posture) | `identity_lifecycle.rs` (`DeviceIdentityClass::IDevID`, `IdentityTermMap`) |
| **LDevID** (locally-issued operational identity) | EXACTLY the credential term the P9.33 ceremony binds at Approved → Active: the P6.06 `DeviceCredentialRecord`. No second issuance path exists and none was added | `DeviceIdentityClass::LDevID` = `IdentityTermMap::CEREMONY_SUBJECT` |
| Ceremony vocabulary | CONSUMED verbatim — every transition, refusal (`SelfApprovalRefused`, `CompromisedIdentityVouchingRefused`), and the append-only register discipline of `enrollment_ceremony.rs` is unchanged | `crate::enrollment_ceremony` (untouched) |

The mapping is pinned in code by `IdentityTermMap::MAPPING_STATEMENT` and by
the in-module tests. The ceremony is NOT extended: phase 18 adds the CLASS of
each term, not a new identity path.

## 2. Credential rotation schedule

The schedule (`CredentialRotationSchedule`) formalizes WHEN a ceremony re-run
is DUE; it computes a pure assessment over one validated P6.06 credential
record against an INJECTED instant (never wall time) and never runs anything:

| Assessment | Meaning |
|---|---|
| `Current` | Inside the lifetime, outside the rotation window |
| `RotateDue` | Inside the rotation window — a NORMAL ceremony re-run: the platform authority issues a fresh single-use, short-lived token; the ledger P9.33 step-2 discipline applies identically (there is NO second token path) |
| `Expired` | Past the recorded expiry (the P6.06 `CredentialExpired` rule carries over) |
| `NoExpiryRecorded` | The shipped seam's honest `None` — NEVER assumed fine; the recommendation is an operator-reviewed re-issue |
| `LifetimeExcessive` | The term exceeds the configured lifetime ceiling — refused at assessment |

Defaults: rotation window 30 days, maximum term lifetime 90 days — both are
**DEFAULT-RECOMMENDATIONS, not sourced figures**; they are per-deployment
configuration, pre-registered (X-8) before any run that could fill them.
Fail-closed construction: a non-positive window, or a maximum not exceeding
the window, is refused by name (`IDENTITY_SCHEDULE_INVALID`).

## 3. Revocation propagation (the kernel interplay — consumed, never changed)

The kernel's P7.38 register (`crates/flexcore-kernel/src/revocation.rs`) is
PLUGIN-scoped (keyed by plugin id/version and signer key reference,
append-only, NO un-revoke) and **stays byte-identical** — the phase-18 ban
and the recorded `KERNEL_REVOCATION_INTERPLAY_STATEMENT` (pinned in code)
state this verbatim, including the no-percentage-canary reinterpretation
rule (context constraint 2).

Device-identity revocation is a SEPARATE register family: the 0018
`edge_device_registry` REVOKED term row + the 0018 `edge_enrollment_journal`
REVOKED event, propagated to the closed target set in canonical order
(`PROPAGATION_ORDER`; a gate recorded before its predecessor is the named
`IDENTITY_PROPAGATION_OUT_OF_ORDER` refusal):

| # | Target | Surface | Execution owner |
|---|---|---|---|
| 1 | `IdentityRegister` | 0018 REVOKED term + journal event (append-only, INSERT-only grants) | Platform identity authority (deployment operation) |
| 2 | `ProtocolTrustChain` | The device leaves the aggregator-side trust posture per protocol (P14.03 operations design §4 "Revocation" row; FlexCore stores anchor LABELS, never key material) | Human PKI operations (P14.03 §4) |
| 3 | `UplinkChannelGate` | The edge's fail-closed `RevocationChecker` (P6.06) refuses the channel; the control plane refuses the session (P14.02 `MtlsRequirements` fail-closed posture) | Platform + edge (seams already shipped) |
| 4 | `UpdateAuthorizationGate` | `crate::attestation::AttestationDecision::RefuseRevokedDevice` — a revoked device is refused attestation/authorization for ANY update (P18.13) | This module's consumer (P18.06 orchestrator, Agent A) |
| 5 | `ZeroTouchVoucher` | `crate::zero_touch::VoucherEventKind::VoucherInvalidated` — a revoked device can never be (re)claimed (P18.09) | This module's consumer |

Per-step statuses are the closed `PropagationStepStatus` vocabulary
(`SpecifiedNotPerformed` / `RecordedOnTestDouble` /
`RequiresDeploymentOperation`) — a DESIGN vocabulary for runbooks and test
doubles; it carries NO "performed on a real device" state at all.

## 4. Compliance mapping (mapping, never a certification claim)

| Deliverable aspect | Mapped requirement | Basis |
|---|---|---|
| Rotation schedule + expiry discipline | IEC 62443-4-1 SAR 3.4 (process); IEC 62443-4-2 CR 2.1 (component) [S-163][S-164] | As cited by the phase-18 plan from the gap analysis §11 register. The clause CONTENT interpretation is **UNVERIFIED** — the program holds the standard webstore reference, not the text; the mapping records WHERE the evidence lives, never conformance |
| Revocation propagation (fail-closed gates) | IEC 62443-4-2 CR 2.1 [S-163] | Same UNVERIFIED-clause-content caveat |

## 5. Honesty register

1. **WRITTEN-UNRUN** — the module and its tests have never been built or
   executed (loop ban 2/3).
2. **No fleet** — no device exists; no identity term is active; no
   certificate was ever issued, rotated, or revoked; the propagation plan
   has never been executed against anything but test doubles (in-module
   `BTreeMap` plan, never run).
3. **UNVERIFIED** — the 802.1AR/T2TRG model specifics [S-165][S-166]; the
   62443 clause-content interpretations [S-163][S-164].
4. **PENDING PILOT HARDWARE SPEC** — the IDevID's usefulness as an
   attestation/claim anchor (P18-E-2; P6.34 posture).
5. **Kernel untouched** — `crates/flexcore-kernel/src/revocation.rs` is
   byte-identical; this module only CONSUMES the pattern.
