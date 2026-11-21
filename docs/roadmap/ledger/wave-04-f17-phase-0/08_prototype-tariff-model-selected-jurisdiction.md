# W4.08 — Prototype tariff model for the selected jurisdiction

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Prototype the tariff model for the selected jurisdiction from design §10.3: executable tariff data with source and validity, where unsupported features return UNSUPPORTED_TARIFF_RULE and are never silently dropped.
- **Design trace:** Design §10.3; §15 ADR-F17-02
- **Owns:** Owns ADR-F17-02 (local tariff pack is required for bill-savings claims; unsupported tariff constructs refuse).
- **Inputs / prerequisites:** Design §10.3; W4.02 tariff sources; W4.01 launch jurisdiction.
- **Steps:**
  1. Model the tariff as executable data with a source and validity period, not one scalar price per kWh, covering where applicable: import energy rate by season, month, weekday/weekend and time-of-use window; export compensation/net billing/net metering and credit carry-forward/expiry; fixed customer charges and minimum monthly bill; demand charges and demand windows; tiers/block rates; taxes, fuel adjustments and other surcharges; customer class, utility, voltage/service class and jurisdiction; currency and effective dates.
  2. Implement the refusal rule: if the tariff includes a feature the calculator does not support, return UNSUPPORTED_TARIFF_RULE or require an explicitly simplified scenario; never drop the component silently.
  3. Use the provider-neutral TariffCatalog port; note that OpenEI URDB is useful for the U.S. and includes international entries in API versions 4+, but its documentation describes most U.S. utility rates and it is not a global source-of-truth, so the launch-country pack uses official utility/regulator schedules and source dates.
  4. Record ADR-F17-02 as owned by this subtask. Name the owners (product, local energy-market expert) and the status (proposed, pending approval).
- **Deliverable / output:** A prototype tariff model for the selected jurisdiction with executable tariff data, the UNSUPPORTED_TARIFF_RULE refusal, and ADR-F17-02.
- **Acceptance criteria / exit check:**
  - [ ] The tariff model covers the §10.3 components where applicable, with source and validity per item.
  - [ ] The UNSUPPORTED_TARIFF_RULE refusal rule is stated and no silent dropping is permitted.
  - [ ] The OpenEI URDB boundary (not a global source-of-truth) is stated; the launch pack uses official schedules.
  - [ ] ADR-F17-02 is recorded as owned here.
  - [ ] Owners (product, local energy-market expert) and status are recorded.
- **Owner role:** Product + local energy-market expert
- **Dependencies:** W4.01, W4.02
- **Verification method:** Tariff model checked against design §10.3 and the W4.02 source pack; refusal behaviour checked per unsupported feature.
- **Risks / failure modes:** Dropping an unsupported tariff component and overstating savings. Mitigation: refusal is a named return, not a silent default.
