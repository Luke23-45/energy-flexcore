# CIM/CGMES Network-Model Import Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-cim` adapter (`crates/adapters/protocol-cim/`): the CIM/CGMES network-model import — RDF/XML profile parsing design, class mapping into the grid-api network IR, and the IEC 61968-5 DER-group registry |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/44_p14_12_cim_cgmes_import.md` |
| Implementation status | **WRITTEN-UNRUN.** The CIM vocabulary, the profile-source SEAM, the declared class mapping, the import register, and the DER-group registry exist as written code; nothing has compiled, no profile file was read, no DSO model was received (`UnwiredProfileSource` refuses with `SeamNotWired`) |
| Conformance status | **No CGMES/CIM conformance claim of any kind.** No certification program is named in the repository's source register (UNVERIFIED-ABSENCE — evidence plan, records 101 §2) |
| Control status | Ingest-only. No command path exists (structural absence); the import produces network-model and DER-group DATA, never device actions |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | Model-import surface (common CLIENT role, MONITOR-ONLY, MODEL-IMPORT resource class — not a telemetry adapter) |
| Parsing | The `RdfProfileSource` SEAM (parse bytes → `ProfileDocument`) with declared normalization rules (rdf:ID → mRID; predicates → declared keys; rdf:resource → referenced mRID) — NO XML/RDF parser dependency lands until a human-gated implementation decision |
| Mapped classes | ConnectivityNode → BusSpec, ACLineSegment → BranchSpec, PowerTransformer → TransformerSpec (VA → MVA canonical conversion), ExternalNetworkInjection → ExternalGridSpec — attribute spellings ALL UNVERIFIED against the CGMES profile texts [S-17] |
| DER groups | `DerGroupRegistry` (IEC 61968-5 [S-18]) — every member must exist in the imported profile; each member's network-mapping status travels verbatim from the import register; a DECLARED design shape (payload classes UNVERIFIED) |
| Unmapped classes | EnergyConsumer, PowerElectronicsConnection (+ units), Breaker/Disconnector, Substation/VoltageLevel/Bay/Terminal, BaseVoltage, TopologicalNode — EXPLICITLY_UNMAPPED with recorded reasons; never guessed |

## 2. Deployment (what a deployment would configure)

| Configuration | Honesty note |
|---|---|
| The profile file source | the deployment supplies the profile document(s) to the seam; no file is read by any code in this program |
| The declared `GridModelRef` + base MVA | declared per import, never inferred |
| The slack rule | the declared first-external-grid slack convention (a design pin — UNVERIFIED against practice) |
| DER-group declarations | validated ids + members per deployment; member existence enforced |

## 3. Security

| Topic | Design position |
|---|---|
| Document trust | Profile documents are untrusted data: duplicate mRIDs refused, empty documents refused, unknown classes recorded (never guessed), details carried as untrusted data at presentation (the P1.08 rule) |
| Secrets | None handled — the adapter reads no credentials and holds no secret surface |
| Counterparty | No DSO boundary is touched; the import is a file/ingest surface with no listener (nothing inbound) |

## 4. Operations (completeness gate, observability, failure modes)

| Topic | Design position |
|---|---|
| Classification | Every discovered object lands in EXACTLY ONE status: MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS (the openems.rs pattern verbatim) |
| Completeness gate | `finalize` REFUSES while any AMBIGUOUS object remains — the import never proceeds past a refusal and never silently drops an object it refused to map |
| Coverage record | `ModelImportSummary` carries the coverage-limitation statements verbatim to every consumer (the P4.02 pattern) |
| Cadence | Event-driven (a new profile version) — the manifest's declared cadence is the budget seam; the four status names are the observability vocabulary (consumed, never redefined) |
| Failure modes | Empty/duplicate documents refused; missing/invalid declared values → AMBIGUOUS (blocking); missing line rated-current carrier / transformer ratedS carriers → the ambiguity names the check; unwired source → `SeamNotWired` |

## 5. Honest boundary

No profile was parsed, no network model was assembled from real data, no
DER group was registered from a real exchange, no DSO model was
exchanged with anyone. The mapping was exercised ONLY against
constructed in-repo fixtures in written-unrun tests. The CGMES/CIM and
IEC 61968-5 text verification passes are named human gates (record 44
§4). FlexCore makes no CGMES conformance claim.
