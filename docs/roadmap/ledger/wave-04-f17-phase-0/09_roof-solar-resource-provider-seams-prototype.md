# W4.09 — Roof and solar-resource provider seams, prototype scope

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the roof and solar-resource provider seams in prototype scope from design §10.1 (RoofGeometryProvider, SolarResourceProvider, PVYieldProvider), with PVWatts V8 as the first benchmark/provider candidate and PVGIS only with a pinned version.
- **Design trace:** Design §10.1; §10.4 (optimizer-port reuse condition); §1.1 weather and optimization rows; §15 ADR-F17-03
- **Owns:** Owns ADR-F17-03 (PVWatts/roof/REopt engines are behind versioned provider adapters).
- **Inputs / prerequisites:** Design §10.1; W4.03 roof comparison; W0.03 D4 record.
- **Steps:**
  1. Define the three provider-neutral seams: RoofGeometryProvider (building footprint, roof planes, dimensions, imagery/model quality, source timestamp); SolarResourceProvider (weather/climate station, solar irradiance, temperature, wind, time-series resource with dataset version); PVYieldProvider (normalized location, roof-plane and system inputs; monthly/annual or interval AC/DC production, losses, station/dataset details, warnings, model version).
  2. Record PVWatts V8 as the first benchmark/provider candidate, with the adapter recording warnings, service version and weather-station metadata including station distance and source, converting units explicitly, and not describing PVWatts TMY output as a guaranteed roof-specific forecast.
  3. Record the PVGIS rule: evaluate as a second independent provider only with a pinned production API version and version-compatibility testing; do not silently switch a production run to a beta API because the endpoint or dataset changed.
  4. Record ADR-F17-03 as owned by this subtask, and note the D4 license/terms position for each provider from W0.03; name the owners (numerical engineering, license/operations review) and the status (proposed, pending approval).
  5. State the §1.1 reuse boundary for F-17: the FlexCore weather seam and the FlexCore optimizer port are not directly reusable by F-17's independent consumer identity boundary; the optimizer port is reused only under the §10.4 condition (faithful problem-IR expression plus provider capability declaration; restated in W5.03 step 4).
- **Deliverable / output:** A provider-seams note with the three seams, the PVWatts V8 candidate, the PVGIS pinning rule, and ADR-F17-03.
- **Acceptance criteria / exit check:**
  - [ ] All three seams are defined with their inputs and outputs.
  - [ ] PVWatts V8 is named as first benchmark/provider candidate, with station metadata and unit-conversion requirements stated.
  - [ ] The PVGIS pinned-version rule and the no-silent-beta-switch rule are stated.
  - [ ] ADR-F17-03 is recorded as owned here.
  - [ ] Owners (numerical engineering, license/operations review) and status are recorded.
  - [ ] The D4 license/terms position for each provider (PVWatts, PVGIS, REopt) is recorded from W0.03.
  - [ ] The §1.1 reuse boundary for F-17 is stated (FlexCore weather seam and optimizer port not directly reusable by F-17 except under the §10.4 condition, restated in W5.03).
- **Owner role:** Numerical engineering + license/operations review
- **Dependencies:** W4.03, W0.03
- **Verification method:** Seams checked against design §10.1; PVWatts V8 API reference cited from design §17.
- **Risks / failure modes:** Switching a production run to a beta PVGIS API, or describing TMY output as a roof forecast. Mitigation: both are forbidden in the note.
