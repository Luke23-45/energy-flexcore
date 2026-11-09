# P3.09–P3.11 — Volt-var scoping, flexible load, and EV fleet: decisions and review record

- **Date:** 2026-11-07.
- **Scope:** P3.09 (volt-var/volt-watt capability scoping), P3.10 (flexible
  load model), P3.11 (EV fleet model).
- **Normative sources:** report §11.3 (inverter P-Q envelope, volt-var
  scope), §11.4 (flexible load model), §11.5 (EV fleet model), §11.1
  (generic capability model), §10.2 (units and sign), §10.3 (time model),
  §25.2 (D3 network class). The frozen P3.02 provenance rule
  (`01_04_capability_contract.md`) governs unknown handling; foundations
  documents govern on conflict. Report §11.5 is quoted where its wording is
  load-bearing (the five bidirectional-support conditions; the OCPP 2.1
  [R19] scope sentence).
- **Files:** `crates/flexcore-domain/src/volt_var_scoping.rs`,
  `crates/flexcore-domain/src/flexible_load_model.rs`,
  `crates/flexcore-domain/src/ev_fleet_model.rs`,
  `crates/flexcore-domain/src/lib.rs` (additive module declarations only),
  `crates/flexcore-domain/tests/flexible_load_model.rs`,
  `crates/flexcore-domain/tests/ev_fleet_model.rs`.

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P3.09-1 | **Unknown-by-default (decided once):** a volt-var/volt-watt response is credited ONLY where a curve is configured with a traceable source (`DEVICE_DATASHEET` or `UTILITY_PROFILE`). Where curves are absent — or the only source is a labelled ASSUMPTION — the scheduler must NOT assume a volt-var response: the capability is recorded UNKNOWN per the frozen P3.02 rule and the dimension narrows conservatively (declared window ∩ collapse-to-zero default for `INVERTER_PQ`; empty intersection infeasible, never widened). A source labelled ASSUMPTION maps to UNKNOWN provenance and credits nothing until re-pointed to a traceable source. | `volt_var_scoping.rs` (`CurveSource::provenance`, `narrow_volt_var_reactive_credit`, `narrow_volt_watt_active_credit`) |
| D-P3.09-2 | **The scoping table is typed, not prose-only:** `ProfileVoltVarScope` carries the per-profile status; `synthetic_portfolio_scopes()` is the code-side survey of scenario-v2.json (verified: the scenario declares no curve fields for any profile), mirroring §2 below. The P3.08 P-Q envelope keeps the numeric tables; this module decides whether a curve may be assumed at all. | `volt_var_scoping.rs` (`ProfileVoltVarScope`, `synthetic_portfolio_scopes`) |
| D-P3.10-1 | **Baseline rule DECIDED ONCE:** the baseline method is `scenario-declared-baseline-profile` — the per-interval baseline demand is the value the scenario fixture itself declares (scenario-v2.json behaviour profiles). It is recorded SYNTHETIC and is NOT a market/DR baseline or a measurement claim; region/product-specific DR baseline and measurement-and-verification rules are out of scope (§11.4; D1/D2 open). The constructor REJECTS any other method name — a market baseline cannot enter through configuration. | `flexible_load_model.rs` (`SCENARIO_DECLARED_BASELINE_METHOD`, `SYNTHETIC_BASELINE_DISCLAIMER`, `BaselineMethod`) |
| D-P3.10-2 | **Duration/recovery policy DECIDED ONCE: EXCLUDE-AND-FLAG.** An event exceeding the maximum event duration, or starting before the minimum recovery completes, is (a) flagged with a named structured violation and (b) excluded from credited flexibility — its energy is never credited and it owes no rebound. Setpoints are never clipped or silently repaired (the BESS REJECT honesty discipline). The enforcement is the pure function `enforce_event` — the single implementation the P3.12 envelope and P3.17 scheduler reuse rather than reimplement. | `flexible_load_model.rs` (`DURATION_RECOVERY_POLICY_STATEMENT`, `enforce_event`, `evaluate_schedule`) |
| D-P3.10-3 | **Rebound convention DECID ONCE:** rebound energy is a REQUIRED field; zero only if explicitly configured as zero WITH a recorded reason, never defaulted. Payback owed by a credited event is prorated linearly by event duration against the full maximum-duration event (`owed = rebound_mwh × duration/max_duration`) — a declared synthetic convention, not a measured load law. Net flexibility never overstates: `net = credited_reduction − max(rebound_owed, scheduled_increase)`; every scheduled increase is treated as rebound payback (conservative recorded assumption), and unpaid rebound is subtracted even though it occurs beyond the horizon. A shortfall raises `ReboundPaybackIncomplete`. | `flexible_load_model.rs` (`ReboundEnergy`, `REBOUND_CONVENTION_STATEMENT`) |
| D-P3.10-4 | **Response time and comfort/process constraints are recorded, not enforced.** Response time is required, validated configuration; its LAG is not enforced at interval resolution (a cross-interval P3.17 formulation concern) and the omission is recorded as an unknown dimension on every output. Comfort/process constraints are declared descriptors — no numeric comfort law exists in the scenario and none is invented; undeclared constraints are recorded as an unknown dimension. | `flexible_load_model.rs` (`unknown_dimensions` in `FlexibleLoadEvaluation`) |
| D-P3.10-5 | **Unknown provenance narrows (frozen P3.02 rule reused, not reimplemented):** an UNKNOWN-provenance reducible/increasable range collapses to zero credit via `narrow_interval(AssetClass::FlexibleLoad, ACTIVE_POWER, …)` — never "unconstrained". | `flexible_load_model.rs` (constructor) |
| D-P3.11-1 | **V2X OFF by default, DECID ONCE (§11.5):** bidirectional capability is enabled ONLY when the EV, the EVSE, the protocol, the interconnection agreement, AND the market rules ALL support it — five conditions, each with recorded non-empty per-vehicle evidence; an unevidenced or partial enablement is a construction error. In the synthetic portfolio V2X stays disabled except for the NAMED test fixture `v2x-enabled-fixture-001` (§4 below documents all five conditions, each labelled synthetic). Any discharge while disabled is a named `DischargeWithV2xDisabled` violation and contributes zero to the balance. | `ev_fleet_model.rs` (`V2xConfig`, `V2xEvidence`, `V2X_DISABLED_BY_DEFAULT_REASON`) |
| D-P3.11-2 | **Departure energy is a HARD constraint, DECID ONCE:** a vehicle must reach its required departure SOC by its departure instant in every feasible schedule (`DepartureEnergyPolicy::Hard` — the only variant). The declaration is exposed on the model and every evaluation so the P3.17 problem IR carries it into the formulation; any relaxation is a named, reviewed decision recorded in the schedule (P3.19) — this module provides no relaxation variant to call. A shortfall raises `DepartureEnergyNotMet`; the schedule is never repaired. | `ev_fleet_model.rs` (`DepartureEnergyPolicy`, `DEPARTURE_ENERGY_POLICY_STATEMENT`) |
| D-P3.11-3 | **Opt-out removes the vehicle from the feasible set FROM THE OPT-OUT INSTANT ONWARD** (start-inclusive interval semantics): from that instant the vehicle appears in no active-vehicle list, contributes nothing to the dispatchable capacity, accepts no setpoint (named `DispatchedAfterOptOut` violation), and exchanges no energy. Capacity is not silently retained; the envelope re-derivation is P3.24's job. | `ev_fleet_model.rs` (`evaluate_schedule`, `vehicle_active_at`) |
| D-P3.11-4 | **Energy balance is a declared lossless convention with physical gates:** `E(t+1) = E(t) + (P_charge − P_discharge)·Δt`, applied ONLY where the vehicle is connected and not opted out; negative magnitudes (violations) and V2X-disabled discharges contribute ZERO (impossible energy flow; each still reports its violation); over-limit magnitudes are used as declared so the trajectory shows the consequence (the BESS REJECT precedent). No charging-efficiency value exists anywhere in the inputs and none is invented — the lossless balance is recorded as an unknown dimension (the SOC trajectory may overstate delivered energy). | `ev_fleet_model.rs` (module header, `evaluate_schedule`) |
| D-P3.11-5 | **Departure must fall on an interval boundary, inside the horizon, and be covered by a connection window** (strictly after a window start, at or before its end, §10.3 end-exclusive semantics). The horizon end is the latest fleet departure; opt-out instants must lie inside the horizon. A mid-interval departure is a P3.17 formulation concern and is rejected at construction rather than silently rounded. | `ev_fleet_model.rs` (constructor) |
| D-P3.11-6 | **Site transformer limit applies to the fleet NET import per interval** (Σ effective charge − discharge over active vehicles). No export-side limit data exists; the absence is recorded as an unknown dimension, never silently unconstrained. Charging priority is declared metadata (larger = higher priority); ordering is a P3.17 scheduler concern — recorded on every per-vehicle evaluation, never enforced here. | `ev_fleet_model.rs` (aggregation, `EvUnknownDimension`) |

## 2. P3.09 — Per-profile volt-var/volt-watt scoping table (ledger step 1)

Surveyed against `scenarios/synthetic-portfolio/scenario-v2.json`: **the
scenario format declares no volt-var or volt-watt curve fields for any
profile** — this is a verified absence of configuration, not an assumption.
Every profile therefore scopes to NOT-CONFIGURED with UNKNOWN provenance
(D-P3.09-1): the scheduler must not assume a volt-var response anywhere in
the synthetic portfolio, and no invented curve may enter any envelope input
(P3.09 acceptance test: `unconfigured_profile_yields_no_volt_var_contribution`
and `unconfigured_profile_narrows_a_declared_q_window_to_zero_credit`).

| Behaviour profile (scenario-v2.json) | Assets | Inverter-coupled | Volt-var curve | Volt-watt curve | Source | Provenance | Effect on the envelope (§11.3/§11.6) |
|---|---|---|---|---|---|---|---|
| `bess_daily_cycle` | site-north-bess-1, site-south-bess-1 | Yes (BESS inverter) | Not configured | Not configured | None in scenario (verified) | UNKNOWN | No volt-var credit; declared Q window narrows to [0, 0] MVAr per P3.02; reactive capability stays unknown until curve data with a traceable source exists |
| `pv_clear_sky` | site-north-pv-1, site-south-pv-1 | Yes (PV inverter) | Not configured | Not configured | None in scenario (verified) | UNKNOWN | Same narrowing; the P-Q envelope of P3.08 applies only to explicitly declared tables |
| `load_baseline_reducible` | site-north-load-1, site-south-load-1 | No (load) | Not configured | n/a | None in scenario (verified) | UNKNOWN | No reactive capability is declared for the loads; nothing to credit |
| `evse_connection_windows` | site-north-evse-1, site-south-evse-1 | No (EVSE, P-only model) | Not configured | n/a | None in scenario (verified) | UNKNOWN | The §11.5 EV model declares no reactive capability; nothing to credit |
| `site_meter_aggregate` | site-north-meter-1, site-south-meter-1 | No (metering fixture) | Not configured | n/a | None in scenario (verified) | UNKNOWN | Metering consistency fixture; no inverter scope |

**Effect on the envelope (ledger P3.09 step 3):** unknown → conservative
narrowing. For any declared Q window the effective volt-var credit collapses
to [0, 0] MVAr (the recorded P3.02 collapse-to-zero fallback — intentionally
over-restrictive in simulation, recorded as a limitation, never repaired by
widening); the same rule applies to the volt-watt dimension against a
declared P window. This is over-restriction, not a claim about real devices:
scoping a profile to "no curve configured" says nothing about what any
inverter implements. IEEE 1547-2018 Clause 5 [R18] scope statements are
unchanged from P3.08, and nothing here claims conformance with any standard.

**Review note (ledger acceptance: "scoping table reviewed by power-systems
engineer"): PENDING** — same status as the P3.01–P3.08 records; a human
power-systems review must confirm §2 before the P3.09 checkbox is ticked.

## 3. P3.10 — The baseline rule and the duration/recovery policy (ledger steps 2–3)

Both are quoted in full in the module header of `flexible_load_model.rs` and
summarised in §1 (D-P3.10-1, D-P3.10-2, D-P3.10-3). Recorded here because the
ledger asks for a "baseline-rule note":

- **Baseline method:** `scenario-declared-baseline-profile` (named, DECIDED
  ONCE). The scenario declares the baseline shape (`morning_evening_peaks`,
  `reducible_range_mw` 0.3/0.4 for site-north/site-south) but no numeric
  per-interval series; the test fixtures therefore declare an explicit flat
  0.5 MW synthetic baseline and label it as such — a fixture value, not the
  scenario's shape made numeric. The baseline is SYNTHETIC; it is not a
  market/DR baseline and not a measurement claim. Region/product-specific DR
  baseline and M&V rules are out of scope (§11.4); D1/D2 open.
- **Duration/recovery policy:** EXCLUDE-AND-FLAG (chosen over the ledger's
  "exclude or flagged" alternative as the single decided policy: a violating
  event is both flagged AND excluded from credit). Boundary-inclusive within
  `LOAD_EVALUATION_TOLERANCE` (1e-9): an event of exactly the maximum
  duration and a gap of exactly the minimum recovery are feasible.
- **Fixture values traceable to scenario-v2.json:** reducible range
  0.3 MW (site-north-load-1). **ASSUMPTION-labelled fixture values** (the
  scenario declares none of these): increasable range 0.2 MW, maximum event
  duration 2 h, minimum recovery 1 h, rebound 0.3 MWh per full event,
  availability window 06:00–22:00 UTC, response time 300 s.

## 4. P3.11 — V2X enablement rule, the named fixture, and departure policy (ledger steps 2–3)

- **V2X is OFF by default.** Enabling requires all five §11.5 conditions —
  the EV, the EVSE, the protocol, the interconnection agreement, and the
  market rules — each supported AND evidenced with a recorded non-empty
  string; anything less is a construction error. The synthetic portfolio
  keeps V2X disabled everywhere except:
- **Named fixture `v2x-enabled-fixture-001`** (in
  `tests/ev_fleet_model.rs::v2x_enabled_fixture`), the only V2X-enabled case,
  with all five conditions documented and labelled synthetic: the fixture EV
  and EVSE are declared bidirectional-capable (ASSUMPTION — no real
  datasheet exists); the protocol condition cites OCPP 2.1 for capability
  SCOPE only [R19] — explicitly NOT a claim that any device implements or
  conforms to OCPP 2.1; the interconnection agreement and market-rules
  conditions are declared synthetic (D1 open). The test
  `v2x_enablement_requires_all_five_documented_conditions` rejects every
  partial-evidence enablement.
- **Departure energy is HARD** (D-P3.11-2): the tests
  `departure_energy_is_a_hard_constraint` and
  `opt_out_mid_charge_leaves_the_departure_constraint_hard_and_unmet` assert
  the named violation on every shortfall; no test relaxes it, and the module
  has no relaxation variant. The declaration travels on the evaluation
  (`departure_policy`, `departure_policy_statement`) for the P3.17 problem
  IR and the P3.19 relaxation ledger.
- **Opt-out mid-horizon** (ledger step 4): the tests
  `opted_out_vehicle_drops_out_of_the_feasible_set_from_the_opt_out_instant`
  and `dispatched_after_opt_out_is_a_named_violation_and_gated` assert that
  the vehicle leaves the active lists and the dispatchable capacity exactly
  at the opt-out instant (start-inclusive), that a post-opt-out setpoint is
  a named violation, and that capacity is not silently retained. The envelope
  re-derivation itself is P3.24's job.
- **OCPP 2.1 [R19] scope statement** (ledger acceptance): quoted in full as
  `OCPP_SCOPE_STATEMENT` in the module and asserted by
  `ocpp_2_1_is_cited_for_capability_scope_only`: cited for capability scope
  only, never as a claim that a device supports it.
- **Fixture values traceable to scenario-v2.json:** EVSE rated power
  0.05 MW / 0.15 MW (site-north/site-south `charging_power_mw`),
  initial SOC 0.3 / 0.25 and required departure SOC 0.9 / 0.85
  (`initial_soc`/`soc_target`), connection windows (declared
  `connection_windows_utc` expressed on the test horizon). **ASSUMPTION-
  labelled fixture values:** battery capacities 0.06 / 0.08 MWh (no capacity
  in the scenario), EVSE phase "three-phase" (no phase declared),
  transformer limits (none declared), minimum charging power 0.0 (no
  minimum declared — an explicit fixture value, not a silent default).

## 5. Registry survey (ledger P3.10/P3.11 step 1 inputs): what Stage 2 holds, and the gaps

Extends the §3 survey of `05_08_bess_pq_models.md` (same registry: `entities.rs`
+ the frozen capability schema). Load/EV-relevant gaps, recorded as unknowns,
never filled with invented values:

| # | Gap | Effect in the load / EV model |
|---|---|---|
| G11 | No baseline series in the registry — the scenario declares a baseline shape but no numeric per-interval values; no DR baseline method exists (D1/D2 open). | The baseline is required per-interval configuration from the scenario fixture, under the one decided synthetic method (D-P3.10-1). |
| G12 | No increasable-range, response-time, duration/recovery, rebound, or availability data anywhere in the registry or scenario. | Required model configuration; test fixtures label each value ASSUMPTION. Rebound is never defaulted (D-P3.10-3). |
| G13 | No comfort/process constraint data. | Declared descriptors only; undeclared → recorded unknown dimension (D-P3.10-4). |
| G14 | No vehicle-level data (battery capacity, per-vehicle SOC targets, connection windows beyond the aggregate EVSE profile, priorities, opt-outs). The scenario models ONE aggregate EV per EVSE. | Vehicle-level fields are required configuration; fixtures label the invented values ASSUMPTION; the aggregate-EV-per-EVSE scenario assumption is preserved in the single-vehicle fixtures. |
| G15 | No charging-efficiency value for EVSEs. | The lossless balance is a recorded convention and an unknown dimension — no value invented (D-P3.11-4). |
| G16 | No V2X capability, interconnection-agreement, or market-rules evidence. | V2X disabled by default; the only enabled case is the named five-condition fixture (D-P3.11-1). |
| G17 | No site transformer limit. | Required configuration; fixture values labelled ASSUMPTION. |
| G18 | No volt-var/volt-watt curve data (extends 05_08 gap G10). | The P3.09 scoping table (§2): every profile NOT-CONFIGURED, UNKNOWN provenance, zero volt-var credit. |

## 6. Review status and verification

- **Power-systems review sign-off: PENDING** (same status as the P3.01–P3.08
  records). A human power-systems review must confirm the decisions in §1 —
  in particular D-P3.10-2 (EXCLUDE-AND-FLAG), D-P3.10-3 (the linear rebound
  proration convention and the net-flexibility formula), and D-P3.11-3/4
  (opt-out and balance gates) — and the §2 scoping table, before the ledger
  P3.09–P3.11 checkboxes are ticked.
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for these
  files. The inline tests (`volt_var_scoping.rs`) and the two integration
  suites are written to compile against the recorded APIs; the first
  `cargo test -p flexcore-domain` run must confirm, and any fix must keep
  the decisions in §1 unchanged.
- **No frozen-schema change:** the models consume the frozen contract types
  (`Provenance`, `ProvenancedValue` from the BESS module, the P3.02
  narrowing functions) and take everything else as domain-side
  configuration. No PROPOSED migration note is required this batch.
- **lib.rs doc tables:** the module tables in `lib.rs` were left untouched
  on purpose (ownership limited this task to additive module declarations);
  the three new modules should be added to that table in a later docs pass.
- **PLAN.md pointers:** the ledger asks for the P3.09 scoping documentation
  in PLAN.md; it is recorded HERE (§2) because PLAN.md was outside this
  task's file ownership. A later docs pass should add the pointer.
- **Terminology:** a violation-free evaluation makes a schedule a CANDIDATE
  with respect to asset constraints — feasibility, authorization, and
  dispatch are Phase 4 gates and are never implied by these modules
  (simulation-only 0.x; no field-write path; FlexCore performs no inverter
  inner-loop control and no protection-grade functionality).
