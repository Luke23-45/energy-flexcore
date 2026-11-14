# P6.34 — Secure-boot position record for edge hosts: per-profile STRUCTURE, hardware determination PENDING

Date: 2026-11-13. Subtask: P6.34, Phase 6 ledger (`ledger_phase_6.md` §P6.34).
Owner roles: DevOps / security (per the ledger). Report trace: §17.3 ("secure
boot/update policy for edge hosts where customer platform supports it"); §18.3;
§21.2.

**Honesty status:** no gateway hardware exists, no secure-boot verification
was performed, and no hardware specification is recorded anywhere in the
repository (P6.20 records "target gateway hardware profile (CPU/RAM/disk per
architecture)" as absent — CONTEXT.md open question 3, human decision). This
record therefore documents the POSITION STRUCTURE per profile and marks every
hardware determination **PENDING PILOT HARDWARE SPEC**. It never claims a
blanket "secure boot: yes" — that is exactly the ledger's recorded risk
("'secure boot' claimed for a profile whose hardware does not support it"),
and the mitigation is the position's form: per profile, tied to the hardware
spec, not a blanket claim.

## 1. The profile set (from P6.20)

P6.20 (`20_deployment_profile.md` §2) defines the edge deployment profiles.
The hardware spec itself is a pilot input; the profiles are recorded as
composition/architecture shape only.

| Profile | P6.20 content | Hardware determination |
|---|---|---|
| `edge-x86_64-systemd` | Single static Rust binary under systemd, Linux x86-64; dedicated non-login system user; outbound-only network posture; writable state confined to `/var/lib/flexcore-edge` | **PENDING PILOT HARDWARE SPEC** — no x86-64 gateway platform is named anywhere in-repo |
| `edge-aarch64-systemd` | The same shape on Linux ARM64 (the report §18.3/§21.4 requires both architectures ship; neither is exercised) | **PENDING PILOT HARDWARE SPEC** — no ARM64 gateway platform is named anywhere in-repo |
| `edge-oci` (alternative) | An approved container runtime instead of systemd — for sites that mandate containers; NOT built in this wave | **PENDING PILOT HARDWARE SPEC** — dependent on the site's runtime host, itself unnamed |
| Site/customer-managed server variant | The profile's §2 allows "customer-managed servers" per report §18.1/§18.3 | **PENDING PILOT HARDWARE SPEC** — and additionally dependent on the customer platform's own boot policy, which report §17.3 scopes explicitly ("where customer platform supports it") |

## 2. Per-profile secure-boot position (STRUCTURE; nothing determined)

Position categories, per the ledger: **supported + enabled + verified** /
**supported-not-enabled** (reason + owner) / **unsupported** (fallback =
P6.20 signed updates + residual risk stated).

| Profile | Secure boot supported? | Enabled + verified? | Reason / owner | Fallback + residual risk |
|---|---|---|---|---|
| `edge-x86_64-systemd` | **PENDING PILOT HARDWARE SPEC** | NOT DETERMINED — no verification has been performed | The named platform decision is a pilot input (owner: DevOps + security, at the pilot hardware selection; CONTEXT.md open question 3). No platform is named, so neither "supported" nor "unsupported" may be asserted | Regardless of the determination, the P6.20 signed-update path is the primary integrity control (§4 below). Residual risk if secure boot is ultimately unsupported: a physical attacker with host access could replace the OS/bootloader below the FlexCore update layer — the P6.20 signature verification would still refuse a tampered FlexCore bundle, but it cannot protect the layers beneath it. Stated, not quantified |
| `edge-aarch64-systemd` | **PENDING PILOT HARDWARE SPEC** | NOT DETERMINED | Same as above; ARM64 embedded gateways vary widely in boot-ROM signing support — the determination is per actual board, never per architecture | Same as above |
| `edge-oci` | **PENDING PILOT HARDWARE SPEC** | NOT DETERMINED | Additionally depends on the site's container-host policy (image signature verification at the runtime is a related-but-distinct control; it does not substitute for boot integrity, and it is likewise undetermined) | Same as above |
| Customer-managed server variant | **PENDING PILOT HARDWARE SPEC** + customer platform policy | NOT DETERMINED | Report §17.3 scopes the control to "where customer platform supports it" — the customer's platform policy is a site input (owner: security, at the D6/site review) | Same as above; the customer's own platform controls apply |

**No row above claims supported, enabled, or verified. Every hardware cell
is PENDING PILOT HARDWARE SPEC.** Filling any cell requires the named
hardware spec and (where "enabled + verified" would be claimed) an executed
verification at install and after update — which has not happened.

## 3. Where supported: how verification WOULD happen (the recorded procedure, tied to P6.20)

If a profile's hardware determination comes back "supported", the
verification path is recorded NOW so the claim, when finally made, has a
defined mechanism tied to the P6.20 upgrade/rollback paths:

| Stage | Verification step | Trace |
|---|---|---|
| Install | Platform secure boot verifies the bootloader/kernel chain up to the host OS; the FlexCore edge agent is then installed per the P6.20 build/package procedure (checksums + SBOM published with the release) | §18.3; P6.20 §5 |
| Update | The P6.20 update bundle is signature-verified BEFORE any swap (manifest version/checksums/expiry against the platform release key); secure boot covers the chain beneath this — the two controls are complementary, neither substitutes for the other | §9.3; §17.3; P6.20 §6 |
| Rollback / downgrade | The previous version retained on disk is the same signature-verified artifact set; automatic rollback on failed health re-activates a previously verified bundle; a downgrade is an explicit operator-signed action | P6.20 §6 (rollback + downgrade rows) |
| Post-update confirmation | The running version is visible to the operator (P6.17/P6.25 health surface); the "enabled + verified" claim for a profile is confirmed by an executed install/upgrade test per the ledger P6.34 verification method ("install/upgrade test confirms the verified-boot claim where made") — currently UNRUN | ledger P6.34 verification method |

**Carried status (verbatim): no PKI is implemented.** The P6.20 record
states: "the signing keys, the trust anchor distribution to edges, rotation,
and revocation are Stage-6+ deployment requirements, not implemented. No
real PKI exists: no release key pair was generated, no signature format was
chosen, no verification code was written, and nothing was signed." Secure
boot rides on top of that gap — a "verified" claim today would be doubly
false (no hardware, no signing infrastructure), which is why every such
cell stays PENDING.

## 4. Filing / citation note (ledger P6.34 step 3)

| Consumer | What it cites |
|---|---|
| **P6.03 (D6 cyber/OT boundary review)** — `03_d6_boundary_review.md` | This record, as the §17.3 secure-boot/update control position for the edge hosts. The D6 reviewer adds the site-specific layer: which profile the actual pilot site uses, and the customer platform's own boot policy (the §2 fourth row). D6 remains OPEN (review PREPARED, sign-off NOT SIGNED, reviewer NOT ASSIGNED) — this record gives the reviewer one input, it does not close anything |
| **P6.30 (v0.4.0 release-gate record)** | This record for the CHECKLIST.md item "Secure-boot position recorded per gateway profile — Evidence: per-profile record (supported / not enabled / unsupported with fallback + risk)". The honest position P6.30 inherits: the per-profile STRUCTURE exists; every hardware determination is PENDING PILOT HARDWARE SPEC; the fallback (P6.20 signed updates, DESIGN ONLY) is recorded with its residual risk |
| **P6.20 (deployment profile)** | This record files alongside `20_deployment_profile.md` and closes "the one §17.3 control P6.20 does not cover" as a recorded position-with-pending-hardware, not as an implemented control |

## 5. Honest limitations

1. No gateway hardware exists in-repo or in any record; no secure-boot
   capability was checked on any board; no boot measurement was taken.
   Every hardware cell is PENDING PILOT HARDWARE SPEC.
2. The fallback control itself is DESIGN ONLY (P6.20 §6: no PKI, no signing
   keys, no verification code, install/upgrade/rollback tests UNRUN) — the
   fallback is therefore also not yet an executed control, and this record
   does not present it as one.
3. This record adds no code and owns exactly one new ledger file plus the
   P6.34 ledger tick.
