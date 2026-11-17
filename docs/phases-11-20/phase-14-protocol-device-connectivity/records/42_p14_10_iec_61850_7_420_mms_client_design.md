# P14.10 — IEC 61850-7-420 MMS client design: DSO-boundary surface (RECORD-ONLY depth) — P14-E-3 registered

| Field | Value |
|---|---|
| Subtask | P14.10 — the IEC 61850-7-420 MMS client design as a RECORD: MMS client only; SCL/ICD ingestion; logical-node mapping for DER control (DRCT, ZRCT, DRCC — node semantics UNVERIFIED until checked against the standard text); the explicit GOOSE/SV out-of-scope statement (protection-grade, foundations §0.3). Plan §1 assigns RECORD-ONLY depth; plan §2 row P14.10 |
| Wave / agent | Phase 14 wave 2, Agent A |
| Deliverable class | **Design record only.** No crate was created — the engineering judgment call is recorded in §2, naming the proposed crate. WRITTEN-UNRUN throughout; no build, no test execution, nothing installed |
| Sources | [S-12] (IEC 61850-7-420:2021 webstore catalog entry — the standard text was NOT consulted), [S-13] (NIST TN 2217 DER profile), foundations §0.3 (the protection-grade boundary); the P14.01 common port layer; record 41 (the sibling 104 design and its crate judgment) |
| Files touched | This record ONLY. No code, no Cargo.toml entries, no checker entries for P14.10 |

## 1. The protocol surface (all details UNVERIFIED secondary-source readings)

IEC 61850-7-420 extends the IEC 61850 data model (logical nodes) for
DER. The communication profiles of 61850 are MMS (client/server
reporting and control), GOOSE (fast peer-to-peer events), and SV
(sampled values) — the phase-14 context §4 glossary and foundations
§0.3 govern what FlexCore may touch:

**GOOSE and SV are protection-grade and OUT OF SCOPE — entirely, in
every batch, without exception.** FlexCore is a supervisory orchestration
layer; it performs no relay protection, no inverter inner-loop control,
no sub-cycle control (AGENTS.md §3 rule 3; foundations §0.3). This
record designs the **MMS client** surface only: the client/server
reporting-and-control profile at the DSO boundary, where an EU DSO's
61850 server exposes a DER device's data model and FlexCore connects
out as the client to read reports and receive control objects.

| Element | Design reading | UNVERIFIED check at implementation |
|---|---|---|
| Logical nodes | The 61850 data model: a physical device (IED) hosts logical devices; each hosts logical nodes (e.g. `DRCT` — DER control, `ZRCT` — reactive control, `DRCC` — DER curve/characteristic), each with data objects and attributes | The exact LN prefixes, data-object names, and attribute spellings for 7-420 DER nodes against the 61850-7-420 text [S-12] and the 7-4xx base (7-410/7-420 node semantics) |
| MMS client/server | MMS (ISO 9506) over the MMS/ACSE/TCP stack; the client reads/names data, writes control values, receives reports | The exact MMS object-name syntax (domain/LD/LN/DO/DA), report-control-block semantics, and the write paths the DSO permits |
| SCL / ICD | The System Configuration language; an ICD file describes one device's data model and is INGESTED, never handwritten | The SCL schema sections the ingestion needs (IED/LNode/DOI structure), and the profile-validation rules |
| Reporting | Report control blocks (URCB/BRCB — UNVERIFIED spellings) delivering data-object changes to the client | The buffering/trigger semantics and the DSO's reporting configuration |
| Control model | The 61850 control model (status-only / direct / SBO-with-normal-/enhanced-security — UNVERIFIED spellings) | Which control model the DSO requires for DER control and its parameterization |

## 2. The engineering judgment: why NO crate lands in this batch (the P14-E-3 DEFAULT)

Same judgment as record 41 §2, recorded per the subtask instruction:

1. **Record-only depth is the plan's own decision**, and P14-E-3's
   DEFAULT (a) is design-only in Phase 14. No EU integration is named.
2. **The logical-node semantics are explicitly UNVERIFIED by the
   subtask itself** ("DRCT, ZRCT, DRCC — node semantics UNVERIFIED until
   checked against the standard text"). Landing `Drct`, `Zrct`, `Drcc`
   model types whose attribute sets are guesses would be worse than no
   crate: the wave-1 crates pinned single spellings; 7-420 mapping is a
   whole data-model vocabulary.
3. **SCL/ICD ingestion is a designed seam of the same class as the CIM
   RDF seam** (record 44 §2) — a schema-driven parse with no parser
   dependency. The ingestion DESIGN below is the deliverable; the crate
   lands with the first named EU integration.

**Consequence:** the proposed crate `crates/adapters/protocol-61850/`
(package name `flexcore-protocol-61850`, per the P14.01 layout table)
is NOT created, NOT added as a workspace member, and NOT added to the
checker. It lands in the batch that implements the first named EU DSO
integration. This record is the design it would implement.

## 3. SCL/ICD ingestion — the design

Mirroring the CIM profile-source seam (record 44): a typed
`SclDocument` data model (IED → logical device → logical node → data
objects, with declared attribute keys), an `SclSource` SEAM trait
(parse from bytes; the shipped `UnwiredSclSource` refuses with
`SeamNotWired` — no XML parser dependency lands), and a completeness
gate: every discovered logical node is classified EXACTLY ONE of
MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS against the
declared 7-420 DER-node mapping vocabulary — the openems.rs pattern
and the record-44 discipline, applied to 61850. An ICD that contains
protection-grade node families (protection LNs, GOOSE/SV control
blocks) records them EXPLICITLY_UNMAPPED with the foundations-§0.3
reason — they are never read, never subscribed, never mapped.

## 4. Logical-node mapping for DER control — the declared skeleton

The mapping vocabulary the future crate declares (ALL node semantics
UNVERIFIED until checked against the 7-420 text [S-12]; the
implementation-time check is the first gate of that batch):

| Declared LN | Design role (UNVERIFIED) | Maps to |
|---|---|---|
| `DRCT` | DER control: the setpoint/enable surface the DSO writes | Command-lifecycle PROPOSED candidates (external events — never issued setpoints) |
| `ZRCT` | Reactive control (Q setpoints / power-factor mode) | PROPOSED candidates with the declared reactive-sign convention (units.rs rule) |
| `DRCC` | DER characteristic/curve data | Model/parameter data into the flexcore-domain envelopes — a proposal input, never a direct device write |

The control-path rule is the same as every Phase 14 adapter: an MMS
write is a **command-relay** surface (after the three gates), an MMS
report of an external control object enters at **PROPOSED**; the client
never constructs an APPROVED or ISSUED state from external input. The
capability manifest would declare `Read` + `ExternalEventIngest` at
monitor-first scope (the record-24 pattern); `CommandRelay` awaits the
v0.5+ release-ladder gate.

## 5. GOOSE/SV out of scope — the explicit statement

**GOOSE and SV are out of scope.** They are protection-grade
communication (foundations §0.3; phase-14 context §5 boundary 2):
FlexCore does not perform relay protection, synthetic inertia, primary
frequency response, or any sub-cycle control, and a GOOSE/SV surface
would exist precisely to carry those. The future crate's scope
declaration carries this as a mandatory `not_modelled` entry; the SCL
ingestion records protection-grade node families as EXPLICITLY_UNMAPPED
(§3); no GOOSE subscription, SV stream, or publication buffer exists in
any design surface here. This statement is a boundary of the platform,
not a phase-14 choice that a later phase can quietly reverse — reversing
it is a foundations-level change.

## 6. P14-E-3 — IEC 61850-7-420 and IEC 60870-5-104 EU timing (registered OPEN)

- **Question:** when do the EU DSO surfaces move from design records to
  implementation?
- **Options:** (a) design-only in Phase 14, implement on the first named
  EU integration; (b) implement both in a dedicated later phase; (c)
  implement 104 only (telecontrol precedes 61850).
- **DEFAULT (RECOMMENDATION):** **(a)** — design records exist so the EU
  surface is scopeable, but no EU integration is named, and protocol
  stacks without a counterparty rot. This batch implements the DEFAULT's
  consequence structurally: records 41/42 exist; no protocol-104 or
  protocol-61850 crate lands.
- **What closes it:** the human owner naming the first EU DSO
  integration and its protocol requirements (conventions §4 — agents
  never close E-decisions; this record registers it, it does not close
  it).
- **Evidence that would close it:** a named DSO integration (counterparty,
  point list / ICD file, security requirements — the record-41 §5 and §1
  check lists become the implementation batch's first gates).

## 7. Honest boundaries

No MMS session exists, no crate was created, no ICD file was read, no
logical node was resolved, no report was received, no control object
was written, no message was exchanged. No conformance, security, or
certification claim of any kind (conventions §7.3). Every node name in
§4 is a secondary-source reading; a mismatch at the implementation-time
check is a recorded edit to the named design point — never a silent
reinterpretation (plan §7 note 6). The simulated counterpart (a
simulated 61850 server publishing ICD-shaped models) is wave 3 /
P14.17's deliverable surface when the crate exists.

## 8. Handoff

- **The implementing batch** (post-P14-E-3 closure): records 01/23/24
  for the crate pattern; record 44 for the ingestion-seam pattern (§3);
  the [S-12]/[S-13] texts must be procured BEFORE any model type lands —
  the check list of §1/§4 is that batch's first gate.
- **Wave 3 / P14.17**: the simulated 61850 server names
  `crates/adapters/protocol-61850/`'s SCL seam as its exercise point
  when that crate exists.
