# W5.05 — Immutable calculation report

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 immutable calculation report from design §13.1, with trustworthy-result disclosure per §13.5 and determinism/repeatability per §13.2: input snapshot/digest, provider/model/tariff versions, solver status, assumptions, uncertainty, and a new run for new evidence.
- **Design trace:** Design §14 F-17 Phase 1; §13.1; §13.2; §13.5; §8.1; §8.2 step 8; foundations.md §3 item 5
- **Inputs / prerequisites:** Design §13.1, §13.2, §13.5, §8.1; foundations.md §3 item 5; W5.03 scenarios; W5.04 financial model.
- **Steps:**
  1. Define the immutable report contents from §13.1: input snapshot ID and canonical digest; raw-data/document digest references; parser/importer version; source/provider and API version; weather dataset or station ID/distance/quality; tariff profile ID/version and effective date; ECM catalog or PV/battery model version; optimizer/solver version, termination status and gap where relevant; assumptions and user overrides; data completeness and exclusions; uncertainty/limitations and warnings; produced-at time and code build/version.
  2. State the immutability rule: new evidence or changed assumptions create a new immutable run; do not overwrite a historic recommendation with a newer model result.
  3. Apply §13.5: the report must state what data was used and absent, what is measured, user-declared, provider-derived or assumed, which tariff, weather and equipment model versions were used, whether the solver found an optimum, a feasible result, a time-limited result or no supported solution, what uncertainty/limitations meaningfully affect the conclusion, whether more data or professional inspection is required, and whether any financial conclusion is unsupported by local rates/costs.
  4. State that a valid result may be not enough evidence, tariff unsupported, roof model unavailable, no economic case, or no flexibility credit, and that these are correct outputs, not product failures.
  5. Apply §13.2 determinism/repeatability for the companion product's scenario jobs: record tolerances and solver status for numerical providers that are not bitwise deterministic, and do not advertise exact replay if the engine cannot provide it; store actual external provider request parameters and provider response digest, with credentials removed; keep model evaluation datasets and hand-calculated reference cases separate from live user data.
  6. State the next-steps content as a required report item, delivered in the §8.2 step 8 report: what the household needs to do to proceed — the next steps for a site-specific quotation or inspection (design §8.1) and the foundations.md §3 item 5 list, including professional site inspection where required and the installation steps that remain.
- **Deliverable / output:** An immutable calculation report definition with the §13.1 contents, the immutability rule, and the §13.5 disclosure requirements.
- **Acceptance criteria / exit check:**
  - [ ] All §13.1 report contents are listed.
  - [ ] The new-run-for-new-evidence rule is stated; historic recommendations are not overwritten.
  - [ ] All §13.5 disclosure items are required in every report.
  - [ ] The valid negative/refusal outputs are listed as correct outputs.
  - [ ] §13.2 determinism rules for F-17 scenario jobs are stated (tolerances and solver status recorded, no exact-replay claim the engine cannot support, request/response digests without credentials, evaluation datasets separate from live user data).
  - [ ] The next-steps content (design §8.1 + foundations.md §3 item 5, delivered in the §8.2 step 8 report) is a required report item.
- **Owner role:** Numerical engineering + product
- **Dependencies:** W5.03, W5.04
- **Verification method:** Report contents checked against design §13.1 and §13.5 item by item.
- **Risks / failure modes:** Overwriting a historic recommendation with a newer model result. Mitigation: immutability is a report rule, not a preference.
