# W0.03 — Close D4 license/dependency audit before adopting new code, schemas, datasets or libraries

Workstream: FlexCore control-plane work

- **Objective:** Close the D4 license/dependency audit (or record its scheduled closure with owner, venue and date) before any new code, schema, dataset or library is adopted for F-16 or F-17, including BuildingSync, REopt and all other dependencies, datasets and providers.
- **Design trace:** Design §1.3; §14 Gate 0 item 3; §16 gate G9
- **Owns:** Owns §16 gate G9 (BuildingSync, REopt and all dependencies/datasets/providers included in D4 or the companion equivalent).
- **Inputs / prerequisites:** Design §1.3 and §16 item 9; .agent/PROVENANCE.md; D4 record in the v1 ledger (P1.27/P1.28, P10.19); candidate dependency/dataset/provider lists for F-16 (BuildingSync) and F-17 (PVWatts, REopt, Google Solar API, PVGIS, OpenEI URDB).
- **Steps:**
  1. Inventory every dependency, dataset, schema and provider the design names for F-16 and F-17, including BuildingSync, REopt (API v3 / REopt.jl), PVWatts, Google Solar API, PVGIS and OpenEI URDB.
  2. Record the license and terms for each item from its primary source, and flag any item whose license or terms are unverified or incompatible with the intended distribution.
  3. Record the D4 decision (closed with evidence, or scheduled with owner, venue and date) and the consequence of deciding late for each wave.
  4. Confirm that no later subtask adopts a library, schema, dataset or provider before its D4 entry is cleared.
- **Deliverable / output:** A D4 audit record covering all F-16/F-17 dependencies, datasets, schemas and providers, with license/terms per item and the D4 closure or scheduling decision.
- **Acceptance criteria / exit check:**
  - [ ] Every dependency, dataset, schema and provider named in the design has a D4 entry with a license/terms source.
  - [ ] No item is recorded as cleared without a primary-source license or terms reference.
  - [ ] The D4 decision is recorded as closed with evidence or scheduled with owner, venue and date — never as silently decided.
  - [ ] The record states that adoption of any uncleared item is blocked until its D4 entry closes.
- **Owner role:** Legal + engineering
- **Dependencies:** W0.01, W0.02
- **Verification method:** License/terms checked against primary sources per item; D4 row inspected in the decision record.
- **Risks / failure modes:** Adopting a GPL or terms-restricted dataset before the audit and discovering the restriction after build. Mitigation: the inventory is complete before any adoption subtask starts.
