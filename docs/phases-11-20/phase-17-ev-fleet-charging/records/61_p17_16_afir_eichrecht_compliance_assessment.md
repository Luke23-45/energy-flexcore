# P17.16 — AFIR/Eichrecht compliance assessment: the depot fleet behind a site meter is ADJACENT, not directly AFIR-scoped — the EU hooks, the verdict, and the conditions that would change it

| Field | Value |
|---|---|
| Subtask | P17.16 (AFIR/Eichrecht compliance assessment record — EU hooks: MeterValuesSign, ad-hoc payment awareness, price-per-kWh display adjacency; the scope statement with the conditions that would change the verdict [S-135][S-136]) |
| Wave / agent | Phase 17 wave 2, Agent B (P17.16–P17.18) |
| Deliverable class | RECORD-ONLY (plan §1: "assessment and decision records where the honest deliverable is the analysis"). No code, no crate, no test file belongs to this subtask. |
| Status | ASSESSMENT RECORD — an engineering scope assessment of written artifacts, not legal advice and not a compliance claim. **No metrology, calibration, conformance, certification, or safety claim is made anywhere in this record.** No charge point is connected; no meter value was ever observed; no signed meter value was ever received. |
| Sources | [S-135] AFIR — Regulation (EU) 2023/1804 on the deployment of alternative fuels infrastructure (EUR-Lex consolidated text, CELEX 02023R1804, the consolidation version dated 2026-01-08 per the gap-analysis §11 entry); [S-136] Eichrecht guide (elvo.io / ampeco.com). Both are cited from the program's recorded source list and were NOT consulted page-by-page in this assessment: **every clause-level regulatory statement below carries an UNVERIFIED label naming the re-verification against the primary texts at use time** (the no-hallucinated-facts rule, AGENTS.md §3.1). |
| Files touched | This record only. |

## 1. What the program's sources actually record (the evidence base)

| Fact | As recorded in this program | Source |
|---|---|---|
| AFIR is Regulation (EU) 2023/1804 on the deployment of alternative fuels infrastructure | gap analysis §11 Track F1 entry; the EUR-Lex URL is a CONSOLIDATED CELEX text whose consolidation date (2026-01-08) is itself recorded data — re-verify at use | [S-135] |
| Eichrecht governs the German metering/calibration law surface for billing-grade EV charging (calibrated meters, signed meter values as the integrity mechanism) | gap analysis §11 Track F1 entry; phase-17 context §4 glossary ("MeterValuesSign — the data hook that keeps Eichrecht/AFIR-caliber metrological integrity addressable") | [S-136] |
| MeterValuesSign is OCPP 2.1's signed-meter-value mechanism, keeping the metrological surface ADDRESSABLE without claiming compliance | phase-17 context §1, §4; P17.07 record 22 §3 | [S-135][S-136][S-125] |
| AFIR and Eichrecht obligations attach to PUBLIC charging and its metering/billing chain | the gap analysis §7 framing ("AFIR/Eichrecht data hooks … keep the EU metrology surface addressable without claiming compliance"); the specific actor classes and obligations are UNVERIFIED against [S-135]/[S-136] primary texts | [S-135][S-136], UNVERIFIED |

The AFIR actor classes, the exact scope of "publicly accessible recharging points", the ad-hoc payment requirement details, and the price-display obligation details are **UNVERIFIED against the consolidated [S-135] text** — this assessment's verdict is constructed so that those UNVERIFIED details cannot flip the verdict for the in-scope depot case (§3), but the conditions in §5 name exactly which verified fact would be needed at revisit time.

## 2. What phase 17 actually built at the EU-adjacent surface (the three hooks, assessed)

| Hook | What exists (written-unrun) | What the assessment records | What is NOT claimed |
|---|---|---|---|
| **MeterValuesSign** | P17.07's `SignedMeterValueRecord` + `SignedMeterValueSink` (`crates/flexcore-application/src/ev_meter_values_ingest.rs`; record 22): the signed value arrives OPAQUE (verbatim payload, never parsed), is recorded with provenance, forwarded to a sink; a failed write is a named `Err` | The seam keeps the metrological surface ADDRESSABLE: a deployment whose compliance chain requires signed meter values has an ingestion path that transports them verbatim without destroying evidence. Signature verification is a metrological (calibration-body / lab) process — a HUMAN GATE this platform never performs (record 22 §3.2) | No Eichrecht conformity, no calibration status, no metrological integrity claim — the platform neither verifies nor certifies anything; the hook transports opaque evidence |
| **Ad-hoc payment awareness** | Nothing — deliberately. No payment data model, no payment processing, no terminal integration exists anywhere in phase 17 (or phases 11–16: the P16 billing surface is revenue-share BETWEEN business parties, not end-customer charge-point payment) | AWARENESS only: the domain never assumes payment-gated charging; no phase-17 surface requires, produces, or consumes a payment instrument; if public charging operation ever enters scope, end-customer ad-hoc payment is NEW scope with its own record (the AFIR payment requirement classes themselves — card terminal, contactless, EMV — are UNVERIFIED against [S-135]) | No payment-capability claim; no PCI-class claim; no statement that any deployment satisfies an ad-hoc payment obligation |
| **Price-per-kWh display adjacency** | Nothing driver-facing. The phase-17 surfaces are operator-facing (depot views, portfolio views); no price display exists at any charge point | ADJACENCY: price-display obligations attach to the public charging context (the CPO/eMSP side of a deployment); FlexCore in the in-scope depot case displays internal operator analytics, not AFIR-class public price displays. If a deployment ever surfaces per-kWh prices for public display from FlexCore-sourced data, FlexCore is a DATA PROVIDER to the obligated party, and that surface enters as a new recorded scope with its own assessment | No claim that any FlexCore view satisfies or substitutes for a price-display obligation; no tariff computation exists in phase 17 |

## 3. The verdict: ADJACENT, not directly AFIR-scoped

**Scope statement.** The phase-17 deliverable is a depot fleet behind a site meter:
the vehicles charge on private/depot infrastructure operated for the fleet
owner's own purposes; the grid connection is metered at the SITE level (the
site transformer limit, the P14.15 DOE envelope, and the depot import cap are
all site-metered quantities — records 26, 22). FlexCore's role is supervisory
orchestration of that fleet (ADR-010; the command-relay-only rule).

The AFIR-charged actors are the operators of publicly accessible recharging
points and the metering/billing chain serving end-customer charging — roles
the in-scope depot deployment does not play, and which FlexCore (a software
orchestration layer, never a site operator or billing party) cannot play by
itself. Eichrecht's calibration obligations attach to the metering chain used
for billing-grade public charging; a depot fleet billing internally behind a
site meter is outside that chain. **Verdict: ADJACENT** — the EU surfaces are
kept addressable by the three hooks above so that no phase-17 design decision
FORECLOSES a future compliant deployment, but no phase-17 artifact is
AFIR/Eichrecht-scoped, and none claims to be.

Two structural facts make the verdict robust against the UNVERIFIED
clause-level details (§1): (1) the verdict's subject (the depot fleet behind
a site meter) is defined by the program's own scope, not by a regulation
clause; (2) the hooks claim only ADDRESSABILITY (transport of opaque
evidence), never compliance — so no clause reading can make a phase-17
artifact non-compliant, and none can make it compliant.

## 4. What this assessment is NOT

1. **Not legal advice** — it is an engineering scope assessment using the
   program's recorded sources; the deployment owner's compliance review is a
   human gate entirely outside this program.
2. **Not a metrology claim** — nothing here states or implies calibrated
   meters, verified signatures, or compliant billing. The verification of a
   signed meter value never happens in this platform (record 22 §3).
3. **Not a per-deployment verdict** — the verdict covers the phase-17 scope
   (depot fleet, site meter, own-fleet operation). Any named deployment
   re-runs this assessment against its actual roles (§5).

## 5. The conditions that would change the verdict

Any ONE of these flips the verdict from ADJACENT to (at least partially)
directly-scoped, and each names what it triggers:

| # | Condition | What changes | What it triggers |
|---|---|---|---|
| 1 | A FlexCore deployment operates publicly accessible recharging points (a CPO role) | The infrastructure itself enters AFIR's scope (UNVERIFIED against [S-135] which actor classes and which accessibility thresholds apply) | A new scope assessment record; the P17.17 OCPI/OICP revisit trigger fires simultaneously (the same condition); ad-hoc payment and price-display move from awareness to delivery surfaces |
| 2 | FlexCore (or a deployment using it) becomes party to per-kWh END-CUSTOMER billing for charging sessions — e.g., relaying MeterValues/MeterValuesSign into an external billing chain for public charging | The data FlexCore produces feeds a metering/billing chain; the metrological requirements attach downstream but the evidence-integrity expectations flow upstream into the transport path | A recorded data-quality/evidence-integrity assessment of the relay path; the SignedMeterValue sink's consumer contract gets a compliance-chain review; no metrology claim ever moves INTO FlexCore |
| 3 | A member-state deployment's compliance chain depends on data FlexCore produces or relays (FlexCore as an evidence/data provider into a metering or billing chain) | The same class as #2, member-state-specific (e.g., a German-market chain per [S-136]; the exact calibration-law data requirements are UNVERIFIED against the primary text) | The same treatment as #2 plus a jurisdiction record |
| 4 | A regulatory change or new consolidation extends AFIR-type obligations to private/depot infrastructure of the kind FlexCore orchestrates | The subject itself moves into scope | Re-verification against the then-current consolidated [S-135] text (the recorded consolidation date is 2026-01-08 — recorded data with the re-verify-at-use rule); a new assessment record |

What does NOT change the verdict: internal fleet cost allocation, depot
energy management, site-limit enforcement, scheduling, or the platform's
operator-facing analytics — none of these is public charging, end-customer
per-kWh billing at public points, or a price display at a public point.

## 6. Honest boundaries

1. RECORD-ONLY deliverable (plan §1): the analysis is the deliverable; no
   implementation exists or is implied.
2. Every clause-level AFIR/Eichrecht statement is UNVERIFIED against the
   primary texts [S-135][S-136]; the re-verification is an
   implementation/deployment-time check, and a mismatch is a recorded edit
   of this verdict, never a silent reinterpretation (plan §7.5).
3. No metrology, calibration, certification, conformance, or safety claim
   anywhere in this record; the P17.07 hooks claim addressability only.
4. The verdict holds for the phase-17 scope as written; conditions §5.1–§5.4
   are the recorded triggers for reassessment — none of them is currently
   met (no site is named, no deployment exists, no public CPO network is
   operated, no billing chain consumes FlexCore data).
