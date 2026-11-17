# P17.09 — Departure-time constraint model

| Field | Value |
|---|---|
| Subtask | P17.09 — Vehicle calendar/telematics ingestion → per-vehicle energy-by-time constraints feeding the composer (P17.05, Agent A's — consumed by citation); maps onto the domain model's HARD departure-energy policy (required departure SOC by the departure instant — never relaxed by ingestion) [S-140] |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the constraint model and its tests are written, never compiled, never executed. No vehicle calendar was ingested, no telematics value was observed, no fleet operator is named, no composer input was ever produced. |
| Sources | [S-140] DOE FEMP smart charge management. The FEMP reference motivates the per-vehicle "energy required by departure time" framing; the HARD-policy semantics come from the DOMAIN model (P3.11), which governs — no external claim is made. |
| A-dependence | The P17.05 composer is Agent A's. This module produces the constraint SET the composer consumes; the consumption is BY CITATION (the composer consumes `DepartureConstraintSet::operative()` — deterministic vehicle-key order). No composer file was touched. |
| Files touched | `crates/flexcore-domain/src/ev_departure_constraints.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive registration only). |

## 1. What landed (deliverable class: constraint model)

| Piece | Where | Honest status |
|---|---|---|
| `DepartureConstraint` — one inbound per-vehicle constraint (vehicle key, required departure SOC in [0,1], constraint departure instant UTC ms, declared source VehicleCalendar / TelematicsPort / OperatorDeclared, provenance, received-at) | `ev_departure_constraints.rs` | Implemented; UNRUN |
| `map_constraint_onto_fleet_config` — the ONE place the never-relaxed rule lives: SOC operative = max(configured floor, constraint); departure operative = min(configured departure, constraint); a SOC relaxation or a later departure instant is REFUSED with the named `RelaxationRefused` error — the raw inbound value is recorded as data but never becomes operative | same | Implemented; UNRUN |
| `MappedDepartureConstraint` — the operative constraint (the composer's input); constructible only as the mapping output, so a raw inbound record cannot bypass the direction check; `required_energy_mwh` derives the energy-by-time requirement from the fleet configuration's battery capacity | same | Implemented; UNRUN |
| `DepartureConstraintSet` — BTreeMap-backed deterministic set, one operative constraint per vehicle; duplicates refused; the explicit `supersede` path retains the superseded record for audit and requires strictly newer received-at (a supersession that does not advance time is refused) | same | Implemented; UNRUN |
| `DEPARTURE_CONSTRAINT_GOVERNING_STATEMENT` — the recorded statement: ingestion may TIGHTEN and may never RELAX; any relaxation remains a named, reviewed decision recorded in the schedule (P3.19) — never an ingestion outcome | same | Implemented; UNRUN |

## 2. The HARD policy is consumed, never re-encoded

The departure-energy policy itself is the domain model's
(`crates/flexcore-domain/src/ev_fleet_model.rs`,
`DepartureEnergyPolicy::Hard`, decided once in P3.11). This module adds
no variant, no relaxation knob, and no second policy vocabulary; the
test `the_hard_policy_declaration_is_untouched_by_the_mapping` pins that
`EvFleetModel::departure_policy()` still returns `Hard` after the mapping
exists. The mapping's only authority is directional: it can RAISE the
required SOC and PULL the departure instant earlier — nothing else.

## 3. What this module is NOT

1. **Not a scheduler** — it produces constraints; the optimization and
   the composer are elsewhere (P3.17 problem IR; P17.05 composer).
2. **Not a relaxation path** — `RelaxationRefused` is a refusal, not a
   soft-penalty; the P3.19 relaxation ledger is untouched.
3. **Not a calendar parser** — the constraint arrives as declared,
   validated data; no calendar format is parsed here.

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | Whether fleet calendars in practice carry an explicit required-departure-SOC field or only a departure time (this module requires the SOC; an SOC-less calendar is an upstream mapping gap, not a default) [S-140] | `DepartureConstraint::new` (the SOC is mandatory) |
| 2 | The FEMP SMQ framing of "departure" (whether vehicle-side schedules or site-side windows dominate) [S-140] | the `DepartureConstraintSource` vocabulary |

## 5. Tests (written-unrun; execution is a human gate)

The governing statement, the tightening mapping (SOC raised and departure
pulled earlier), the matching no-op, the SOC-relaxation refusal, the
later-departure refusal, the HARD-policy-untouched pin, the unknown-
vehicle refusal, the key/SOC construction validation, the deterministic
BTreeMap set with duplicate refusal, the supersession audit retention
with the not-newer refusal, and the invalid-capacity refusal.
