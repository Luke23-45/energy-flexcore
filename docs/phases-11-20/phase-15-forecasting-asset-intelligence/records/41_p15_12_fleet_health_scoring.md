# P15.12 — Fleet health scoring: deviation-from-normal models per asset class, alarm normalization into prioritized issues, availability KPI independent of O&M

| Field | Value |
|---|---|
| Subtask | P15.12 (deviation-from-normal models per asset class, alarm normalization into prioritized issues, availability KPI computation independent of O&M — Nispera pattern [S-53]; scores are RECORDS feeding operator views — never dispatch inputs) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the module and its tests are written, never compiled, never executed. No fleet health score was computed from real data (the platform has never run); every test value is hand-computed over CONSTRUCTED inputs; no health threshold is registered anywhere. |
| Sources | [S-53] (Fluence Nispera — asset performance management: the deviation-from-normal / alarm-normalization / O&M-independent availability pattern, cited as a DESIGN reference only; no vendor integration, benchmark, or conformance claim is made) |
| Files touched | `crates/flexcore-application/src/fleet_health.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line + pub use block) |

## 1. What landed (deliverable class: pure application records)

| Item | Content |
|---|---|
| Deviation-from-normal (`evaluate_deviation`) | Per asset class (the domain `capability_provenance::AssetClass` — reused, never restated), a DECLARED `NormalExpectation` per quantity (normal mean, tolerance fraction in [0, 1), provenance, recorded source); the observed window mean is pure fixed-order arithmetic; the relative deviation is compared EXCLUSIVELY against the tolerance (deviation == tolerance is NOT exceeding); a zero normal mean yields an UNDEFINED relative deviation (`None`), never a zero-deviation verdict; SOC observations are range-guarded to [0, 1] (P0.07 §4) |
| Alarm normalization (`normalize_alarms`) | Consumes the P5.31 alarm model vocabulary (`flexcore_contracts::alarm::{AlarmCode, AlarmSeverity}` — alarms are operator-attention records, never protection signals) plus per-asset out-of-tolerance deviation flags; the priority rule is DETERMINISTIC: base = severity rank (Critical→P1, Warning→P2, Info→P3), promoted one tier when the asset carries an out-of-tolerance deviation (P1 is the ceiling); the mapping is fixed and test-pinned |
| Availability KPI (`availability_kpi`) | The duration-weighted mean of the DECLARED per-interval availability fractions (the P2.16 availability rule's outputs, with a recorded source) over a declared window; a window of all-zero weights is a refusal (a KPI over zero duration is not a KPI); no O&M input exists in the platform, none is consumed, none is assumed — the independence is by construction (`AVAILABILITY_KPI_INDEPENDENCE_STATEMENT`) |
| Honesty constants | `HEALTH_SCORE_IS_A_RECORD_STATEMENT` (ADR-010: records, never dispatch inputs), `NISPERA_PATTERN_STATEMENT` ([S-53] DESIGN reference only), `NORMAL_REFERENCE_IS_DECLARED_STATEMENT` (P15.14 owns declared-parameter records; no fitting procedure exists here) |

## 2. Honest boundaries

1. No score from real data: every number is arithmetic over declared inputs; the standing posture is that a fleet-health score computed here is an operator-view record only.
2. No dispatch reach (ADR-010 extension): no type here constructs or carries a dispatch command; the scheduling objective consumes degradation cost curves (P15.08), never health scores.
3. No threshold is registered: tolerances are DECLARED inputs; a registered fleet threshold would be an X-8-gated, versioned artifact (the P15.05 discipline), never a silent constant.
4. Every test is WRITTEN-UNRUN (conventions §1.3); the permitted-tool outputs are quoted in the close-out record (48).
