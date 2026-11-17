# P14.09 — IEC 60870-5-104 client design: EU DSO telecontrol surface (RECORD-ONLY depth)

| Field | Value |
|---|---|
| Subtask | P14.09 — the IEC 60870-5-104 (IEC 104) client design as a RECORD: ASDU/IOA modeling, interrogation, and countermeasures for the no-native-encryption reality (compensating controls note: network-layer protection, IEC 62351 assessment). Plan §1 assigns RECORD-ONLY depth; plan §2 row P14.09; Track B (CESI via the [S-14] analogue) |
| Wave / agent | Phase 14 wave 2, Agent A |
| Deliverable class | **Design record only.** No crate was created — the engineering judgment call is recorded in §2 below, naming the proposed crate for the batch that lands it. WRITTEN-UNRUN throughout; no build, no test execution, nothing installed |
| Sources | Track B (CESI / the [S-14] DNP3 analogue — the IEC 60870-5-104 standard text was NOT consulted and is not in the repository); the P14.01 common port layer (`crates/flexcore-protocol-ports`); the wave-1 DNP3 record (24) as the structural template |
| Files touched | This record ONLY. No code, no Cargo.toml entries, no checker entries for P14.09 |

## 1. The protocol surface (all details UNVERIFIED secondary-source readings)

IEC 60870-5-104 is the EU DSO telecontrol standard: a TCP-based
telecontrol protocol (port 2404 by convention — UNVERIFIED) carrying
application frames (APCIs) whose application layer is the IEC
60870-5-101 ASDU structure over TCP. FlexCore's role on this surface is
the **CLIENT** (the controlling station): an EU DSO's SCADA would be the
server, and FlexCore connects out to it to exchange fleet telemetry and
receive telecontrol commands. All structural claims below are
secondary-source readings; the implementation-time check is a full pass
against the standard text (IEC 60870-5-104 and the 60870-5-101 ASDU
definitions) plus the DSO's interrogation/point-list documentation.

| Element | Design reading | UNVERIFIED check at implementation |
|---|---|---|
| ASDU structure | A traffic unit: common address, information objects, cause of transmission (COT), originator address, transfer reason | Exact field layout and COT vocabulary (spontaneous 3, interrogated 20, command acks 7/9, etc.) against the 60870-5-101 text |
| IOA (information object address) | The per-point address the DSO's point list assigns — the direct analogue of the DNP3 point-index discipline (record 24) | IOA width (8/16/24/32 bit — structure-variant dependent) and the DSO's allocation |
| Type IDs | Single point (M_SP), double point (M_DP), measured value scaled/short (M_ME_NC and family), single/double commands (C_SC/C_DC), setpoint commands (C_SE) | Exact type-ID numbering and which the DSO profile permits for DER points |
| CP56Time2a | The 7-byte timestamp carried by time-tagged ASDUs | Byte layout, ms resolution, the invalid/summer-time flag semantics |
| Interrogation | C-IC: the client requests a full static dump (class-0 analogue); the server responds with the full point set | The interrogation COT, the response framing (many ASDUs, one COT), and the termination indication |
| STARTDT/STOPDT | The data-transfer start/stop handshake before any measured value flows | Whether STARTDT ack is a usable liveness signal, and the timeout semantics (t0/t1/t2/t3 — values UNVERIFIED) |
| Sequence control | Send/receive sequence numbers inside the APCI; k/w window parameters | The exact k/w defaults and the reconnection behavior |

## 2. The engineering judgment: why NO crate lands in this batch (the P14-E-3 DEFAULT)

The subtask allowed a `crates/adapters/protocol-104/` crate **if model
types with real code justify it**. The judgment, recorded as required:
**they do not, yet.** Three reasons, each checkable:

1. **Record-only depth is the plan's own decision** (plan §1 lists
   P14.09 among the record-only subtasks), and the P14-E-3 DEFAULT (a)
   — registered in record 42 — is *design-only in Phase 14, implement on
   the first named EU integration*. No EU DSO integration is named; a
   protocol stack without a counterparty rots (plan §4, P14-E-3
   rationale).
2. **The ASDU/IOA vocabulary is unverified at the level the DNP3 crate
   tolerated.** Wave-1's DNP3 crate pinned object-group numbers that the
   [S-14] announcement corroborates structurally (a published standard
   with a DER profile); for 104, EVERY element in §1 — type IDs, COT
   numbering, IOA widths, timeouts — is a secondary-source reading with
   no corroborating anchor in the repository. Landing that as code would
   encode a vocabulary whose only reviewer would be a future
   implementation-time pass that will rewrite it anyway.
3. **The transport IS the protocol's hard part.** Unlike 2030.5/OpenADR
   (HTTP-shaped) or Sparkplug (topic-shaped, models separable), 104's
   value lies in the APDU framing, sequence windows, and timeout
   machine — exactly the part that stays a designed seam. A model-types
   crate would carry the least-verified, least-useful half.

**Consequence:** the proposed crate `crates/adapters/protocol-104/`
(package name `flexcore-protocol-104`, per the P14.01 layout table) is
NOT created, NOT added as a workspace member, and NOT added to the
checker. It lands in the batch that implements the first named EU DSO
integration (P14-E-3 closure evidence), following the wave-1 crate
pattern (scope declaration + manifest consuming
`flexcore-protocol-ports`; transport seam; UNVERIFIED register; written-unrun
tests). This record is the design it would implement.

## 3. ASDU/IOA modeling — the design the future crate implements

Mirroring the DNP3 outstation record (record 24) on the client side:

- **`DerPointKind` inventory** (the same fleet quantities as DNP3:
  active power, reactive power, SOC, availability, energy delivered,
  active-power setpoint) mapped to the client's IOA allocation — the
  DSO's point list is authoritative per deployment, conflicts refused.
- **`DeclaredScaling`** — canonical MW ↔ the point's scaled engineering
  unit, DECLARED per point, never inferred (the units.rs rule); reactive
  sign declared per the DSO's convention, never read from the sign alone.
- **`InterrogationSnapshot`** — the client's post-C-IC full point set,
  versioned by the interrogation transaction, so a partial response
  never presents itself as complete (the completeness-gate pattern).
- **Command arrival** — a telecontrol command arriving from the DSO
  enters the frozen 14-state command lifecycle at **PROPOSED**
  (const `entry_state()`; the record-24 §3 pattern), passing the three
  gates like any other candidate. The client never echoes an
  ACTCON/DEACTCON status that implies the platform executed anything —
  the relay path owns any response, after the gates.
- **Transport seam** — `LinkConfig` (address, port DECLARED per
  deployment — the 2404 convention is a configuration choice, never a
  default), the t0/t1/t2/t3 timeouts as DECLARED parameters
  (UNVERIFIED defaults), and the sync client-seam trait with an
  `UnwiredTransport` refusal.

## 4. No-native-encryption: compensating controls (the security note)

**The fact (secondary-source reading, UNVERIFIED against the standard
text):** IEC 60870-5-104 predates application-layer security and has NO
native message encryption or authentication. A 104 session on an
unprotected network is plaintext telecontrol. **No security claim is
made here** (AGENTS.md §3 rule 2; conventions §7.3) — this is a design
input to a security review, not a posture statement.

The compensating-controls design (what the future implementation record
must require, per deployment):

| Control | Design requirement |
|---|---|
| Network-layer protection | The 104 hop runs only on a dedicated, closed telecontrol network segment (or a site-to-site VPN/IPsec tunnel terminated outside this crate); the transport seam REFUSES any configuration without a declared network-protection profile — the same fail-closed shape as the MQTT TLS rule |
| Perimeter isolation | No inbound listener on the FlexCore side (the P6.07 structural rule — the client dials out); the DSO connection is the only route to the segment |
| IEC 62351 assessment | IEC 62351 defines security extensions for the 60870-5 family (e.g. TLS profiles for 104 per the 62351-3/-6 documents — UNVERIFIED exact part numbers); the implementation batch MUST record an assessment of the DSO's 62351 requirements before the first session — a human-gated security review input, never a conformance claim |
| Monitoring | Session-establishment and unexpected-connection events land in the P11 metric/log taxonomy (P14.21 consumes; never redefined) |
| Point-list authority | The IOA allocation and permitted type IDs come from the DSO's signed point list per deployment — a mutation outside that list is refused at the model layer |

## 5. UNVERIFIED register (the implementation-time checks)

1. Every §1 element (type IDs, COT vocabulary, IOA widths, CP56Time2a
   layout, STARTDT semantics, t0/t1/t2/t3 values, k/w defaults) against
   the IEC 60870-5-104 / -101 standard texts.
2. The IEC 62351 part numbers and TLS profile requirements applicable to
   104 (§4) against the 62351 texts.
3. The DSO's point list, permitted commands, and interrogation schedule
   per named integration (P14-E-3 closure evidence).
4. The port-2404 convention and the DSO's actual addressing.

A mismatch found at those gates is a recorded edit to the named design
point — never a silent reinterpretation (plan §7 note 6).

## 6. Honest boundaries

No 104 client exists, no crate was created, no TCP session was opened,
no ASDU was encoded or decoded, no DSO was contacted, no message was
exchanged, no interrogation was performed. No conformance, security, or
certification claim of any kind (conventions §7.3). The simulated
counterpart (a simulated 104 server) is wave 3 / P14.17's deliverable
surface, on the same simulator-first policy as every other adapter
(record 01 §6).

## 7. Handoff

- **Record 42 (P14.10)** registers P14-E-3 — the timing decision this
  design defers to (design-only in Phase 14; implement on the first
  named EU integration).
- **The implementing batch** (post-P14-E-3 closure): follow records
  01/21/23/24 for the crate pattern; this record §3–§5 is the design
  and the UNVERIFIED register to resolve first.
- **Wave 3 / P14.17**: the simulated 104 server names
  `crates/adapters/protocol-104/`'s transport seam as its exercise
  point when that crate exists.
