# P15.09 — SoH ingestion (BMS-reported, provenance + staleness)

| Field | Value |
|---|---|
| Subtask | P15.09 — BMS-reported SoH ingested with provenance and staleness rules (max-age bounds per the envelope-staleness pattern); platform-side SoH estimation DEFERRED and recorded as such; estimation methods surveyed, none implemented [S-145] |
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11) |
| Status | WRITTEN-UNRUN: the domain module and its tests are written, never compiled, never executed. No BMS was connected, no SoH value was received, no freshness verdict was observed in operation. |
| Sources | [S-145] Werner et al. 2021 calendar aging — cited for the SoH/method-family SURVEY context only; no method is implemented and no method performance is claimed. |
| Files touched | `crates/flexcore-domain/src/bess_soh.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). `crates/flexcore-application/src/envelope_staleness.rs` NOT touched (the pattern is mirrored BY CITATION — the domain crate cannot depend on the application crate; the dependency rule forbids the back-edge). `crates/flexcore-ports/src/telemetry_source_contract.rs` NOT touched (the recorded integration point; no parallel ingestion path is created — see §3). |

## 1. What landed (deliverable class: domain ingestion vocabulary)

| Piece | Where | Honest status |
|---|---|---|
| `BmsSohReport` — the ingested datum: SoH fraction in [0,1], event time, ingest time (the P0.09 `ingest_time >= event_time` ordering enforced at construction — refused, never re-stamped), provenance, source reference (bounded) | `bess_soh.rs` | Implemented; UNRUN |
| `SohStalenessConfig` — the configured maximum-age bound with `SohStalenessConfig::new` refusing non-positive bounds; `DEFAULT_SOH_MAX_AGE_MILLIS` = 7 days, labelled ASSUMPTION (a configuration default with no measured basis) | same | Implemented; UNRUN |
| `evaluate_soh_freshness(report, now, config)` — the P3.13 max-age pattern applied: age evaluated against the EVENT time; `age == max_age` is NOT stale, `age > max_age` is (the P2.08 convention the envelope-staleness pattern carries); a clock read before the event time is its own `ClockBeforeEventTime` staleness reason — never a silent fresh verdict | same | Implemented; UNRUN |
| `soh_report_creditable` — the single creditability gate consumed by P15.10: fresh AND known provenance (P3.02: an UNKNOWN-provenance SoH report credits nothing — unknown narrows, never widens) | same | Implemented; UNRUN |
| `SOH_ESTIMATION_DEFERRAL_STATEMENT` — the recorded deferral: platform-side SoH estimation is DEFERRED; FlexCore ingests BMS-reported SoH only | same | Implemented; UNRUN |
| `SURVEYED_ESTIMATION_METHODS` — the recorded survey: semi-empirical calendar+cycle aging parameterization (BLAST-Lite class [S-146][S-147]), coulomb counting, full-cycle capacity derivation, impedance/EIS, data-driven ML — five families, EVERY one marked NOT-IMPLEMENTED; a survey record, never a capability claim | same | Implemented; UNRUN |
| `STALE_SOH_POLICY` — the recorded staleness policy: a STALE SoH value is never credited for warranty headroom or capability narrowing; re-query or fail loudly, never silently proceed | same | Implemented; UNRUN |
| Tests: construction validation (range, ordering, source), the equality/inequality staleness boundary, the default bound, clock-before-event, unknown-provenance non-creditability, the survey/statements record | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. The staleness pattern, mirrored by citation (no dependency)

The envelope-staleness pattern (`flexcore-application::envelope_staleness.rs`,
P3.13) contributes the RULE SHAPE: a CONFIGURED maximum-age bound, the
P2.08 equality convention, and the stale-never-silently-used policy. The
domain crate cannot depend on flexcore-application (the dependency rule),
so the pattern is mirrored by citation in the module header and pinned by
tests; `envelope_staleness.rs` itself is untouched. The debounce half of
the P3.13 config has no SoH analogue (SoH reports are periodic
observations, not event-triggered recomputations) and is not mirrored.

## 3. The telemetry integration point (recorded, not wired)

SoH arrives through the EXISTING Phase-2 telemetry path: the adapter-side
ingestion binds to the `TelemetrySource` port
(`crates/flexcore-ports/src/telemetry_source_contract.rs` — the recorded
integration point), and `BmsSohReport` is the domain type the port-side
adapter produces. No parallel ingestion path is created; no ports file is
edited by this subtask; the wiring is adapter/composition-root work recorded
as a handoff (see §5).

## 4. What this module is NOT

1. **Not an estimator.** No SoH value is computed by FlexCore; the survey
   is a record of method families, every one NOT-IMPLEMENTED. No ML model
   is registered or trained (no runtime data exists — context §3
   constraint 4).
2. **Not a dispatch authority** (ADR-010): a SoH value never authorizes or
   produces a dispatch command.
3. **Not a quality-state bypass**: the report carries provenance and source;
   a BAD/MISSING telemetry quality flag is never silently treated as GOOD
   (§10.4) — the adapter-side binding to the telemetry port is where the
   quality states ride, unchanged.

## 5. Handoffs

| Who | What |
|---|---|
| Wave 2 (P15.12/P15.16 owners) | The adapter-side wiring that produces `BmsSohReport` from the telemetry path (per-asset freshness configuration), and any health-scoring consumption — scores stay RECORDS, never dispatch inputs |
| P15.10/P15.11 (this wave) | `soh_report_creditable` is the single gate the headroom computation applies (implemented in `bess_warranty.rs`) |

## 6. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The 7-day default max-age bound — ASSUMPTION; the binding value is per-deployment configuration | `DEFAULT_SOH_MAX_AGE_MILLIS` |
| 2 | The BMS SoH reporting semantics (what a vendor BMS actually reports: capacity-based? impedance-derived?) — UNVERIFIED; the fraction-in-[0,1] boundary is the platform's declared convention | `BmsSohReport` |

## 7. Verification (permitted tools)

Outputs quoted verbatim in `26_phase_15_wave1_agent_b_close_out.md`. No
build, no test run (conventions §1.3).
