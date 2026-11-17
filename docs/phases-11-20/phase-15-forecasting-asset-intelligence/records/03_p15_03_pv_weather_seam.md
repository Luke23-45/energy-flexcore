# P15.03 — PV generation forecasting design + the weather-data ingestion seam (P15-E-1 registered OPEN with DEFAULT)

| Field | Value |
|---|---|
| Subtask | P15.03 (PV forecasting design + a weather-data ingestion PORT under `crates/flexcore-ports/src/`; build-vs-buy P15-E-1: DEFAULT buy irradiance/power, ingested as DATA with provenance; vendor naming = RECOMMENDATION label; design only — no vendor integration without the E-decision) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the port and the design module are written, never compiled or executed; no weather data exists (no vendor account, no token — UNSIGNED); no PV forecast is produced; no skill is claimed (conventions §1, §5) |
| Sources | [S-41][S-50][S-51] (forecasting table stakes, gap analysis §3 row 3); [S-112] (the analogous bought-forecast pattern — Amperon price forecasting, cited as the buy-as-data analogy); gap analysis §3 row 3, §7 (weather as an input seam, not a build item) |
| Files touched | `crates/flexcore-ports/src/weather_data.rs` (NEW); `crates/flexcore-ports/src/lib.rs` (additive: module, doc rows, SOURCES table 13→15, object-safety test); `crates/flexcore-application/src/forecast_pv.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive) |

## 1. The weather-data ingestion PORT (`flexcore-ports::weather_data`)

| Item | Content |
|---|---|
| Closed quantity vocabulary | GHI / DNI / DHI (W/m²), air temperature (degC), wind speed (m/s), cloud cover (a DECLARED fraction in [0,1]) — each quantity names its unit; no conversion in the port (the P14.07 declared-conversion pattern) |
| Provenance classes | `CommercialService` / `SiteMeasured` / `PublicFeed` — the closed provenance class travels WITH every sample (the linked-provenance rule); unknown narrows, never widens |
| Sample record | `WeatherSample` Result-constructed (no partial records): finite values, negative irradiance and out-of-unit-interval fractions refused, non-empty source REFERENCE (never credentials), the six-state telemetry quality vocabulary carried (bought data can be SUSPECT/STALE/ESTIMATED honestly in-band) |
| Port trait | `WeatherDataProvider`: `latest_observation` + `forecast_series` per site/quantity, tenant-scoped (P0.16 rules 1/5 — every method takes the `TenantContext`), sync traits, fail-closed errors (`ProviderUnavailable` is never answered with synthetic weather) |
| Vendor neutrality | The port crate's vendor-name grep gate holds: no vendor, product, protocol, or endpoint is named anywhere in `weather_data.rs`; P15-E-1 names the vendor (if ever) in deployment configuration, never in the port |

## 2. The PV forecasting DESIGN (`flexcore-application::forecast_pv`)

`pv_model_design_registry()` — three designed PV model declarations, ALL `Untrained` (the design-record enum has exactly one training-state variant; the model registry's first-class UNTRAINED state is P15.06):

| Model | Basis | Status |
|---|---|---|
| `pv-model/persistence-design` | non-weather (the P15.02 persistence rule set applied to the site's own PV history) | designed, UNTRAINED, unevaluated |
| `pv-model/irradiance-physics-design` | GHI/DNI/DHI + air temperature + wind speed requirements against the weather port; NO conversion constants recorded, none fitted; array geometry would be CONFIGURED data with provenance | designed, UNTRAINED, unevaluated |
| `pv-model/statistical-ml-design` | reserved model class; no architecture selected, no framework pinned (the P8.03 DECISION-POINT discipline carries over); training data only through the P15.18 datasets policy | designed, UNTRAINED, unevaluated |

The structural no-vendor binding: `PvForecastModelDeclaration` has NO vendor/product/endpoint field — the closed field vocabulary cannot name one (`NO_VENDOR_FIELD_STATEMENT`, pinned by a test enumerating the fields). Until evaluated, the persistence and seasonal-naive baselines (P15.02) are the operative PV reference forecasts.

## 3. P15-E-1 — Weather/irradiance data build-vs-buy (registered OPEN — agents never close E-decisions, conventions §4)

- **Question:** where do PV-forecast weather inputs come from?
- **Options:** (a) buy irradiance/power from a commercial weather-data vendor (Solcast/DNV-class naming is a RECOMMENDATION, not a register source); (b) ingest raw public weather feeds and model irradiance in-platform; (c) site-measured irradiance only.
- **DEFAULT (RECOMMENDATION):** (a) — bought service ingested as DATA with full provenance through the weather port; forecasting is table stakes (gap analysis §3 row 3 [S-41][S-50][S-51]) and the F2 build-list treats weather as an input seam (§7), not a build item; the analogous bought-forecast pattern is [S-112].
- **What closes it:** the human owner's budget and vendor selection; the vendor account/API key is a HUMAN GATE input (UNSIGNED until registered — conventions §5).
- **Recorded in:** this record (the subtask's decision record per the wave plan); mirrored as `P15_E_1_STATUS` in `forecast_pv.rs`.

## 4. Honest boundaries

1. DESIGN ONLY: no vendor adapter, no HTTP client, no API-key path; the weather port has NO implementation in this delivery (the absence is recorded, not papered over).
2. No weather data exists in this repository; no PV forecast is produced; every evaluation cell stays EMPTY.
3. Vendor-class names (Solcast/DNV) appear ONLY in plan/context/record prose as RECOMMENDATION labels — never in source, never as register sources.
4. Every test is WRITTEN-UNRUN; expected values are hand-derived from the closed vocabularies.
5. The weather-quantity units (W/m², degC, m/s) are weather-domain units declared by this port; the platform's canonical power/energy freeze (MW/MVAr/MVA/MWh) applies to the quantities DERIVED from weather, not to weather itself — recorded so no summary conflates the two.
