# P17.08 — Dual-source SoC arbitration

| Field | Value |
|---|---|
| Subtask | P17.08 — OCPP MeterValues SoC authoritative-while-plugged vs the telematics-API seam; staleness rules (STALE per the quality vocabulary); Tesla 500 ms telemetry the outlier [S-137]; telematics stale-when-sleeping documented [S-138]; plug-state as participation ground truth [S-139][S-140]. Decision record + arbitration module. The telematics seam is a PORT (vendor-agnostic — P17-E-3 DEFAULT (a); P17-E-3 registered OPEN). |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the arbitration module and its tests are written, never compiled, never executed. No charge point is connected, no SoC value was ever observed by this platform, no telematics vendor is named as integrated, no vendor API terms were accepted (a human gate). |
| Sources | [S-137] Tesla Fleet Telemetry (available-data); [S-138] evcc stale-SoC issue (github.com/evcc-io/evcc/issues/26486); [S-139] Geotab EV monitoring; [S-140] DOE FEMP smart charge management. **The Tesla 500 ms cadence figure and the sleeping-vehicle staleness behavior are SECONDARY-SOURCED and UNVERIFIED against the primary sources** — §4 registers the checks. |
| Files touched | `crates/flexcore-domain/src/ev_soc_arbitration.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive registration only). |

## 1. What landed (deliverable class: arbitration module + decision record)

| Piece | Where | Honest status |
|---|---|---|
| The recorded policy statements: `PLUG_STATE_GROUND_TRUTH_STATEMENT` [S-139][S-140], `OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED_STATEMENT` [S-137][S-140], `TELEMATICS_STALE_WHEN_SLEEPING_STATEMENT` [S-138], `TESLA_500MS_TELEMETRY_OUTLIER_NOTE` [S-137], `P17_E_3_STATUS` | `ev_soc_arbitration.rs` | Implemented; UNRUN |
| `SocSource` (OcppMeterValues / TelematicsPort) — the telematics source is the vendor-agnostic PORT (no vendor named; adapters implement the port later) | same | Implemented; UNRUN |
| `SocObservation` / `PlugStateObservation` — declared observations carrying the frozen quality states (consumed, never redefined); `MISSING`/`ESTIMATED` inputs refused (participation is never gap-marked or model-derived) | same | Implemented; UNRUN |
| `arbitrate_soc` — the pure arbitration function: no plug state → REFUSED (never assumed, either direction); disconnected → `None` (no participation SoC, whatever telematics say); plugged → OCPP authoritative (a stale-but-present OCPP value still wins over fresh telematics and carries STALE honestly); OCPP absent → telematics fallback; both absent → `None` (an unknown SoC narrows, never invents — the P3.02 rule) | same | Implemented; UNRUN |
| The staleness rules: per-source declared `FreshnessLimit`s (P2.08 `staleness` REUSED — never recomputed; age > limit is STALE, age == limit is not); a telematics observation with the declared sleeping flag carries STALE regardless of age [S-138] | same | Implemented; UNRUN |
| The non-silencing guarantee: the output quality is the worse of the input quality and the staleness verdict — a fresh SUSPECT stays SUSPECT, a stale BAD stays BAD (worse than stale), no seventh state | same | Implemented; UNRUN |

## 2. P17-E-3 — registered OPEN with DEFAULT (a) (agents never close E-decisions)

- **Question:** which vehicle-telematics integrations, in what order?
- **Options:** (a) vendor-agnostic port first (staleness rules specified at
  the port; no vendor named), named vendor adapters later on demand;
  (b) a named vendor first; (c) OCPP-only SoC (no telematics seam).
- **DEFAULT (a), implemented as the port:** `SocSource::TelematicsPort` is
  a port-shaped source with declared staleness rules; no vendor adapter
  exists, no vendor is promised, and no terms of service are accepted by
  any artifact in this batch (vendor terms are HUMAN-GATE inputs,
  conventions §5).
- **Status: OPEN.** **What closes it:** the human owner naming the first
  fleet vendor whose API terms are accepted. No agent closes it.

## 3. The arbitration policy, decided once (consumable by the scheduler and the composer)

| Rule | Encoded as |
|---|---|
| Plug-state is the participation ground truth [S-139][S-140]; connection loss is not unavailability | Missing plug state → `SocArbitrationError::NoPlugStateObservation`; disconnected → `Ok(None)`; the vehicle leaves the dispatchable set, never the registry |
| OCPP MeterValues SoC authoritative while plugged [S-137][S-140] | The OCPP branch runs first whenever an observation exists; telematics are the fallback only when it is absent |
| Staleness per source, STALE per the frozen vocabulary | P2.08 `staleness` reused; per-source freshness limits are declared parameters (never hard-coded) |
| Telematics stale-when-sleeping [S-138] | The declared `reported_sleeping` flag forces `STALE`; the flag is port data, never inferred |
| Tesla 500 ms telemetry is the outlier [S-137] | `TESLA_500MS_TELEMETRY_OUTLIER_NOTE`; no default freshness limit assumes sub-second telemetry; a vendor adapter declaring a faster cadence declares its own smaller limit explicitly |
| Non-silencing (P0.08) | `worse_with_staleness`: degradation survives; no source state maps to `GOOD`; no seventh state |

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The Tesla ~500 ms telemetry cadence figure [S-137] | `TESLA_500MS_TELEMETRY_OUTLIER_NOTE` |
| 2 | The stale-when-sleeping behavior and its operational conditions (what evcc records as the failure mode) [S-138] | `TELEMATICS_STALE_WHEN_SLEEPING_STATEMENT`, the `reported_sleeping` flag |
| 3 | Whether OCPP MeterValues SoC is sampled per the declared cadence in real deployments (the authoritative-while-plugged assumption) [S-124][S-140] | the OCPP-first branch |
| 4 | Whether plugged vehicles keep the telematics link awake (this module's recorded convention: the charge link keeps the vehicle awake while energy flows) | the `reported_sleeping`-always-false convention for OCPP observations |

## 5. Tests (written-unrun; execution is a human gate)

The policy statements, the disconnected-vehicle `None`, the missing-plug-
state refusal, the `MISSING`/`ESTIMATED` refusals (plug state and
observations), the authoritative-while-plugged ordering (OCPP wins over
NEWER telematics), the stale-OCPP-carries-STALE case, the exactly-at-limit
fresh boundary, the telematics fallback, the stale-OCPP-still-wins case,
the sleeping flag forcing STALE, the SUSPECT/BAD survival (non-silencing),
the no-observations `None` (unknown narrows), the freshness-limit
construction refusals, the future-dated-clock-skew case, and the
live-value helper.
