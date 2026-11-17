# P14.12 — CIM/CGMES network-model import: profile parsing design, class mapping, IEC 61968-5 DER groups

| Field | Value |
|---|---|
| Subtask | P14.12 — the CIM/CGMES network-model import: RDF/XML profile parsing design, class mapping (PowerElectronicsConnection, EnergyConsumer, and the DER-relevant classes), IEC 61968-5 DER group payloads, mapping into grid-api models with `not_modelled` scope honesty — every unmapped CIM class is EXPLICITLY_UNMAPPED, never guessed [S-17][S-18] |
| Wave / agent | Phase 14 wave 2, Agent A |
| Status | WRITTEN-UNRUN: model types, profile-source SEAM, declared class mapping, the register + completeness gate, the DER-group registry, scope declaration — written, never compiled or executed. No profile file was read, no DSO boundary touched, no model exchanged (conventions §1; the first human build is the program gate) |
| Sources | [S-17] (ENTSO-E CIM for grid-model exchange — the CGMES profile overview; the profile texts were NOT consulted), [S-18] (IEC 61968-5:2020 catalog entry — the standard text was NOT consulted); the grid-api network IR (`crates/flexcore-grid-api/src/port.rs` — `VersionedNetworkModel`), the openems.rs classification pattern, the P4.02 completeness-gate pattern, the P14.01 common port layer (record 01) |
| Files touched | `crates/adapters/protocol-cim/` (NEW: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/rdf_profile.rs`, `src/class_mapping.rs`, `src/unmapped.rs`, `src/der_groups.rs`, `src/scope.rs`, `tests/class_mapping.rs`); additive root `Cargo.toml` entries; additive `tools/check_dependency_rule.py` entries |

## 1. What landed (deliverable class: design-and-model)

| Module | Content |
|---|---|
| `model.rs` | The CIM object vocabulary: `CimMRid`, `CimObject` (mRID + raw class name + declared attributes in a BTreeMap — the determinism rule; the import result must not depend on document element order), `ProfileDocument` (refuses empty documents and duplicate mRIDs), `CimKnownClass` (15 declared class spellings — ALL UNVERIFIED), and the DECLARED attribute-name constants (each with its implementation-time check: `BaseVoltage.nominalV`, `ConnectivityNode.BaseVoltage`, `ACLineSegment.r/x/ratedCurrent/fromBus/toBus`, `PowerTransformer.ratedS/vkPercent/vkRPercent/hvBus/lvBus`, `ExternalNetworkInjection.bus/vmPu/vaDegree` — all UNVERIFIED spellings and units) |
| `rdf_profile.rs` | The RDF/XML profile-source SEAM (the record 02 §5 / record 43 transport-seam template): the `RdfProfileSource` trait (parse bytes → `ProfileDocument`), the declared normalization rules (rdf:ID → mRID; predicates → declared keys; rdf:resource → referenced mRID; object order irrelevant), `UnwiredProfileSource` with `SeamNotWired` — **no XML/RDF parser dependency lands** |
| `class_mapping.rs` | The declared mapping into the grid-api network IR (`flexcore-grid-api::port`): `ClassMappingStatus` — exactly one of MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS (the openems.rs pattern verbatim); the four mapped classes (ConnectivityNode→BusSpec with declared BaseVoltage-registry voltage; ACLineSegment→BranchSpec; PowerTransformer→TransformerSpec with the VA→MVA canonical conversion; ExternalNetworkInjection→ExternalGridSpec); `BaseVoltageRegistry` (a BaseVoltage is reference data — registered, never mapped as an element); every missing/invalid declared value is AMBIGUOUS — never defaulted |
| `unmapped.rs` | The EXPLICITLY_UNMAPPED register and the fail-closed completeness gate: `ImportRegister::classify_profile` (one classification per object, structural), `finalize` REFUSING while any AMBIGUOUS object remains (the mapping refused to guess — the import never proceeds past a refusal), `ModelImportSummary` with the coverage-limitation statements traveling verbatim (the P4.02 pattern), `assemble_network_model` (mapped elements only; declared GridModelRef + base MVA; the declared first-external-grid slack rule; the IR's own structural validation re-raised NAMED) |
| `der_groups.rs` | The IEC 61968-5 DER-group registry: `DerGroupDeclaration` (validated: non-empty id, unique members), `DerGroupRegistry::assemble` (every member must exist in the imported profile; each member's network-mapping status recorded VERBATIM from the import register — the registry surface and the network-IR surface cannot be confused); NO command path (structural absence) |
| `scope.rs` | CONSUMES the common port layer: `import_scope()` (role CLIENT — ingest side; MONITOR ONLY; resource MODEL-IMPORT; the not_modelled list carries the DER/load/switch/parser/spelling/61968-5 limitations) and `import_manifest()` (the one common manifest shape, `SupervisoryRelayOnly`, capability `ModelImport`, event-driven cadence) |

## 2. Dependence on Agent A's P14.01 layer

Same pattern as records 21/23/24/43: the crate consumes the common
`ProtocolScopeDeclaration` and `ProtocolAdapterManifest` (workspace
pin). Role mapping: profile ingest = the common CLIENT role. The
monitor-only gate refuses command relay AND telemetry requests — this
is a model-import surface, not a telemetry adapter. The `MODEL-IMPORT`
resource class is the common vocabulary's own name for CIM/CGMES
surfaces (record 01 §3).

Additional crate dependency, recorded: `flexcore-grid-api` (a CORE
crate — the checker's adapter rules permit adapters to depend on core
crates). The network IR (`VersionedNetworkModel`, `BusSpec`,
`BranchSpec`, `TransformerSpec`, `ExternalGridSpec`) is the mapping
TARGET named by the subtask; the crate defines no parallel network
model. `flexcore-contracts` validates the canonical-unit boundary (the
VA→MVA conversion happens at the mapping, and no scaled unit crosses
into the IR).

## 3. The EXPLICITLY_UNMAPPED discipline (the subtask's core deliverable)

**Every unmapped CIM class is EXPLICITLY_UNMAPPED, never guessed** —
enforced structurally:

1. **Four statuses, exactly one per object** (the openems.rs pattern):
   MAPPED (a concrete IR element was produced), EXPLICITLY_UNMAPPED
   (the vocabulary recognizes the class and DECLARES it unmapped, with
   a recorded reason), UNMAPPED (no vocabulary entry — recorded, never
   guessed), AMBIGUOUS (map-eligible but a required declared value is
   missing/invalid — the units-are-never-guessed rule).
2. **The recorded reasons name where the truth lives**: EnergyConsumer
   — the network IR has no load element, load aggregation is not part
   of this import; PowerElectronicsConnection (+ its units) — DER
   equipment is modelled in flexcore-domain envelopes, not the network
   IR, and an import that guessed a bus injection would fabricate a
   generator the model does not own; Breaker/Disconnector — switching
   state is not part of the bus-branch import; Substation/VoltageLevel/
   Bay/Terminal — container/connectivity refinement, not IR elements;
   BaseVoltage — reference data, registered not mapped.
3. **An ambiguity blocks the summary**: `finalize` refuses with the
   count and the first object named — the import never silently drops
   an object it refused to map.
4. **The coverage-limitation record travels**: every explicitly-unmapped
   and unmapped entry is surfaced verbatim wherever the import's
   outputs are consumed (the P4.02 coverage-limitation pattern; the
   scope's `not_modelled` list is the reviewed summary of the same
   truth).

## 4. UNVERIFIED register (implementation-time checks against [S-17]/[S-18] and the CGMES profile text)

| # | Detail | Where |
|---|---|---|
| 1 | The RDF namespace URIs and whether objects carry `rdf:ID` or `rdf:about` | `rdf_profile.rs` normalization rules |
| 2 | The exact CGMES profile(s) each declared attribute belongs to (EQ/TP/SV/DY) and whether r/x are per-length (a length attribute this mapping would need) | `model.rs` ATTR_* constants |
| 3 | The line-rated-current carrier (declared direct attribute; the real carrier is likely OperationalLimitSet — the mapping refuses when absent and the ambiguity names the check) | `model.rs::ATTR_LINE_RATED_CURRENT_A`; `class_mapping.rs::classify_line` |
| 4 | The transformer ratedS / vk / vk_r carriers (declared on the transformer; the real carriers are likely TransformerEnd / TransformerEndInfo / ShortCircuitTest) | `model.rs::ATTR_TRANSFORMER_*`; `classify_transformer` |
| 5 | The bus/voltage reference chain (declared direct BaseVoltage reference on ConnectivityNode; the real path may go through VoltageLevel) | `model.rs::ATTR_NODE_BASE_VOLTAGE_REF`; `classify_connectivity_node` |
| 6 | The external-grid class spelling (ExternalNetworkInjection vs EnergySource) and its attribute set | `classify_external_grid` |
| 7 | Class spellings (the 15-entry `CimKnownClass` vocabulary) per CGMES conformance profile | `class_mapping.rs::CimKnownClassVocabulary` |
| 8 | The 61968-5 DER-group payload classes, attribute spellings, and reference semantics | `der_groups.rs` (the whole structure is a DECLARED design shape) |
| 9 | Whether the CGMES bus-branch profile's bus source is ConnectivityNode or TopologicalNode (the declared pin is ConnectivityNode; TopologicalNode is explicitly unmapped) | `classify_connectivity_node`; the TopologicalNode reason in record 44's vocabulary (design-level note: declared in the mapping's Terminal/TopologicalNode dispositions) |

A mismatch found at those gates is a recorded edit to the named
constant/module — never a silent reinterpretation (plan §7 note 6).

## 5. Honest boundaries

The profile source is unwired: no XML/RDF parser exists in the
dependency line, no profile file was read, no DSO model was received,
no network model was exchanged with anyone. The mapping was exercised
ONLY against constructed in-repo fixtures in written-unrun tests. No
CGMES conformance claim of any kind (conventions §7.3); the manifest is
a declaration, never a conformance statement. No control path exists
(structural). The test file (`tests/class_mapping.rs`) is written
COMPLETE and UNRUN — execution is a human gate. The simulated
counterpart (a profile-file fixture set) is P14.17's deliverable
(wave 3).

## 6. Handoff

- **Wave 3 (P14.17)**: the simulated counterpart is a fixture profile
  set exercising `rdf_profile::RdfProfileSource` (a real parser behind
  the seam) and the mapping/register path.
- **Wave 3 (P14.20/P14.21)**: the import is event-driven (a new profile
  version) — the manifest's declared cadence is the budget seam; the
  classification status names (MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED /
  AMBIGUOUS) are the observability vocabulary (consumed, never
  redefined).
- **The human gates**: first workspace build; the mapping-test
  execution; the CGMES/CIM text verification pass (§4); the IEC
  61968-5 text verification pass (the DER-group payload structure).
