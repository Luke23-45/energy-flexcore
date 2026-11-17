# P14.03 — IEEE 2030.5 security: design + operations record (CA chain, SFDI/LFDI, certificate lifecycle, trust anchors)

| Field | Value |
|---|---|
| Subtask | P14.03 — CA-chain validation, SFDI/LFDI handling, certificate lifecycle (issuance/rotation/revocation), trust-anchor management as a design + operations record [S-7][S-8][S-9]. Explicitly RECORD-ONLY: **no CA is stood up**, no certificate is issued, no key material exists; real PKI ties to Phase 13 (secrets/Vault) and Phase 18 (device identity) |
| Wave / agent | Phase 14 wave 1, Agent A |
| Status | DESIGN RECORD ONLY. No security component is implemented by this subtask; the only code anchor is the fail-closed `MtlsRequirements` type already written by P14.02 (UNRUN). Every protocol-security detail below is cited from SECONDARY sources and is UNVERIFIED against the IEEE 2030.5-2018 text and the CSIP Implementation Guide. |
| Sources | [S-7] dersec.io 2030.5 compliance summary, [S-8] Cogito Group IEEE 2030.5 CA types, [S-9] SunSpec CA Rule 21 PKI primer (gap analysis §11); [S-1] IEEE 2030.5-2018, [S-3] CSIP Implementation Guide v2.1; `crates/adapters/protocol-20305/src/server.rs` (`MtlsRequirements`); Phase 13 secrets surface (phase-13 records); Phase 18 device-identity roadmap (phase-18 plan) |
| Files touched | this record only (plus the P14.02 crate files it references — unchanged by this subtask) |

## 1. Why this is a record and not code

PKI operations — standing up CAs, issuing certificates, guarding root key
material in HSM-class custody, running revocation infrastructure — are
OPERATIONAL capabilities exercised by people and processes, not code an
agent can write honestly in this program. The gap analysis places "PKI
operations (2030.5 CA hierarchy)" in the standards-obligations table with
Phase 14/17/18 as the affected surfaces; the phase boundaries (context §5
boundary 6) explicitly forbid standing up a CA here. What Phase 14 CAN
honestly deliver is the design + operations record below: what the 2030.5
security model requires, what FlexCore will consume, who owns what, and
which checks are UNVERIFIED.

## 2. The 2030.5 security model as FlexCore consumes it (design)

| Element | Design position (secondary-source basis, UNVERIFIED) | FlexCore consumption |
|---|---|---|
| Mutual TLS | The AggregatorServer requires client certificates on every device session; server certificates authenticate the aggregator to devices [S-1][S-3][S-7] | `MtlsRequirements` (P14.02) makes client-cert enforcement STRUCTURAL — the type has no field that could disable it; construction with an empty trust-anchor set is an error |
| CA chain | Device certificates chain to a device CA under a utility/program root; the server validates the chain to its configured trust anchors [S-8][S-9] | Chain validation is transport-seam work (record 02 §5) against the DEPLOYED anchor set; FlexCore stores anchor LABELS, never key material |
| SFDI / LFDI | LFDI is a long-form device identifier derived from the device certificate; SFDI is the short form used for enrollment and addressing [S-8][S-9] — **the exact derivation, hash input, and bit lengths are UNVERIFIED** | `model::Sfdi`/`model::Lfdi` are shape-only validated value types; the platform never infers an SFDI; the mapping LFDI↔SFDI is a security-component concern (Phase 18), not a model concern |
| Device enrollment | A device presents its certificate; the server derives its identifiers and enrolls the EndDevice resource [S-3] | Enrollment is operator-mediated in FlexCore's design (the deployment approves the device), never an unattended write into the tree |
| Revocation | The server checks revocation status before accepting a session [S-9] | `MtlsRequirements.check_revocation` DEFAULTS `true` (REGISTERED RECOMMENDATION, fail-closed): an unverifiable status is a rejection, not a pass. The check itself is transport-seam work |
| Session security | TLS protects the resource exchanges; FlexCore adds no second crypto layer | Nothing in the adapter crates implements crypto; production-grade crypto in new security surfaces is a DEPLOYMENT REQUIREMENT (conventions §6.6) — test-grade digests are never claimed as release signatures |

## 3. UNVERIFIED register (the implementation-time checks this record owns)

Every item below is the named human-gate check before any security-bearing
implementation work:

1. **LFDI derivation** — the certificate-field input, hash algorithm, and
   bit length; and the SFDI shortening rule (digits, checksum) [S-8][S-9].
   The model types' recorded shapes (lowercase hex / non-zero decimal)
   are placeholders pending this check.
2. **Certificate profile** — required extensions, key usages, and the
   validity rules the 2030.5 security profile demands of device and server
   certificates [S-7][S-9].
3. **CA hierarchy shapes** — how many CA tiers the utility/program
   hierarchies use, and which tier issues device vs server certificates
   [S-8] (the secondary material names device CA / utility CA / root roles;
   the exact structure is program-specific and UNVERIFIED).
4. **Revocation mechanism** — CRL vs OCSP in 2030.5 practice, and the
   fail-closed semantics the standard actually requires [S-9].
5. **DNS-SD + security interplay** — whether discovery announcements carry
   security metadata (record 02 §3 owns the service-name check).

## 4. Certificate lifecycle (operations design, owned by humans)

The lifecycle below is the OPERATIONS design FlexCore's deployment runbooks
(P14.23, later) will exercise; none of it exists today and none of it is
claimed exercised.

| Stage | Operation | Owner | FlexCore surface |
|---|---|---|---|
| Root / device-CA standup | Create the program's CA hierarchy; custody root keys (HSM-class) | Human PKI operations (deployment org) | None — outside the platform |
| Trust-anchor publication | Distribute the anchor set the aggregator validates against | Human PKI operations | `MtlsRequirements.trust_anchors` LABELS, composed at deployment |
| Device issuance | Issue device certificates at enrollment | Human PKI operations + Phase 18 device identity | The platform consumes the issued identity; it never issues certificates |
| Rotation | Reissue server/device certificates before expiry; update anchors on CA rotation | Human PKI operations; Phase 18 rollout | Composition-root reconfiguration (anchor labels); no runtime mutation of the tree |
| Revocation | Revoke compromised device certificates; propagate status | Human PKI operations | The fail-closed revocation check at the transport seam |
| Secrets custody | Store server keys and anchor material | Phase 13 secrets surface (the P13 integration) | FlexCore reads secret REFERENCES, never raw material — `secret.read` remains in the never-granted set |

**The never-granted permission set is untouched**: `dispatch.authorize`,
`dispatch.issue`, `secret.read`, `network.any` stay byte-identical
(conventions §6.4). The security design gives adapters no new permissions —
mTLS posture is a transport property, not a platform permission.

## 5. Ties to Phase 13 and Phase 18

- **Phase 13 (enterprise identity)**: the secrets/Vault integration and the
  machine-identity work are where server key custody and secret references
  live. The security record consumes Phase 13's surfaces; it adds none.
- **Phase 18 (device identity / fleet lifecycle)**: LFDI/SFDI derivation,
  device-certificate issuance hooks, and rotation/revocation automation are
  Phase 18 deliverables. This record registers the UNVERIFIED checks (§3)
  so Phase 18 inherits named work, not silent assumptions.

## 6. ASSUMPTION / status register

1. **NOT-STARTED** — every operation in §4. No CA exists, no certificate is
   issued, no anchor is published, no key material is stored.
2. **UNVERIFIED** — every secondary-source security detail (§2, §3);
   verification against the IEEE 2030.5-2018 text, the CSIP Implementation
   Guide, and (where applicable) a lab Edge is the implementation-time
   human gate.
3. **UNRUN** — the `MtlsRequirements` code anchor (P14.02) and the whole
   0–10 base (conventions §7.1).
4. **No security or certification claim** — nothing here states or implies
   that FlexCore is secure, compliant, or certifiable; the record designs
   what operations WOULD have to exist (conventions §7.3, §7.4).

## 7. Handoff notes

1. **P14.19 (evidence plan)**: add the 2030.5 security-evidence items
   (chain-validation test vectors, revocation behaviour) to the lab list.
2. **Phase 18**: inherit §3's UNVERIFIED checks as the device-identity
   sub-phase's verification list.
3. **Transport implementer (record 02 §5)**: `MtlsRequirements` is the
   contract; §2 and §4 are the design context; §3 is what you must verify
   against the standard text BEFORE implementing chain validation.
