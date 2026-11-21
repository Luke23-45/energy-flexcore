# W1.15 — BuildingSync v2.7.0 interoperability

Workstream: FlexCore control-plane work

- **Objective:** Define BuildingSync v2.7.0 interoperability for F-16 from design §7.1: pinned XSD plus a selected Schematron use case, round-trip tests, unknown extensions preserved or reported (not silently discarded), and no BuildingSync-certified claim without granted status.
- **Design trace:** Design §7.1; §15 ADR-F16-02
- **Owns:** Owns ADR-F16-02 (BuildingSync v2.7.0 importer/exporter and selected Schematron use case).
- **Inputs / prerequisites:** Design §7.1; W1.09 ingestion; W0.03 D4 record (BuildingSync license/terms).
- **Steps:**
  1. State the BuildingSync boundary: it standardizes the exchange structure and terminology; it does not standardize the calculations performed by the software.
  2. Define the FlexCore-specific BuildingSync use case/model view, and the import/export mapping against the pinned v2.7.0 XSD and a selected Schematron use case.
  3. Require round-trip testing of representative sample files, and require unknown extension elements to be preserved or reported as unmapped rather than silently discarded.
  4. State that the internal domain model stays independent of the XML schema, and that no BuildingSync-certified claim is made unless the relevant program actually grants such status.
  5. Record ADR-F16-02 as owned by this subtask, and note the D4 license/terms position for BuildingSync from W0.03; name the owners (product, building-energy reviewer, license review) and the status (proposed, pending approval).
- **Deliverable / output:** A BuildingSync interoperability note with the use case, the pinned XSD/Schematron mapping, the round-trip and unknown-extension rules, and ADR-F16-02.
- **Acceptance criteria / exit check:**
  - [ ] The exchange-structure-not-calculations boundary is stated.
  - [ ] The pinned v2.7.0 XSD and selected Schematron use case are named as the mapping basis.
  - [ ] Round-trip testing and the preserve-or-report unknown-extension rule are stated.
  - [ ] No BuildingSync-certified claim is permitted without granted status.
  - [ ] ADR-F16-02 is recorded as owned here.
  - [ ] Owners (product, building-energy reviewer, license review) and status are recorded.
- **Owner role:** Product + building-energy reviewer + license review
- **Dependencies:** W0.03, W1.09
- **Verification method:** Comparison against design §7.1; BuildingSync v2.7.0 release and validation resources cited from the design §17 references.
- **Risks / failure modes:** Claiming BuildingSync certification, or silently discarding unknown extensions. Mitigation: both are forbidden in the note and tested in W1.17.
