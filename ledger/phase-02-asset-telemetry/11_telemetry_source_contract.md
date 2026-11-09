# P2.11 — Telemetry source contract (`TelemetrySource` port, simulator as first provider)

| Field | Value |
|---|---|
| Subtask | P2.11 — Define the telemetry source contract (`TelemetrySource` port, simulator as first provider) |
| Report trace | §9.1 (`TelemetrySource`: subscribe/decode), §5.2 (capability seam), §5.3 (telemetry source category) |
| Inputs | P1.07 (ports crate — the shape-only seam this subtask makes real); P2.04 (canonical telemetry model, decisions D-T1…D-T6); P0.07–P0.09 (cited, not re-defined) |
| Status | Implemented: `crates/flexcore-ports/src/telemetry_source.rs` (contract + contract tests); README section updated. No provider ships here — the Phase 2 provider set is P2.14's |
| Decisions | D1–D6 remain OPEN. Decisions D-S1…D-S5 below are made once for Phase 2 |

---

## 1. Scope and relationship to P1.07

P1.07 pinned the seam as a **shape-only** port (`TelemetryScope`,
`DecodedSample`, opaque subscription handle) so a Phase 2 adapter could not
invent an application-coupled signature. This subtask replaces that shape
with the real contract. The superseded types had **no consumer anywhere**
(no Stage 1 use case touches telemetry; verified by grep at change time), so
removing them touches nothing else. The transition is recorded in the ports
crate README ("`TelemetrySource`: the real contract (P2.11)").

The port defines what a provider is and what it must emit; it does **not**
define the ingestion pipeline (P2.07), quality-assignment rules (P2.08), or
storage (P2.05–P2.06). A provider implements `TelemetrySource` and nothing
else — no direct DB/NATS access (P2.14 acceptance).

## 2. Decisions made once

| ID | Decision | Rationale | Alternative considered and rejected |
|---|---|---|---|
| D-S1 | `TelemetryQuery::new` takes the domain **`TenantContext`** and derives the tenant from it; there is no other constructor, no `From<TenantId>`, and no public field | P0.16 rules 1 and 5 (same structural pattern as the repository ports and the domain tenant-context type): tenant scoping must be a compile-time property, not a runtime check that can be forgotten. The query cannot disagree with the authenticated scope it came from because the tenant is not a parameter | A `tenant_id` parameter or builder field: rejected — a raw tenant-ID path is exactly the bypass P0.16 rule 5 prohibits; scoping would then be enforced somewhere else, or nowhere |
| D-S2 | A query names a **non-empty asset set and a non-empty quantity set**; there is no implicit "all assets"/"all quantities" wildcard | Explicit is auditable: a query that names what it wants cannot silently widen into a full-portfolio pull (the P2.12/P2.19 unbounded-query risk starts here); an empty set is always a caller bug, so the constructor rejects it | An `Option<Vec<…>>` / wildcard meaning "all": rejected — the wildcard path is the one that silently scans everything |
| D-S3 | One canonical record is **one quantity of one asset at one `event_time`** (P2.04 D-T2, restated here as the port's record shape); `decode` returns the **complete set** of records one frame carries, and a successful result is non-empty | A sample-level quality field would violate the P0.08 exactly-one-per-value rule; the per-record shape matches the replay comparison fields (P2.24: `asset_id`, `event_time`, quantity, value, quality, `source_sequence`) and the storage key (P2.05/P2.09) | `decode` returning one multi-quantity sample struct: rejected — per-value quality is unrepresentable there |
| D-S4 | Record construction is a **`Result` with whole-record rejection**: empty source identity, a measured record without a value, a value on a `MISSING` marker, a `MISSING` marker outside the `flexcore:derived:` namespace, a derived-namespace record claiming a measured state, a quantity/value kind mismatch, an empty text state, or `ingest_time < event_time` each reject the record — there is no partial record (P2.11 step 3) | The no-partial-record rule must live in the type, not in reviewer vigilance: a `Result`-constructed record means downstream code (pipeline P2.10, storage P2.06) can never see an incomplete record. The `flexcore:` namespace rules implement P2.04 D-T6 (measured sources never use the reserved platform namespace; `MISSING` gap markers and explicitly configured `ESTIMATED` records are platform-derived and use `flexcore:derived:<detector>`) | An `Option`-fielded record validated later by the pipeline: rejected — the invalid record would exist and travel; "validate at the boundary" without a type is a convention, not a boundary |
| D-S5 | The provider rule is **structural**: `provider_declaration()` is a required trait method returning the cadence, the quality-state mapping, and the clock-quality behaviour; provider source IDs must never use the reserved `flexcore:` namespace | P2.11 step 4 acceptance ("every provider must declare cadence, quality mapping, and clock behaviour") holds by construction — a provider cannot exist without declaring them. Making it data (not a doc obligation) lets P2.14's declaration be diffed in review and consumed by the pipeline | A documentation-only obligation: rejected — nothing enforces a paragraph |

## 3. The port

`trait TelemetrySource` (sync — house rule; see §7):

| Method | Contract |
|---|---|
| `provider_declaration()` | The [`ProviderDeclaration`] — cadence, quality mapping, clock behaviour, source identity (§6) |
| `subscribe(&TelemetryQuery)` | Registers interest; returns the opaque `TelemetrySubscription` handle. Queries the source cannot serve are rejected with `TelemetrySourceError::InvalidQuery` (human-safe reason) |
| `decode(&TelemetrySubscription, &[u8])` | One opaque raw frame in; the complete set of canonical records out. The wire protocol is the adapter's concern. Success is non-empty and complete; anything less is `Err` |
| `source_id()` | The provider's source identity (default-derived from the declaration; matches `source_id` on every record it emits) |
| `health()` | The provider's self-reported `TelemetrySourceHealth` (`Healthy` / `Degraded{reason}` / `Unavailable{reason}`); reasons are human-safe, never raw backend text (P1.08 rule). It is the provider's status, not a platform judgement — freshness is the consumer's rule (P2.08) |

Error vocabulary: `NotSubscribed`, `InvalidQuery{reason}`,
`DecodeFailed{reason}`, `Backend(BackendError)`.

## 4. The query (`TelemetryQuery`)

- **Tenant-scoped structurally** (D-S1): constructed from a `TenantContext`;
  `tenant_id()` returns the context's tenant.
- Asset set: non-empty, `Vec<AssetId>` (D-S2). Site/portfolio scoping is not
  part of the query — the registry relationships resolve by JOIN (P2.04
  D-T1); the asset set is the selection mechanism.
- Time range: `from`/`until` as `EventTime` (the P0.09 canonical type),
  inclusive on both ends, `from <= until` enforced.
- Quantities: non-empty, from the frozen P2.04 §4 vocabulary
  (`TelemetryQuantity`: `ActivePower`, `ReactivePower`, `Soc`, `Energy`,
  `Availability`, `OperatingMode`, `Health`, `ConnectionState`,
  `BaselineReference` — each variant documents its canonical unit).

Contract tests: the positive path pins that the tenant comes from the
context; the compile-level negatives (struct literal, `From<TenantId>`) are
documented in the test module the same way the domain tenant-context
negative test documents its bypasses.

## 5. The normalized record (`NormalizedTelemetry`)

The P2.04 §3 canonical record, in the port's types:

- **One quantity per record** (P2.04 D-T2 / D-S3).
- **Value**: `QuantityValue` — `Power(ActivePowerMW)` (active power and
  baseline reference, MW, P > 0 = export, P0.07 §2);
  `Reactive { power: ReactivePowerMVAr, convention: ReactiveSignConvention }`
  (the convention is a **required field of the value** — a reactive value
  without its declared convention is unrepresentable, P0.07 §3 / P2.04
  D-T4); `Fraction(StateOfCharge)` ([0,1], P0.07 §4);
  `Energy(EnergyMWh)`; `Flag(bool)`; `State(String)` (verbatim, non-empty).
  The newtype constructors already reject NaN/∞/out-of-range, so the record
  cannot hold a non-canonical number by construction.
- **Quality**: `TelemetryQuality` (exactly the six canonical states, P0.08);
  `source_quality_code: Option<String>` retains the original richer code as
  diagnostic metadata (P0.08 §3) — never a seventh state.
- **Time**: `event_time` / `ingest_time` as the distinct P0.09 types, with
  `ingest_time >= event_time` enforced at construction. `clock_quality` is
  a recorded `ClockQuality` (`SYNCHRONIZED`/`UNSYNCHRONIZED`/`UNKNOWN`),
  never assumed (P2.04 D-T5).
- **Identity**: `source_id` (non-empty; namespace rules per D-S4) and
  `source_sequence: u64` (monotonically increasing per (asset, source),
  P2.04 §8) are mandatory — the dedup identity
  `(asset_id, event_time, source_sequence)` is P2.09's to enforce.
- `tenant_id` / `asset_id` from the authenticated source context and the
  frame respectively (P2.04 §3 fields 2–3; source-identity tenant scoping
  is tested end-to-end in P2.22).

**Decode-failure rule** (ledger acceptance "decode failure is specified, not
silent"): a provider that cannot produce a complete canonical record —
missing quality, missing sequence, missing value, unmappable frame — returns
`TelemetrySourceError::DecodeFailed`. It never emits a partial record, never
an empty success, and never silently degrades a quality state (P0.08 §4
non-silencing at the port boundary). The quarantine handling of a failed
frame is pipeline work (P2.07), not the port's.

## 6. The provider rule (`ProviderDeclaration`)

Every provider declares, structurally (D-S5):

1. **Cadence**: the expected emission interval between consecutive samples
   of one asset (`std::time::Duration`). Where per-asset cadences differ
   from the declared default, the per-asset values live in the provider's
   own versioned configuration — for the Phase 2 simulator, the scenario
   (P2.13) — and must not contradict the declaration.
2. **Quality-state mapping** (`QualityMappingDeclaration`): every verbatim
   source-side quality code the provider can report, mapped to exactly one
   of the six canonical states (P0.08 §3), with the original code retained
   downstream as `source_quality_code`. Construction rejects an empty
   declaration, empty codes, and duplicate codes. (Mapping a source's own
   explicit "OK" to `GOOD` is legitimate; the P0.08 non-silencing rule
   prohibits silently converting a *non-normal* flag into `GOOD`, which is
   `map_source_state`'s guarantee in the contracts crate.)
3. **Clock-quality behaviour**: the `ClockQuality` value the provider
   records on its emissions (the simulator records `SYNCHRONIZED` — its
   event times are on the deterministic scenario clock, P2.13; a real
   source that does not report synchronisation records `UNKNOWN`, never a
   silent assumption of good, P2.04 D-T5).

A provider `source_id` must be non-empty and must never use the reserved
`flexcore:` namespace (P2.04 D-T6) — enforced by the declaration
constructor, so a mis-namespaced provider cannot be constructed at all.

## 7. Sync choice (house rule, restated for this port)

The port is deliberately synchronous: no future in, no future out, no
runtime named. `subscribe` returns an opaque handle rather than a stream;
delivery, buffering, and ordering are the adapter's concern, bridged by the
adapter (blocking hand-off, worker thread, or an async wrapper around the
sync trait) and recorded with the adapter. This keeps the application layer
dependent on nothing runtime-shaped — the same rule every port in this
crate follows (crate README, "Sync traits").

## 8. Phase 2 provider set (ledger step 5)

**Exactly one**: the simulator telemetry source (P2.14), a built-in provider
implementing `TelemetrySource` and nothing else. Real adapters (MQTT,
Modbus, OpenEMS) are later phases (report §5.3 category; Stage 6 site
integration). Consequences, recorded:

- No provider code, protocol parsing, or broker/DB access exists in
  `flexcore-ports` — the crate has no external dependencies beyond
  contracts + domain (`tools/check_dependency_rule.py`).
- The P2.27 telemetry contract suite (§19.2 layer 3) runs against the
  simulator now and against every future provider with the same tests; the
  in-module fake (`MinimalFakeSource`) is the seed of that suite.
- `simulated: true` marking (P2.17/P2.18) is derived downstream from the
  record's `source_id` belonging to the registered simulator source —
  P2.04 §3 provenance note — so every simulator value is distinguishable
  from measured data at the API.

## 9. Acceptance-criteria coverage (ledger P2.11)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| The port has subscribe/decode/source_id/health with tenant-scoped queries | §3 (trait table), §4 (query; `TenantContext`-required constructor, D-S1), implemented in `crates/flexcore-ports/src/telemetry_source.rs` (`trait TelemetrySource`, `TelemetryQuery`) |
| Decode failure (non-canonical output) is specified, not silent | §5 (decode-failure rule), D-S4, `TelemetrySourceError::DecodeFailed`; contract test `decode_failure_is_err_and_never_a_partial_record`; `record_construction_rejects_missing_canonical_fields` pins whole-record rejection |
| Every provider must declare cadence, quality mapping, and clock behaviour | §6, D-S5, `ProviderDeclaration`/`QualityMappingDeclaration` as a required trait method; tests `provider_declarations_reject_bad_source_ids_and_bad_mappings`, declaration assertions in the decode-failure test |
| The Phase 2 provider set is exactly one (simulator) | §8 |

Verification method (ledger): port review against §9.1 and the provider-rule
checklist — performed in §§3–8; port-level tests live in the module
(`#[cfg(test)]`) and are exercised by P2.14/P2.27 contract suites.

## 10. What this document does not do

- It ships no provider: the simulator is P2.14; the contract tests use a
  minimal fake only.
- It defines no quality-assignment rule (P2.08), no dedup identity
  enforcement (P2.09), no storage shape (P2.05–P2.06), and no pipeline
  stages (P2.07) — it fixes only what crosses the port.
- It makes no field-read or protocol-conformance claim: real adapters are
  later phases (§8), and the entire 0.x line remains simulation-only.
