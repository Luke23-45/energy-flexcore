# P15.07 — BESS degradation data model (domain extension)

| Field | Value |
|---|---|
| Subtask | P15.07 — monotonic counters (cumulative discharge MWh, FEC), rainflow DOD × mean-SOC histogram, temperature-exposure distribution, high-SOC dwell accumulation, periodic SoH observations with provenance — all carrying the P3.02 capability-provenance discipline [S-145][S-146][S-147] |
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11) |
| Status | WRITTEN-UNRUN: the domain module and its tests are written, never compiled, never executed. No counter was fed by a real asset, no histogram cell was filled from field data, no SoH observation was ingested. |
| Sources | [S-145] Werner et al. 2021 calendar aging (calendar-aging drivers: time, temperature, SoC dwell); [S-146] NREL BLAST; [S-147] BLAST-Lite (parameterized semi-empirical cycle aging, DOD/mean-SOC sensitivity). Cited as the source of the DATA MODEL's field set only — no degradation parameter, no fade law, and no model behavior is claimed from them. |
| Files touched | `crates/flexcore-domain/src/bess_degradation.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). `crates/flexcore-contracts/src/units.rs` NOT touched (read-only). `bess_model.rs` NOT touched. |

## 1. What landed (deliverable class: domain records, no I/O)

| Piece | Where | Honest status |
|---|---|---|
| `DegradationChemistry` — LFP / NMC in scope (P15-E-3 DEFAULT (a) per BLAST-Lite coverage [S-147]); `OutOfScope { name }` a RECORDED LIMITATION with `is_in_scope()` false and `label()` `"OUT_OF_SCOPE(...)"` | `bess_degradation.rs` | Implemented; UNRUN |
| `MonotonicCounter` — cumulative, value only grows, refuses negative/non-finite deltas with named errors; merged provenance is the WEAKEST observed (a counter that ever received UNKNOWN stays UNKNOWN forever — unknown narrows, never widens); fresh counter = zero + UNKNOWN provenance + `is_creditable() == false` (zero with unknown provenance credits nothing) | same | Implemented; UNRUN |
| `DodMeanSocHistogram` — the STORAGE for already-counted cycles: declared strictly-ascending bin edges in [0,1] (DOD edges start at 0.0, a zero-depth cycle is not a cycle; last edges pinned at 1.0), row-major counts, per-record provenance merge, `total_cycles()` | same | Implemented; UNRUN |
| `TemperatureExposure` — accumulated seconds per declared °C band; exposures outside the declared bands are REFUSED, never extrapolated | same | Implemented; UNRUN |
| `HighSocDwell` — total seconds at or above a declared SOC threshold (the [S-145] calendar-aging dwell driver as an accumulated RECORD) | same | Implemented; UNRUN |
| `SohObservation` — BMS-reported fraction in [0,1], provenance, observation time, source; the ledger is append-ordered (out-of-order appends refused, never re-ordered) | same | Implemented; UNRUN |
| `BessDegradationLedger` — the aggregate: schema version, asset, chemistry, declared usable energy (FEC normalization input, with provenance), the four counters, histogram, exposure, dwell, SoH observations; `record_throughput(charge, discharge, provenance)` updates the counters and FEC (`(charge + discharge) / (2 · usable energy)` — a declared convention, not a measurement); `counters_creditable()` gates warranty use | same | Implemented; UNRUN |
| Recorded statements: `DEGRADATION_LEDGER_STATEMENT` (not a degradation model; ADR-010) and `RAINFLOW_EXTRACTION_SEAM` (extraction NOT implemented — the histogram stores caller-counted cycles) travel with the module's records, not in comments | same | Implemented; UNRUN |
| Tests: chemistry scope, counter monotonicity and provenance weakening (incl. the forever-UNKNOWN property), histogram binning/boundary/edge validation, exposure band refusal, dwell threshold boundary, SoH ordering, FEC normalization arithmetic, ledger creditability | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. Provenance discipline (P3.02, reused — never reimplemented)

The `weaker_provenance` merge (MEASURED > CONFIGURED > INFERRED > UNKNOWN)
is the frozen P3.05 rule restated as a private helper (the P3.05 function is
private and the domain crate has one dependency); a test pins the restated
rule to the recorded precedence so the two can never drift. Consequences,
enforced by construction:

1. A counter that has EVER received an UNKNOWN increment carries UNKNOWN
   forever — unknown narrows, never widens.
2. UNKNOWN data is not creditable: `counters_creditable()` requires known
   provenance on every counter plus a positive, known-provenance usable
   energy; the P15.10 headroom computation consumes exactly this gate.
3. The evidence pack (P15.11) exports every datum WITH its provenance label
   — an UNKNOWN-provenance counter is visible as such, never silently
   presented as known data.

## 3. What this module is NOT

1. **Not a degradation model.** No fade equation, no aging law, no
   degradation cost is computed anywhere; the P3.06 placeholder statement
   still governs every cost use (P15.08 supersedes the placeholder AS DATA,
   never as physics).
2. **Not a rainflow implementation.** `RAINFLOW_EXTRACTION_SEAM` records
   that extraction from an SOC time series is NOT implemented; the histogram
   accepts already-counted `(DOD, mean-SOC)` pairs from the caller.
3. **Not a dispatch authority** (ADR-010, conventions §6.3): counters,
   histograms, and observations are INPUTS and RECORDS — never commands.
4. **Not a units change**: MWh, dimensionless FEC, SOC fractions in [0,1];
   temperature is a DECLARED INPUT unit (°C) outside the canonical
   MW/MVAr/MVA/MWh freeze, named on every field, never converted —
   `flexcore-contracts::units.rs` is untouched (read-only).

## 4. P15-E-3 registration (OPEN; agents never close)

Registered here per the subtask brief: **P15-E-3 — degradation-model
chemistry coverage**, question: which cell chemistries does the
P15.07/P15.08 degradation data model parameterize first? Options: (a) LFP
and NMC first per BLAST-Lite's parameterized semi-empirical model coverage
[S-147]; (b) LFP only; (c) all chemistries from the start. **DEFAULT
(RECOMMENDATION): (a) LFP + NMC**; per-chemistry parameters are INPUTS
declared with provenance, never universal laws [S-145][S-146]; out-of-scope
chemistries are recorded limitations, never silently extrapolated (enforced
by `is_in_scope()` / `require_in_scope()` downstream). What closes it: the
human owner's fleet-composition review; per-chemistry parameters are
human-gate inputs before any cost curve activates. The full E-decision
record is wave-2 Agent B's P15.19 deliverable; this module implements the
DEFAULT only as recorded above.

## 5. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The FEC normalization convention (one full equivalent cycle = 2 × usable energy of throughput) — a declared convention, not validated against any fleet standard | `record_throughput` |
| 2 | The °C band edges and high-SOC thresholds per asset class (declared configuration; no defaults invented) | `TemperatureExposure::new`, `HighSocDwell::new` |
| 3 | BLAST-Lite's exact parameterization scope for LFP/NMC [S-147] — cited for the chemistry SCOPE only; no parameter value is claimed | `DegradationChemistry` |

## 6. Verification (permitted tools; quoted verbatim in the close-out record)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — outputs quoted in
`26_phase_15_wave1_agent_b_close_out.md`. No build, no test run (conventions
§1.3; the first human build gates all implementation).
