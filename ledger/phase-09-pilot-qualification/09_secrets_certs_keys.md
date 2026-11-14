<!-- P9.09 — Secrets, certificates, and key management. DESIGN RECORD only: inventory skeleton, references-only rule by citation, cert lifecycle/key-custody/revocation/compromise DESIGN. NOTHING installed, issued, rotated, scanned, or measured: every store, owner, expiry, and test-result cell is EMPTY/UNSIGNED. Test-grade FNV-1a-64 and missing-CSPRNG/SHA-256 positions carried honestly from P9.33/P9.34/P6.15. -->
# P9.09 — Secrets, Certificates, and Key Management (Design Record — nothing installed, issued, or measured)

Date: 2026-11-14. Subtask: P9.09 (`ledger_phase_9.md` §P9.09).
Owner role: Security / platform engineering. Report trace: §23 Stage 9;
§17.3; §18.1; §18.3. Inputs: P9.04 integration design (TEMPLATE);
P9.05 network design (TEMPLATE); customer secrets-manager / PKI
capability — **does not exist yet**; the prior records this design
carries forward: P9.33 (`33_edge_enrollment.md`), P9.34
(`34_signed_update_verification.md`), P9.31 (`31_edge_rejection_proof.md`),
P6.06 (`crates/edge-agent/src/enrollment.rs`), P6.15
(`crates/edge-agent/src/command_verify.rs`).

## 1. Honest status — the plain statement

**No secret, certificate, or key for any pilot exists; no store was
selected or provisioned; no certificate was issued, rotated, revoked, or
monitored; no rotation test ran; no repository scan ran; no revocation
propagation time was measured.** There is no CA, no HSM, no secrets
manager, and no pilot deployment — every operational cell below is
EMPTY/UNSIGNED by construction, not by omission. This record delivers
the DESIGN: the inventory skeleton with each item's required store
CLASSIFICATION (§3), the references-only rule the code already enforces
(§2, cited), the certificate lifecycle with issuance as a human/authority
gate (§4), the key-custody requirements (§5), the CSPRNG/SHA-256
deployment requirements carried honestly from P9.33/P9.34/P6.15 (§6),
and the compromise/revocation procedures as designs with the measurement
cells that the human executions must fill (§7–§8). Nothing here installs,
provisions, or generates anything.

## 2. The references-only rule (what the code already enforces — cited)

No secret value may live in source control, a plugin config file, or an
unencrypted store. The design rule is not new — the code enforces its
shape today:

| Rule | Enforced where (cited) |
|---|---|
| Secret references use EXACTLY the closed schemes `env:`, `file:`, `secret-store:` — a manifest is STRUCTURALLY INCAPABLE of carrying a secret value; a reference without a scheme prefix is rejected as a likely inline credential; error messages never echo the rejected value | `crates/flexcore-kernel/src/manifest.rs` — `SECRET_REFERENCE_SCHEMES` (P7.05), the enforced-subset header |
| Plugin configuration refuses a plain value in a secret-shaped field ("secrets are supplied as env:, file:, or secret-store: references and are never values in config (the value is not echoed)") | `crates/flexcore-kernel/src/plugin_config.rs` (`SecretRefScheme`; the plain-value rejection) |
| `secret.read` and `network.any` are NEVER-granted permissions, refused at the manifest gate — a plugin can never read secret material or open arbitrary network paths | `manifest.rs` `NEVER_GRANTED_PERMISSIONS`; §5.7 |
| The enrollment provisioning token is a redacted type (`Debug`/`Display` render `ProvisioningToken(<redacted>)`); the credential record carries the private key as a REFERENCE, never the value | `crates/edge-agent/src/enrollment.rs` (`ProvisioningToken`, `DeviceCredentialRecord`) |
| Logs carry no secrets (§21.3; §16.2) — secrets-bearing types render redacted | P6.06 redaction discipline; P9.33 §7 check 5 |

Recorded contract gap (P9.33 §6 finding 1, carried verbatim):
`DeviceCredentialRecord.private_key_reference` is `Option<PathBuf>` —
structurally a `file:`-scheme reference only; an edge whose key lives in
a platform secret store cannot be recorded without abusing a path string.
Widening it to a scheme-prefixed reference type is a Phase 7 contract
follow-up (owner: platform engineering). Until then, `secret-store:` on
the EDGE side is a design requirement the record type cannot yet express —
recorded, not fixed.

## 3. Secret and credential inventory (skeleton — store/owner cells EMPTY)

Ledger step 1–2. Each item names the scheme-class its store MUST use
(references-only rule, §2); which concrete store, who owns rotation, and
every expiry are deployment decisions — EMPTY. No item's store may be
"source control", "plugin config file", or an unencrypted file; the
design forbids all three and the scan (§9) verifies the absence.

| # | Item | Purpose (cited) | Store scheme class (design requirement) | Concrete store | Rotation owner | Expiry recorded |
|---|---|---|---|---|---|---|
| S1 | Edge mTLS private key + certificate | The edge identity on the uplink (§18.1; `uplink_mtls.rs`) | Hardware-backed where the platform supports it (§5); else `file:` under OS-level protection with a recorded compensating control | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S2 | Edge enrollment provisioning token | Single-use, ≤15-min bootstrap credential (P9.33 §1) | Out-of-band human delivery; consumed once at the platform (P6.06 issuer contract); never persisted after consumption | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S3 | Control-plane uplink server certificate(s) | The server side of the mTLS session the edge verifies against the pinned root (P9.06 U1) | `secret-store:` via the customer secrets manager / PKI | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S4 | Control-plane service identities (DB credentials, broker credentials) | §17.3 short-lived credentials; §16.2 runtime role | `secret-store:`; runtime role must not own tenant tables or bypass RLS (§16.2 — P9.10 confirms) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S5 | IdP client secret(s) | The OIDC/OAuth2 integration (P9.08 §8) | `secret-store:`; never in frontend code or logs | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S6 | Command-signing key(s) (edge side) | The P6.15 keyed command signature (`edge-sig:...`) — TODAY a keyed checksum, production cryptographic MAC = DEPLOYMENT REQUIREMENT (§6) | `secret-store:` / hardware-backed; delivery via the enrollment exchange only | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S7 | Update signer key(s) (platform release keys) | The P9.34 trusted-signer set — TODAY a lab string; the REAL set is composed at deployment from this inventory (P9.34 §7) | `secret-store:` / HSM per §5 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S8 | Adapter/EMS credentials (e.g. the OpenEMS API key) | Adapter-internal, never FlexCore's channel (P9.06 PB1/PB3); vendor defaults (`admin`/`admin`, `Metadata.Dummy`) must never be the pilot model | Customer EMS's own credential posture — site confirmation cell (P9.06 PB3) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| S9 | Plugin-provided secrets | Resolved by the host via P7.19 (`env:`/`file:`/`secret-store:`); the plugin never sees the value (§5.7) | The host's store integration (deployment composition) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 4. Certificate lifecycle (design; issuance = human/authority gate)

| Lifecycle stage | Design (and what already exists) |
|---|---|
| Issuance | A HUMAN/AUTHORITY GATE, never an automated edge act: no CA exists anywhere in the 0.x line — certificate issuance is the recorded P6.06 DEPLOYMENT REQUIREMENT (P9.33 §1: the edge generates nothing; the ceremony gates and records an exchange whose issuance side is still a requirement). The enrollment `Approve` transition is a platform human/authority gate with no self-approval path (P9.33 §2) |
| Delivery | The single-use token travels out-of-band (a human delivers it at install time); the token→credential exchange rides the outbound-only mTLS enrollment flow (P9.33 §1) |
| Rotation | Every certificate carries a RECORDED rotation owner and a recorded expiry (ledger acceptance (b)) — cells EMPTY (§3); rotation of an edge identity follows the re-enrollment discipline (§7.1), not an in-place key overwrite |
| Expiry monitoring | DEPLOYMENT REQUIREMENT, NOT IMPLEMENTED: §18.3 requires certificate expiry to be operator-visible alongside edge health; no expiry-monitoring code exists in the code base (recorded honestly); the monitoring design (thresholds, alert routing) is a P9.25/P9.07-linked reviewer cell: `<EMPTY>` |
| Revocation | Via the APPEND-ONLY REGISTERS, never a status overwrite: the edge-local `FileIdentityEventRegister` appends `CEREMONY_REVOKED` with the revoked term kept in history next to the term it ended (P9.33 §4); the PLATFORM-side identity registry is the authoritative lifecycle record and the revocation AUTHORITY (P9.33 §5 honesty row). Revocation propagation time is MEASURED, not assumed (§7.3) |
| Trust pinning | The edge verifies the central certificate against the root CA pinned FROM the credential record; an empty/unparsable store is a named fail-closed error (`uplink_mtls.rs`; P9.06 U1). Root rotation is therefore a re-credentialing event (enrollment ceremony), not a config tweak |

## 5. Key custody requirements (recorded — nothing installed)

| Requirement | Design position |
|---|---|
| Edge private key (S1) | Hardware-backed key storage (TPM/secure element/TPM-backed store) WHERE THE CUSTOMER PLATFORM SUPPORTS IT (§18.3; ledger step 4). Where it is NOT available: the compensating control is recorded per site — OS-level key-store protection with restricted permissions, plus the D6 review's acceptance — site determination cell `<EMPTY>`; no "hardware-backed: yes" claim may be made until the pilot hardware spec exists (the P6.34 posture: every hardware determination PENDING PILOT HARDWARE SPEC) |
| Platform signing keys (S7) | Held in the customer secrets manager or an HSM per the customer's PKI capability (input that does not exist yet); key ceremonies/dual control for release-key generation are the customer's recorded process: `<EMPTY>` |
| Command-signing keys (S6) | Distributed ONLY through the enrollment exchange; never embedded in images, configs, or repositories; per-site keys RECOMMENDED so one site's key cannot forge another site's commands — RECOMMENDATION label, decision cell `<EMPTY>` |
| What is explicitly NOT installed | No HSM, no vault, no KMS, no CA, no certificate — this record installs nothing; "nothing installed" is the honest status, not a plan skip |

## 6. CSPRNG / SHA-256 deployment requirements (carried honestly — nothing upgraded here)

The code today contains NO CSPRNG and NO SHA-256 (no crypto dependency
exists in the 0.x line); the cryptographic-grade replacements are
recorded DEPLOYMENT REQUIREMENTS, and no production hash is embedded:

| Site in code | What exists (test-grade, labelled) | Production requirement (deployment, not code) |
|---|---|---|
| Ceremony nonce derivation | FNV-1a-64 integrity-checksum pattern over versioned binding strings (P9.33 §3 — an INTEGRITY CHECKSUM, not a security primitive; an un-guessable nonce needs a CSPRNG) | CSPRNG-derived nonce under the deployment's reviewed crypto (P9.33 §8 item 3) |
| Update bundle digest | `TestOnlyFnv1a64UpdateVerifier`, loudly labelled; default gate `NoUpdateVerifier` refuses everything (P9.34 §2) | SHA-256-or-stronger verifier wired through the `UpdateVerifier` port under reviewed key management (P9.34 §10 item 2) |
| Command signature | P6.15 keyed-checksum scheme `edge-sig:...:fnv1a64:<digest>` — tamper-EVIDENCE, not tamper-PROOF (P9.31 §4) | Cryptographic signature (HMAC-SHA-256 or asymmetric) under reviewed key management (P9.31 §7 item 2) |
| Provenance digests | P7.14 digest-and-record seam; `sha-256` NAME accepted while no SHA-256 implementation exists in the kernel (P9.34 §8 finding 2 — inherited, honestly labelled) | The same reviewed-crypto requirement; the versioned scheme prefix makes each replacement a detectable format change |

This record does not implement any of these; it records that the pilot
MUST NOT run command signing, update verification, or nonce generation
on the test-grade primitives without the deployment explicitly accepting
the residual risk (a P9.07 finding — assessment cell EMPTY).

## 7. Compromise and revocation procedures (design; measurement cells EMPTY)

### 7.1 Compromised edge identity

1. `revoke(actor, reason)` appends `CEREMONY_REVOKED` to the
   `FileIdentityEventRegister` — append-only, the history survives a
   restart (P9.33 §4).
2. Re-enrollment is opened only on a fresh `PlatformAuthorityTicket`
   from the platform; the old (compromised) identity is refused as an
   authorization basis — `CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED`
   (P9.33 §4; the ledger's named risk is refused structurally).
3. The platform-side decision of whether a compromised DEVICE ID may
   ever re-enroll (vs the hardware re-registering under a new id) is the
   recorded P9.33 §6 finding 2 / §8 item 5 policy gap — decision cell
   `<EMPTY>` (owner: security + platform engineering).
4. Anything the compromised identity could reach (its scope is
   server-side fixed to one (tenant, site) binding — P9.06 U4) is
   reviewed for residual access; the review checklist cell `<EMPTY>`.

### 7.2 Compromised operator credential

1. Revoke at the customer IdP (session invalidation + credential
   revocation) — the mechanisms are the IdP's; FlexCore holds no local
   password store to revoke.
2. The authz surface denies by default once the resolver stops
   resolving the principal; no cached authorization exists to purge
   (decisions are evaluated per request, never cached in this crate).
3. Any commands the compromised principal APPROVED while compromised are
   reviewed for supersession/cancellation via the command lifecycle
   (`CANCELLED`/`SUPERSEDED`, §14) — procedure checklist cell `<EMPTY>`.
4. Break-glass continuation follows P9.08 §6 (dual authorization,
   time-boxed, audited).

### 7.3 Compromised signing key (edge command key or release key)

1. Edge command key (S6): revoke the edge identity (§7.1), re-enroll
   under a fresh key; in-flight commands signed by the old key fail the
   edge signature check from the moment the credential record changes.
2. Release key (S7): the trusted-signer set is updated on the platform;
   previously verified bundles remain activated in the registers (the
   P9.34 register is the source of truth for active version, not a
   trust decision re-evaluation); new bundles from the compromised key
   refuse with `UPDATE_UNTRUSTED_SIGNER`.
3. Propagation time for every revocation path is MEASURED at the pilot,
   never assumed (ledger acceptance (d)) — measurement cells:
   revocation-to-denied-session `<EMPTY>`, revocation-to-refused-command
   `<EMPTY>`, measured-by `<EMPTY>`.

## 8. Rotation test record (TEMPLATE — UNRUN human execution)

Ledger step 6: rotation is tested on ONE non-production identity
end-to-end BEFORE the pilot starts. Template for the record the human
execution fills:

| Field | Value |
|---|---|
| Identity rotated | `<EMPTY>` |
| Test environment (non-production) | `<EMPTY>` |
| Date / executor | `<EMPTY>` / `<EMPTY>` |
| Old credential invalidated and verified refused | `<EMPTY>` |
| New credential issued via the enrollment ceremony gate | `<EMPTY>` |
| Edge reconnected and verified against the new credential | `<EMPTY>` |
| Revocation propagation measured (§7.3) | `<EMPTY>` |
| Result (pass/fail + evidence reference) | `<EMPTY — UNRUN>` |
| Signature | **UNSIGNED** |

## 9. The no-secret-in-source scan (requirement; execution UNRUN)

Ledger acceptance (a) requires an automated scan of the pilot
repositories and config for secret patterns. Structural guarantees the
scan can rely on (cited, §2) and what it adds:

| Layer | Structural guarantee (already enforced) | Scan adds |
|---|---|---|
| Plugin manifests/config | Cannot carry secret values at all (`manifest.rs`, `plugin_config.rs` refusals) | Verification that deployments use only the three schemes |
| Edge credential records | References only; token types redacted (`enrollment.rs`) | Detection of real key material accidentally placed in config/journal paths |
| Repositories | No secret material introduced by any phase record (P9.31/P9.33/P9.34 §7 checks 5/6 assert redaction and lab strings) | The pilot-period scan cadence — owner and frequency cells `<EMPTY>` (links to P9.07 §11) |

The scan itself has NOT run (no pilot repositories/config exist to scan);
its result cell is EMPTY and no clean-scan claim is made.

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every concrete-store, owner,
   expiry, measurement, and result cell in §3–§9 is `<EMPTY>`/UNRUN/
   UNSIGNED.
2. Confirm the references-only rule is cited to the REAL code
   (`manifest.rs` `SECRET_REFERENCE_SCHEMES`, `plugin_config.rs`
   `SecretRefScheme`, `enrollment.rs` redaction) and that §2's recorded
   `private_key_reference` gap is present, not papered over.
3. Confirm §4 states issuance is a human/authority gate with no CA
   existing (the P6.06 deployment requirement stands) and no certificate
   claim is made anywhere.
4. Confirm §6 carries the test-grade FNV-1a-64 honesty verbatim per
   site (nonce / update digest / command signature) and that NO
   production hash or CSPRNG is claimed as implemented or embedded.
5. Confirm §7's compromise procedures cite the real named errors
   (`CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED`,
   `UPDATE_UNTRUSTED_SIGNER`) and that propagation time is recorded as
   MEASURED-not-assumed with EMPTY measurements.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test".

## 11. Filling commands (for the human executor)

- With the customer security/PKI owner: fill §3 store, owner, and expiry
  cells; record the §5 hardware-backed determination per pilot profile
  (or the compensating control with owner and expiry).
- Commission the certificate issuance path (the P6.06 deployment
  requirement) and record it in §4; define expiry monitoring and its
  operator-visible surface (§18.3) with P9.25.
- Execute the §8 rotation test on one non-production identity; measure
  the §7.3 propagation times; sign the record.
- Run the §9 scan against the pilot repositories and deployed config;
  file the result with the P9.09 evidence named in `CHECKLIST.md`
  ("Secrets and certificates managed").
- Raise the §2 reference-type gap and the §7.3 device-id policy decision
  into the Phase 7 contract list (P9.33 §8 items 1 and 5).

## 12. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No CA, no secrets manager, no HSM, no certificates exist — every §17.3/§18.1 secrets control is future deployment work | §1, §5 |
| Production crypto absent: command signing is a keyed checksum, nonce and update digests are test-grade FNV-1a-64 — the pilot must not run them as-if-secure without explicit acceptance | §6 |
| `private_key_reference` cannot express a `secret-store:` reference (P9.33 finding 1) — an edge key in a platform store is unrecordable until the Phase 7 widening | §2 |
| Certificate expiry monitoring is a requirement with NO implementation — "a certificate expiring mid-pilot with no monitoring" is the ledger's named risk until built | §4 |
| Revocation propagation is unmeasured (nothing to measure) — acceptance (d) is unmet until the pilot measures it | §7.3 |
| Compromised-device-id re-enrollment policy undecided (P9.33 §8 item 5) | §7.1 step 3 |
| Hardware-backed key custody undetermined (no pilot hardware spec); compensating control unrecorded | §5 |
| Vendor default credentials (OpenEMS `admin`/`admin`, `ws://`) remain a standing anti-pattern the site must affirmatively exclude | §3 S8; P9.06 PB3 |

Status: **WRITTEN-UNRUN** — a key-management design record cannot be
"run"; the store provisioning, certificate issuance, rotation test,
scan, and propagation measurements are the human gates and have not
happened and are not scheduled. Nothing was installed or generated; no
pilot exists and none is claimed.
