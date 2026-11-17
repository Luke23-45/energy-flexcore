<!-- P19.04 — Zone/conduit deployment architecture documentation (IEC
     62443-3-2/3-3 posture). HONEST RECORD: a reference deployment
     architecture DOCUMENT — extending the frozen P0.17 five-zone model,
     never editing it. All SL targets are RECOMMENDATION, never achieved
     posture. The 3-2/3-3 requirement sets are REQUIRES-STANDARD-TEXT
     (research covers [S-22] ISASecure, not the 3-2/3-3 texts). The
     not-a-certification discipline of P10.25 §7 (§8) carries verbatim. -->
# P19.04 — Zone and Conduit Deployment Architecture Documentation (3-2/3-3 posture; reference zones + conduits; SL targets RECOMMENDATION only)

Date: 2026-11-17. Subtask: P19.04 (`ledger_phase_19.md` §P19.04).
Plan trace: `../plan.md` §2 row P19.04, §3 blocking order row 3 (consumes
the P19.03 component placement by path). Sources: [S-22] (ISASecure /
62443 scheme family; NOT the 3-2/3-3 standard texts); the frozen P0.17
threat-model zones and topology reinforcement
(`ledger/phase-00-product-freeze/17_threat_model.md` §3, §7); the P19.03
component record (`03_p19_03_iec_62443_4_2_component_mapping.md`).

## 1. Honest status — the plain statement

**No deployment exists, so no zone or conduit is operating anywhere, and
no SL is achieved.** This record is a REFERENCE deployment architecture
documentation: the zones a FlexCore deployment would partition into (§3),
the conduits with their documented flows (§4), and recommended SL targets
(§5) — all extending the frozen P0.17 five-zone model by composition, and
none of it contradicting or editing that freeze. The 3-2 zone/conduit
method and the 3-3 system requirement sets are **UNVERIFIED /
REQUIRES-STANDARD-TEXT** ([S-22] covers the ISASecure scheme family, not
the 3-2/3-3 texts); every requirement reference below is a skeleton label
awaiting the standard texts.

## 2. The frozen base this record extends (never re-decides)

From `ledger/phase-00-product-freeze/17_threat_model.md` §3 (cited, not
restated row-for-row): the five zones (Enterprise IT; Central application;
Customer integration/DMZ; OT/site control; Field/device), the inter-zone
flow rule (permit only documented flows; no direct database, NATS, or
arbitrary plugin network access from a field-device network), and the
deployment-topology reinforcement (§3.3: the site connection initiates
outward from the customer edge; outbound mTLS carrying approved commands
only; no cloud-to-device inbound access into OT networks; a customer
architecture may add a DMZ, unidirectional gateway, jump host, or other
approved pattern). **A zone diagram permitting cloud-to-device access
contradicts the freeze — this record adds none.** The edge trust boundary
(§7 of the freeze) is carried unchanged.

## 3. Reference deployment zones (extending the P0.17 model)

The P0.17 five zones map onto three deployment aggregation tiers; the
five-zone model is unchanged, this is the deployment-view grouping:

| Reference zone (deployment tier) | P0.17 zone(s) it realizes | Contents | Hosted by | Zone-type label (3-2 skeleton, UNVERIFIED) |
|---|---|---|---|---|
| Z-D — Cloud control plane | Zone 1 (Enterprise IT) + Zone 2 (Central application) | Operator console, control plane, database, event broker, plugin host, compute workers, identity integration | Cloud (deployment-operated) | IT/EAS zone (label UNVERIFIED) |
| Z-E — Edge OT zone (boundary) | Zone 3 (Customer integration/DMZ) | Edge agent host, approved gateways, local broker/journal storage | Customer site | Local/supervisory zone at the IT/OT boundary (label UNVERIFIED) |
| Z-S — Site devices | Zone 4 (OT/site control) + Zone 5 (Field/device) | Site EMS, SCADA, PLCs, local controllers; inverters, batteries, EVSE, meters | Customer site, deployment/customer-owned | OT/process zone (label UNVERIFIED) |

Zone discipline carried from the freeze:

- FlexCore components live in Z-D and Z-E ONLY. Nothing FlexCore ships
  installs into Z-S as a control component; site devices remain
  deployment/customer-owned (the P19.03 scope note).
- Z-E is the trust boundary: the edge agent authenticates each central
  session and validates every command; the central platform never assumes
  an issued command was applied without acknowledgement and telemetry
  evidence (freeze §7).

## 4. Conduits — documented flows only

Every conduit below is a DOCUMENTED flow in the freeze's sense; any flow
not listed here is not permitted by this architecture, and the list is
extended only by a reviewed change to a deployment's own documentation.

| Conduit | Direction | From → To | Transport / mechanism | Flow content | Guards (citation-backed design) |
|---|---|---|---|---|---|
| K-1 | Z-D → Z-D (internal) | Console/API clients → control plane | TLS, OIDC-authenticated | Operator actions, portfolio views | Identity federation + RBAC + step-up (P13 records; §17.3 controls 1–3) |
| K-2 | Z-E → Z-D | Edge agent → control plane | **Outbound-initiated mTLS** (the §3.3 reinforcement) | Telemetry uplink; approved-command receipt; enrollment/lifecycle | mTLS, signed/sequenced/expiring commands, acknowledgement separate from delivery (freeze §7; §17.3 controls 4, 6) |
| K-3 | Z-D → Z-E (logical) | Control plane → edge agent session | Over the K-2 outbound channel (no inbound listener into Z-E is assumed or documented) | Command issuance through the command lifecycle | Validation/feasibility/approval gates distinct; update manifests per P18.02 |
| K-4 | Z-S → Z-E | Site EMS/SCADA/PLC → edge agent | Site-local integration (protocol adapters, P14 — deployment-configured) | Telemetry read; supervisory setpoints through approved adapters | Approved-adapter rule (§17.5 invariant 6); adapter permission model (P14.16); no direct cloud reach into Z-S |
| K-5 | Z-S → Z-S | Site controllers → field devices | Site-owned industrial networks | Local control loops (NOT FlexCore traffic) | Outside FlexCore scope entirely; site-local safety independent of FlexCore (§17.5 invariant 10) |

No conduit permits: direct database/NATS/plugin network access from a
field-device network (freeze §3); unauthenticated device onboarding
(enrollment is operator-mediated — P14.03 §2); automatic third-party
network paths (plugins have no ambient network — §17.4 threat 2 controls).

## 5. SL targets — RECOMMENDATION, never achieved posture

Per-component SL recommendations come from the P19.03 record §5 (cited by
path: edge agent SL 2 RECOMMENDED, control plane SL 2 RECOMMENDED). Zone
targets, in the same RECOMMENDATION discipline:

| Zone | SL target (RECOMMENDATION) | Rationale (skeleton, UNVERIFIED) | Achieved |
|---|---|---|---|
| Z-D | SL 2 (RECOMMENDED) | Enterprise-hosted supervisory platform; moderate capability assumptions | **NONE** |
| Z-E | SL 2 (RECOMMENDED) | Boundary zone; highest exposure of the FlexCore surfaces | **NONE** |
| Z-S | Not set by FlexCore | Site devices are deployment/customer-owned; a zone SL is the deployment's 3-2 assessment outcome | **NONE — outside FlexCore's component scope** |

Every target cell is a RECOMMENDATION for a future 3-2 assessment to
confirm or revise; the actual per-zone SL follows the standard's
zone/capability definitions (REQUIRES-STANDARD-TEXT) and the deployment's
risk assessment. **No SL is achieved, evidenced, or claimed.**

## 6. What a real deployment must do with this document

1. The deployment owner instantiates Z-D/Z-E/Z-S with concrete hosts,
   networks, and the conduit implementations, producing a
   deployment-specific zone/conduit diagram — this record is the template.
2. Conduit additions beyond §4 are reviewed changes recorded in the
   deployment's own documentation (never silently added).
3. The 3-2/3-3 texts are obtained and the labels/method re-validated
   before any assessment engagement.

## 7. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

This is reference architecture documentation; it claims no zone
assessment, SL achievement, or conformance.

## 8. Reviewer-verification procedure (human)

1. Confirm §2: the P0.17 freeze file is untouched and no conduit here
   permits cloud-to-device inbound access into OT networks.
2. Confirm §3: FlexCore components are placed in Z-D and Z-E only.
3. Confirm §5: every SL cell reads RECOMMENDED with Achieved = NONE.
4. Confirm all 3-2/3-3 method/requirement labels are marked UNVERIFIED /
   REQUIRES-STANDARD-TEXT.

## 9. Status

**RECORD-ONLY / WRITTEN.** Reference documentation only; no deployment,
zone implementation, conduit, or SL exists or is claimed.
