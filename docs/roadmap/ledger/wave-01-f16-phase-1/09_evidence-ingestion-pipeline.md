# W1.09 — Evidence ingestion pipeline

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 evidence ingestion pipeline from design §5.2: upload intent, signed URL, checksum/MIME verification, quarantine and malware scan, BuildingSync validation, staging with row-level errors, human mapping review, and transactional commit; bills are NOT telemetry.
- **Design trace:** Design §5.2; §1.1 units row; §15 ADR-F16-03
- **Owns:** Owns ADR-F16-03 (utility bills and interval datasets are separate records; bills are not telemetry).
- **Inputs / prerequisites:** Design §5.2; §7.1 BuildingSync; W1.06, W1.07.
- **Steps:**
  1. Reproduce the eight §5.2 pipeline steps in order (upload intent with tenant, assessment, file type, declared period, size limit and checksum expectation; short-lived signed upload URL to a private evidence bucket, never proxying very large files through control-plane memory; finalize with size, checksum, MIME signature and ownership verification; quarantine and malware scan with parsing in a restricted worker process; BuildingSync XML validation against the pinned XSD and applicable Schematron use case with DTDs/external entities disabled and expansion, recursion, archive extraction and parser resources bounded; normalized staging record plus row/cell/XPath-level errors, with questionable rows never written directly into authoritative telemetry or bills; authorized-user review of mapping, period, units, direction and meter/boundary association; transactional commit of accepted records with audit references and preservation of the original evidence object and content hash).
  2. State that bill PDFs, invoices and BuildingSync files are not telemetry, and that UtilityBill and MeterDataset stay separate.
  3. State the interval-mapping rule: interval samples may later be mapped into Core SiteMeter telemetry only when source, measurement semantics, quantity, unit, time and quality match the telemetry contract.
  4. Record ADR-F16-03 as owned by this subtask. Name the owners (domain architecture) and the status (proposed, pending approval).
  5. State the §1.1 units rule: the FlexCore boundary uses MW, MVAr, MVA and MWh and SOC is a fraction; convert kW/kWh at adapters when normalizing bills and meter datasets.
- **Deliverable / output:** An evidence-ingestion pipeline note with the eight steps, the bills-are-not-telemetry rule, the interval-mapping rule, and ADR-F16-03.
- **Acceptance criteria / exit check:**
  - [ ] All eight §5.2 steps appear in order with their stated safeguards (signed URL, checksum/MIME, quarantine/scan, XSD/Schematron with DTDs disabled, staging errors, human review, transactional commit).
  - [ ] The bills-are-not-telemetry rule and the UtilityBill/MeterDataset separation are stated.
  - [ ] The interval-to-SiteMeter mapping condition is stated.
  - [ ] ADR-F16-03 is recorded as owned here.
  - [ ] Owners (domain architecture) and status are recorded.
  - [ ] The §1.1 units rule is stated (MW/MVAr/MVA/MWh boundary, SOC as a fraction, kW/kWh converted at adapters).
- **Owner role:** Domain architecture + security
- **Dependencies:** W1.06, W1.07
- **Verification method:** Step-by-step comparison against design §5.2; ADR-F16-03 reviewed by domain architecture.
- **Risks / failure modes:** Questionable rows written straight into authoritative bills or telemetry. Mitigation: staging with row-level errors and human review are mandatory steps, not optional.
