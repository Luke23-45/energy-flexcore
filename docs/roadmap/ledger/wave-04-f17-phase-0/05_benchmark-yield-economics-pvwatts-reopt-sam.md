# W4.05 — Benchmark yield and economics against PVWatts/REopt/SAM for identical inputs

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Benchmark the prototype yield and economics against PVWatts/REopt/SAM for identical inputs, per design §14 F-17 Phase 0 and §10.4 step 5, following the foundations.md F-17 validation rule.
- **Design trace:** Design §14 F-17 Phase 0; §10.4 step 5; foundations.md F-17 validation rule
- **Inputs / prerequisites:** Design §10.4; §14 F-17 Phase 0; foundations.md §8 (validation exercise items 1–2); W4.04 prototype.
- **Steps:**
  1. Select representative cases and run the prototype and each benchmark (PVWatts, REopt, SAM) on identical inputs (location, load, roof, system, tariff, financial assumptions).
  2. Record the differences per case, including differences in weather inputs, tariff interpretation and battery models, rather than expecting blind numerical identity.
  3. Record the benchmark versions used (for example PVWatts V8) and the station/dataset details per run.
  4. Record the verdict per case: whether the prototype output is sufficiently accurate and useful to justify maintaining the regional data, or what must change.
- **Deliverable / output:** A benchmark record: identical-input cases, prototype vs PVWatts/REopt/SAM outputs, documented differences, versions, and a per-case verdict.
- **Acceptance criteria / exit check:**
  - [ ] Benchmark cases use identical inputs across prototype and PVWatts/REopt/SAM.
  - [ ] Differences are documented (weather, tariff interpretation, battery models), not hidden.
  - [ ] Benchmark versions and station/dataset details are recorded per run.
  - [ ] Each case has a verdict on accuracy and usefulness.
- **Owner role:** Numerical engineering
- **Dependencies:** W4.04, W4.08
- **Verification method:** Benchmark record checked case by case; identical-input discipline verified; versions recorded.
- **Risks / failure modes:** Expecting blind numerical identity and hiding model differences. Mitigation: differences are documented per case.
