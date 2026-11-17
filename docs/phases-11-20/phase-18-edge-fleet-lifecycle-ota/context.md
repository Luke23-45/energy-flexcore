# Phase 18 — Edge Fleet Lifecycle and OTA Updates — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/16 — see `ledger_phase_18.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3). Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

The Phase 10 exit-gate record names the gap this phase owns
(`ledger/phase-10-v1-release-hardening/31_phase_exit_gate_record.md` §5):

> No update-bundle producer exists (P9.34/P10.12 largest recorded gap).

The edge agent therefore verifies nothing: the P9.34 signed-update
verification surface is complete and fail-closed, and nothing can be
produced for it to verify. The P10.12 upgrade/rollback run on the release
candidate is an EMPTY run record. Device identity is the same shape one
level up: the ceremony-based lifecycle (P9.33) was designed for
one-pilot-at-a-time enrollment and lacks zero-touch scale patterns, a
rotation schedule beyond the P6.06 expiry monitor, and any decommissioning
path.

The gap analysis (§7, Track F2 edge-lifecycle paragraph) converts this gap
into a build-list anchored in OTA-security research:

| Research finding | Source(s) |
|---|---|
| TUF-style metadata repository: root offline / targets / snapshot / timestamp roles, N+1 root rotation, expiry discipline | [S-161] |
| SUIT-shaped signed manifests: applicability metadata, sequence-number anti-rollback | [S-162] |
| Device flow: download → verify → install → status incl. explicit failure states; A/B slots with health-check auto-rollback and anti-rollback counters | [S-161][S-162] |
| Staged cohort rollouts with abort thresholds | gap analysis §7 (Track F2) |
| Compliance mapping: IEC 62443-4-1 SAR 3.4 (process) / 4-2 CR 3.4 + CR 2.1 (component); update-safety standard landscape | [S-163][S-164]; posture row [S-22] |
| Device identity: IDevID/LDevID (IEEE 802.1AR model), FDO-pattern late binding (ownership voucher, rendezvous), decommissioning | [S-165][S-166] |
| eSIM/cellular profile state tracked as data only | [S-167] |

Two program interlocks frame the scope. The gap-analysis §4 PKI row ("trust
anchors, issuance, revocation, renewal" [S-8][S-9]) assigns the
device-identity slice to phases 14, 17, and 18 — this phase takes the
device-identity lifecycle and the fleet-scale patterns. And P14.03 (2030.5
security) already records that "real PKI ties to ... Phase 18 (device
identity)". Phase 18 consumes those patterns; it never stands up a CA.

Provenance note (honest): the §7 edge-lifecycle paragraph cites this
research as [S-162][S-163] / [S-164][S-165] / [S-166][S-167] / [S-168],
while the §11 source register resolves TUF → [S-161], SUIT → [S-162],
IEC 62443-4-2 → [S-163], UL 2900 → [S-164], FDO → [S-165], T2TRG → [S-166],
eSIM → [S-167]. This phase cites per the §11 register; the one-paragraph
offset in §7 is a known citation slip and the register is authoritative.

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
written modules with records, not verified behavior. Every one was re-read
for this context.

| Seam | Path | What it honestly contains |
|---|---|---|
| Signed-update verification (P9.34) | `crates/edge-agent/src/update_verification.rs` | `UpdateGate::verify_before_staging` — verification BEFORE any write (refusal order: unsigned → invalid manifest → algorithm mismatch → untrusted signer → below floor → digest mismatch); `NoUpdateVerifier` fail-closed default (refuses EVERYTHING when nothing is configured); `TestOnlyFnv1a64UpdateVerifier` loudly labeled test-grade; `UpdateManifest` v1 (closed vocabularies, key REFERENCE only, minimum-version floor, mandatory approval reference); `FileUpdateLedger` append-only activation/rollback register — rollback re-activates a PREVIOUSLY-ACTIVATED version, below-floor rollback requires a recorded `RollbackOverride` (never a silent downgrade); `UPDATE_SLOT_DECISION` (the register is the source of truth; two-slot A/B is the RECOMMENDED deployment shape); `UPDATE_STATEMENT` honesty record |
| P9.34 record | `ledger/phase-09-pilot-qualification/34_signed_update_verification.md` | The signed-update policy record; carries the no-producer gap |
| Enrollment ceremony (P9.33) | `crates/edge-agent/src/enrollment_ceremony.rs` | Closed state machine (Unenrolled → Requested → Challenged → Approved → Active; Revoke; ResetForReEnrollment — a revoked identity can never authorize the new one); single-use short-lived tokens (edge-side `TokenSingleUseLedger`); nonce challenge with replay protection; `SelfApprovalRefused` — no self-approval path exists; append-only `FileIdentityEventRegister` |
| Enrollment seam (P6.06) | `crates/edge-agent/src/enrollment.rs` | `EnrollmentClient` / `CredentialStore` / `RevocationChecker` ports; `DenyAllCredentialIssuer` (the shipped issuer refuses every request); `UnconfiguredRevocationChecker` fails closed; `FilesystemCredentialStore`; `CredentialExpiryMonitor`; NO key-generation code anywhere in the crate |
| Uplink runner | `crates/edge-agent/src/runner.rs` | HEALTHY/DEGRADED buffer state machine; drain-oldest-batch forward path; audited backpressure on buffer-full |
| Command gate (P6.14) | `crates/edge-agent/src/command_path.rs` | Four-condition, all-or-nothing, fail-closed enablement gate; no inbound listener in any state; a receipt is a GATE ADMISSION, never physical delivery |
| Kernel revocation + staged rollout (P7.38) | `crates/flexcore-kernel/src/revocation.rs` | Append-only `RevocationStore` (NO un-revoke API); `revocation_gate` consulted BEFORE the provenance gate; the PLUGIN staged rollout is cohort/percentage-free BY CONSTRUCTION — no percentage canary — the policy this phase must NOT silently reinterpret (constraint 2) |
| Device registry (P6.06) | `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql` | `edge_device_registry` — append-only per identity term, partial unique index enforcing one ACTIVE term per (device, tenant); `edge_enrollment_journal` — ENROLLED / ROTATED / REVOKED / EXPIRED / REJECTED, append-only with defence-in-depth triggers; NO key material, NO certificate columns |
| Audit crate (P1.10) | `crates/flexcore-audit/` | `AuditRecord` plus the single-method append-only `AuditSink`; P9.32 integrity proof record (`ledger/phase-09-pilot-qualification/32_audit_integrity_proof.md`); chain digests are test-grade FNV-1a (LR-203) until the P11.15 production upgrade |
| Adapter conformance harness (P6.13) | `crates/edge-agent/src/conformance.rs` | Protocol-agnostic fault-injection harness — the pattern the post-update re-check interlock consumes; NOT a certification claim |
| Deployment/upgrade model (P10.24) | `docs/operations/deployment-upgrade-model.md` | DESIGN document — not a deployed system; edge install/enroll/update/recover procedures referenced; the P10.12 upgrade/rollback run record is EMPTY |

## 3. Constraints (critical)

1. **Fail-closed preserved.** With no configured verifier the agent still
   refuses updates: `NoUpdateVerifier` semantics stay exactly as shipped,
   and a real verifier lands BEHIND the P9.34 port as a replacement for the
   default, never as a bypass. Verification-before-any-write, the refusal
   order, the append-only register, and the never-silent-downgrade rule are
   inherited unchanged.
2. **The kernel's "no percentage canary" scope is plugin swapping — and
   stays that way.** `crates/flexcore-kernel/src/revocation.rs` records the
   v0.5.0 plugin staged rollout as cohort/percentage-free BY CONSTRUCTION.
   Device-fleet cohort rollouts (P18.06) are a NEW, separately-argued
   mechanism with their own argument and evidence (abort thresholds,
   attestation gates, pre-registered parameters) — distinct from the kernel
   policy, which remains canary-free and byte-identical. Every phase-18
   record that touches rollout states this distinction explicitly; silently
   reinterpreting the kernel policy as if it had been relaxed is a
   compliance failure.
3. **Updates are BLOCKED while commands are ISSUED/TRACKING on the device.**
   Drain-before-update mirrors the report §5.6 lifecycle drain discipline
   (the same rule the kernel applies to revoke-a-running-plugin):
   in-flight commands are drained to a terminal state before an update may
   proceed, and the refusal while any command is ISSUED/TRACKING is a named,
   audited refusal (P18.15).
4. **Signing keys are a human ceremony.** Production signing keys never
   exist at planning or implementation time; every key ceremony is a human
   gate (P10.21 interplay: signed artifacts, checksums, support matrix).
   Test-grade signing (the FNV-1a family) stays labeled test-grade and is
   never claimed as a release signature; production crypto remains a
   DEPLOYMENT REQUIREMENT (conventions §6.6).
5. **Hardware trust anchors are ASSUMPTIONS, recorded as P18-E-2.** TPM /
   secure element presence is unknown (the P6.34 posture stands: every
   hardware determination is PENDING PILOT HARDWARE SPEC). The platform is
   designed to work with AND without hardware anchors, and every
   attestation statement says honestly what changes in each case — never
   implying that a root of trust exists.
6. **No fleet exists.** Everything in this phase is rehearsal-ready design
   plus test doubles: no device is enrolled, claimed, updated, or
   decommissioned; no cohort is rolled out; the gap is closed at DESIGN
   level and the close record says so.

## 4. Glossary (OTA and fleet-lifecycle terms used in this phase)

| Term | Meaning |
|---|---|
| OTA | Over-the-air update — remote delivery and application of new device software/firmware [S-161][S-162] |
| Update manifest | Signed metadata describing an update (version, applicability, digests, sequence number, size) — verified BEFORE any write; extends the P9.34 `UpdateManifest` |
| Applicability | Manifest metadata declaring which devices may install the update (vendor, device class, device IDs) [S-162] |
| Anti-rollback sequence number | Monotonically increasing counter in signed metadata; anything below the last-seen value is refused — prevents reinstalling old, vulnerable versions [S-162] |
| TUF roles | The Update Framework's four metadata roles: root (trust anchors), targets (artifact digests + delegations), snapshot (targets consistency), timestamp (freshness) [S-161] |
| SUIT | IETF Software Updates for Internet of Things — manifest architecture and information model for constrained devices [S-162] |
| A/B slots | Two storage slots holding alternate system versions; the inactive slot is updated and swapped after verification (`UPDATE_SLOT_DECISION`: recommended shape; the register is the truth) |
| Watchdog boot counters | Device-side reboot counters/watchdogs used as health evidence: a device that fails the new version repeatedly is auto-rolled-back |
| Cohort rollout | Staged fleet update: canary group first, then progressively larger blocks, each gated on the previous block's health |
| Abort threshold | A pre-registered failure-rate condition that halts a rollout (and optionally auto-rolls-back the cohort) — registered BEFORE any run (X-8) |
| Attestation | Signed evidence from a device about its boot-time state (measurements, counters) that the platform checks before authorizing an update [S-166] |
| IDevID / LDevID | IEEE 802.1AR device-identity model, as surveyed by the T2TRG security-setup draft [S-166]: factory-installed initial (IDevID) and locally-issued operational (LDevID) certificates |
| Ownership voucher | The FDO concept binding a device to an owner so it can take instructions after being released to a rendezvous service [S-165] |
| Rendezvous | The FDO-mediated service directing a factory-provisioned device to its current owner's onboarding endpoint [S-165] |
| Zero-touch provisioning | Onboarding with no on-site manual step: device + voucher + rendezvous replace the operator-delivered token (the P9.33 manual path stays) |
| Decommissioning | The recorded end-of-fleet path: revoke credentials, release the voucher, deregister, verify data wipe, note second-life disposition |
