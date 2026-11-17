# P17.03 — OCPP security Profile 3 onboarding: per-device client certificates, the certificate flows, identity proofing, security-event ingestion — no CA is stood up

| Field | Value |
|---|---|
| Subtask | P17.03 (OCPP security Profile 3 onboarding: per-device client certificates, SignCertificate / CertificateSigned / InstallCertificate flows, SecurityEventNotification ingestion, identity proofing at registration [S-126][S-127][S-133]. Design + message types; no CA is stood up) |
| Wave / agent | Phase 17 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the onboarding policy, certificate-flow state machine, identity-proofing ledger, and security-event ledger are written model code, never compiled or executed; NO certificate is ever issued, validated, or verified in this program; no CA exists (P14.03 interplay); no security claim of any kind |
| Sources | [S-126] (OCPPLab security profiles explained), [S-127] (OCA Security Operations Guide, 63 pp.), [S-133] (Argonne 2024 EVSE security report); P14.03 (the phase-14 security record — the PKI-operations boundary) |
| Files touched | `crates/adapters/protocol-ocpp/src/security.rs` (NEW); `crates/adapters/protocol-ocpp/src/model.rs` (the security-block payload types: `SignCertificateRequest`, `CertificateSignedRequest`/`Status`, `InstallCertificateRequest`, `SecurityEventNotificationRequest`, `CertificateType`); `crates/adapters/protocol-ocpp/src/csms.rs` (`CsmsSecurityRequirements` — Profile-3 structural posture); `tests/adapter_surface.rs` |

## 1. Profile 3 is the onboarding target — structurally

`OnboardingPolicy` (one value, no constructor that could weaken it):
`accepts_for_onboarding(Profile3)` is the only `true`; Profiles 1
(unsecured transport) and 2 (TLS + HTTP basic auth) are RECORDED legacy
interop facts, never accepted for new onboarding [S-126]. The policy
statement travels with every surface. The endpoint posture
(`CsmsSecurityRequirements`, record 01) has no field that could disable
TLS or downgrade the profile: the trust-anchor label set must be
non-empty (an endpoint with no trust anchor accepts nothing — and must
not silently accept everything instead), and revocation checking
defaults `true` (an unverifiable revocation status is a rejection, not
a pass — the P14.02 fail-closed pattern).

## 2. Labels, never material

Every certificate-shaped type carries LABELS only:
`DeviceCertificateRecord` (charge point, certificate type, certificate
label, trust-anchor label, recorded instant), the CSR label inside
`SignCertificateRequest`, the chain label inside `CertificateSignedRequest`,
the certificate label inside `InstallCertificateRequest`. There is no
field in this crate a key, a CSR blob, or a certificate blob could hide
behind beyond the declared label strings; the material behind a label is
an artifact of the deployment's PKI operations (composition-root),
never this crate's. **No CA is stood up anywhere in this program**
(P14.03 interplay).

## 3. The certificate flows (state machine, not issuance)

`CertificateFlowLedger` records the PROTOCOL positions, one flow per
(charge point, certificate type):

| State | Meaning |
|---|---|
| `REQUESTED` | A SignCertificate REQUEST was received and recorded (the CSR is a label; nothing was issued) |
| `SIGNED_AVAILABLE` | The EXTERNAL PKI-operations gate produced a signed certificate and the CertificateSigned REQUEST was sent — the issuance act is external (human/ops gate), this module records the OUTCOME only |
| `INSTALLED` | The charge point confirmed installation (its Accepted response was recorded) |
| `REJECTED{reason}` | Named end (charge point rejection, PKI refusal, timeout); a rejected flow may be re-opened |

Transitions are named (`SecurityError::FlowStateInvalid` carries the
current state and the attempted step); an open flow refuses a second open
(one flow per target at a time).

## 4. Identity proofing at registration [S-126][S-133]

`IdentityProofingLedger` records the OUTCOME of the binding between a
charge-point identity and a validated client certificate against a
declared trust anchor. The certificate VALIDATION itself is the transport
seam's TLS layer + the deployment's trust anchors; the ledger never
fabricates a proofing. The registration gate is fail-closed:
`require_proofed` refuses any charge point without a proofing record
(`SecurityError::NotProofed`) — identity proofing at registration is a
requirement, not a best practice.

## 5. SecurityEventNotification ingestion = records, never reactions

The `SecurityEventLedger` ingests station-reported security events as
bounded EVENT-LOG records. Classification is a DECLARED table with a
deliberately SMALL set of names (UNVERIFIED spellings — the OCA Security
Operations Guide [S-127] and the OCPP security sections are the
implementation-time checks): `InvalidCSMSCertificate`,
`InvalidChargePointCertificate`, `RevokedCertificate`,
`FailedAuthenticationAttempt` → Critical; `ConfigChange`, `FirmwareUpdate`,
`MasterKeySet` → Notable. An UNKNOWN type classifies
`Unclassified` — explicitly named, never guessed as benign, never guessed
as critical. An empty event type is refused. Ingestion triggers NO
automatic revocation, teardown, or other reaction — reactions are
operator/policy decisions (the record is the input to those decisions,
never the act itself).

## 6. UNVERIFIED register (implementation-time checks)

| # | Detail | Check |
|---|---|---|
| 1 | The security-block action spellings (SignCertificate, CertificateSigned, InstallCertificate, SecurityEventNotification, GetInstalledCertificateIds, DeleteCertificate) | OCA OCPP 2.0.1/2.1 security sections |
| 2 | The certificate-type strings (V2GRootCertificate, V2gCertificate, CSMSCertificate, ManufacturerCertificate) | OCA text [S-127] |
| 3 | The security-event type names and their severities | OCA Security Operations Guide [S-127] |
| 4 | The CSR/chain/certificate field encodings (base64, PEM, DER) | OCA text |
| 5 | Profile numbering semantics (1/2/3) | [S-126] / OCA text |

A mismatch is a code or record edit, never a silent reinterpretation
(plan §7.5).

## 7. Honest boundaries

1. No certificate is issued, validated, or verified in this program; no CA
   exists; no key material enters the repository.
2. No security claim of any kind: the Profile-3 posture is a design
   requirement for the written-unrun adapter, not an achieved state.
3. No charge point connected, no security event ever ingested from a real
   device; the ledgers are model surfaces exercised only by written-unrun
   tests.
