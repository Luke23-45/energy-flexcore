# P13.14 — Jurisdiction registry (per-tenant residency and legal-basis capture)

| Field | Value |
|---|---|
| Subtask | P13.14 — per-tenant residency and legal-basis capture design closing the LR-062 direction; retention basis becomes jurisdiction-qualified data; legal sign-off = human gate |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN — design record only. No jurisdiction is named, no privacy regime is asserted, no residency constraint is recorded, no sign-off exists. Every jurisdiction/regime/basis VALUE cell in this record's registry shape is EMPTY by design. |
| Ledger row | `docs/phases-11-20/phase-13-enterprise-identity-integration/ledger_phase_13.md` P13.14 (NOT ticked by this agent — ticks are the orchestrator's, applied with this record as citation) |
| Sources | LR-062 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8 direction; the privacy-regime gap itself is `ledger/phase-09-pilot-qualification/10_data_residency_retention.md`, cited throughout as "the P9.10 record"); D1 OPEN (`.agent/DECISIONS.md`, read-only); P1.10 seven data classes (`crates/flexcore-storage/src/retention.rs`); gap analysis §11 (`00_GAP_ANALYSIS.md`) |
| Files touched | This record only. No migration, no code, no edit to `docs/foundations/*`, `docs/plan/*`, `.agent/*`, or the P9.10 record (read-only reference — never rewritten). |
| Authoring order note | This record was WRITTEN FIRST of Agent B's range (before the P13.13 GDPR/privacy record, `25_p13_13_gdpr_privacy.md`) because P13.13's retention matrix keys on the registry defined here (plan §2.1: P13.14 → P13.13, wave 1 internal). The record NUMBER follows the subtask order convention (`21_p13_09` … `26_p13_14`), not the authoring order. |

## 1. What this record is — and the one thing it refuses to do

The Phase 9 data-residency record (P9.10) left the platform's privacy posture
honest but inert: D1 (target jurisdiction) is OPEN, so there is no residency
constraint, no named privacy regime, and no basis for any retention period —
every period/basis cell in its §6 retention schedule is EMPTY, and its §13
carries that state to the residual-risk register as LR-062 (D1 OPEN).

**This record does not close LR-062 by deciding anything.** No agent may name
a jurisdiction, pick a privacy regime, or sign off a retention basis (that is
D1, a human-owner decision; the P9.10 record's §2 consequence table stands
unchanged). What closes the LR-062 *direction* is the missing MECHANISM: the
platform has no place to PUT a jurisdiction decision once the human makes it.
Today, even after D1 closes, the only landing spots for the decision are
prose documents — nothing in the data model can carry "this tenant's data is
subject to regime X, held in region Y, with legal basis Z per data class, as
signed off by human H on date D". This record designs that place: the
**jurisdiction registry** — per-tenant residency and legal-basis capture as
queryable, auditable data, with legal sign-off modeled as a human gate
(`UNSIGNED` until a named human signs).

Two structural rules inherited verbatim (never re-derived here):

| Rule | Source | How this design honors it |
|---|---|---|
| No universal retention period in code; retention is per data class, configurable per contract / regional market rules / privacy requirements | P0.16 §4 (report §16.3); `crates/flexcore-storage/src/retention.rs` ("no default duration anywhere"); migration 0010 D-P5 (no TTL, no scheduled-delete rule in DDL) | The registry stores per-**(tenant, jurisdiction, data class)** values only. There is no platform-default row, no fallback value, and no code constant anywhere in this design. An unset value is `UNSET` (the P1.10 `RetentionSetting::Unset` semantics), which consumers MUST treat as "no retention action authorized", never "keep forever" and never "apply a default". |
| A value enters the privacy posture only from a primary source, cited | P9.10 §3 (scope rule) | Every registry value row carries a mandatory `basis_reference` (contract clause / market-rule capture / applicable-requirement citation) and `signed_off_by` + `signed_off_at`. A row without a basis reference is not deployable — the P9.10 §11 check-3 reviewer rule, expressed as data. |

## 2. Registry shape — the data this design captures

The registry is per-tenant, versioned, and sign-off-gated. The shape below is
a design draft (DDL-style); it is NOT a migration in this batch. Storage
follows the P13.15 tenant-lifecycle seam (wave 2) — the registry rows are
tenant data and land under the same RLS discipline as every other
tenant-scoped table (`tenant_id` + ENABLE/FORCE, migration 0008 pattern). No
DDL below was applied anywhere; it exists so the wave-2/human implementation
has a reviewed starting shape.

| Field | Type | Rule |
|---|---|---|
| `registry_id` | uuid PK | One row per (tenant, jurisdiction, capture version) |
| `tenant_id` | uuid, NOT NULL, FK → tenants | The tenant whose residency/legal basis is captured (P0.16 rule 1) |
| `jurisdiction_code` | text, NOT NULL | The D1-style jurisdiction identifier. EMPTY until the human decision lands — the registry may hold a row with jurisdiction captured but basis cells UNSET; it may never hold an INVENTED code |
| `privacy_regime` | text, nullable | The named regime(s) for that jurisdiction, cited (P9.10 §7 row 2). NULL while D1 is open — never presupposed, never defaulted |
| `data_class` | text, NOT NULL, CHECK in the seven P1.10 class names | Exactly the seven §16.3 classes, in report order (`raw_high_frequency_telemetry`, `normalized_telemetry`, `aggregated_intervals`, `command_audit_evidence`, `market_records`, `simulation_artifacts`, `model_forecast_outputs`) — the closed vocabulary `crates/flexcore-storage/src/retention.rs` already carries. No eighth class is invented; a new class is a reviewed change to THAT type plus this CHECK, never a free-text value |
| `retention_period` | nullable interval | EMPTY/NULL = `UNSET` (the P1.10 semantics). Filled ONLY per the basis rule (§1). **No default row and no fallback exists anywhere in this design** |
| `basis_reference` | text, NOT NULL per filled value | The primary-source citation the value came from (contract clause / market-rule capture / applicable requirement). A filled `retention_period` without a `basis_reference` is a CHECK violation — the P9.10 §6 filling rule as a constraint |
| `legal_basis` | text, nullable, per data class | The lawful-basis/processing-basis capture for the class under the named regime, cited like `basis_reference`. Vocabulary is regime-owned — this design does NOT enumerate GDPR articles or any other regime's categories (that would presuppose a regime; the regime is named per row or absent) |
| `residency_constraint` | text, nullable | The residency constraint the deployment must satisfy, cited (P9.10 §4 row: "may central data leave the jurisdiction?" — captured per tenant, not per platform) |
| `cross_border_transfer` | text, nullable | Sub-processor / transfer constraints, cited (P9.10 §4) |
| `capture_version` | integer, NOT NULL | Monotonic per (tenant, jurisdiction): amendments are NEW rows referencing the superseded version — the audit append-only discipline (P1.10 step 3); no in-place edit, no delete path |
| `supersedes_registry_id` | uuid, nullable, FK | The version this row replaces (NULL for the first capture) |
| `sign_off_state` | text, NOT NULL, CHECK in {`UNSIGNED`, `SIGNED`} | The human gate. `SIGNED` requires both fields below; the CHECK enforces it |
| `signed_off_by` | text, nullable | The named human signatory (identity reference, §21.3 style — never a credential) |
| `signed_off_at` | nullable timestamp | When the human signed. NOT agent-fillable; UNSIGNED until a human acts |
| `recorded_by_audit_id` | uuid, NOT NULL | The `audit_records` row that recorded this capture (the registry mutation itself is audited — §17.3) |

Design rules the shape enforces by construction:

1. **No universal default** — there is no row with `tenant_id IS NULL`
   acting as a platform-wide policy. Retention lookup is ALWAYS
   (tenant, jurisdiction, data class)-keyed; a miss is `UNSET`.
2. **Jurisdiction-qualified retention** — a retention value is never bare;
   it is always attached to a jurisdiction row and a basis reference. This
   is the LR-062 direction made concrete: "retention basis becomes
   jurisdiction-qualified data".
3. **Sign-off is data, and it is human** — the registry can be FILLED by
   operators (capture), but `SIGNED` can only be set with a human name and
   timestamp; no automated path sets it, and this batch writes no code that
   could. The sign-off act is a human gate input recorded UNSIGNED here.
4. **Append-only amendments** — corrections are new versions superseding old
   ones; the old rows stay (evidence), consistent with the P9.10 §10.B
   append-only conflict analysis.

## 3. How the pieces consume the registry

| Consumer | Consumption |
|---|---|
| P13.13 retention matrix (record `25_p13_13_gdpr_privacy.md` §3) | The per-table matrix fills each table's row by resolving (tenant, jurisdiction, data class) through the registry. A table with no resolved registry row has retention `UNSET` — no deletion action is authorized for it. The matrix is the TABLE-level projection of the registry's TENANT-level capture. |
| P13.13 DSAR export / erasure jobs | Scope and per-class obligations read from the signed registry rows of the subject's tenant. An UNSIGNED registry row blocks the erasure job's obligation determination (fail closed — an UNSIGNED basis authorizes nothing), while DSAR export can proceed on the export-side obligations per the named regime (regime NULL → the job records the regime as UNNAMED and does not assert any regime-specific duty). |
| The P1.10 `RetentionPolicy` configuration type | The registry is the UPSTREAM of the deployment configuration that fills `RetentionSetting::Configured` values: a deployment loader MAY project signed registry rows into a `RetentionPolicy` for the tenant's deployment. The type is unchanged (no code edit in this batch); `Unset` remains the only state this crate constructs without deployment input. |
| P9.10 record (§4/§6/§7/§8.C cells) | When a human signs registry rows, the P9.10 template's cells get their primary-source values FROM the registry capture (the registry is the machine-readable half; the P9.10 note stays the human-readable record of the same facts — one source of truth in data, mirrored in prose by the human filling step, never two independently-maintained copies). |
| Phase 12 backups / DR (consumed, not re-decided) | Residency constraints on backups (where a backup copy may live, how long it may be retained) read the same registry rows; the Phase 12 records' EMPTY cells stay EMPTY until a signed row exists. This record does not modify any Phase 12 artifact. |
| LR-062 closure | The direction closes as DATA CAPTURE + HUMAN GATE: LR-062's residual risk shrinks to exactly what remains after the mechanism exists — the human decision itself (D1) and the human sign-offs. D1 stays OPEN through this batch (context §3 item 7). |

## 4. Jurisdiction lifecycle — the states a capture passes through

| State | Entry condition | What is allowed | Exit |
|---|---|---|---|
| `UNSIGNED` capture | An operator records the tenant's jurisdiction/regime/basis values with citations | Values are queryable for DESIGN and DRAFT purposes; retention actions are NOT authorized; erasure obligation determination is NOT authorized | A named human signs (→ `SIGNED`), or a superseding version is captured |
| `SIGNED` capture | The human sign-off (name + timestamp recorded) | The row may be projected into deployment retention configuration; DSAR/erasure obligation determination may read it; the P9.10 cells may be filled from it | A new version supersedes it (the old row remains as evidence, its signed state frozen) |
| Superseded | A newer version's `supersedes_registry_id` points at it | Read-only evidence; consumers MUST resolve to the newest signed version | Terminal |

The multi-jurisdiction case: a tenant operates assets across jurisdictions —
the registry keys on (tenant, jurisdiction, data class), so one tenant may
hold several jurisdiction rows simultaneously; per-class basis values differ
per row. There is no "tenant default jurisdiction" and this design adds none.

## 5. What this record does NOT do (honest limits)

1. No jurisdiction is named; no privacy regime is asserted or presupposed;
   no GDPR article, CCPA-style statute, or other regime rule is cited as
   applying anywhere — the only regime mention in this record is the P9.10
   record's own structure (which names none).
2. No legal sign-off exists. `signed_off_by`/`signed_off_at` are EMPTY
   everywhere, always, by design.
3. No migration was created: the registry's storage lands with the tenant
   lifecycle work (P13.15, wave 2) that owns tenant-row lifecycle, under the
   P11.18 migration policy (expand-only; the §2 shape is already
   expand-compatible — additive tables, CHECK-not-VALID or new-table CHECKs,
   no index on populated tables).
4. No code changed: `crates/flexcore-storage/src/retention.rs` is consumed
   as-is (the seven-class vocabulary and `Unset` semantics), never edited by
   this batch.
5. The P9.10 record is cited, never rewritten (AGENTS.md §11; read-only
   reference discipline).

## 6. Reviewer verification

1. Confirm §1 refuses the closure-by-decision and §5's limits hold: no
   jurisdiction, regime, or basis VALUE is filled anywhere in this record.
2. Confirm §2's shape has no platform-default row, no fallback value, and
   that the `basis_reference` rule matches the P9.10 §6 filling rule.
3. Confirm §4's lifecycle makes the sign-off a human-only transition.
4. Confirm no ledger checkbox was ticked by this agent and the P9.10 record
   is byte-identical (`git diff -- ledger/phase-09-pilot-qualification/10_data_residency_retention.md` is empty).
