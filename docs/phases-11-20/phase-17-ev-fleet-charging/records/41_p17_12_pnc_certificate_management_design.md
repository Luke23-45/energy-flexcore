# P17.12 — ISO 15118 Plug&Charge certificate management: PHASE-GATED design record (no implementation, no CA, no PKI)

| Field | Value |
|---|---|
| Subtask | P17.12 (ISO 15118 PnC certificate management design, PHASE-GATED, record-only depth): V2G Root CA trust store, OCPP certificate flows (Get15118EVCertificate, CertificateSigned, InstallCertificate, GetCertificateChainStatus for 2.1; block letters UNVERIFIED against the OCA text), OCSP validation; implementation GATED on PKI operations maturity (P14.03 interplay — no CA is stood up anywhere in this program) [S-128][S-130][S-125] |
| Wave / agent | Phase 17 wave 2, Agent A |
| Status | **RECORD-ONLY / PHASE-GATED.** No PnC code was written by this subtask; nothing is implemented, compiled, or run. No certificate exists, no key material exists, no CA is stood up, no OCSP query was ever made, no ISO 15118 session ever existed. Every protocol detail below is cited from SECONDARY sources and is UNVERIFIED against the ISO 15118 and OCA texts. |
| Sources | [S-128] ISO 15118-20:2022 (iso.org), [S-130] TRENDPOWER PnC guide, [S-125] ocpp.md OCPP 2.1, [S-127] OCA Security Operations Guide, [S-133] Argonne 2024 EVSE security report; P14.03 (the phase-14 2030.5 security record — the PKI-operations boundary); wave-1 record 03 (P17.03 Profile 3 onboarding — the certificate-flow states this design REUSES) |
| Files touched | this record only (the wave-1 security surfaces it references — `crates/adapters/protocol-ocpp/src/security.rs`, `model.rs` — are UNCHANGED by this subtask) |

## 1. Why this is a record and not code

PKI operations — standing up a V2G Root CA hierarchy, issuing contract
certificates, guarding root key material, running OCSP responders — are
OPERATIONAL capabilities exercised by people and processes, not code an
agent can write honestly in this program. The P14.03 record (§1) states the
boundary verbatim for 2030.5 and it transfers unchanged to ISO 15118: **no
CA is stood up anywhere in this program**; there is no PKI operator, no
HSM-class custody, no revocation infrastructure, and no contract-certificate
issuance chain. An implementation written now would be protocol code with
nothing to talk to — the phase-17 plan §1 "record-only depth" rationale.
What CAN be delivered honestly is the design below: what the PnC trust
model requires, what FlexCore would CONSUME, who owns what, and which
checks are UNVERIFIED.

## 2. The ISO 15118 PnC trust model as FlexCore would consume it (design, UNVERIFIED)

| Element | Design position (secondary-source basis [S-128][S-130]; UNVERIFIED against ISO 15118-2/-20 and the OCA text) | FlexCore consumption |
|---|---|---|
| V2G Root CA trust store | Contract certificates chain through a V2G root hierarchy (OEM sub-CA, mobility operator sub-CA); the EV validates the EVSE/CSMS chain and vice versa [S-130] | A trust-store RECORD surface carrying ANCHOR LABELS only (the P14.03 pattern: `MtlsRequirements`/`CsmsSecurityRequirements` carry labels, never material). The store's content is a deployment PKI-operations artifact |
| Contract certificate (the EV's) | Issued by the mobility-operator chain; presented by the EV during authorization for Plug&Charge [S-128][S-130] | NEVER held by FlexCore: the contract certificate lives in the vehicle. FlexCore records only that a PnC authorization OCCURRED (a record), never the credential |
| EVSE/CSMS certificates | The CSMS presents its chain; the EVSE holds its leaf [S-130] | The wave-1 `CertificateFlowLedger` states (REQUESTED → SIGNED_AVAILABLE → INSTALLED / REJECTED, record 03 §3) model the protocol positions; issuance between REQUESTED and SIGNED_AVAILABLE stays EXTERNAL |
| Get15118EVCertificate | OCPP message through which the charge point obtains/updates EV certificates on the EV's behalf (2.0.1 security-era message; 2.1 extends; exact spelling UNVERIFIED) [S-125][S-127] | DESIGN-ONLY: an inbound relay surface would record the request and route it to the deployment's PKI operations; this program implements no such routing. NOT an action of the closed vocabulary (no widening) |
| CertificateSigned / InstallCertificate | The wave-1 flows (record 03) — REUSED as-is; issuance stays EXTERNAL | Reused by citation; no code change |
| GetCertificateChainStatus | OCPP 2.1-era message for chain-status (OCSP-stapling-style) checks; exact spelling and semantics UNVERIFIED [S-125] | DESIGN-ONLY: a record that a status check was REQUESTED, plus the OUTCOME label; FlexCore never operates an OCSP responder |
| OCSP validation | Revocation status checked against an OCSP service; fail-closed semantics assumed (an unverifiable status is a rejection, not a pass — the P14.02/P17.03 DEFAULT) | The `check_revocation` DEFAULT-true discipline carries over; the actual OCSP call is deployment/transport-seam work against real responders — never this program's |

## 3. The gate: what would UNGATE implementation

The implementation gate is PKI OPERATIONS MATURITY, and it opens only when
ALL of the following exist (each is a human-gate input, recorded NOT-STARTED
today — mirroring the P14.03 §6 register):

1. **A named PKI operator** for the deployment's V2G Root CA hierarchy
   (commercial entity, contract, custody decision — HSM-class; program-
   level build-vs-buy, an E-decision for the human owner).
2. **Trust-anchor distribution** agreed: who publishes the anchor set the
   CSMS validates against, and in what artifact form.
3. **OCSP (or equivalent revocation) service** available to the deployment
   at the reliability the fail-closed posture requires — a fail-closed
   check against an unreachable responder denies every PnC session.
4. **The OCA-text verification pass** over every UNVERIFIED item in §4
   (and the Edition 2 publication-date conflict in
   `ledger/phase-06-edge-protocol/RESEARCH.md` §3 resolved, since the
   2.1-era messages' part assignments depend on it).
5. **D5 closed by the human owner** naming OCPP (and the edition) — the
   PnC design rides on the P17-E-1 edition baseline, which stays OPEN.

Until then: the design is a record, the wave-1 flow ledgers record protocol
positions with EXTERNAL issuance, and nothing else exists.

## 4. UNVERIFIED register (implementation-time checks this record owns)

| # | Detail | Check |
|---|---|---|
| 1 | The Get15118EVCertificate / GetCertificateChainStatus message names, field spellings, and request/response shapes | OCA OCPP 2.0.1/2.1 security sections + the Security Operations Guide [S-127] |
| 2 | The ISO 15118-20 certificate-installation/update service names and the PnC authorization flow steps | ISO 15118-20:2022 text [S-128] (no ISO text in the repository) |
| 3 | Which party performs the OCSP check (EVSE vs CSMS) and the fail-closed semantics the standards actually require | ISO 15118 text + OCA text [S-128][S-127] |
| 4 | The V2G root hierarchy's real tier structure (root / OEM sub-CA / MO sub-CA) for the target market | The market's PKI operator (does not exist yet — §3.1) |
| 5 | The contract-certificate lifecycle events FlexCore would record (installation, update, revocation notice) | OCA text + the named PKI operator's process |

A mismatch at any check is a code or record edit, never a silent
reinterpretation (phase-17 plan §7.5).

## 5. Honest boundaries

1. **No PnC implementation exists.** No ISO 15118 message was ever
   composed, sent, or parsed; no PnC session ever existed; the adapter's
   closed action vocabulary was NOT widened (Get15118EVCertificate and
   GetCertificateChainStatus are design names in this record, never
   `framing::Action` variants).
2. **No CA, no key material, no certificate.** The never-granted
   `secret.read` set is untouched; certificate surfaces carry LABELS only
   (record 03 §2, unchanged).
3. **No security or conformance claim.** The trust model in §2 is a design
   requirement statement, not an achieved or achievable-today state.
4. The block letters for the OCPP 2.1 certificate messages are UNVERIFIED
   against the OCA text (§4.1); nothing here cites a specific letter as
   fact.

## 6. Handoffs

1. **Wave-2 Agent B (P17.16/P17.18)**: this record is the PnC leg of the
   phase's gate inventory; P17-E-1's edition dependence is recorded in §3.4
   — the E-decision records must not imply the PnC gate is closable before
   it.
2. **P14.03 / Phase 18**: the §4 register extends the P14.03 §3 pattern to
   the ISO 15118 tier; Phase 18 (device identity) inherits both.
3. **The human gates**: §3's five conditions are the ungate checklist;
   none is started.
