# P3.14–P3.16 — Scheduling objective, term provenance, and PCC tracking:
#                 decisions and review record

- **Date:** 2026-11-07.
- **Scope:** P3.14 (scheduling objective definition), P3.15 (objective-term
  validation and cost-curve provenance), P3.16 (market/PCC tracking).
- **Normative sources:** report §12.1 (scheduling objective), §12.2
  (market/PCC tracking), §10.2 (units and sign conventions); foundations
  §7.5 (extra objective terms — excluded, see D-P3.14-2) governs on
  conflict. Report §12.4–§12.5 (grid model, feasibility gate) are read for
  the boundary only: no grid model and no feasibility claim is made here.
- **Files:** `crates/flexcore-application/src/scheduling_objective.rs`,
  `crates/flexcore-application/src/pcc_tracking.rs`,
  `crates/flexcore-application/src/lib.rs` (additive module declarations
  only), `crates/flexcore-application/tests/scheduling_objective.rs`,
  `crates/flexcore-application/tests/pcc_tracking.rs`.
- **Prior-phase inputs consumed:** the P3.06 degradation placeholder and
  its `DegradationMetadata` (flexcore-domain `bess_model`); the Phase 2
  site registry's declared PCC (`Site::point_of_common_coupling`).

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P3.14-1 | **Objective term set is the CLOSED §12.1 five-term vocabulary.** `min Σ_t (C_energy + C_degradation + C_deviation + C_curtailment + C_service)` with each term switchable/configurable. The term set is an enum (`ObjectiveTerm`), so adding a term requires a new variant AND an `OBJECTIVE_SCHEMA_VERSION` bump; a config with an unsupported version is rejected. There is no catch-all "other" slot: a silent objective extension is impossible by construction. | `scheduling_objective.rs` (`ObjectiveTerm`, `OBJECTIVE_SCHEMA_VERSION`, `ObjectiveConfig::new`) |
| D-P3.14-2 | **Foundations §7.5 extras are NOT in the v0.2.0 objective — recorded exclusion, carried in the record output (not a comment).** Flex discomfort, commitment deviation, reserve shortfall, and switching costs appear in foundations §7.5 as terms an objective "may minimize" (illustrative, not mandatory), while report §12.1 defines the objective as exactly the five terms above. The v0.2.0 objective is the §12.1 terms only; later adoption is a recorded design decision requiring a schema-version change, never a quiet extension. The exclusion statement (`EXCLUDED_TERMS_STATEMENT`) is a record field (`objective.excluded_terms`) on every schedule record. | `scheduling_objective.rs` (`EXCLUDED_TERMS_STATEMENT`, `ObjectiveConfig::record_fields`) |
| D-P3.14-3 | **Default v0.2.0 objective: `SYNTHETIC_ENERGY_COST_DEVIATION_V1`** — minimise energy cost against the versioned synthetic price curve plus a quadratic deviation penalty against a versioned synthetic reference trajectory. Energy weight 1.0 (dimensionless); deviation coefficient 50 currency-units/(MW²·h) (ASSUMPTION, ledger open question 6 — a planning-aid value with no market meaning); degradation/curtailment/service disabled (switchable). Labelled SYNTHETIC in the config name, the provenance enum (single variant), every record field, and the evaluation provenance — not a market product; D1/D2 OPEN. No objective is hard-coded as universal: the name is a required, validated field. | `scheduling_objective.rs` (`ObjectiveConfig::default_synthetic`, `ObjectiveProvenance`) |
| D-P3.14-4 | **Term units and sign conventions are declared and recorded per schedule.** Energy: `C = −w·price·P_export·Δt` (the NEGATIVE price-times-energy product — revenue convention: export at a positive price earns a credit, import incurs cost; signs flip at negative prices); w dimensionless ≥ 0. Degradation: `w` dimensionless ≥ 0 on the P3.06 placeholder cost. Deviation: coefficient currency-units/(MW²·h) ≥ 0 (pure penalty). Curtailment: coefficient currency-units/MWh ≥ 0. Service: coefficient currency-units/MWh with a DECLARED sign (positive = cost, negative = credit) — the one term whose sign is a configuration choice (§12.1: sign conventions are region/product-specific). "currency-units" is deliberately generic (the P3.06 precedent): the cost unit is an objective-configuration choice, not a canonical platform unit, and no market currency is implied (D1 OPEN). Every term's unit and sign rule is a record field (`term.<name>.weight_unit`, `term.<name>.sign_convention`). | `scheduling_objective.rs` (term config structs, `ObjectiveTerm::weight_unit`/`sign_convention`, `record_fields`) |
| D-P3.14-5 | **Objective identity travels per schedule; different objectives are never the same problem.** The config serialises as ordered record fields for the P3.21 record (`ObjectiveConfig::record_fields`); `same_problem` compares schema version, name, all weights, and the threshold. The module header documents the three-way distinction: the §12.1 deviation term (synthetic reference trajectory) ≠ the §12.2 tracking term (P3.16 synthetic commitments, separate module) ≠ the §7.5 "commitment deviation" term (excluded). | `scheduling_objective.rs` (`record_fields`, `same_problem`, module header) |
| D-P3.15-1 | **Every ENABLED term traces to a named, VERSIONED input — no untracked constants.** Energy → the `SyntheticPriceCurve` version; degradation → the P3.06 `DegradationMetadata` per asset (placeholder status and the recorded placeholder statement travel with the term; metadata whose placeholder statement is not the recorded P3.06 statement is REJECTED); deviation → the `SyntheticSeries` reference version; curtailment/service → named `RuleRef` (name + version). A term built from a missing or unversioned input is a validation error (`TermInputMissing` / `UnversionedTermInput`). Provenance records are carried on every evaluation (`TermProvenanceRecord`). | `scheduling_objective.rs` (input resolution in `evaluate_objective`, `TermProvenanceRecord`) |
| D-P3.15-2 | **Dominance threshold: 100.0 (= two orders of magnitude), configuration, default `DEFAULT_DOMINANCE_THRESHOLD`.** A term whose absolute total reaches `threshold ×` the largest absolute total among the OTHER enabled nonzero terms triggers a `ScalingWarning` (term, other term, ratio, threshold, `review_required: true`, the recorded statement) in the evaluation diagnostics. With fewer than two nonzero enabled terms there is nothing to dominate — no warning (recorded rule). Thresholds below 1.0 or non-finite are rejected. Solver scaling itself is a P3.19 concern; this check records the review obligation only. | `scheduling_objective.rs` (`dominance_warnings`, `ScalingWarning`, `SCALING_REVIEW_STATEMENT`) |
| D-P3.15-3 | **Zero- and negative-price behaviour is documented, carried per interval, and explainable in diagnostics.** Zero price: the energy term contributes exactly nothing that interval; the schedule there is governed by the other enabled terms. Negative price: the energy signs flip — export incurs cost, import earns a credit — so the documented objective rewards consumption there. Each interval records its `PriceRegime` and a diagnostics string with the regime explanation and the term contributions. The recorded v1 curve is designed to exercise both regimes. | `scheduling_objective.rs` (`PriceRegime::explain`, `IntervalObjectiveCost.diagnostics`) |
| D-P3.15-4 | **Degradation totals are not silently split.** The P3.06 `DegradationMetadata` carries schedule totals (throughput, cost), not per-interval costs; the objective reports the weighted total in the totals and records the placeholder note (`degradation_note`) instead of inventing a per-interval split. | `scheduling_objective.rs` (degradation block, `ObjectiveEvaluation.degradation_note`) |
| D-P3.16-1 | **Tracking is at the DECLARED PCC, whose identity is a REQUIRED, VALIDATED field.** `PccRef` validates non-empty, whitespace-free identity and is constructible straight from the Phase 2 site registry (`PccRef::from_site` over `Site::point_of_common_coupling`). Tracking is never computed at the sum of arbitrary bus injections (§12.2); the attributed-asset aggregation per delivery point is declared caller configuration — no bus topology is re-derived (there is no grid model in v0.2.0; that is Phase 4). | `pcc_tracking.rs` (`PccRef`, `TrackingPoint`, module header) |
| D-P3.16-2 | **Loss model: declared net-export fraction per delivery point, `P_PCC = (1 − loss_fraction) · Σ P_asset`, `loss_fraction ∈ [0, 1)` — an ASSUMPTION-labeled linearisation.** For import intervals (net P < 0) the fraction reduces the import magnitude, which understates losses on import; the network-level loss treatment belongs to the Phase 4 grid model (§12.4–§12.5). The note (`LOSS_MODEL_NOTE`) is a record field. `LossModel::None` is the zero-fraction case. | `pcc_tracking.rs` (`LossModel`, `LOSS_MODEL_NOTE`) |
| D-P3.16-3 | **Multiple delivery points are SEPARATE tracking terms, never merged.** Each configured point carries its own PCC identity, λ, loss model, per-interval series, and total; duplicate identities in one config or input list are rejected (two terms for one identity would be a merge mistake). The evaluation's `total` is an explicitly labelled sum of the separate per-point records, which are the primary output. | `pcc_tracking.rs` (`PccTrackingConfig::new`, `PccTrackingEvaluation`, `tracking_record_fields`) |
| D-P3.16-4 | **Commitments in v0.2.0 are SYNTHETIC test commitments — labelled in the type, not by convention.** `SyntheticCommitment` requires a version and exposes its values ONLY through the labelled accessor (`labeled_values` → provenance `SYNTHETIC`, version, values, the recorded no-market-qualification statement) — the label cannot be dropped (the P3.31 four-place labelling precedent: type here, record fields, evaluation label field; P3.21 record downstream). D1 is OPEN: no market, no jurisdiction, no market qualification is claimed anywhere (§0.3, §12.2). | `pcc_tracking.rs` (`SyntheticCommitment`, `COMMITMENTS_SYNTHETIC_LABEL`, `SYNTHETIC_COMMITMENT_NOTE`) |
| D-P3.16-5 | **Sign convention frozen at §10.2:** `P > 0` export at the PCC, `P < 0` import; mismatch `= P_PCC − P_commitment` (positive = over-delivery); the cost uses the square (sign-symmetric) and the signed mismatch is recorded per interval for diagnostics. λ_track ∈ finite ≥ 0, unit currency-units/(MW²·h), recorded per point. | `pcc_tracking.rs` (`TRACKING_SIGN_CONVENTION`, `IntervalTracking`) |

## 2. The synthetic default configuration (recorded with ASSUMPTION labels)

**Default objective name:** `SYNTHETIC_ENERGY_COST_DEVIATION_V1`
(`DEFAULT_SYNTHETIC_OBJECTIVE_NAME`).

**Synthetic price curve `synthetic-price-curve-v1`** — hourly values in
generic currency-units/MWh, indexed by UTC hour offset from the horizon
start modulo 24 (`SYNTHETIC_PRICE_CURVE_V1_HOURLY`). **ASSUMPTION:** these
are recorded configuration values for the synthetic baseline — not measured
data, not a market price, not a forecast of any market (D1 OPEN). Designed
to exercise the documented zero-price (h12 = 0.0) and negative-price
(h20 = −12.0) behaviour of P3.15 step 3:

| h00 | h01 | h02 | h03 | h04 | h05 | h06 | h07 | h08 | h09 | h10 | h11 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 42 | 38 | 35 | 33 | 32 | 36 | 55 | 72 | 80 | 74 | 60 | 50 |

| h12 | h13 | h14 | h15 | h16 | h17 | h18 | h19 | h20 | h21 | h22 | h23 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 44 | 40 | 38 | 52 | 78 | 95 | 105 | −12 | 60 | 50 | 46 |

**Deviation reference `synthetic-reference-trajectory-v1`:** flat zero
export (`SyntheticSeries::v1_default_flat_zero`). **ASSUMPTION:** the
neutral synthetic baseline; the reference is schedule-vs-reference
deviation input, NOT a market commitment (see D-P3.14-5).

**Deviation coefficient:** 50 currency-units/(MW²·h)
(`DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H`). **ASSUMPTION:** ledger open
question 6 — product to confirm; the weight is configuration either way.

**Dominance threshold:** 100.0 (`DEFAULT_DOMINANCE_THRESHOLD`), recorded
per D-P3.15-2; overridable per config.

## 3. Scope and boundary statements

- The entire 0.x line is simulation-only; nothing in these modules
  authorises field control or device writes. A solved schedule is a
  CANDIDATE — feasibility, authorization, and dispatch are Phase 4 gates
  and are never implied.
- No solver is claimed or invoked: `select_minimising_candidate` is a pure
  argmin over an explicitly supplied candidate set (a fixture/test seam),
  not an optimizer and not an optimality claim beyond the enumerated
  candidates. Real optimisation is P3.17 behind the `Optimizer` port.
- No safety, certification, or protection-grade claims; the tracking
  evaluation is a candidate metric, not a delivery or settlement record.
- The foundations §7.5 exclusion (D-P3.14-2) means the v0.2.0 objective
  contains no flex-discomfort, commitment-deviation, reserve-shortfall, or
  switching-cost term; the §12.2 tracking term implemented here is a
  separately recorded term of report §12.2, composed into the Tier 1
  formulation at P3.17.

## 4. Review status and verification

- **Power-systems review sign-off: PENDING** (same status as the other
  Phase 3 records). A human power-systems review must confirm the
  decisions in §1 — in particular D-P3.14-4 (the negative
  price-times-energy revenue convention for `C_energy`), D-P3.15-2
  (threshold 100.0 and the fewer-than-two-nonzero rule), and D-P3.16-2
  (the net-export-fraction loss linearisation) — before the ledger
  P3.14–P3.16 checkboxes are ticked.
- **Worked example, hand-computed** (asserted in
  `tests/scheduling_objective.rs`): 1-hour intervals, prices [40, −10],
  export [2, 1] MW, reference [1, 1], k_dev = 50 → C_energy = −80 + 10 =
  −70; C_dev = 50·(1 + 0) = 50; total = −20. Weight-sensitivity fixture:
  candidates A [5, 0] vs B [0, 5] over prices [42, 38], reference [0, 5];
  default weights select B (−190 vs 2290); deviation weight 0.1 selects A
  (−205 vs −187.5). PCC loss fixture (asserted in
  `tests/pcc_tracking.rs`): commitment 2.0 MW export, λ = 10, loss 0.05 →
  asset-sum schedule costs 0.1; the schedule injecting 2.0/0.95 MW meets
  the PCC target at cost 0 and scores strictly better.
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for
  these files. The first `cargo test -p flexcore-application` run must
  confirm; any fix must keep the decisions in §1 unchanged.
