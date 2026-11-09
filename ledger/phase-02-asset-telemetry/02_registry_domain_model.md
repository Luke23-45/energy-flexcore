# P2.02 — Registry domain model (tenant / portfolio / site / asset)

| Field | Value |
|---|---|
| Subtask | P2.02 — Design the registry domain model |
| Report trace | §10.1, §2.2, §4 (Asset Registry, Portfolio & Sites), §1.4; P0.10 |
| Inputs | P2.01 (entry note — gate unheld, recorded exception); P0.10; P1.06 (domain entities already in skeleton form) |
| Status | Implemented alongside P2.03 (this document records both the design and the already-implemented / newly-added split) |
| Entry-gate caveat | The Stage 1 exit gate was NOT held (see `ENTRY_NOTE.md`); nothing here may state that the gate passed |
| Simulation status | Registry vocabulary for the 0.x simulation-only line; no field reads, no device writes, no control authority above Simulation (§21.4) |

## 1. Entity graph and cardinalities

```text
Tenant 1───* Portfolio 1───* Site 1───* Asset
                                    │
Tenant 1───* Site (derived, through the Portfolio)   Asset *───1 bus reference
Site 1───1 PCC (point of common coupling)
Site *───1 GridModel reference (model + version, as a reference string)
```

| Relationship | Cardinality | Enforcement | Status |
|---|---|---|---|
| Tenant — Portfolio | 1 — * | `Portfolio::try_new` takes the parent `Tenant` by reference and derives `tenant_id`; a portfolio cannot exist without a tenant (compile-level) | **Already implemented in P1.06** (`crates/flexcore-domain/src/entities.rs`) |
| Portfolio — Site | 1 — * | `Site::try_new` takes the parent `Portfolio`; `tenant_id` derived through it, so a site can never carry a different tenant than its portfolio | **Already implemented in P1.06** |
| Site — Asset | 1 — * | `Asset::try_new` takes the parent `Site`; the full ownership chain (asset → site → portfolio → tenant) is derived, never supplied | **Already implemented in P1.06** |
| Site — PCC | 1 — 1 | `Site.point_of_common_coupling`: a validated non-empty reference string (§10.1 "PCC") | **Already implemented in P1.06** |
| Site — GridModel | * — 1 | `Site.grid_model_reference`: a reference string carrying model + version (e.g. `grid-model:rev-42`). The `GridModel` §10.1 entity itself is a later subtask; Phase 2 stores the reference only | **Already implemented in P1.06** (as a reference; the GridModel aggregate is out of Phase 2 scope) |
| Asset — bus | * — 1 | `Asset.bus_reference`: a validated non-empty reference into the grid model | **Already implemented in P1.06** |
| Tenant — Asset | 1 — * (derived) | Every tenant-owned entity carries `tenant_id` derived from its parent (P0.16 rule 1) | **Already implemented in P1.06** |

No entity, field, or relationship was redefined by P2.02/P2.03: the P1.06
types are the base, and the P2.03 additions below extend them.

## 2. Asset fields and the asset-type enum

### 2.1 Asset fields (report §10.1 row 4)

| Field | Type in `flexcore-domain::entities::Asset` | Status |
|---|---|---|
| `asset_id` | `AssetId` (typed wrapper over `FlexUuid`; P1.05) | **Already implemented in P1.06** |
| site | ownership derived from the parent `Site` | **Already implemented in P1.06** |
| type | `AssetType` enum (below) | **Already implemented in P1.06** |
| vendor/model | `vendor_model: String` (validated non-empty) | **Already implemented in P1.06** |
| rated power | `rated_power_mw: ActivePowerMW` — canonical MW (P0.07), strictly positive | **Already implemented in P1.06** |
| rated energy | `rated_energy_mwh: Option<EnergyMWh>` — canonical MWh (P0.07), strictly positive when present (storage assets) | **Already implemented in P1.06** |
| phase | `phase: String` — validated non-empty reference; no phase enumeration is frozen in P0.10, none invented (P0.10 §1 risk note) | **Already implemented in P1.06** |
| bus reference | `bus_reference: String` (validated non-empty) | **Already implemented in P1.06** |
| capability profile | `capability_profile: String` — **reference only in Phase 2** (§4 below) | **Already implemented in P1.06** |
| lifecycle status | `lifecycle_status: AssetLifecycleStatus` + `deactivation_reason` + `deactivated_at` | **Added in P2.03** (deactivation transition `Asset::deactivate`, terminal in Phase 2) |

### 2.2 Asset-type enum (report §2.2; P0.10 §2)

Exactly seven first-class types, frozen:

| Report §2.2 row | Enum variant | Status |
|---|---|---|
| BESS | `AssetType::Bess` | **Already implemented in P1.06** |
| PV inverter | `AssetType::PvInverter` | **Already implemented in P1.06** |
| Flexible C&I load | `AssetType::FlexibleCiLoad` | **Already implemented in P1.06** |
| EVSE / EV fleet | `AssetType::EvseEvFleet` | **Already implemented in P1.06** |
| Site meter | `AssetType::SiteMeter` | **Already implemented in P1.06** |
| Wind (generic interface) | `AssetType::WindGeneration` | **Already implemented in P1.06** |
| Thermal storage / heat pump (generic envelope) | `AssetType::ThermalStorageHeatPump` | **Already implemented in P1.06** |

**Backup generation is NOT a first-class type, and the exclusion is
deliberate** (recorded in P0.10 §2 and carried in the P1.06 enum docs): it
may only be added with emissions, fuel, permit, and operating-policy
support — none of which the 0.x line models. It is not part of the frozen
vocabulary and may not be smuggled in as a stringly-typed variant.

## 3. Site and Portfolio fields; reserved / label-only markings

### 3.1 Site fields (report §10.1 row 3)

| Field | Status |
|---|---|
| `site_id`, portfolio (derived), location, timezone (`SiteTimezone`, validated; canonical timestamps stay UTC per P0.09) | **Already implemented in P1.06** |
| PCC (1—1, validated reference) | **Already implemented in P1.06** |
| Grid-model reference (model + version) | **Already implemented in P1.06** |
| **Edge identity** | Field exists and is validated/tenant-scoped (P0.16 rule 7), but it is **RESERVED / NOT POPULATED by a live enrollment**: no edge agent exists in Phase 2 (Phase 6 per the release ladder). The value is a tenant-scoped reference string only; no code may present it as evidence of a live edge enrollment, and the P2.17 views must report edge connectivity honestly ("not connected / simulator") |

### 3.2 Portfolio fields (report §10.1 row 2)

| Field | Status |
|---|---|
| `portfolio_id`, tenant (derived), operator, aggregation rules | **Already implemented in P1.06** |
| **Market region** | Field exists (`market_region: String`), but it is a **LABEL, NOT LOGIC**: decision **D1 is OPEN** (jurisdiction-specific code is excluded until D1 closes — `.agent/DECISIONS.md`), so `market_region` must not drive any market, settlement, or jurisdiction behaviour in Phase 2. It is displayed and stored; nothing branches on it. (Carried forward from `ENTRY_NOTE.md`: "market_region stays a label") |

## 4. Capability profile — reference only in Phase 2

Each asset carries a capability-profile **reference** (`capability_profile`),
per P0.10 §1 row 4. The profile's **contents** (flexibility envelope
semantics, P0.10 §5) are **Phase 3** work: Phase 2 stores and re-points the
reference and keeps the asset's rated power/energy as the only capability-
adjacent quantities. No Phase 2 code may read the reference as if the
profile existed, and the P2.28 handover restates this boundary to Phase 3.
`UpdateAssetMetadata` (P2.03) may re-point the reference; it cannot create or
modify profile contents.

## 5. Registry invariants

| Invariant | Enforcement | Status |
|---|---|---|
| Portfolio belongs to exactly one tenant | Derived `tenant_id` from the parent `Tenant` (compile-level) | **Already implemented in P1.06** |
| Site belongs to exactly one portfolio | Derived from the parent `Portfolio` | **Already implemented in P1.06** |
| Asset IDs unique within a tenant | Typed `AssetId` uniqueness enforced by the repositories (ports + fakes + P1.13 adapter conflict on the identifier); the identifier is globally unique, hence unique within the tenant. The recorded tenant-scoped natural key is asset-name-within-site (repository `Conflict` rule; a recorded assumption, not a frozen rule) | **Already implemented in P1.07/P1.13** |
| Cross-tenant access rejected | `TenantContext` derived only from authenticated claims (P0.16 rule 5); repositories read foreign rows as missing (no existence leak) and refuse cross-tenant writes (`TenantMismatch`); the application use cases re-check parents through the context | **Already implemented in P1.06–P1.08; exercised by the new P2.03 tests** |
| Deletion follows a recorded rule | **RESTRICT for non-empty parents — decided once** (§6) | **Decided in P2.02; guards added in P2.03** (`flexcore-domain::registry_rules`) |
| Asset lifecycle | Two states, `Active` → `Deactivated` (terminal), reason mandatory | **Added in P2.03** |

## 6. Deletion rule — DECIDED ONCE: RESTRICT for non-empty parents

**Decision.** A registry node that still owns dependents cannot be deleted:

- a tenant that still owns portfolios cannot be deleted;
- a portfolio that still owns sites cannot be deleted;
- a site that still owns assets cannot be deleted.

Empty parents and leaves may be deleted through the normal CRUD surface
(P2.12 `DELETE` routes enforce the guard via `registry_rules::check_*_deletion`).

**Rationale (recorded; not re-decidable per call site):**

1. **Cascade would orphan referenced state.** Every downstream artefact
   carries the ownership chain: canonical telemetry (P2.04) stores
   tenant/site/asset IDs; the §15.3 events carry the full chain; deterministic
   replay (P2.24) compares stored records against regenerated ones. A cascade
   delete would either strand telemetry pointing at unresolvable identifiers
   or destroy the historical record.
2. **Restrict is the safe default under incomplete information.** Deletion can
   always be re-issued after the dependents are explicitly removed — each
   removal individually visible and auditable. An accidental cascade cannot
   be undone.
3. **The audit trail is preserved either way.** An accepted deletion is a
   working-data state change (`StateChangeOperation::Delete`) paired with an
   outbox event (§6.3); the append-only audit history (P1.10 `AuditSink`,
   INSERT-only at three enforcement layers) is never cascaded — audit records
   outlive the registry rows they describe by design. No deletion path in any
   phase may delete audit records.

**Mechanical form.** `flexcore-domain::registry_rules` (added in P2.03)
exposes three pure guards — `check_tenant_deletion(portfolio_count)`,
`check_portfolio_deletion(site_count)`, `check_site_deletion(asset_count)` —
returning `DeletionRuleError::NonEmptyParent { entity, dependent, count }`.
The domain stays I/O-free: the caller (P2.12 API layer, backed by the
tenant-scoped `list_for_tenant` ports) supplies the child counts. One
spelling of the rule, cited by fakes, adapters, and API tests alike.

## 7. P2.03 implementation map (what existed vs what was added)

**Already present from P1.06–P1.08 (extended, not recreated):** the domain
entities (Tenant/Portfolio/Site/Asset/AssetState) with deriving
constructors, typed IDs, canonical units, PCC, grid-model reference, bus
reference, asset-type enum, capability-profile reference field; the
repository ports; and the four registry creation use cases (`RegisterTenant`,
`CreatePortfolio`, `CreateSite`, `RegisterAsset` emitting `AssetRegistered`).

**Added by P2.02/P2.03:**

| Artefact | Location | Notes |
|---|---|---|
| `AssetLifecycleStatus` (`Active`/`Deactivated`), `Asset::deactivate` (terminal, reason mandatory), `Asset::update_capability_metadata` (revalidating, identity/ownership untouched) | `crates/flexcore-domain/src/entities.rs` | New `EntityError::AlreadyDeactivated` variant |
| RESTRICT deletion guards + rationale record | `crates/flexcore-domain/src/registry_rules.rs` | Pure functions; unit-tested |
| `UpdateAssetMetadata` — emits frozen `AssetCapabilityUpdated` via the outbox | `crates/flexcore-application/src/update_asset_metadata.rs` | Outbox-only; never direct NATS |
| `DeactivateAsset` — emits frozen `AssetAvailabilityChanged` via the outbox | `crates/flexcore-application/src/deactivate_asset.rs` | Reason persisted on the asset record; see the audit gap below |
| `QueryRegistry` — tenant-scoped list/get, thin wrappers over the repository ports | `crates/flexcore-application/src/query_registry.rs` | Reads emit no events |
| Tests against the fakes (`FakeWriteJournal` ordering, outbox-key assertions, cross-tenant + missing-entity + revalidation rejections) | `#[cfg(test)]` modules of the three use cases | `TenantContext` constructed via the documented `VerifiedClaims → AuthenticatedClaims::from_verified → TenantContext` path |

## 8. Recorded gaps and assumptions

- **AUDIT-SINK GAP (P2.03, blocking-free):** the ledger step for
  `DeactivateAsset` names an audit-record write via the `AuditSink` port.
  That port is single-homed in `flexcore-audit` by the recorded P1.10
  decision (it is explicitly not a repository trait, so `flexcore-ports`
  does not define it), and `tools/check_dependency_rule.py` forbids a
  `flexcore-application → flexcore-audit` edge without an ADR-granted
  exception in `ALLOWED_EXTRA_EDGES`. Both changes are outside this
  subtask's edit scope, so the direct append is NOT implemented. Audit
  evidence that exists today: the deactivation reason and timestamp are
  persisted on the asset record (repository `Update` state change, visible
  in the write journal), and the `AssetAvailabilityChanged` outbox record
  carries the fact. Resolution path (either): grant the dependency edge via
  ADR and add the `AuditSink` parameter to `DeactivateAsset::new`; or append
  the audit record at the composition root, where the unit of work
  implementing `AuditSink` (P1.13 `PgUnitOfWork`) is already open.
- **Uniqueness rules** (asset name within site, portfolio name within
  tenant, one tenant per identity-provider reference) are recorded
  assumptions from P1.07/P1.08, not frozen rules; they are reused unchanged.
- **Deactivation is terminal** in Phase 2; no reactivation capability exists
  or is claimed.
- **Market region and edge identity** carry the §3 markings above; the
  P2.12/P2.17 implementers must preserve them.
- **Entry-gate caveat** (from `ENTRY_NOTE.md`): the Stage 1 exit gate was
  not held; this design proceeds under the recorded exception and claims no
  gate pass.
