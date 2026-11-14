<!-- P9.02 — Site and asset register reconciliation (decision record). Procedure + cross-check template; every result cell EMPTY. No reconciliation performed: no pilot site exists. -->
# P9.02 — Site and Asset Register Reconciliation (Procedure + Cross-Check Template)

Date: 2026-11-14. Subtask: P9.02 (`ledger_phase_9.md` §P9.02).
Owner role: Integration engineering (with the customer site contact).
Report trace: §23 Stage 9; §10.1; §4 (Asset Registry, Portfolio & Sites).
Inputs: P9.01 freeze (template — UNSIGNED, `01_pilot_scope_freeze.md`);
customer asset lists, nameplates, single-line diagrams; site EMS inventory
exports — **none of which exist yet**.

## 1. Honest status — the plain statement

**No reconciliation has been performed and none is claimed.** There is no
pilot site, so there is no physical reality to reconcile against. This
record delivers (a) the reconciliation PROCEDURE, (b) the exact registry
artifacts the cross-check reads (by citation, §2), and (c) the cross-check
table TEMPLATE with every value/result cell EMPTY. Every result cell stays
EMPTY until a human runs the procedure against a real site; presenting an
EMPTY cell as a passed check would be the failure mode this phase exists
to prevent.

## 2. Registry artifacts the cross-check reads (by citation)

The reconciliation compares the **FlexCore registry content** — not
documentation about it — against customer/site records. The artifacts and
what each supplies:

| Artifact | Supplies | Cross-checked field group |
|---|---|---|
| `crates/flexcore-application/src/register_asset.rs` | asset registration use case (§10.1 fields) | asset ID, type, vendor/model, rated power (MW) / energy (MWh), phase, bus reference |
| `crates/flexcore-application/src/query_registry.rs` | registry read path (the extract source) | all asset + site fields, per site |
| `crates/flexcore-application/src/update_asset_metadata.rs` | correction path (disposition "corrected in registry") | capability profile, metadata fields |
| `crates/flexcore-application/src/deactivate_asset.rs` | registry lifecycle (retired/unknown assets) | asset state vs customer list |
| `crates/flexcore-domain/src/entities.rs` | canonical Asset/Site entities | field names and units the extract must carry |
| `crates/flexcore-domain/src/capability_provenance.rs` | measured / configured / inferred / unknown provenance (§11.1) | capability-profile provenance check (§5) |
| `crates/adapters/postgres/migrations/0002_tenants.up.sql` … `0006_asset_state.up.sql`, `0009_registry_lifecycle_p2_03.up.sql` | the persisted registry schema (tenants, portfolios, sites, assets, asset state, lifecycle) | site records: portfolio, location, timezone, PCC, grid-model reference |
| `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql` | edge device registry | site edge identity (§10.1) |
| `crates/flexcore-storage/src/command_store.rs` + `crates/adapters/postgres/src/command_store.rs` + `migrations/0016_commands_p4_10.up.sql` | command store schemas | proof that pilot asset IDs resolve in the command path (a schedule/command built from a phantom asset ID must fail, not silently pass) |
| `crates/adapters/postgres/migrations/0012_correction_versions_p2_31.up.sql` | correction versioning | the audit trail for every "corrected in registry" disposition |

The registry extract for the cross-check is produced from the query path
(`query_registry.rs`) against the Postgres adapter; the extract is
versioned and dated and filed with the reconciliation log.

## 3. Fail-closed rule and procedure

**Any unresolved mismatch BLOCKS the freeze (P9.01) — fail-closed.** The
P9.01 sign-off cannot complete while a discrepancy row has no disposition;
a "reconcile later" row is an unresolved discrepancy, not a disposition.

Procedure (per site, human-executed):

1. Produce the versioned registry extract (query path, §2) for the site's
   frozen asset list (P9.01 §3.C).
2. For every asset: compare registry value vs customer asset list vs
   nameplate vs single-line diagram — asset ID, type, vendor/model, rated
   power (MW) / energy (MWh), phase, bus reference, capability profile.
   Fill one §4 row per asset.
3. For every site record: compare portfolio, location, timezone, PCC, and
   grid-model reference vs the customer's records; confirm the edge
   identity exists in the edge device registry (§2).
4. Assign exactly one disposition to every discrepancy: `corrected in
   registry` (via `update_asset_metadata.rs`; correction versioned per
   0012) / `customer data wrong` (customer re-issues the record) /
   `needs site visit` (blocks until resolved).
5. Confirm §5 (capability provenance) for every asset.
6. Re-run the full procedure after any correction; record the FINAL CLEAN
   PASS (zero unresolved rows) with date and extract version. Only the
   final clean pass is the P9.02 evidence for `CHECKLIST.md`.
7. Independent verification (ledger): a second person re-checks at least
   20% of assets — or all assets if fewer than 25 — against nameplates and
   diagrams, independently of the first pass.

Vendor data rule (ledger risk): values copied from a vendor datasheet that
does not match the installed firmware are treated as UNVERIFIED until the
site confirms them; a datasheet is never a substitute for a nameplate.

## 4. Cross-check table template — ALL RESULT CELLS EMPTY

### 4.A Asset reconciliation (one table per site; rows added per frozen asset)

| Site ID | Asset ID | Field | Registry value | Customer list | Nameplate / diagram | Match? | Disposition |
|---|---|---|---|---|---|---|---|
| `<SITE-ID-1>` | `<ASSET-ID-1>` | type | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | vendor/model | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | rated power (MW) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | rated energy (MWh) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | phase | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | bus reference | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | `<ASSET-ID-1>` | capability profile (§11.1) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Canonical units: MW / MVAr / MVA / MWh; reactive-power sign declared per
grid model and device profile (never assumed in a nameplate transcription);
SOC a fraction in [0,1] at the API boundary.

### 4.B Site record reconciliation

| Site ID | Field | Registry value | Customer record | Match? | Disposition |
|---|---|---|---|---|---|
| `<SITE-ID-1>` | portfolio | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | location | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | timezone | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | PCC | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | grid-model reference | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<SITE-ID-1>` | edge identity | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

### 4.C Final clean pass record

| Site ID | Extract version | Clean pass date | Independent check sample (≥20% or all) | Checker 2 name | Zero unresolved rows? |
|---|---|---|---|---|---|
| `<SITE-ID-1>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 5. Capability-profile provenance check (§11.1)

Every pilot asset's capability profile must state, per constraint, whether
the value is `measured`, `configured`, `inferred`, or `unknown` — backed by
`crates/flexcore-domain/src/capability_provenance.rs`. Template:

| Asset ID | Constraint | Value | Units | Provenance (measured/configured/inferred/unknown) | Evidence |
|---|---|---|---|---|---|
| `<ASSET-ID-1>` | `<EMPTY>` | `<EMPTY>` | MW / MVAr / MWh / fraction [0,1] | `<EMPTY>` | `<EMPTY>` |

An `inferred` or `unknown` value that a later subtask (envelope, feasibility,
scheduling) would consume as measured is a discrepancy with disposition
`needs site visit` — it blocks, fail-closed.

## 6. What this procedure does NOT establish

- It does not prove the registry matches reality (no site, no run).
- It does not validate the network model (P9.03) — the grid-model
  reference cell only records that a reference exists.
- It does not upgrade the P9.01 freeze; it is a blocking input to it.
- It does not verify telemetry quality or clock sync (P9.18).

## 7. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| Stale customer asset lists; unrecorded replacements or retrofits | ledger P9.02 risk row; no mitigation exists before a real site |
| Capability values from datasheets that do not match installed firmware | §3 vendor-data rule; provenance check §5 cannot run yet |
| Registry schema vs site reality drift (PCC, grid-model reference absent pre-P9.03) | §4.B cells cannot be filled until P9.03 produces a model reference |
| The command-store ID-resolution proof (§2) is itself UNRUN — no command-store round trip has ever been executed against Postgres | crates are WRITTEN-UNRUN (Phase 1–8 standing caveat) |

## 8. Reviewer verification procedure

1. Confirm every §4/§5 result cell is `<EMPTY>` and no site/asset row
   contains a real name.
2. Trace each §2 citation to the file; confirm the listed migrations exist
   in `crates/adapters/postgres/migrations/`.
3. Confirm §3 step 7 states the ≥20%-or-all independent re-check.
4. Confirm the fail-closed statement (§3 first paragraph) is unconditional.
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the procedure is written; it has NOT been executed and
   must never be reported as "reconciled" or "passed".

## 9. Filling commands (for the human executor)

- Extract: run the registry query path against the pilot deployment's
  Postgres instance (the extract command belongs to the deployed system's
  operator runbook — this loop builds and runs nothing).
- Fill §4/§5 per asset/site during the site visit; record dispositions;
  re-run until §4.C records a clean pass.
- Only after §4.C: unblock the P9.01 signature and P9.03 inputs.
- Ledger tick (after the clean pass is filed): `[x]` on P9.02 is justified
  by the ledger acceptance criteria; today's `[x]` means **implemented,
  awaiting human build/test**.

Status: **WRITTEN-UNRUN.** No pilot exists yet and none is claimed; no
reconciliation, extract, or site visit has occurred or is scheduled.
