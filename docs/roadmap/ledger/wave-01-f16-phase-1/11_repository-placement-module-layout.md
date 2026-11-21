# W1.11 — Repository placement and module layout

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 repository placement and module layout from design §6.1: keep the modular monolith, with domain/application/ports/Postgres/control-plane/worker placement and a dedicated Assessment Studio module, and no empty crates.
- **Design trace:** Design §6.1
- **Inputs / prerequisites:** Design §6.1; §2.2; W1.05, W1.06.
- **Steps:**
  1. Reproduce the §6.1 placement in substance: domain (facility; facility_assessment), application (facility_assessment use cases and pure calculations/orchestration; efficiency_savings separate from DR event M&V), ports (facility_assessment tenant-scoped repositories/import/evidence seams; building_data_exchange BuildingSync boundary; tariff_data versioned tariff pack access), Postgres adapter (tenant-owned assessment tables, RLS, indexes; repositories and transactional adapter mapping), control-plane (routes_facility_assessment, dto_facility_assessment), worker (existing job supervision and consumers), and a new writable facility-assessment-studio surface.
  2. State the rules: do not create a microservice for every assessment concept; long-running file parsing and numerical analysis use the existing worker/supervisor boundary; domain, application workflows and authoritative metadata belong in the existing FlexCore layers.
  3. State that module names are illustrative and crates are created only with real implementation, and that no empty crates are created; reuse flexcore-domain, flexcore-application, flexcore-ports, the Postgres adapter, the control plane and the worker until an independent deployment or dependency boundary is demonstrated.
  4. Record that the Assessment Studio module placement is detailed in W1.13.
  5. Cross-reference the §2.1 logical architecture record (W0.09) for the F-16 placement in the joint cross-product diagram.
- **Deliverable / output:** A repository-placement note with the §6.1 layout, the monolith rules, and the no-empty-crates rule.
- **Acceptance criteria / exit check:**
  - [ ] The §6.1 placement appears for domain, application, ports, Postgres adapter, control-plane, worker and Assessment Studio.
  - [ ] The keep-the-modular-monolith rule and the worker-boundary rule are stated.
  - [ ] The no-empty-crates rule and the reuse-until-boundary-demonstrated rule are stated.
  - [ ] The note states module names are illustrative, not a creation order.
- **Owner role:** Architecture
- **Dependencies:** W1.05, W1.06, W0.09
- **Verification method:** Comparison against design §6.1 and §2.2.
- **Risks / failure modes:** Creating a microservice per assessment concept or empty crates in advance. Mitigation: placement is illustrative and creation requires real implementation.
