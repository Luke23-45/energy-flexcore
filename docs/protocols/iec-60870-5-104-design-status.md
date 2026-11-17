# IEC 60870-5-104 Client — DESIGN-STATUS Document

| Field | Value |
|---|---|
| Scope | The IEC 60870-5-104 EU DSO telecontrol SURFACE as DESIGNED — this is a design-status document, NOT an adapter guide: **no adapter exists** |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/41_p14_09_iec_60870_5_104_client_design.md` |
| Implementation status | **DESIGN RECORD ONLY.** No crate exists (`crates/adapters/protocol-104/` is a PROPOSED path, not created, not a workspace member, not in the dependency checker). The design lands as code only in the batch that implements the first NAMED EU DSO integration — the P14-E-3 DEFAULT |
| Conformance status | **No conformance, certification, compliance, or security claim of any kind.** No certification program is named in the repository's source register (UNVERIFIED-ABSENCE — evidence plan, records 101 §2) |
| Control status | Supervisory relay only (in the design): a telecontrol command arriving from the DSO would enter the frozen 14-state command lifecycle at PROPOSED — never an issued setpoint |
| Why this document exists | P14.23 requires operator documentation per protocol adapter; for the two design-record protocols the honest form is a DESIGN-STATUS document that states clearly what does NOT exist yet |

## 1. The designed surface (all details UNVERIFIED secondary-source readings)

| Element | Design reading | Verification gate |
|---|---|---|
| Role | FlexCore as the 104 CLIENT (controlling station); an EU DSO's SCADA would be the server | the named DSO integration defines the actual profile |
| ASDU structure | Common address, information objects, cause of transmission, originator address | exact field layout and COT vocabulary against the IEC 60870-5-104/-101 texts |
| IOA / point list | The DSO's point list assigns per-point addresses; the DSO's list is authoritative, conflicts refused | the DSO's allocation per named integration |
| Interrogation | C-IC full static dump → `InterrogationSnapshot` versioned by transaction (a partial response never presents as complete) | the interrogation framing and termination indication |
| STARTDT/STOPDT, k/w windows | The data-transfer handshake and sequence windows as DECLARED parameters | timeout semantics (t0/t1/t2/t3 — values UNVERIFIED) |
| Timestamps | CP56Time2a byte layout | layout, resolution, flag semantics |
| Port | 2404 by CONVENTION — declared per deployment, never a default | the DSO's actual addressing |

## 2. Security posture (the no-native-encryption reality — design input, NOT a posture statement)

IEC 104 has NO native message encryption or authentication (secondary-source
reading, UNVERIFIED against the standard text). The compensating-controls
DESIGN the future implementation must require, per deployment:

| Control | Design requirement |
|---|---|
| Network-layer protection | The 104 hop runs only on a dedicated closed telecontrol segment or a site-to-site VPN/IPsec tunnel terminated OUTSIDE the future crate; the transport refuses any configuration without a declared network-protection profile |
| Perimeter isolation | No inbound listener on the FlexCore side (the client dials out) |
| IEC 62351 assessment | A human-gated security review of the DSO's 62351 requirements before the first session (exact applicable parts UNVERIFIED) — never a conformance claim |
| Monitoring | Session-establishment and unexpected-connection events land in the P11 taxonomy |
| Point-list authority | The IOA allocation and permitted type IDs come from the DSO's signed point list per deployment |

## 3. What does NOT exist (the honest inventory)

| Item | Status |
|---|---|
| `crates/adapters/protocol-104/` | NOT CREATED — proposed path only |
| Workspace member / checker entry | NONE |
| Any 104 code, model type, or test | NONE |
| Any TCP session, ASDU, or DSO contact | NONE — no DSO was contacted, no message was exchanged |
| Certification path | NONE named (UNVERIFIED-ABSENCE; evidence plan, records 101) |

## 4. What would start it

The named EU DSO integration (counterparty, point list, security
requirements) closes P14-E-3; the implementing batch then follows the
crate pattern of records 01/21/23/24/43/44 with record 41 §3–§5 as the
design and its UNVERIFIED register as the first gate. Until then this
surface is design-only, and every statement above is a statement about
a DESIGN, never about behavior.
