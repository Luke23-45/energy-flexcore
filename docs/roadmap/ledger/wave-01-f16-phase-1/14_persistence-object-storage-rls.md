# W1.14 — Persistence, object storage, RLS

Workstream: FlexCore control-plane work

- **Objective:** Define F-16 persistence, object storage and RLS from design §6.4: tenant tables with forced RLS, immutable original evidence, normalized datasets separate from raw objects, data-class-specific retention, and transaction plus outbox atomicity.
- **Design trace:** Design §6.4
- **Inputs / prerequisites:** Design §6.4; W1.06 records; W1.09 ingestion; W1.12 events; W0.10 G8 retention/consent decisions.
- **Steps:**
  1. Reproduce the §6.4 rules: queryable domain state, versions, references and audit metadata in PostgreSQL; raw bills, reports, photos, BuildingSync XML and large meter files in a private object store, with object key, content hash, source, MIME, uploader, ingestion result and retention class in PostgreSQL.
  2. State the RLS rules: enable and FORCE RLS for every tenant-owned table, and use composite tenant foreign keys to prevent cross-tenant parent-child references.
  3. State the immutability rules: keep original evidence immutable; corrections create new records linked to the previous record rather than overwriting source evidence; store normalized datasets separately from raw objects, with an import-run version and a reversible mapping to source rows/cells.
  4. State the retention rule: retention, export and deletion behavior must be data-class-specific and explicit, with no default inferred from an unset retention policy; the retention periods and deletion behaviour follow the W0.10 G8 record for the F-16 jurisdiction; and use existing transaction plus outbox atomicity for authoritative changes and event publication.
- **Deliverable / output:** A persistence note with the §6.4 storage, RLS, immutability, retention and atomicity rules.
- **Acceptance criteria / exit check:**
  - [ ] The PostgreSQL/object-store split and the per-object metadata fields are stated.
  - [ ] FORCE RLS for every tenant-owned table and composite tenant foreign keys are stated.
  - [ ] The immutable-original-evidence and corrections-as-new-records rules are stated.
  - [ ] Data-class-specific retention with no inferred default, and transaction plus outbox atomicity, are stated.
- **Owner role:** Platform engineering
- **Dependencies:** W1.06, W1.09, W1.11, W1.12, W0.10
- **Verification method:** Comparison against design §6.4; RLS and retention rules checked against the v1 ledger tenancy rules.
- **Risks / failure modes:** A tenant table without forced RLS, or source evidence overwritten by a correction. Mitigation: both rules are stated as mandatory, and W1.17 tests them.
