# Ledger — F-06 Live CGMES network-model import

**Owning code:** `crates/adapters/protocol-cim`
**Phase record:** P14.12
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-06); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F06.01 | Minimal RDF/XML pull parser (hand-written: elements, attributes, namespaces, entities, CDATA-free subset used by CGMES profiles) fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.02 | Profile-source implementation of the existing rdf_profile seam: parse into the CIM object vocabulary, class mapping into VersionedNetworkModel via the grid-api IR fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.03 | Completeness gate enforcement: import refused while any AMBIGUOUS object remains fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.04 | Unmapped classes are EXPLICITLY_UNMAPPED, never guessed fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.05 | Class-spelling verification pass against fetchable primary sources (CGMES profile schema); anything unverifiable stays labeled UNVERIFIED fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.06 | Pinned tests: parser fixtures, import mapping, gate refusal, unmapped-class recording fixed (written, awaiting human build/test) — wave 3 |
| FIM-F06.07 | Honesty header: real DSO profile interop is a HUMAN GATE fixed (written, awaiting human build/test) — wave 3 |

---

**Status:** IMPLEMENTED (written) — 7/7 subtasks fixed (written, awaiting human build/test) — wave 3. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.