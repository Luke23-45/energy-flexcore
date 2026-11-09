# P2.04 — Canonical telemetry model and units (design)

| Field | Value |
|---|---|
| Subtask | P2.04 — Design the canonical telemetry model and units |
| Report trace | §10.2–§10.4 (normative), §3.4 (workflow steps 3–4), §10.1 (AssetState), §16.1 (storage consumer); P0.07–P0.09 |
| Inputs | P2.02 (registry model); P0.07 (units/sign), P0.08 (quality), P0.09 (time); P1.05 (`crates/flexcore-contracts/src/units.rs`, `time_model.rs`, `quality.rs` — the implemented contract types) |
| Status | Design for Phase 2 implementation (consumed by P2.06, P2.09, P2.11, P2.14). No code is written by this document |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them. Decisions D-T1…D-T6 below are made once for Phase 2 |

---

## 1. Scope

This document defines the canonical telemetry record — fields, types,
nullability, units, sign, time, quality, and source identity — that every
telemetry path in Phase 2 produces and stores: the `TelemetrySource` port
output (P2.11), the simulator (P2.14), the ingestion pipeline (P2.07), and the
PostgreSQL storage (P2.05–P2.06). It cites P0.07–P0.09 and does not re-define
them; where a rule is restated, it is restated verbatim in substance.

Consumers that depend on decisions made here: P2.06 (migrations/repositories),
P2.09 (dedup identity), P2.11 (port contract), P2.14 (simulator emission),
P2.24 (replay comparison fields), P2.31 (versioned corrections).

## 2. Decisions made once

| ID | Decision | Rationale | Alternative considered and rejected |
|---|---|---|---|
| D-T1 | `site_id`/`portfolio_id` are **not stored on telemetry rows**; they are resolved by JOIN through `assets` → `sites` → `portfolios` (the Phase 1 registry tables). Only `tenant_id` is denormalized onto every telemetry row | `tenant_id` must be on every tenant-owned row for RLS and query scope (P0.16 rule 1; §16.2) — it is denormalized for that reason alone, and its consistency is proven by a composite FK to `assets(asset_id, tenant_id)` (P1.12 §4.5 pattern). Site/portfolio are stable registry relationships resolvable with a cheap join against small current-state tables; denormalizing them would fan out writes on every sample and risk drift if a registry relationship is ever corrected | Denormalizing `site_id`/`portfolio_id` onto telemetry rows: rejected — three copies of one relationship per row, no RLS or query need, and a consistency burden the composite-FK pattern gives for free |
| D-T2 | One canonical record represents **one quantity of one asset at one `event_time`**. A multi-quantity sample is a set of records sharing `source_id`, `source_sequence`, and `event_time` | Matches the report's replay comparison fields (§19.2/P2.24: `asset_id`, `event_time`, quantity, value, quality, `source_sequence`), enables the one-table-with-`quantity`-column storage shape (P2.05 D-P1), and gives every quantity its own quality state — required by the P0.08 exactly-one-per-value rule, which is per *value*, not per sample | One record carrying all quantities of a sample: rejected — a sample-level quality field would violate the per-value exactly-one rule (P0.08 §2) and would couple unrelated quantities' quality |
| D-T3 | `telemetry_id` is a **technical surrogate** (UUIDv7, assigned at persistence, `DEFAULT uuidv7()`), not the business identity. The dedup/business identity is `(asset_id, event_time, source_sequence)` per sample — the row-level unique key adds `quantity` (P2.09 alignment; full interaction recorded in P2.05 §5) | The source-assigned identity (P0.09 §1 "source sequence or version") is what duplicate detection needs (P2.09); a surrogate UUID adds nothing to dedup and cannot carry uniqueness on a partitioned table without including the partition key (`event_time`). UUIDv7 insertion-order locality is retained without using it for ordering | `(telemetry_id, event_time)` as PK: rejected — dedup would then need a second unique constraint on the composite identity, doubling the unique-index cost for no gain; `ORDER BY telemetry_id` is prohibited anyway (P1.12 §2.3 caveat) |
| D-T4 | The reactive-power sign convention is **carried on the record**: a `q_sign_convention` column, `NOT NULL` exactly when `quantity = 'reactive_power'`, with values matching the implemented `ReactiveSignConvention` (`INJECTION_POSITIVE` / `CONSUMPTION_POSITIVE`) | P0.07 §3: platform contracts carry the declared convention explicitly, never inferred from sign alone. Carrying it per row makes every stored Q value self-describing and history-safe if a convention is ever re-declared for an asset. The adapter additionally records the unit + sign-mapping profile it applies at the boundary (P0.07 §5, provider declaration per P2.11 step 4) — the profile is the conversion record; the row is the interpretation record | Convention on the asset profile reference only (row carries the bare number): rejected — a consumer of history could not interpret stored Q values without resolving a mutable profile, and a re-declared convention would silently reinterpret old rows |
| D-T5 | `clock_quality` is a **`NOT NULL`** field with an explicit `UNKNOWN` value (matching the implemented `ClockQuality` enum: `SYNCHRONIZED` / `UNSYNCHRONIZED` / `UNKNOWN`). A source that does not report synchronisation status records `UNKNOWN` — never a silent assumption of good | P0.09 §4: synchronisation status is a recorded field, never assumed; `UNKNOWN` is the honest recorded state and the non-silencing analogue of the P0.08 rule for clock quality | `NULL`-able clock quality: rejected — `NULL` is ambiguous between "not reported" and "not modelled"; the explicit third value keeps the field total |
| D-T6 | **Derived records** (MISSING gap markers, and ESTIMATED only if a derivation rule is ever configured, P2.08) use the reserved platform source namespace `source_id = 'flexcore:derived:<detector>'` and a detector-owned monotonic `source_sequence`. Measured records must never use the `flexcore:` source namespace | The identity rule (§8) is stated for source-measured records; derived records still need a total identity for storage and dedup. The reserved namespace makes derived records distinguishable from measured values by construction (P2.08 acceptance: MISSING markers are distinguishable) and prevents a detector from colliding with a real source sequence | Leaving derived-record identity undefined: rejected — P2.08 stores MISSING markers and P2.31 versions corrections; an undefined identity would make the storage key non-total |

## 3. Canonical record fields

The canonical record (the `TelemetrySource` decode output per P2.11; the
persisted row per P2.05 §3). One row per `(asset_id, event_time, quantity)`
within a source sample (D-T2). Every field below has a concrete type and
nullability; no field is TBD.

| # | Field | Type | Nullability | Definition and notes |
|---|---|---|---|---|
| 1 | `telemetry_id` | `uuid` (UUIDv7) | `NOT NULL` on the persisted row (assigned at persistence via `DEFAULT uuidv7()`); absent from the in-flight record (D-T3) | Technical surrogate only. Never used for dedup, ordering, or business identity (P1.12 §2.3: UUID ordering is approximate and never business ordering) |
| 2 | `tenant_id` | `uuid` | `NOT NULL` | The asset's tenant (P0.16 rule 1). Derived from the authenticated, tenant-scoped source context (P2.22); proven consistent with `asset_id` by composite FK (P2.05 §3). `site_id`/`portfolio_id` are not fields (D-T1) |
| 3 | `asset_id` | `uuid` | `NOT NULL` | The measured asset; must exist in the Phase 1 `assets` registry (P2.02) |
| 4 | `quantity` | text — frozen Phase 2 quantity key (§4) | `NOT NULL` | Which measured quantity this record carries; selects the value kind and canonical unit (§4 mapping) |
| 5 | `event_time` | UTC timestamp (`timestamptz`; contract type `EventTime`) | `NOT NULL` | When the measurement occurred (P0.09 §1). UTC only; non-UTC offsets are rejected at construction, never converted (P1.05 `time_model.rs`) |
| 6 | `ingest_time` | UTC timestamp (`timestamptz`; contract type `IngestTime`) | `NOT NULL` | When FlexCore received it (P0.09 §1). Distinct from `event_time` by type and by rule; `ingest_time ≥ event_time` is validated at construction (P1.05 `TimeRecord::validate`). Never compared in replay (P2.13/P2.24) |
| 7 | `source_sequence` | `bigint` (contract type `u64`) | `NOT NULL` | Source-assigned ordering/version identifier (P0.09 §1). Monotonically increasing per asset and source (P2.14). Mandatory — a record without it is not decodable (P2.11: decode failure, not a partial record) |
| 8 | `source_id` | text, non-empty | `NOT NULL` | Source identity: which `TelemetrySource` provider instance produced the value (P0.08 §7: telemetry arrives with source identity). The `flexcore:` prefix namespace is reserved for platform-derived records (D-T6) |
| 9 | `quality` | PostgreSQL enum `telemetry_quality` — exactly `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE` (P0.08 §1; created in Phase 1, P1.12 §3.2) | `NOT NULL` | Exactly one of the six canonical states per value (P0.08 §2). Assignment rules are P2.08; this model only fixes that the field is total and drawn from the frozen six |
| 10 | `source_quality_code` | text | `NULL` — set when the source reported a richer original state | Original source quality code retained as diagnostic metadata (P0.08 §3, e.g. `SUBSTITUTED`, `OUT_OF_RANGE`, `COMMUNICATION_LOSS`). Never a seventh canonical state; never silently dropped (P0.08 §4) |
| 11 | `value_num` | `double precision` | `NULL`; `NOT NULL` exactly when the quantity's value kind is numeric (§4) and `quality <> 'MISSING'` | The numeric value in the record's canonical unit (§5). Must be finite — NaN/infinite values are rejected at the contract boundary (P1.05 `UnitsError::NotFinite`) and re-checked at the database (P2.05 §3) |
| 12 | `value_text` | text, non-empty | `NULL`; `NOT NULL` exactly when the quantity's value kind is a state string (§4) | Source-declared state value carried verbatim (e.g. operating mode, health); no canonical vocabulary is frozen for these in Phase 2 (D-T5 companion note below) |
| 13 | `value_bool` | `boolean` | `NULL`; `NOT NULL` exactly when the quantity's value kind is boolean (§4) | Boolean state value (availability, connection state) |
| 14 | `unit` | text — canonical unit vocabulary: `MW`, `MVAr`, `MVA`, `MWh`, `1` | `NOT NULL` | The canonical unit of the stored value (§5). `1` is the dimensionless unit (SOC fraction, boolean and state values). No kW/kWh and no other scaled unit appears, ever (P0.07 §1) |
| 15 | `q_sign_convention` | PostgreSQL enum matching the contract `ReactiveSignConvention`: `INJECTION_POSITIVE` / `CONSUMPTION_POSITIVE` | `NOT NULL` exactly when `quantity = 'reactive_power'`; `NULL` otherwise (enforced by CHECK, P2.05 §3) | The declared convention under which this Q value's sign is to be read (P0.07 §3; D-T4). There is no default and no universal Q sign |
| 16 | `clock_quality` | PostgreSQL enum matching the contract `ClockQuality`: `SYNCHRONIZED` / `UNSYNCHRONIZED` / `UNKNOWN` | `NOT NULL` (D-T5) | Recorded synchronisation status (P0.09 §4). `UNKNOWN` is the explicit recorded-unknown, never a silent assumption |

Value-field totality rule (one CHECK constraint at storage, P2.05 §3): exactly
one of `value_num` / `value_text` / `value_bool` is non-null, except that all
three are `NULL` exactly when `quality = 'MISSING'` — a MISSING record is a
derived gap marker with no value (P0.08 §1: "no value was received for the
expected interval"; P2.08 step 4).

Companion note to D-T5 (recorded decision, not an omission): Phase 2 freezes
no canonical vocabulary for `operating_mode`, `health`, or EVSE connection
*text* states — these are source-declared states carried verbatim in
`value_text`, with a non-empty constraint. A canonical state vocabulary, if
one is ever needed, is added by an explicit later decision (mirroring the
P1.12 §4.5 treatment of `asset_state.health`); this is stated so the absence
is a decision, not a gap.

Provenance note (`simulated` flag): the Phase 2 provider set is exactly one —
the simulator (P2.11 step 5). The API layer derives the P2.17/P2.18
`simulated: true` marking from the record's `source_id` belonging to the
registered simulator source; no separate column exists in Phase 2. If mixed
provenance providers exist later, provenance becomes an explicit
source-registry attribute — recorded then, never inferred.

## 4. Quantity vocabulary and per-asset measured quantities

The Phase 2 quantity set (frozen here; extension is an explicit migration and
decision record, never a silent widening — mirroring the P1.12 §4.4 asset-type
rule). Every quantity names its canonical unit per P0.07 §1.

| Quantity key | Value kind | Canonical unit | Notes |
|---|---|---|---|
| `active_power` | numeric | `MW` | Sign rule per §6: P > 0 = export at the point of connection |
| `reactive_power` | numeric | `MVAr` | Sign per the declared `q_sign_convention` on the record (D-T4); never inferred from sign alone |
| `soc` | numeric | `1` (fraction) | Fraction in [0,1] at the API boundary (P0.07 §4); enforced by CHECK at storage (P2.05 §3). Percent never persisted (P0.07 §4) |
| `energy` | numeric | `MWh` | Interval energy (e.g. EVSE delivered energy where the source reports it); SOC is not energy (P0.07 §1) |
| `availability` | boolean | `1` | Asset availability signal (§10.1 AssetState) |
| `operating_mode` | text state | `1` (dimensionless state) | Source-declared mode, carried verbatim (D-T5 companion note) |
| `health` | text state | `1` (dimensionless state) | Source-declared health, carried verbatim (D-T5 companion note) |
| `connection_state` | boolean | `1` | EVSE connection state (connected / not connected) |
| `baseline_reference` | numeric | `MW` | Flexible-load baseline reference, present only where a baseline is explicitly configured (see the load row below); the open D1/D2 baseline-method question is not decided here |

`MVA` is in the canonical unit vocabulary (P0.07 §1) but **no Phase 2 quantity
emits it** — apparent power is P–Q envelope vocabulary (§11.3), which is Phase
3. Recorded so the absence is explicit.

Per-asset quantities (ledger P2.04 step 2; asset types per P2.02 / §2.2):

| Asset type | Quantities (quantity key — canonical unit) |
|---|---|
| BESS | `active_power` — MW (net export sign per §6; `P_net = P_dis − P_ch`, P0.07 §6); `reactive_power` — MVAr (declared convention); `soc` — fraction [0,1]; `availability` — boolean; `operating_mode` — state; `health` — state |
| PV inverter | `active_power` — MW (generation = export, positive per §6); `reactive_power` — MVAr (declared convention); `availability` — boolean |
| Flexible load | `active_power` — MW (consumption = import, negative per §6); `baseline_reference` — MW, emitted only where the scenario/config explicitly defines a baseline (in Phase 2, the synthetic scenario's documented baseline shape; no baseline method is derived or assumed, D1/D2 open) |
| EVSE / EV fleet | `active_power` — MW (charging = import, negative per §6); `connection_state` — boolean; `soc` — fraction [0,1] where the source reports EV/EV-fleet SOC; `energy` — MWh where the source reports delivered energy |
| Site meter | `active_power` — MW at the PCC (site net export positive per §6); `reactive_power` — MVAr at the PCC (declared convention) |

Wind and thermal storage/heat pump are §2.2 first-class asset types but are
not in the Phase 2 exit-gate portfolio (P2.25: BESS + PV + load + EVSE +
meter); their quantity sets follow the same table when they are first
simulated — the record model is type-agnostic by construction.

## 5. Unit rule

1. **Adapters convert at the boundary.** Every provider — including the
   simulator source (P2.14) — converts vendor/protocol/simulated units into
   the canonical units of P0.07 §1 (MW, MVAr, MVA, MWh; SOC as a fraction in
   [0,1]) before emitting a canonical record, and records the **unit mapping**
   and **sign-mapping profile** it applies (P0.07 §5; declared per provider,
   P2.11 step 4). Do not rely on implicit unit or sign assumptions.
2. **Persisted values are canonical only.** The `unit` column carries the
   canonical unit token (§3 field 14); the (quantity, value kind, unit)
   mapping of §4 is enforced by constraint at storage (P2.05 §3). No kW, kWh,
   or other scaled unit exists in any persisted field, in any API contract, or
   in this document's tables — scaled units are display-only (P0.07 §5).
3. **SOC is a fraction in [0,1]** — the single canonical representation;
   percent values are converted at the adapter via the explicitly named
   boundary constructor and never persisted (P0.07 §4; P1.05
   `StateOfCharge::from_percent`).

## 6. Sign rule

1. **P > 0 means export/injection into the grid** at the point of connection;
   P < 0 means import/consumption (P0.07 §2). The point of connection is the
   asset's own connection point for BESS, PV, flexible load, and EVSE
   quantities, and the site PCC for site-meter quantities (§4 table).
2. **Reactive power has no universal sign.** Q > 0 means injection or
   consumption according to the convention declared by the governing grid
   model / device profile; that declared convention travels on the record in
   `q_sign_convention` (D-T4), and interpretation goes through the implemented
   `interpret_reactive_power(convention, q)` path — reading the sign directly
   is the prohibited "inferred from sign alone" mistake (P0.07 §3; foundations
   §6.2).
3. BESS net power follows P0.07 §6: `P_net = P_dis − P_ch` with both
   magnitudes non-negative; the sign rule of item 1 applies to `P_net`.

## 7. Time rule

1. `event_time` is when the measurement occurred; `ingest_time` is when
   FlexCore received it. They are distinct fields, distinct contract types
   (`EventTime` / `IngestTime`), and must never be merged or substituted
   (P0.09 §1; enforced at compile time by the distinct newtypes in P1.05).
2. **UTC internally.** Every timestamp carries the UTC offset at construction;
   non-UTC offsets are rejected, never silently converted (P0.09 §2; P1.05
   `require_utc`). Local time is never stored as the canonical timestamp.
3. The site timezone (`sites.timezone`, IANA name, Phase 1 registry) is
   retained for **display only** and for market-calendar arithmetic — never
   stored on or derivable into the canonical timestamp (P0.09 §2). Local
   calendar days are not always 24 hours (DST 23/25-hour rule, P0.09 §3); any
   consumer bucketing by local day handles that explicitly.
4. Simulation time is separate from wall-clock time (foundations §6.2 Time
   row, adopted by P0.09 §5): the simulator's `event_time` is on the scenario
   clock (P2.13/P2.14); `ingest_time` is wall-clock at receipt and is excluded
   from replay comparison (P2.13 determinism scope).

## 8. Identity rule

1. Every **measured** record carries `source_id` and `source_sequence`, both
   mandatory and non-null (P0.09 §1 "source sequence or version"; P0.08 §7
   source identity). A provider that cannot produce them fails decoding — it
   does not emit a partial record (P2.11 step 3).
2. The deduplication identity of a sample is
   `(asset_id, event_time, source_sequence)` — decided once here, enforced
   with the unique/PK design of P2.05 §5, and implemented by P2.09 (exact
   duplicate acknowledged; conflicting duplicate quarantined, never silently
   overwritten).
3. `source_sequence` is monotonically increasing per (asset, source) and is
   not assumed gap-free; out-of-order arrival is tolerated and handled by
   P2.09 — arrival order is never assumed to equal event order.
4. **Derived records** carry the reserved platform namespace identity
   (`source_id = 'flexcore:derived:<detector>'`, D-T6) with a detector-owned
   monotonic sequence, so every stored row has a total identity and derived
   markers are distinguishable from measured values by construction (P2.08).
5. `telemetry_id` is not part of the identity (D-T3); see P2.05 §5 for the
   exact PK/uniqueness interaction.

## 9. Quality metadata rule

The quality-assignment rules (range checks, SUSPECT-vs-BAD boundary,
staleness, gap detection) are P2.08 and are not re-defined here. This model
fixes only the record-level obligations: `quality` is total and drawn from the
frozen six states (P0.08 §1–§2); a richer source state is mapped at the
adapter boundary with the original code retained in `source_quality_code`
(P0.08 §3); no quality flag is silently downgraded, dropped, or omitted
(P0.08 §4). A failed record is quarantined with a reason — never silently
dropped and never silently stored as `GOOD` (P2.07 pipeline rule).

## 10. Alignment with the implemented contracts (P1.05)

| Model field | Implemented contract type (file) | Alignment |
|---|---|---|
| `active_power` value | `ActivePowerMW` (`crates/flexcore-contracts/src/units.rs`) | MW; sign rule §6; `as_kw_display` is display-only and never touches storage |
| `reactive_power` value | `ReactivePowerMVAr` + `ReactiveSignConvention` + `interpret_reactive_power` (`units.rs`) | MVAr; interpretation requires the declared convention (D-T4) — the API offers no convention-free path |
| `soc` value | `StateOfCharge` (`units.rs`) | Fraction [0,1]; range rejected outside [0,1]; percent only via the named boundary constructor |
| `energy` value | `EnergyMWh` (`units.rs`) | MWh; distinct type from `ActivePowerMW` — unit confusion is a compile error |
| `event_time` / `ingest_time` | `EventTime` / `IngestTime` (`crates/flexcore-contracts/src/time_model.rs`) | Distinct types, UTC enforced at construction, `ingest_time ≥ event_time` validated |
| `clock_quality` | `ClockQuality` (`time_model.rs`) | `SYNCHRONIZED` / `UNSYNCHRONIZED` / `UNKNOWN`; recorded, never assumed (D-T5) |
| `source_sequence` | `TimeRecord.source_sequence: u64` (`time_model.rs`) | Mandatory ordering identifier; `bigint` at storage |
| `quality` | `TelemetryQuality` (`crates/flexcore-contracts/src/quality.rs`) | Exactly the six frozen states; source states map via `map_source_state` with the original retained (§3 field 10) |
| site timezone | `SiteTimezone` (`time_model.rs`) | Display-only metadata; carried by the registry (`sites.timezone`), not by telemetry rows (§7 item 3) |

The storage representation (P2.05 §3) maps these onto `timestamptz`,
`double precision`, and PostgreSQL enums exactly as P1.12 §2.2 does for the
Phase 1 tables — same conventions, no new representation decisions.

## 11. Acceptance-criteria coverage (ledger P2.04)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Every field from step 1 is defined with type and nullability; no field is left "TBD" | §3 (fields 1–16, every row typed and nullable-or-total), §2 D-T3 (telemetry_id decision) |
| Every per-asset quantity names its canonical unit; no kW/kWh in any persisted field | §4 (quantity table + per-asset table, unit per quantity), §5 (unit rule, canonical-only persistence) |
| P > 0 = export and the per-profile Q convention are stated | §6 (sign rule; per-record `q_sign_convention` per D-T4), §2 D-T4 |
| `event_time` vs `ingest_time` distinguished; UTC-internal stated | §7 (time rule, items 1–2), §3 fields 5–6 |
| Source identity + source sequence are mandatory fields | §8 (identity rule, items 1–3), §3 fields 7–8 |

Verification method (ledger): design review against §10.2–§10.4 and P0.07–P0.09 — performed in §§5–7 and §10; contract tests that pin the record shape are P2.06/P2.11/P2.14 work, not this document's.
