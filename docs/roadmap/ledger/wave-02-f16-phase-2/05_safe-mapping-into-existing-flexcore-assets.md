# W2.05 — Safe mapping into existing FlexCore assets

Workstream: FlexCore control-plane work

- **Objective:** Define the safe mapping of assessment objects into existing FlexCore assets from the design §5.6 table, with NO new AssetType enum values and no building/roof/chiller/solar-panel asset type; a genuinely new resource class needs a schema-versioned decision plus an additive migration.
- **Design trace:** Design §5.6 table; §1.1 BESS row
- **Inputs / prerequisites:** Design §5.6 table; §1.1 site/asset schema and capability rows; W1.05 domain model.
- **Steps:**
  1. Reproduce the §5.6 mapping table with qualification caveats: rooftop PV plant to existing PvInverter asset(s) (inverter AC limits and site connection must be evidenced; PV module nameplate alone is not dispatch capability); battery energy storage to existing Bess asset (requires BMS limits, SOC/energy window, efficiency, reserve and operating constraints with provenance); HVAC/flexible facility load to FlexibleCiLoad or ThermalStorageHeatPump where supported (numeric comfort/thermal dynamics must be supported by the capability model and site integration; a descriptor alone is not enough for automated dispatch); EV charging to existing EvseEvFleet asset (charger capability, vehicle connection/departure and charging requirements are needed); site meter to existing SiteMeter asset (map only normalized interval values whose quality/time/unit and measurement boundary are compatible).
  2. State the closed-enum rule: no building, roof, chiller or solar-panel asset type is injected into the existing closed AssetType enum.
  3. State the new-class rule: if a genuinely new resource class is needed, write a schema-versioned design decision and an additive migration with compatibility tests.
  4. State that only candidate capabilities that can be represented and validated by the supported capability schema (v1) are published.
  5. State the §1.1 BESS reuse rule: reuse battery capability rules where the source data is adequate; unknown limits must not create flexibility credit.
- **Deliverable / output:** A safe-mapping note with the §5.6 table, the closed-enum rule and the new-class rule.
- **Acceptance criteria / exit check:**
  - [ ] All five §5.6 mappings appear with their qualification caveats.
  - [ ] The no-new-AssetType-values rule and the no-building/roof/chiller/solar-panel-type rule are stated.
  - [ ] The schema-versioned-decision plus additive-migration rule for a genuinely new resource class is stated.
  - [ ] The supported-schema-only publication rule is stated.
  - [ ] The §1.1 BESS reuse rule is stated (reuse battery capability rules where source data is adequate; unknown limits must not create flexibility credit).
- **Owner role:** Domain architecture + power systems
- **Dependencies:** W2.03, W2.04, W1.05
- **Verification method:** Mapping table checked against design §5.6; AssetType enum checked against the schema checks in §1.1.
- **Risks / failure modes:** Injecting a building asset type to make mapping convenient. Mitigation: the enum is closed by schema checks and the new-class rule requires a versioned decision.
