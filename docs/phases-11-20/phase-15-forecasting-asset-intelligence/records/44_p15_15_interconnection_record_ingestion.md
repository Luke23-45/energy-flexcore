# P15.15 — Interconnection-record ingestion: UL 1741 SB COCs, IEEE 1547.1-2020 test reports, certified function categories, settings profiles per device model

| Field | Value |
|---|---|
| Subtask | P15.15 (UL 1741 SB COC, IEEE 1547.1-2020 test reports, certified function category, settings profile per device model; a document store with model-number linkage to the registry; the platform LISTS certified equipment — it never claims certification for itself [S-21][S-169][S-170]; postgres document-store pattern, next free migration, expand-only per MIGRATION_POLICY.md) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the domain vocabulary, storage contract, adapter, and migration are written, never compiled, never migrated, never executed. No certification document was ingested; no record exists; the platform makes NO certification claim anywhere. |
| Sources | [S-21] (CEC: the UL 1741 SB addition to the Solar Equipment Lists — certified equipment is LISTED by certification bodies), [S-169] (the CHINT IEEE 1547-2018 / UL 1741 SB bulletin), [S-170] (UL 1741 SB vs IEEE 1547.1-2020 test procedures) |
| Files touched | `crates/flexcore-domain/src/interconnection_records.rs` (NEW); `crates/flexcore-domain/src/lib.rs` (additive module line); `crates/flexcore-storage/src/interconnection_store.rs` (NEW); `crates/flexcore-storage/src/lib.rs` (additive module line); `crates/adapters/postgres/src/interconnection_store.rs` (NEW); `crates/adapters/postgres/src/lib.rs` (additive module line); `crates/adapters/postgres/migrations/0021_interconnection_records_p15_15.{up,down}.sql` (NEW) |

## 1. What landed

| Layer | Item | Content |
|---|---|---|
| Domain | `InterconnectionRecord` | Per DEVICE MODEL: document kind (closed: `Ul1741SbCoc`, `Ieee1547TestReport`), document reference + ISSUER's version label + issuer reference (all recorded data, never invented), the abnormal-condition-performance category (`CategoryA` / `CategoryB` / `NotClassified` — UNVERIFIED vocabulary label carried per the P14.13 discipline), the certified-function list REUSED from the P14.13 `SettingFunctionId` catalog (single home), ride-through categories (`RideThroughCategory`), the settings-profile reference into the P14.13 profile repository, ingest time (distinct from any document date, P0.09) and ingestion source |
| Domain | `InterconnectionDocumentStore` | The P14.13 `SettingsDocumentStore` pattern mirrored: BTreeMap, deterministic ordering, register-only (no update, no delete); the (device_model, document_ref) identity collision is a named refusal — a new document version is a NEW record |
| Domain | Linkage | EXACT string equality on the registry's `vendor_model` (P0.10 §1 row 4): no fuzzy, no prefix, no case/whitespace normalization; a near-miss resolves to NOTHING and FLAGS (`MODEL_LINKAGE_IS_EXACT_STATEMENT`; the P8.11 exact-equality discipline mirrored) |
| Storage | `InterconnectionRecordStore` | The `envelope_store` pattern (same recorded D-P3.21-4 home deviation): insert + per-device-model list, `Conflict` on the identity collision, opaque `Backend` failures, INSERT/SELECT-only adapter obligation |
| Postgres | adapter | `impl InterconnectionRecordStore for PgUnitOfWork` (the P3.28 transaction pattern); the adapter owns the JSON binding and validates on insert that `certified_functions` / `ride_through_categories` parse as arrays whose EVERY entry names a closed P14.13 catalog/category — an unknown name is refused, never stored (fail-closed) |
| Postgres | migration 0021 | `interconnection_records`: CREATE TABLE (expand-only, always safe per MIGRATION_POLICY §2), (device_model, document_ref) UNIQUE, CHECK constraints on the closed vocabularies and text bounds, INSERT+SELECT grants only (immutable records), NO tenant RLS — see the tenancy decision below |

## 2. Recorded decisions

1. **The platform LISTS certified equipment — it never claims certification for itself, and never certifies equipment** (`PLATFORM_LISTS_NEVER_CERTIFIES_STATEMENT`, pinned by tests). A record is a REFERENCE to a document issued by a certification body / laboratory; certification belongs to the document and its issuer [S-21].
2. **Tenancy: a platform-global equipment catalog.** Certification is a property of the equipment MODEL, not of any tenant's instance; the table carries NO tenant_id and NO tenant RLS (the same posture as the P14.13 jurisdiction document store). Recorded in the migration header, the storage trait header, and the adapter header.
3. **No device-model FK.** The registry carries `vendor_model` on assets, not a device-model catalog; an FK would invent one. The linkage is the exact-string contract.
4. **Category vocabulary UNVERIFIED (secondary-source transcription)**: Category A / B is cited from [S-169][S-170] secondary material over IEEE 1547-2018 [S-20]; the implementation-time check against the [S-20] text must confirm the letters and assignment rules (the P14.13 UNVERIFIED-label discipline mirrored).

## 3. Honest boundaries

1. No document was ingested; the store is empty everywhere; no record of any listed device exists.
2. No certification, listing, or conformance claim is made by or for FlexCore anywhere in these files.
3. Every test is WRITTEN-UNRUN; migration 0021 has never been applied; the permitted-tool outputs are quoted in the close-out record (48).
