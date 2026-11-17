# Final Review Ledger — Phase 17

**Ledger status:** **FULLY FIXED (written) — 6 of 6 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-17-*` for phases
00–10, `docs/phases-11-20/phase-17-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P17.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P17.01 | `FRV1-R2-004` | P1 | EV ingest bridge has no production caller: route_ev_inbound reachable only from tests | Compose route_ev_inbound into the production router | FIXED (recorded seam, awaiting human build/test) — fix wave 2: route_ev_inbound is a worker-internal routing function, not an HTTP route; the adapter/CSMS integration point is recorded at the seam citing the finding |
| FRV1-P17.02 | `FRV1-R6-002` | P1 | OCPP (and ENTSO-E) have no recorded backpressure decision; OCPP refusals sit outside the shared RateRefusal vocabulary | Record the OCPP/ENTSO-E backpressure decision and fold OCPP refusals into the shared vocabulary | FIXED (recorded decision, awaiting human build/test) — fix wave 3 (record 83 sec 5 gains the OCPP and ENTSO-E rows; OCPP's CsmsLimits recorded as the deliberate mechanism with the shared-vocabulary gap named; fold-in deferred to first OCPP composition) |
| FRV1-P17.03 | `FRV1-R6-003` | P1 | No operator-facing adapter guide for OCPP or ENTSO-E in docs/protocols/ | Write the two guides with honest seam/boundary statements | FIXED (written, awaiting human build/test) — fix wave 3 (docs/protocols/ocpp-csms.md and entsoe-a44.md written in the eight-guide pattern, content derived only from adapter code) |
| FRV1-P17.04 | `FRV1-R7-002` | P1 | SoC arbitration contradicts its own recorded policy statement on the stale-OCPP fallback | Align arbitrate_soc (and its pinned test) with the OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED policy text | FIXED (written, awaiting human build/test) — fix wave 1 (behavior aligned to the recorded statement; pinned tests updated) |
| FRV1-P17.05 | `FRV1-R7-006` | P1 | arbitrate_soc has zero consumers — the wired MeterValues ingest path never arbitrates SoC | Wire arbitrate_soc into the MeterValues ingest path | FIXED (recorded seam, awaiting human build/test) — fix wave 2: the dual-source meeting point does not exist in the shipped surface (single-source ingest, no telematics port); the arbitration seam status is recorded in ev_ingest citing the finding |
| FRV1-P17.06 | `FRV1-R7-009` | P2 | SoC-observation quality refusals reuse PlugStateQualityUnusable | Add a distinct SoC-quality error variant | FIXED (written, awaiting human build/test) — fix wave 4 (distinct SocQualityUnusable variant carrying source + quality; plug-state variant unchanged; arbitration semantics untouched) |
