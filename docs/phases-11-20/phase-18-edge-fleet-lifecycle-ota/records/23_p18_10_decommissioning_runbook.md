# P18.10 — Decommissioning runbook: revoke, release, deregister, verify wipe, second-life note

| Field | Value |
|---|---|
| Subtask | P18.10 — Decommissioning runbook |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **RECORD-ONLY (runbook), AWAITING HUMAN REVIEW**. The runbook is DESIGN: no device is decommissioned, no certificate is revoked, no voucher is released, no wipe is verified — no fleet exists (phase-18 constraint 6) |
| Sources | Seams: `0018_edge_device_registry_p6_06.up.sql` (append-only registry + journal), P14.03 security design (`docs/phases-11-20/phase-14-protocol-device-connectivity/records/03_p14_03_security_design.md`), P18.08 propagation design (`records/21_p18_08_identity_lifecycle.md`), P18.09 voucher register (`records/22_p18_09_zero_touch_onboarding.md`), P18.12 state journal (`records/25_p18_12_fleet_registry_hardening.md`) |
| Files touched | this record only (the flow TYPES it consumes live in `identity_lifecycle.rs` / `zero_touch.rs`, written under P18.08/P18.09) |

## 1. The runbook

The procedure for taking ONE edge device permanently out of a FlexCore
fleet. Every step is attributed (actor + ticket reference mandatory — the
P18.08 `RevocationPropagationPlan` refuses an unattributable step) and the
step ORDER is the canonical `PROPAGATION_ORDER` (a gate recorded before the
registry evidence exists is the named
`IDENTITY_PROPAGATION_OUT_OF_ORDER` refusal).

| Step | Action | Owner / surface | Honest execution status |
|---|---|---|---|
| 1. Authorize | Open a decommission ticket naming the device identity, tenant, site, reason | Human operator (platform side) | NOT-STARTED — no fleet |
| 2. Revoke the operational identity (LDevID) | The P9.33 ceremony `revoke` (actor + reason mandatory; append-only register event) + a REVOKED term row in `edge_device_registry` + a REVOKED event in `edge_enrollment_journal` (0018, INSERT-only grants) | Platform identity authority; PKI operations revokes the real certificate (P14.03 §4 "Revocation" row — FlexCore never issues or revokes certificates itself) | NOT-STARTED — no CA, no certificate exists (P14.03 §6.1) |
| 3. Release the protocol trust chains | The device leaves the aggregator-side trust posture for its protocol (2030.5 et al.): operator/PKI action against the deployed chain; FlexCore stores anchor LABELS only (P14.03 §2) | Human PKI operations | NOT-STARTED |
| 4. Release the ownership voucher | `VoucherEventKind::VoucherReleased` on the P18.09 register (`Claimed → ReleasedForDecommission`) — the device may enter a second life with a NEW voucher identity, never an edit | Platform operator | NOT-STARTED — no voucher exists |
| 5. Close the channel | The edge's fail-closed `RevocationChecker` (P6.06) refuses to bring the channel up on the revoked identity; the control plane refuses the session (P14.02 fail-closed posture). Update authorization is refused (`RefuseRevokedDevice`, P18.13) | Shipped fail-closed seams | The refusal behavior is SPECIFIED; never exercised against a real device |
| 6. Deregister from the fleet registry | Record the end-of-fleet state in the migration-0023 state journal (e.g. a final `LAST_SEEN_HEALTH` / cohort `OUT` row) so fleet views stop counting the device as a rollout target; the 0018 identity evidence is NEVER deleted (append-only) | Platform (INSERT-only on the journal) | NOT-STARTED |
| 7. Invalidate any zero-touch surface | `VoucherEventKind::VoucherInvalidated` (`Held`/`ReleasedToRendezvous → Invalidated`) so the device can never be (re)claimed | Platform | NOT-STARTED |
| 8. Data-wipe verification | The SITE/deployment procedure wipes device-local state (`/var/lib/flexcore-edge` per the P6.20 profile shape; the credential store, the identity register, the telemetry buffer, the audit buffer). Verification is a RECORDED human-gate check (the operator attests the wipe; the platform records the attestation reference — it cannot itself verify a physical wipe) | Site operator; platform records the reference | NOT-STARTED — no device-local state exists to wipe |
| 9. Second-life note | The decommission ticket records the disposition: (a) re-enrollment into the SAME tenant requires the NORMAL compromise-re-enrollment discipline (fresh platform-authority ticket; the revoked identity can never authorize its successor — `CompromisedIdentityVouchingRefused` stands), (b) transfer to another tenant/site requires a NEW voucher identity + the P9.33 ceremony from `Unenrolled`, (c) retirement/second life outside the fleet ends the record here | Human owner | NOT-STARTED |

## 2. What this runbook deliberately does NOT do

- **No un-revoke.** Revocation is append-only everywhere (0018 registry and
  journal; the kernel register is plugin-scoped, byte-identical, and
  irrelevant here except as the CONSUMED pattern). A "re-activated" device
  is a NEW identity term, never a reversal.
- **No evidence deletion.** The 0018 identity history, the enrollment
  journal, and the migration-0023 state journal keep every row (the
  MIGRATION_POLICY lifecycle-table rule: evidence removal is never a
  code-only decision and additionally requires operator sign-off).
- **No certificate/CA operation.** Steps that touch real certificates are
  PKI-operations work (P14.03 §4); FlexCore consumes the outcomes.

## 3. Compliance mapping (mapping, never a certification claim)

| Runbook aspect | Mapped requirement | Basis |
|---|---|---|
| Attributed, ordered decommissioning with fail-closed gates | IEC 62443-4-1 SAR 3.4 (process) [S-164]; IEC 62443-4-2 CR 2.1 (authorization enforcement) [S-163] | As cited by the phase-18 plan; clause-content interpretation **UNVERIFIED** |
| Data-wipe verification | Same process requirement family; the wipe itself is a SITE procedure outside FlexCore | Recorded, never claimed |

## 4. Honesty register

1. **RECORD-ONLY runbook** — the procedure has never been executed; no
   device exists to decommission.
2. **No certificate issued or revoked** — P14.03 §6.1 stands unchanged
   (NOT-STARTED for every PKI operation).
3. **PENDING PILOT HARDWARE SPEC** — step 8's wipe scope depends on the
   device profile (P6.20/P6.34 posture); the physical wipe mechanics are a
   deployment/hardware concern.
4. The only code this subtask "delivers" is the CONSUMED types from
   P18.08/P18.09/P18.13 (all written, all UNRUN) — no new code was needed
   for the runbook itself, which is the honest deliverable here.
