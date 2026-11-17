# P17.11 — Depot / site load management

| Field | Value |
|---|---|
| Subtask | P17.11 — Site import/transformer limits, simultaneity-factor control across chargers, the P14.15 DOE envelope interplay, and external-constraint profile composition [S-141][S-140] |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the depot load-management model and its tests are written, never compiled, never executed. No depot site is named, no transformer limit was measured, no DOE was received from any operator, no charging-profile payload was ever produced. |
| Sources | [S-141] Ampcontrol OCPP smart charging; [S-140] DOE FEMP smart charge management. **The simultaneity-factor semantics and the shaped-limit approach are SECONDARY-SOURCED and UNVERIFIED against the primary sources** — §4 registers the checks. The DOE semantics are the P14.15 record's (UNVERIFIED there; reused verbatim, never redefined). |
| A-dependence | The P17.05 composer (Agent A) consumes the external-constraint profile BY CITATION: the profile maps onto the `ChargingStationExternalConstraints` purpose (evseId 0 — purpose name SECONDARY-SOURCED [S-124], UNVERIFIED) only through the frozen lifecycle gates. No composer file was touched. |
| Files touched | `crates/flexcore-domain/src/ev_depot_load.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive registration only). |

## 1. What landed (deliverable class: depot load-management model)

| Piece | Where | Honest status |
|---|---|---|
| `DepotSiteConfig` — the site transformer limit (positive MW), the declared non-EV site load (non-negative MW), the declared simultaneity factor ([0,1]; NO default), and the charger list (unique ids, positive ratings); validated at construction and at every use | `ev_depot_load.rs` | Implemented; UNRUN |
| `effective_site_import_limit_mw` — the SITE import cap: the MINIMUM of the transformer limit and the effective DOE import bound; the P14.15 `evaluate_envelope` lapse semantics reused verbatim (not-yet-in-effect / expired / stale → the declared static connection limit governs); a DOE declared WITHOUT its static fallback is REFUSED (never silently unconstrained — the P14.15 mandatory-fallback rule) | same | Implemented; UNRUN |
| `shape_charger_limits` — the simultaneity-factor control: if `factor × Σ(connected ratings)` fits under the headroom (effective limit − non-EV load), chargers keep full ratings; otherwise the headroom is distributed proportionally to ratings (deterministic, capped at each rating); headroom ≤ 0 is refused (no feasible charging — never negative limits); an empty connected set is an idle depot, not an error; an unknown connected charger id is refused | same | Implemented; UNRUN |
| `ExternalConstraintProfile` — the per-interval composition: the effective site import cap evaluated at every interval start over the declared grid (BTreeMap — deterministic ascending order); an unevaluable instant is a refusal, never a silent gap | same | Implemented; UNRUN |
| `DEPOT_LOAD_MANAGEMENT_STATEMENT` — the recorded statement: the profile is CONSTRAINT DATA for the composer; nothing here produces a `SetChargingProfile` payload — that is the composer's job, downstream of the frozen 14-state lifecycle gates | same | Implemented; UNRUN |

## 2. The P14.15 DOE interplay (consumed, never re-encoded)

The DOE types, lapse reasons, mandatory static fallback, and the
`effective_envelope_bounds_mw` sign rule (p_min = −import limit, the
frozen P > 0 = export convention) are the P14.15 module's
(`doe_envelope.rs`); this module reuses them and adds only the depot-side
MIN-composition with the transformer limit. The lapse behavior (the
static limit governing on not-yet-in-effect / expired / stale) is tested
against the P14.15 semantics — including the refusal when a DOE is
declared without a fallback.

## 3. What this module is NOT

1. **Not a dispatch path** — the profile and the shaped limits are
   constraint data; the composer maps them onto the
   `ChargingStationExternalConstraints` purpose [S-124, UNVERIFIED
   spelling] only through the lifecycle gates.
2. **Not a transformer model** — the transformer limit is DECLARED site
   configuration with no measurement of its own; a measured limit is the
   composition root's declared provenance, never inferred here.
3. **Not a load forecaster** — the non-EV load is declared; forecasting
   it is Phase 15's surface (untouched).

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The simultaneity-factor definition in the reference deployments (fraction of connected chargers at full rate; per-charger vs per-site application) [S-141][S-140] | `DepotSiteConfig::simultaneity_factor`, `shape_charger_limits` |
| 2 | The proportional-distribution choice when the coincidence assumption fails (this module's recorded algorithm; the references' exact shaping method is UNVERIFIED) [S-141] | `shape_charger_limits` step 3 |
| 3 | Whether the `ChargingStationExternalConstraints` purpose accepts site-level (evseId 0) profiles with per-interval caps exactly as composed here [S-124] | the composer's mapping (Agent A, by citation) |
| 4 | The FEMP framing of depot load management (whether departure constraints and site caps compose as this module records) [S-140] | the module header's composition order |

## 5. Tests (written-unrun; execution is a human gate)

Config validation (zero/negative transformer limit, negative non-EV load,
factor out of range including NaN, zero-rating charger, duplicate
charger id), the no-DOE effective limit, the DOE-in-effect MIN-composition,
the lapsed-DOE static-fallback case, the DOE-without-fallback refusal
(never unconstrained), the coincidence-assumption-holds full ratings, the
failing-assumption proportional distribution (with the shaped-sum ≤
headroom assertion), the empty connected set, the unknown charger
refusal, the no-feasible-headroom refusal, the deterministic profile
composition (ascending interval order, exact caps, the cap lookup), and
the non-positive interval-grid refusal.
