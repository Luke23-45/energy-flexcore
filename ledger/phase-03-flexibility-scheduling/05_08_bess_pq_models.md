# P3.05–P3.08 — BESS model and inverter P-Q envelope: decisions and review record

- **Date:** 2026-11-07.
- **Scope:** P3.05 (BESS capability model), P3.06 (BESS degradation and
  reserve treatment), P3.07 (BESS edge cases), P3.08 (inverter P-Q
  envelope).
- **Normative sources:** report §11.2 (BESS model), §11.3 (inverter P-Q
  envelope), §11.1 (generic capability model), §12.1 (objective terms),
  §10.2 (units and sign), §10.3 (time model); foundations specification
  §7.2 (auxiliary-load term in the storage energy balance) governs on
  conflict. Report §11.3 is quoted where its wording is load-bearing
  ("necessary but not complete"; the IEEE 1547-2018 Clause 5 scope
  paragraph [R18]).
- **Files:** `crates/flexcore-domain/src/bess_model.rs`,
  `crates/flexcore-domain/src/inverter_pq_model.rs`,
  `crates/flexcore-domain/src/lib.rs` (additive module declarations only),
  `crates/flexcore-domain/tests/bess_model.rs`,
  `crates/flexcore-domain/tests/inverter_pq_model.rs`.

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P3.05-1 | **Simultaneous charge/discharge (decided once): convex-relaxation path with a MANDATORY post-solution check.** The relaxed per-interval feasible set admits P_ch > 0 and P_dis > 0 simultaneously (what an LP relaxation sees); `BessModel::detect_simultaneous_charge_discharge` is the single mandatory post-solution detector — it runs inside every `evaluate_schedule` call and is public for the Tier 1 MILP path (P3.17 problem IR) and the P3.21 schedule validator, which must call it rather than reimplement it. Rationale: this module is the pure feasibility/limits layer and has no solver coupling; a binary mode-variable formulation (u_ch + u_dis ≤ 1) is a Tier 1 formulation choice belonging to P3.17. If P3.17 adopts binary mode variables, the model is a **MIP** and must never be labelled a continuous SOCP (AGENTS.md rule 4). No solver class is claimed anywhere in this module. | `bess_model.rs` (module header + detector) |
| D-P3.05-2 | **Auxiliary load (foundations §7.2): applied where data exists, RECORDED where it does not — never a silent zero.** Per-interval aux data known → `−P_aux·Δt` enters the balance. Aux unknown → the term is omitted (no value is invented) and the omission is carried on the evaluation output as `AuxLoadState::Unknown { intervals, note }`, whose note states that the SOC trajectory may understate energy drain. This is a recorded deviation from the P3.02 collapse-to-zero fallback: that fallback targets feasible-set BOUNDS, while P_aux is an energy-balance term whose most conservative value is not zero (zero understates drain — exactly the failure the ledger forbids). | `bess_model.rs` (`AuxLoadPoint`, `AuxLoadState`) |
| D-P3.05-3 | **SOC ↔ energy mapping (declared convention, not physics):** `SOC = (E − E_min)/(E_max − E_min)` over the effective (post-narrowing) window; a degenerate window maps its single energy value to SOC 0.0. SOC stays a fraction in [0,1], boundary-inclusive, at the API boundary. | `bess_model.rs` (`energy_to_soc`/`soc_to_energy`) |
| D-P3.05-4 | **Unknown provenance narrows (frozen P3.02 rule reused, not reimplemented).** Static bounds carry `Provenance`; an UNKNOWN-provenance energy window is narrowed via `narrow_interval(AssetClass::Bess, ENERGY_SOC, …)` (collapse-to-zero; empty intersection → the energy dimension is infeasible and every evaluation reports `EnergyWindowInfeasible` — never widened back). UNKNOWN-provenance power limits narrow to zero credit. SOC-dependent power limits, temperature derates, ramp/dwell, reserve, and BMS constraints enter ONLY as explicitly configured inputs; anything not configured is listed in `BessEvaluationProvenance::unknown_dimensions` on every output — never silently ignored, never filled with invented values. Temperature data with UNKNOWN provenance applies NO derate (no credit is taken from unknown data; the P3.02 `TEMPERATURE_HEALTH` row). BMS constraints are always recorded unknown (no data exists anywhere in the phase inputs). | `bess_model.rs` (`BessModel::new`, `unknown_dimensions`) |
| D-P3.05-5 | **Efficiencies are required configuration with no default** (`η_ch`, `η_dis` ∈ (0, 1]); no value is invented. Recorded gap: the P3.02 unknown-narrowing rule targets bounds, not balance coefficients, so an unknown-provenance efficiency has no defined narrowing — see §3 (gap list). | `bess_model.rs` (constructor) |
| D-P3.06-1 | **Degradation cost: configurable LINEAR throughput term, default ZERO, explicit placeholder metadata.** No verified degradation model is on record; the rate is expressed in generic cost units per MWh of (P_ch + P_dis)·Δt throughput (the cost unit is an objective-configuration choice, not a canonical platform unit). Every evaluation output carries `DegradationMetadata` with `is_placeholder: true` whenever the term is configured, plus the constant statement "PLACEHOLDER: configurable linear throughput term, not a measured degradation law…" — visible in the output type, not a comment. When not configured: cost zero, `is_placeholder: false`. | `bess_model.rs` (`DegradationConfig`, `DegradationMetadata`, `DEGRADATION_PLACEHOLDER_STATEMENT`) |
| D-P3.06-2 | **Reserve margin: configurable SOC floor narrowing the usable energy window.** Values are configuration, never physics. Usable energy stays ≥ `E_min + floor·(E_max − E_min)` at every trajectory boundary (boundary-inclusive within the tolerance). An UNKNOWN-provenance floor narrows to the most conservative reading — no usable energy credited (effective floor 1.0); unknown never widens. No reserve configured → full window usable, recorded as an unknown dimension. | `bess_model.rs` (`ReserveConfig`, `usable_energy_window`) |
| D-P3.07-1 | **Clip vs reject (decided once): REJECT.** A setpoint or trajectory that violates a bound produces a named, structured `BessViolation`; the model never modifies (clips) a declared setpoint and never silently repairs a trajectory. Rationale: this layer evaluates CANDIDATE schedules from an optimizer; silently clipping would mask solver errors and break the honesty discipline (the recorded schedule must be the schedule that was checked). The only sanctioned adjustment is computational: a NEGATIVE magnitude (a violation) contributes zero to the balance, and the violation is still reported — nothing is silent. An over-limit magnitude is used as declared so the trajectory shows its consequence. (Ledger P3.07 step 3 says "record in PLAN.md"; the task scoping for this batch records the decision HERE instead — PLAN.md was outside this task's file ownership.) | `bess_model.rs` (module header, `evaluate_schedule`) |
| D-P3.07-2 | **Tolerance is part of the API contract:** `BESS_EVALUATION_TOLERANCE = 1e-9` in each value's own unit (MWh, MW, SOC fraction). Comparisons are boundary-inclusive within it (SOC exactly 0 and exactly 1 feasible; power exactly at a limit feasible). The P3.21 residual check must reuse this constant rather than inventing its own; the worked example states it explicitly. | `bess_model.rs` (constant + header) |
| D-P3.07-3 | **Transition constraints where configured:** ramp as a RAMP_RATE configuration value (MW/min, converted per-interval via `interval_seconds`; first interval exempt — no prior setpoint exists in the model) and a minimum dwell in intervals for charge/discharge episodes (the interval-resolution min-up/min-down analogue; horizon-truncated episodes exempt; idle episodes have no configured dwell — the §11.1 cross-interval MILP formulation is a P3.17 concern). Unconfigured → recorded unknown. | `bess_model.rs` (`TransitionConstraints`) |
| D-P3.08-1 | **Voltage data missing → the most restrictive documented value, recorded.** Per-mode limit rows may declare their voltage; with the operating voltage missing or undocumented, the envelope intersects ALL rows of the mode (most restrictive window) and records `VoltageTreatment::MostRestrictiveDocumented` on the output — never silent, never a widened default. | `inverter_pq_model.rs` (`VoltageTreatment`, `voltage_rows`) |
| D-P3.08-2 | **No power-factor inequality assuming P ≥ 0 exists** (§11.3). The envelope uses the symmetric circle constraint plus declared per-mode tables only; P > 0 = export is the frozen sign rule, so charging is negative P and both directions have their own tables. The test `negative_p_point_fails_if_a_power_factor_assumption_is_introduced` fails if such an inequality is ever introduced. | `inverter_pq_model.rs` (module header + test) |
| D-P3.08-3 | **Unknown narrows (P3.02 applied to the envelope):** an undeclared mode credits nothing (`ModeNotDeclared`); a mode with UNKNOWN provenance credits nothing (`ModeNotCredited` — mode credit requires known provenance, §11.1); a (mode, P) point with no covering Q rows collapses Q to [0, 0] MVAr (the recorded collapse-to-zero fallback, never "unconstrained"). Rows may be empty per mode — that dimension then collapses to zero, with the collapse recorded in the output's unknown dimensions. | `inverter_pq_model.rs` (`evaluate`) |
| D-P3.08-4 | **Reactive sign is declared, never assumed:** the envelope takes the declared `ReactiveSignConvention` as a required constructor argument — no default, no inference (§10.2). The convention is interpretation metadata; the numeric bounds are the declared per-mode tables. | `inverter_pq_model.rs` (constructor) |

## 2. IEEE 1547-2018 Clause 5 [R18] — scope statement (ledger P3.08 step 4)

IEEE 1547-2018 defines DER interconnection and interoperability performance
requirements. Its Clause 5 includes reactive-power and voltage/power-control
capabilities; the exact capability and settings depend on the applicable
category, performance requirements, utility profile, and equipment. It is
**not** a universal specification for internal BMS cell-temperature
protection, and equipment safety/protection and conformance testing under
IEEE 1547.1 remain separate responsibilities. **Implementing this envelope
claims NO conformance** with IEEE 1547-2018, IEEE 1547.1, or any other
standard. The statement is carried in the `inverter_pq_model` module header
and repeated here for the review record.

The apparent-power constraint P² + Q² ≤ S_max² is documented as **necessary
but not complete** (§11.3 wording): it is the basic limit, not a complete
universal inverter model; the per-mode, per-voltage tables carry the
device-specific bounds, and neither alone is the whole model.

## 3. Registry survey (ledger P3.06 step 1): what Stage 2 actually holds, and the gaps

Surveyed: `crates/flexcore-domain/src/entities.rs` (the `Asset` and
`AssetState` types are the whole of the Phase 2 registry vocabulary) and the
frozen `flexcore-contracts/src/capability.rs` schema. What the registry
holds per asset: identity, site/tenant chain, type, vendor/model string,
**rated_power_mw** (required, single scalar), **rated_energy_mwh**
(optional, storage assets), phase and bus reference strings, a
**capability_profile reference string**, lifecycle status, timestamps;
`AssetState` adds SOC (optional), active/reactive power (optional, reactive
only with its declared convention), availability (bool), operating-mode
string, a free-form health string, and a telemetry quality flag.

Gaps — recorded as unknowns, not filled with invented values:

| # | Gap | Effect in the BESS model |
|---|---|---|
| G1 | **No charge/discharge power limits** — only one `rated_power_mw` scalar; the registry cannot distinguish P_ch,max from P_dis,max. | `p_ch_max_mw` / `p_dis_max_mw` are required model configuration; the caller supplies them from the device profile, and their provenance is carried and narrowed per D-P3.05-4. |
| G2 | **No efficiencies (η_ch, η_dis)** anywhere in the registry or the capability schema. | Required configuration, no default invented (D-P3.05-5); no provenance-narrowing rule exists for balance coefficients — recorded as an open item. |
| G3 | **No energy window** — `rated_energy_mwh` is at best an E_max candidate; E_min has no registry source. | `e_min_mwh`/`e_max_mwh` are required configuration; UNKNOWN provenance narrows per the frozen rule. |
| G4 | **No auxiliary-load (P_aux) data.** | Per-interval `AuxLoadPoint::Unknown` is the honest state; the omission is recorded on every output (D-P3.05-2), never silently zeroed. |
| G5 | **No degradation parameters and no verified degradation law.** | The linear-throughput placeholder with default zero and mandatory placeholder metadata (D-P3.06-1). Ledger open question 3 (is a zero default acceptable for v0.2.0?) remains open with product. |
| G6 | **No reserve/SOC-band configuration.** | Reserve is configuration-only (D-P3.06-2); unconfigured → recorded unknown. |
| G7 | **No temperature or BMS constraint data** (`health` is a free-form string, not a derate). | Temperature enters only as a configured per-interval MW derate; BMS constraints are always recorded unknown (D-P3.05-4). |
| G8 | **No ramp-rate or minimum up/down declarations** in the registry; the capability schema carries the kinds but Phase 2 populated none. | Ramp/dwell are configuration-only; unconfigured → recorded unknown (D-P3.07-3). |
| G9 | **No reactive-capability / Q-limit data in the registry** (the P-Q envelope configuration must come from the device profile; the registry's `capability_profile` is an unstructured reference string). | P/Q per-mode tables are required envelope configuration; undeclared modes and uncovered Q points narrow to zero credit (D-P3.08-3). |
| G10 | **No volt-var/volt-watt curve data.** | Out of scope here — that scoping decision is ledger P3.09; this module never assumes a curve (P3.09 step 2's unknown-by-default rule). |

## 4. Review status and verification

- **Power-systems review sign-off: PENDING** (same status as the P3.01–P3.04
  record). This record was written by the implementing agent; a human
  power-systems review must confirm the decisions in §1 — in particular
  D-P3.05-1 (relaxation + mandatory post-check as the domain-layer path),
  D-P3.07-1 (REJECT over clip), and the D-P3.05-2 deviation from the
  collapse-to-zero fallback — before the ledger P3.05–P3.08 checkboxes are
  ticked.
- **Worked example (P3.05 step 4), hand-computed:** η_ch = η_dis = 0.9,
  Δt = 1 h, E ∈ [0, 8] MWh, P limits 5 MW, aux configured zero,
  E_0 = 0: charge 5 MW → E_1 = 4.5 (SOC 0.5625); discharge 0.9 MW →
  E_2 = 3.5 (SOC 0.4375); idle → E_3 = 3.5; charge 5 MW → E_4 = 8.0
  (SOC 1.0, full). Asserted within the explicitly stated tolerance 1e-9
  (`tests/bess_model.rs`). A second variant applies the foundations §7.2
  aux term (0.5 MW on the idle interval → E_3 = 3.0, E_4 = 7.5).
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for these
  files. The first `cargo test -p flexcore-domain` run must confirm; any
  fix must keep the decisions in §1 unchanged.
- **Frozen schema unchanged:** no additive `capability.rs` extension was
  needed — the models consume the frozen schema types (e.g. `Provenance`)
  and take everything else as domain-side configuration. **No PROPOSED
  migration note is required this batch.** Candidate schema extensions for
  a later, deliberate version bump (not proposed now): (a) a dedicated
  `AUXILIARY_LOAD` constraint kind so P_aux provenance can travel on the
  wire; (b) a per-row `ConstraintUnit::MVA` for apparent-power tables.
- **PLAN.md clip-vs-reject pointer:** the ledger asks for the decision in
  PLAN.md; it is recorded here (D-P3.07-1) because PLAN.md was outside this
  task's file ownership. A later docs pass should add the pointer.
- **Terminology:** a violation-free evaluation makes a schedule a CANDIDATE
  with respect to asset constraints — feasibility, authorization, and
  dispatch are Phase 4 gates and are never implied by these modules
  (simulation-only 0.x; no field-write path).
