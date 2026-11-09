# flexcore-contracts (P1.05)

Canonical value types and API-facing contracts for Neryva FlexCore. This is
the first crate of the workspace (report §8.1): every other crate shares
these types, so they may depend on nothing infrastructure — only `std`,
`serde`, and `time` (`tools/check_dependency_rule.py` enforces this).

## What it implements (frozen Phase 0 contracts)

| Module | Implements | Source freeze |
|---|---|---|
| `units` | **P0.07** — canonical units and sign conventions | `ledger/phase-00-product-freeze/07_units_sign_conventions.md` (report §10.2, §11.2; foundations §6.2 governs on conflict) |
| `quality` | **P0.08** — telemetry quality vocabulary (exactly six states) | `ledger/phase-00-product-freeze/08_telemetry_quality_vocabulary.md` (report §10.4; foundations §6.4) |
| `time_model` | **P0.09** — time model | `ledger/phase-00-product-freeze/09_time_model.md` (report §10.3, §14.3, §17.4; foundations §6.2 Time row) |
| `api_paths` | **P0.15** — the 16 candidate REST resource paths under `/api/v1` | `ledger/phase-00-product-freeze/15_api_surface.md` (report §15.1–§15.2) |
| `ids` | **P1.05** — `FlexUuid` representation over `[u8; 16]` | This subtask; generation belongs to the P1.07 id-generation port |

The freeze documents are normative; this code must agree with them exactly.
Where this code and a freeze document disagree, the document wins and the
code must be corrected.

## Key frozen rules encoded in code

- **Units:** canonical MW / MVAr / MVA / MWh newtypes; `P > 0` means export
  at the point of connection; **reactive-power direction is only
  interpretable under an explicitly supplied `ReactiveSignConvention` — there
  is no default**; SOC is a fraction in [0,1] with `from_percent` as a
  separate, explicitly named boundary constructor; BESS net power
  `P_net = P_dis − P_ch` with non-negative magnitudes; per-unit values carry
  an explicit `PerUnitBase`. kW/kWh appear only in explicitly display-named
  methods (`as_kw_display()`, `as_kwh_display()`) — **display-only, never
  persisted**.
- **Quality:** exactly six canonical states (`GOOD`, `SUSPECT`, `BAD`,
  `MISSING`, `ESTIMATED`, `STALE`); richer source states are mapped at the
  adapter boundary with the original code retained as metadata; the
  non-silencing rule holds by construction — nothing maps to `GOOD`.
- **Time:** `event_time` and `ingest_time` are distinct types (never merged
  or substituted); every timestamp requires the **UTC offset at
  construction** (non-UTC offsets are rejected, not converted); the site
  timezone travels separately as `SiteTimezone` metadata; validity windows
  enforce `valid_from ≤ valid_until`; clock quality is a recorded field,
  never an assumption.
- **IDs:** `FlexUuid` is representation only (parse/format/nil/bytes). ID
  *generation* is behind the application-layer id-generation port (P1.07);
  no `uuid` crate dependency is taken.

## Serialization

Value types derive `serde` where serialization is meaningful: quality and
convention enums as SCREAMING_SNAKE strings, unit newtypes as their canonical
f64 under an explicit unit field name (for example `{"mw": 3.5}`), timestamps
as RFC 3339 UTC strings. Deserialization always re-runs the validating
constructor, so an out-of-range or non-UTC value can never enter through
serde.

## Scope and control status

Contract types for the design baseline only. The entire 0.x line is
simulation-only; nothing here authorises field control or device writes.
FlexCore is a supervisory orchestration layer and performs no primary
frequency response, synthetic inertia, relay protection, inverter inner-loop
control, or sub-cycle control.
