# W1.05 — Domain model: Facility/Building/EnergyBoundary/MeterPoint/EquipmentItem hierarchy; Facility is NOT Site

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 domain model hierarchy (Tenant, Facility, Building, BuildingSystem, EquipmentItem, InspectionObservation, EnergyBoundary, MeterPoint, UtilityAccountReference, Core Site/SiteMeter references, FacilityAssessment) and record the critical distinction that Facility is NOT Site.
- **Design trace:** Design §4.1; §15 ADR-F16-01
- **Owns:** Owns ADR-F16-01 (Facility / Building / EnergyBoundary is a bounded context separate from Core Site / Asset).
- **Inputs / prerequisites:** Design §4.1; §1.1 ownership and site/asset schema rows; W1.01 scope statement.
- **Steps:**
  1. Reproduce the §4.1 hierarchy in substance, including the 0..n / 1..n cardinalities stated there.
  2. State the distinction explicitly: an existing FlexCore Site is an electrical/operational boundary with a PCC, timezone and grid-model reference; a commercial property may have several buildings, utility accounts and meters; one facility may map to multiple electrical sites and one electrical site may serve more than one building or tenant area.
  3. State that Facility, Building, EnergyBoundary, MeterPoint, EquipmentItem and FlexCore Asset must not be treated as synonyms, and that an EquipmentItem can be recorded for audit purposes without being a grid resource or a dispatchable asset.
  4. Record ADR-F16-01 as owned by this subtask, stating the bounded-context separation from Core Site/Asset; name the owners (product, domain architecture) and the status (proposed, pending approval).
- **Deliverable / output:** A domain-model note with the §4.1 hierarchy, the Facility-is-not-Site distinction, and ADR-F16-01.
- **Acceptance criteria / exit check:**
  - [ ] The hierarchy reproduces the §4.1 structure and cardinalities.
  - [ ] The Facility-is-not-Site distinction is stated with the many-to-many facility/site mapping.
  - [ ] The note states EquipmentItem is not automatically a grid resource or dispatchable asset.
  - [ ] ADR-F16-01 is recorded as owned here.
  - [ ] Owners (product, domain architecture) and status are recorded.
- **Owner role:** Domain architecture
- **Dependencies:** W1.01, W1.02
- **Verification method:** Comparison against design §4.1 and §1.1; ADR-F16-01 text reviewed by product and domain architecture.
- **Risks / failure modes:** Collapsing Facility into Site and corrupting the electrical boundary. Mitigation: the distinction is a named ADR, not a comment.
