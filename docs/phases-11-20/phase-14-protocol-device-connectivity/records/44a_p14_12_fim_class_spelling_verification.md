# P14.12 — FIM-F06.05: implementation-time class-spelling verification record

| Field | Value |
|---|---|
| Subtask | FIM-F06.05 — the implementation-time check declared by record 44 §4: the 15 declared CIM class spellings and the 15 declared attribute-name constants of `crates/adapters/protocol-cim/src/model.rs` are verified against fetchable primary sources; anything unverifiable stays labeled UNVERIFIED |
| Wave / agent | FIM wave 3, Agent A (feature F-06, `docs/releases/implementation_ledger/ledger_F06.md`) |
| Status | WRITTEN (a DOCUMENTATION record — no code claims follow from it). The verification pass was executed 2026-11-19 with live fetches of the sources below; every VERIFIED verdict cites the source and what matched. The wave's hard rule "existing types/tests untouched" means the recorded MISMATCHES were NOT applied as constant renames in this batch — applying them is the recorded edit record 44 §4 names and needs its own reviewed change (see §4). **2026-11-21 addendum (§4a): the ADDITIVE remediation of the verified MISMATCH verdicts is now applied written-unrun (2 accepted aliases + 9 verified-absent entries in `crates/adapters/protocol-cim/src/spelling_remediation.rs`); the declared constants were NOT renamed — see §4a** |
| Sources | [S-173] ENTSO-E, *Common Grid Model Exchange Specification (CGMES) Version 2.5 — Draft IEC 61970-600 Part 1, Edition 2*, document version 1, 15 July 2016 — official ENTSO-E download `https://eepublicdownloads.entsoe.eu/clean-documents/CIM_documents/IOP/CGMES_2_5_TechnicalSpecification_61970-600_Part%201_Ed2.pdf` (fetched 2026-11-19; text extracted with `pdftotext`); [S-174] ENTSO-E, *CGMES Constraints Repository* — official ENTSO-E download `https://eepublicdownloads.entsoe.eu/clean-documents/CIM_documents/Grid_Model_CIM/CGMES_Constraints_Repository.xlsx` (fetched 2026-11-19; the workbook's string cells were read directly); [S-175] UCA International Users Group (CIM Users Group), *CPSM & CPSM-NA Profile Documentation*, `https://cpsm-docs.ucaiug.io/Core%20Equipment/PowerElectronicsWindUnit` (fetched 2026-11-19; schema source `http://iec.ch/TC57/2020/CPSM-CoreEquipment#`, URIs reference IEC CIM 100) |
| Files touched | this record only (plus the F-06 code under `crates/adapters/protocol-cim/` — see the ledger, not this record) |

## 1. What was checked and how

Record 44 §4 declared nine UNVERIFIED items and named the
implementation-time check as the gate for each. This record is that
check, run at implementation time against two official ENTSO-E artifacts
and one UCA CIM Users Group documentation site:

- [S-173] is the normative CGMES exchange-standard draft (IEC 61970-600
  Part 1 Ed. 2) published by ENTSO-E, including its CGMES instance-file
  examples and the GENC/MSPEC conformance rules. Full text searched.
- [S-174] is ENTSO-E's machine-readable constraints repository for CGMES
  model validation — the closest public ENTSO-E artifact to a profile
  schema: every validation constraint is stated over qualified
  `Class.attribute` spellings, so an attribute ABSENT from it is strong
  evidence of absence from the CGMES profile set (not proof against the
  whole CIM standard — the verdicts below are worded accordingly).
- [S-175] is the CIM Users Group's profile documentation of the IEC CIM
  100 edition, used for the one class ([S-173]/[S-174] predate or omit
  it).

Verdict vocabulary:

- **VERIFIED (match)** — the exact declared spelling was found in a
  primary source, with the matched context quoted or counted.
- **MISMATCH (verified)** — the declared spelling was verified ABSENT
  and the actual spelling/carrier was verified present in the same
  source. This is stronger than UNVERIFIED: the mismatch is a fact with
  a source.
- **UNVERIFIED** — neither the declared spelling nor its actual carrier
  could be verified against a fetchable primary source; the declared
  label stays as-is.

## 2. The 15 class spellings (`CimKnownClass`)

| # | Declared spelling | Verdict | Source + what matched |
|---|---|---|---|
| 1 | `ACLineSegment` | VERIFIED (match) | [S-173] instance-file examples: `<cim:ACLineSegment rdf:ID="_f732688e-…">` with child elements `<cim:ACLineSegment.r>1.06601</cim:ACLineSegment.r>`, `<cim:ACLineSegment.x>34.28535</cim:ACLineSegment.x>`, `<cim:ACLineSegment.bch>…`; MSPEC4 ("CGMES supports ACLineSegment-s between different terminal voltages"). [S-174]: 31 occurrences |
| 2 | `PowerTransformer` | VERIFIED (match) | [S-173]: transformer rules reference `PowerTransformerEnd`, "for a two Terminal PowerTransformer the high voltage PowerTransformerEnd has non-zero values on r, r0, x, and x0"; `PowerTransformerEnd.ratedU`. [S-174]: 69 occurrences |
| 3 | `BaseVoltage` | VERIFIED (match) | [S-173]: "name for BaseVoltage may not be required, since it is characterized by attribute BaseVoltage.nominalVoltage"; "All implementations shall use association to a BaseVoltage for the purpose of any per unit calculations". [S-174]: 33 occurrences |
| 4 | `ConnectivityNode` | VERIFIED (match) | [S-173]: ConnectivityNode rules throughout (e.g. MSPEC16, the ground-voltage-level rule); the rdf:ID/mRID rules. [S-174]: 38 occurrences |
| 5 | `EnergyConsumer` | VERIFIED (match) | [S-174]: 16 occurrences (loads and their constraints). NOT present in [S-173] — the 2.5 draft's examples cover the exchange machinery, not every EQ class; [S-174] carries the class in the current profile set |
| 6 | `PowerElectronicsConnection` | VERIFIED (match) | [S-174]: 5 occurrences, including the qualified attribute `PowerElectronicsConnection.ratedS` |
| 7 | `PowerElectronicsUnit` | VERIFIED (match) | [S-175]: inheritance chain `IdentifiedObject → PowerSystemResource → Equipment → PowerElectronicsUnit → PowerElectronicsWindUnit`, with the unit attributes `maxP`/`minP` — IEC CIM 100 edition. ABSENT from [S-173]/[S-174] (pre-CIM-100 artifacts; [S-174] validates `PowerElectronicsConnection`, not its units) |
| 8 | `ExternalNetworkInjection` | VERIFIED (match) | [S-174]: 13 occurrences; the constraint set carries `ExternalNetworkInjection.p/q/minP/maxP/minQ/maxQ` |
| 9 | `Breaker` | VERIFIED (match) | [S-173] GENC15: "Breakers represent busbar couplers in a bus-branch model exchange". [S-174]: 5 occurrences (incl. `Switch.ratedCurrent` on the switch hierarchy) |
| 10 | `Disconnector` | VERIFIED (match) | [S-174]: 13 occurrences |
| 11 | `SynchronousMachine` | VERIFIED (match) | [S-173]: 10 occurrences (machine/terminal rules). [S-174]: 168 occurrences |
| 12 | `Terminal` | VERIFIED (match) | [S-173]: 15 occurrences. [S-174]: 110 occurrences |
| 13 | `Substation` | VERIFIED (match) | [S-173]: 4 occurrences. [S-174]: 23 occurrences |
| 14 | `VoltageLevel` | VERIFIED (match) | [S-173]: "A Switch connecting BusbarSections at different VoltageLevels is an error" (MSPEC13 context). [S-174]: 34 occurrences |
| 15 | `Bay` | VERIFIED (match) | [S-174]: 20 occurrences (container constraints). NOT present in [S-173] |

**Result: 15/15 VERIFIED (match).** The P14.12 record's headline
UNVERIFIED register item 7 ("Class spellings per CGMES conformance
profile") is CLOSED for the class names. Note the honest boundary: the
verdicts establish the spellings exist in the CGMES/CIM profile
documentation; they establish nothing about CGMES conformance of any
implementation (conventions §7.3).

## 3. The 15 declared attribute-name constants (the mapping keys)

| # | Declared key | Verdict | Source + what matched |
|---|---|---|---|
| 1 | `BaseVoltage.nominalV` | **MISMATCH (verified)** | [S-173] verbatim: "it is characterized by attribute **BaseVoltage.nominalVoltage**, which could optionally serve as name". [S-174] carries `BaseVoltage.nominalVoltage` / `BaseVoltage.nominalVoltages`; `nominalValue` (CIM 100 spelling) does NOT appear in either — the CIM 100 rename is itself UNVERIFIED here |
| 2 | `ConnectivityNode.BaseVoltage` | **MISMATCH (verified)** | [S-174]: the ConnectivityNode attributes carried are `ConnectivityNode.ConnectivityNodeContainer` and `ConnectivityNode.TopologicalNode` — no direct BaseVoltage attribute. Record 44 §4 item 5 ("the real path may go through VoltageLevel") is confirmed in direction; the exact traversal is UNVERIFIED in detail |
| 3 | `ACLineSegment.r` | VERIFIED (match) | [S-173] instance file: `<cim:ACLineSegment.r>1.06601</cim:ACLineSegment.r>`. [S-174]: `ACLineSegment.r` |
| 4 | `ACLineSegment.x` | VERIFIED (match) | [S-173] instance file: `<cim:ACLineSegment.x>34.28535</cim:ACLineSegment.x>`. [S-174]: `ACLineSegment.x` |
| 5 | `ACLineSegment.ratedCurrent` | **MISMATCH (verified)** | [S-174]: no `ACLineSegment.ratedCurrent`; the only `ratedCurrent` carried is `Switch.ratedCurrent`. Record 44 §4 item 3 (the real carrier is likely OperationalLimitSet) is confirmed in direction; the limit-set attribute path is UNVERIFIED in detail |
| 6 | `ACLineSegment.fromBus` | **MISMATCH (verified)** | [S-174]: zero occurrences of any `fromBus` qualified attribute; [S-173] carries connectivity through Terminals. The declared key is an import-shorthand decision, not a CIM attribute (record 44 §4 item "declared direct reference") |
| 7 | `ACLineSegment.toBus` | **MISMATCH (verified)** | same evidence as #6 |
| 8 | `PowerTransformer.ratedS` | **MISMATCH (verified)** | [S-174]: the `ratedS` owners carried are `PowerTransformerEnd.ratedS`, `AsynchronousMachine.ratedS`, `RotatingMachine.ratedS`, `PowerElectronicsConnection.ratedS` — NOT `PowerTransformer.ratedS`. Record 44 §4 item 4 confirmed |
| 9 | `PowerTransformer.vkPercent` | UNVERIFIED | absent from [S-173]/[S-174]; the real short-circuit carrier (TransformerEndInfo / ShortCircuitTest — record 44 §4 item 4) was NOT verified in either source, so no actual spelling is recorded |
| 10 | `PowerTransformer.vkRPercent` | UNVERIFIED | same evidence as #9 |
| 11 | `PowerTransformer.hvBus` | **MISMATCH (verified)** | [S-174]: zero occurrences; [S-173] names the two-winding ends through `PowerTransformerEnd` + `TransformerEnd.endNumber` ("the high voltage PowerTransformerEnd … the low voltage PowerTransformerEnd") |
| 12 | `PowerTransformer.lvBus` | **MISMATCH (verified)** | same evidence as #11 |
| 13 | `ExternalNetworkInjection.bus` | **MISMATCH (verified)** | [S-174]: the ExternalNetworkInjection attributes carried are `p`, `q`, `minP`, `maxP`, `minQ`, `maxQ` — no bus/reference attribute. [S-173] carries the external injection at the TopologicalNode boundary (record 44 §4 item 9 context); the exact reference path is UNVERIFIED in detail |
| 14 | `ExternalNetworkInjection.vmPu` | **MISMATCH (verified)** | [S-174]: zero occurrences of `vmPu`; the magnitude the SV profile carries (candidate: `targetVsMagnitude`) is NOT present in either source — UNVERIFIED as to the actual spelling |
| 15 | `ExternalNetworkInjection.vaDegree` | **MISMATCH (verified)** | [S-174]: zero occurrences of `vaDegree`; actual angle spelling UNVERIFIED |

**Result: 4/15 VERIFIED (match), 9/15 MISMATCH (verified), 2/15
UNVERIFIED.** Two structural facts are also VERIFIED by [S-173] and
close record 44 §4 items 1 and part of the normalization rules: the mRID
carrier is the `rdf:ID` ("Only mRID (exchanged as rdf:ID) is used for
this purpose"; GENC3; GENC6 — IEC 61970-552 UUID syntax), and the
profile element shape is exactly the one the parser implements
(`<cim:Class rdf:ID="…">` object elements with `<cim:Class.attribute>`
child elements; `rdf:resource` references appear 27 times in [S-173]).

## 4. What this record deliberately does NOT do

1. **No constant renames in this batch.** The wave's hard rule keeps
   existing types and tests untouched. The nine MISMATCH verdicts are
   recorded facts; renaming the affected `ATTR_*` constants of
   `model.rs` (and re-pointing the mapping and fixtures) is the
   "recorded edit to the named constant" record 44 §4 names — it needs
   its own reviewed change so the mapping vocabulary, the class-spelling
   table, and every fixture move together. Until that change lands, the
   mapping keys remain the DECLARED import vocabulary (pinned by the
   FIM-F06.06 fixtures), NOT verified CGMES attribute spellings — the
   fixture-test header in `crates/adapters/protocol-cim/tests/
   cgmes_import.rs` states exactly this.
2. **No conformance claim.** Verifying spellings against published
   sources is documentation work. Nothing here qualifies any code
   against CGMES conformance (conventions §7.3); interop with a real DSO
   profile remains a HUMAN LAB GATE (ledger F06).
3. **No report edit.** The canonical report's source list (§27,
   [R1]…[R29]) and its 7 November 2026 research cut-off are untouched;
   [S-173]/[S-174]/[S-175] are record-level sources fetched 2026-11-19,
   registered here in the phase-record numbering (highest prior: [S-172]).
   If the report ever cites them, they get the next free [R] IDs there.
4. **No claim about CIM 100 attribute renames.** [S-175] documents the
   CIM 100 edition, where `BaseVoltage.nominalValue` (not
   `nominalVoltage`) is reported by secondary summaries — but the CIM
   100 attribute spellings were NOT verified by a fetch of a CIM 100
   schema in this pass (the [S-175] page fetched documents a different
   class). All CIM-100-era attribute spellings stay UNVERIFIED.

### §4a — 2026-11-21 addendum (FIM wave 8B, Agent B): the ADDITIVE remediation applied (WRITTEN-UNRUN)

This addendum records the reviewed, ADDITIVE change §4 point 1 named: the
verified MISMATCH verdicts of §3 are now applied — as an import-vocabulary
EXTENSION, never as constant renames. Implemented in
`crates/adapters/protocol-cim/src/spelling_remediation.rs` (new module;
`src/lib.rs` registers it; the parse path in `src/xml_profile.rs` resolves
predicate spellings through it; pinned tests in
`crates/adapters/protocol-cim/tests/cgmes_spelling_aliases.rs`). The code
is WRITTEN-UNRUN (the program's no-build/no-test boundary); real-profile
interop stays the HUMAN LAB GATE. What changed, exactly:

1. **Accepted aliases (2)** — only the MISMATCH verdicts where §3 verified
   a replacement spelling in a primary source:

   | Declared key (UNCHANGED) | Accepted CGMES spelling (alias) | Source |
   |---|---|---|
   | `BaseVoltage.nominalV` | `BaseVoltage.nominalVoltage` | §3 #1; [S-173] verbatim; [S-174] |
   | `PowerTransformer.ratedS` | `PowerTransformerEnd.ratedS` | §3 #8; [S-174] ratedS owners |

   An incoming element with the alias spelling normalizes ONTO the declared
   key; the alias spelling is never stored as a key; a duplicate carried
   under both spellings on one object still refuses (the resolved key
   duplicates). The declared spellings stay accepted exactly as before —
   every §2/§3 fixture of the existing pin suite
   (`tests/cgmes_import.rs`) is untouched and stays true.

2. **Verified-absent register (9)** — the MISMATCH verdicts with NO
   verified replacement spelling are recorded as named not-modelled
   statements and are NOT aliases: `ConnectivityNode.BaseVoltage` (the
   container/topological traversal is UNVERIFIED in detail),
   `ACLineSegment.ratedCurrent` (the only verified `ratedCurrent` carrier
   is `Switch.ratedCurrent` — a different class' attribute; the line's
   OperationalLimitSet path is UNVERIFIED in detail),
   `ACLineSegment.fromBus` / `toBus`, `PowerTransformer.hvBus` / `lvBus`,
   `ExternalNetworkInjection.bus` / `vmPu` / `vaDegree`. The declared keys
   remain the DECLARED import vocabulary (still accepted by the fixtures;
   never reinterpreted into the verified carrier paths).

3. **What did NOT change** — no `ATTR_*` constant of `model.rs` was
   renamed, re-pointed, or removed; the 2 UNVERIFIED items (§3 #9/#10, the
   vk / vkR short-circuit spellings) stay UNVERIFIED and unaliased; the
   class-spelling vocabulary (§2, 15/15 VERIFIED) is untouched; no
   conformance claim follows (§4 point 2 still holds verbatim).

4. **Recorded arithmetic discrepancy (left for a human correction)**: this
   record's §3 summary line reads "4/15 VERIFIED (match), 9/15 MISMATCH
   (verified), 2/15 UNVERIFIED", but the §3 table itself carries 2
   VERIFIED (match) rows (#3, #4), 11 MISMATCH rows, and 2 UNVERIFIED
   rows. The remediation remediates the TABLE (2 + 9 = 11 verdicts, each
   exactly once); it does not silently reproduce the summary line's
   arithmetic. The "9" of the summary appears in the implementation as the
   verified-ABSENT count.

## 5. Honest boundaries

The verification pass fetched and searched the three sources above on
2026-11-19; the extraction (`pdftotext` on [S-173], direct workbook-XML
reads on [S-174], page fetch on [S-175]) was mechanical, and the
occurrence counts in §2/§3 are reproducible from the artifacts. Neither
[S-173] nor [S-174] is the full normative CGMES 3.0 profile schema set
(IEC 61970-600-1:2021 remains unfetched — paywalled); attribute
absence verdicts are evidence of absence from these two artifacts, not
from the whole standard. The UNVERIFIED items (§3 #9/#10 and the actual
carrier spellings behind the MISMATCH verdicts) stay labeled UNVERIFIED
exactly as record 44 §4 requires.
