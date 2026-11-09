# P0.16 — Multi-tenancy and storage principles freeze

| Field | Value |
|---|---|
| Subtask | P0.16 — Freeze multi-tenancy and storage principles |
| Report trace | §16.1 (initial storage choices), §16.2 (multi-tenancy, RLS caveat [R25]), §16.3 (retention and data governance), §6.5 (state storage strategy), §17.4 (cross-tenant leakage threat row) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.10 (asset/capability vocabulary), P0.15 (API surface) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. Initial storage choices (§16.1 — all six rows, roles intact)

Reproduced from the §16.1 table:

| Storage | Role | Recommendation |
|---|---|---|
| PostgreSQL | Tenants, users/roles references, sites, assets, grid metadata, schedules, commands, approvals, plugins, outbox, audit metadata | Primary transactional source of truth |
| PostgreSQL partitioned telemetry tables | Initial telemetry storage and time-window queries | Start here; measure before adding a separate analytics database |
| TimescaleDB | Optional time-series extension | Evaluate as an optional deployment/profile; do not make proprietary/optional features mandatory for the open baseline |
| S3-compatible object storage | Simulator artifacts, large datasets, model files, exports, reports, signed plugin packages | Separate large/blob data from OLTP |
| NATS JetStream | Durable event/work stream | Messaging, not the authoritative business database |
| Redis | Optional cache/rate-limit/session acceleration | Do not introduce in v0.1.0 unless measurements justify it |

Frozen corollaries: TimescaleDB is optional and never mandatory for the open baseline; JetStream is messaging, never the authoritative business database (consistent with ADR-007, §22.1); Redis is not in v0.1.0 unless measurements justify it.

## 2. Multi-tenancy rules (§16.2 — all seven, none dropped)

Reproduced from the §16.2 bullet list, in report order:

1. `tenant_id` is part of every tenant-owned record and query scope.
2. Use application authorization and PostgreSQL Row-Level Security (RLS) as defence in depth.
3. The application runtime role must not own tenant tables or bypass RLS.
4. Use `FORCE ROW LEVEL SECURITY` where appropriate and test all service roles.
5. Separate tenant context from untrusted request body fields; derive it from authenticated claims/authorized context.
6. Audit cross-tenant access denials.
7. Keep site edge identities scoped to one tenant and approved site/asset set.

Cross-reference: cross-tenant leakage is a named threat with controls in the §17.4 threat table; the defence here (rules 1–7) is that control baseline applied to storage.

## 3. RLS table-owner caveat (§16.2, [R25])

PostgreSQL RLS can restrict rows per user/policy, but **table owners typically bypass policies unless configured otherwise**; do not rely on a policy while running the application as the table owner (§16.2, [R25]).

Consequences frozen here:

- Rule 2.3 above is the operational form of this caveat: the runtime role must not own tenant tables.
- `FORCE ROW LEVEL SECURITY` (rule 2.4) exists precisely because of owner bypass; "where appropriate" is a per-table decision for Phase 1 migrations, and every service role — including owners — must be tested (rule 2.4, ledger "Risks / failure modes").

## 4. Retention and data governance (§16.3)

Define retention per data class. The §16.3 classes, in report order:

1. raw high-frequency telemetry;
2. normalized telemetry;
3. aggregated intervals;
4. command/audit evidence;
5. market records;
6. simulation artifacts; and
7. model/forecast outputs.

Retention must be configurable by customer contract, regional market rules, and applicable privacy/security requirements. **Do not choose a universal retention period in code** (§16.3). No retention value, default period, or per-class duration is set anywhere in this document — that is the rule, not an omission.

## 5. State storage strategy (§6.5 — selective event sourcing)

Use event sourcing selectively (§6.5):

- append-oriented event history for command lifecycle, operator approvals, plugin changes, configuration changes, and security-relevant actions;
- current-state tables for assets, schedules, configuration, and portfolio state;
- time-series tables for telemetry and measurements;
- object storage for large simulation artifacts, reports, model files, and data exports.

**Do not use a single append-only event log as the only source of truth for all high-volume telemetry** (§6.5, closing sentence).

Consistency note: the §6.5 append-oriented history for command lifecycle, approvals, and plugin/configuration/security changes is the durable record; JetStream (§1, row 5) carries the same facts as transport, never as the authoritative store (§16.1 row 5; §15.5).

## 6. What this document does not do

- It defines no migrations, no table DDL, no partition layout, and no RLS policy text: Phase 1 owns those, reviewed against these principles (`tenant_id` presence per table, per ledger verification method).
- It sets no retention value (§4) and no performance, scale, or availability target for any storage engine.
- It makes no certification or security-assurance claim: these are design requirements, consistent with §17's scope limit.

## 7. Acceptance-criteria coverage (ledger P0.16)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All six §16.1 storage rows are reproduced with roles intact | §1 (verbatim table) |
| All seven §16.2 tenancy rules are stated, including the RLS table-owner caveat | §2 (seven rules) and §3 ([R25] caveat) |
| The per-data-class retention rule is stated with no universal retention value chosen | §4 (all seven classes; no value set anywhere) |
| The §6.5 selective-event-sourcing rule is stated | §5 |

Verification method (ledger): comparison against §16.1–§16.3, §6.5 — performed in §§1–5; Phase 1 migration review checks `tenant_id` presence per table.
