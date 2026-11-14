// P5.21/P5.22 — The scenario-level fault-injection layer and the run-record
//         fault journal (report §19.2 layer 6, §23 Stage 5; ledger
//         `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`
//         P5.21 + P5.22).
//
// WHAT THIS MODULE IS (and is not) — the reuse discipline, stated first:
//
// - The PLATFORM fault-injection suites are P4.31's
//   (`crates/flexcore-application/tests/fault_injection.rs`, decision record
//   `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md` §2). They
//   stay the authority for platform behaviour at the adapter/outbox/runner
//   seams. This module does NOT duplicate them and never re-tests platform
//   behaviour — it adds the §19.2 layer 6 SIMULATOR-RUNNER injection
//   controls: per-scenario fault configuration, composition-level probes,
//   and the per-run fault journal whose entries the console (P5.22,
//   SimulationLab) renders.
// - The SOURCE-LEVEL faults are the P2.15 fault plan — scenario
//   configuration, already validated and applied by the Phase 2 simulator
//   (`crates/flexcore-simulator/src/scenario.rs`) and mapped to platform
//   outcomes by the D-P4.16-4 transport mapping in `scenario_run.rs`. This
//   module MAPS those injections onto the §19.2 layer 6 vocabulary and
//   pre-registers each fault's expected outcome by REUSING two recorded
//   tables: the P4.31 fault matrix (§2 of `31_fault_shadow.md`) and the
//   P2.25 expected-quality plan (the scenario's own pre-registered stored
//   outcome per fault).
//
// ## The scenario fault slot (v3) — the ScenarioRecord alignment note
//
// The wave-4a ScenarioRecord v3 (`replay_record.rs`) landed MID-WAVE: the
// authoring started before it existed (no `ScenarioRecord` type,
// `scenarios/library/` absent, HANDOVER wave 4a "pending"), so the
// fault-slot JSON shape was DEFINED HERE and in the decision record
// (`21_24_faults_visibility.md`) for alignment and implemented
// DEFENSIVELY. When the record schema landed, it was extended ADDITIVELY
// per the P5.21 task order: `ScenarioRecordV3.faults`
// (`Option<ScenarioFaultsSlot>`, `#[serde(default)]`, drift-noted on the
// field — see the field's doc comment for the honest consumption status).
// The OPERATIVE surfaces this module reads are: (1) the telemetry
// scenario JSON's optional top-level `scenario_faults` slot, and (2) the
// telemetry scenario's own P2.15 `faults` plan (the derivation fallback):
//
// ```json
// "scenario_faults": {
//   "schema": "flexcore-scenario-faults/v3",
//   "enabled": true,
//   "note": "free text, optional",
//   "injections": [
//     {
//       "fault_id":   "SF-<short-tag>-<n>",
//       "kind":       "<one ScenarioFaultKind::name() value>",
//       "leg":        "telemetry_source | dispatch_transport | composition_clock | grid_feasibility | fleet_probe",
//       "asset_key":  "<optional; validated against the portfolio>",
//       "from_ms":    0,
//       "until_ms":   0,
//       "note":       "free text, optional"
//     }
//   ]
// }
// ```
//
// Defensive rules: an unknown `schema` string is a NAMED error (never
// silently accepted); an unknown `kind` is a NAMED error; an `asset_key`
// not in the scenario portfolio is a NAMED error; unknown extra fields are
// ignored (forward-compat) and counted as a drift note on the plan. When
// the slot is ABSENT, the plan derives from the P2.15 `faults` plan (the
// scenario configuration that already exists). When `faults.enabled` is
// false and no slot exists, the plan is INERT — no fault is invented.
//
// ## The journal rule (P5.22, "never a silent blank screen")
//
// Every journal entry's `observed_effect` is quoted FROM THIS RUN's record
// facts (availability changes, quality counts, command outcomes, grid
// checks) or from an explicit probe outcome the caller supplies. A fault
// whose effect this run did not observe records the platform response
// [`RESPONSE_NOT_OBSERVED`] — never "passed", never a blank, never a
// fabricated effect. Everything this module produces is synthetic,
// simulation-only (the entire 0.x line, §21.4).

use serde::Serialize;
use serde_json::Value;

use flexcore_domain::ids::{CommandId, ScheduleId};
use flexcore_simulator::{FaultKind, Scenario, load_scenario};

use crate::scenario_run::{
    CommandRunConfig, ScenarioRunError, ScenarioRunHarness, ScenarioRunRecord, deterministic_uuid,
};

/// The scenario fault-slot schema tag. A slot carrying a different schema
/// string is a named error — never silently interpreted.
pub const SCENARIO_FAULTS_SLOT_SCHEMA: &str = "flexcore-scenario-faults/v3";

/// The journal's source-of-truth citation, carried on the plan and in every
/// decision-doc table: the P4.31 pre-registered fault matrix is REUSED, not
/// duplicated.
pub const P4_31_FAULT_TABLE_SOURCE: &str =
    "ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md §2 (the P4.31 \
     pre-registered fault matrix — REUSED, not duplicated; the platform \
     behaviour tests stay in Phase 4)";

/// The statement every fault journal carries (the P5.22 display contract
/// renders it verbatim).
pub const FAULT_JOURNAL_STATEMENT: &str = "\
    The fault journal records INJECTED SIMULATOR faults only: every entry is \
    scenario configuration applied by the sim-runner composition or the \
    scenario's own P2.15 fault plan — never a real device or network event. \
    A fault whose effect this run did not observe is recorded \
    'not_observed_in_this_run', never 'passed'. The platform-behaviour \
    expectations reuse the P4.31 pre-registered fault matrix (Phase 4), not \
    a second table.";

/// The journal platform-response value for a fault this run did not observe
/// (the never-silent-pass rule: an unobserved fault is NAMED, never "passed").
pub const RESPONSE_NOT_OBSERVED: &str = "not_observed_in_this_run";

/// The journal platform-response value for a fault cross-referenced to the
/// P4.31 Phase 4 suites instead of injected here (the no-duplication rule).
pub const RESPONSE_CROSS_REFERENCED: &str = "cross_referenced_to_phase_4_suite";

// --- Fault kinds ------------------------------------------------------------------

/// The §19.2 layer 6 fault vocabulary, extended with the P2.15 source-level
/// faults the scenario configuration already carries (so one vocabulary
/// covers both; the mapping table is in the decision record
/// `21_24_faults_visibility.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioFaultKind {
    /// §19.2 layer 6 "dropped messages" (declared via the v3 slot; the
    /// P2.15 `drop` window realizes the same effect — see the mapping).
    DroppedMessage,
    /// §19.2 layer 6 "reordered messages" (P2.15 `out_of_order`).
    ReorderedMessage,
    /// §19.2 layer 6 "duplicated messages" (P2.15 `duplicate`).
    DuplicatedMessage,
    /// P2.15 `conflicting_duplicate` (the same-identity/different-value
    /// duplicate — a distinct pre-registered stored outcome, P2.25).
    ConflictingDuplicate,
    /// P2.15 `source_invalid` (BAD source code; NACK at issue per
    /// D-P4.16-4).
    InvalidSource,
    /// P2.15 `out_of_range` (SUSPECT stored; out-of-tolerance tracking
    /// sample per D-P4.16-4).
    OutOfRange,
    /// §19.2 layer 6 "clock skew" (P4.31 F4/F5).
    ClockSkew,
    /// §19.2 layer 6 "stale telemetry" (P2.15 `delay`; P4.31 F6).
    StaleTelemetry,
    /// §19.2 layer 6 "unavailable assets" (the P2.15 `drop` window's
    /// primary P5.14 semantics: no frames ⇒ unavailable ⇒ the P3.24 roll).
    UnavailableAsset,
    /// §19.2 layer 6 "solver timeout" (P4.31 F9a/F9b; cross-referenced at
    /// the scenario level — the platform seam tests stay in Phase 4).
    SolverTimeout,
    /// §19.2 layer 6 "plugin crash" — SIMULATED, no kernel exists (v0.5.0);
    /// P4.31 F10; cross-referenced at the scenario level.
    PluginCrash,
    /// §19.2 layer 6 "partial fleet response" (P4.31 F11 shape; the
    /// composition-level fleet probe).
    PartialFleetResponse,
}

impl ScenarioFaultKind {
    /// All twelve kinds (the closed vocabulary; the decision record's
    /// table enumerates exactly these).
    pub const ALL: [ScenarioFaultKind; 12] = [
        ScenarioFaultKind::DroppedMessage,
        ScenarioFaultKind::ReorderedMessage,
        ScenarioFaultKind::DuplicatedMessage,
        ScenarioFaultKind::ConflictingDuplicate,
        ScenarioFaultKind::InvalidSource,
        ScenarioFaultKind::OutOfRange,
        ScenarioFaultKind::ClockSkew,
        ScenarioFaultKind::StaleTelemetry,
        ScenarioFaultKind::UnavailableAsset,
        ScenarioFaultKind::SolverTimeout,
        ScenarioFaultKind::PluginCrash,
        ScenarioFaultKind::PartialFleetResponse,
    ];

    /// The canonical snake_case name (the wire/display spelling; the same
    /// string the v3 slot's `kind` field carries).
    pub const fn name(self) -> &'static str {
        match self {
            ScenarioFaultKind::DroppedMessage => "dropped_message",
            ScenarioFaultKind::ReorderedMessage => "reordered_message",
            ScenarioFaultKind::DuplicatedMessage => "duplicated_message",
            ScenarioFaultKind::ConflictingDuplicate => "conflicting_duplicate",
            ScenarioFaultKind::InvalidSource => "invalid_source",
            ScenarioFaultKind::OutOfRange => "out_of_range",
            ScenarioFaultKind::ClockSkew => "clock_skew",
            ScenarioFaultKind::StaleTelemetry => "stale_telemetry",
            ScenarioFaultKind::UnavailableAsset => "unavailable_asset",
            ScenarioFaultKind::SolverTimeout => "solver_timeout",
            ScenarioFaultKind::PluginCrash => "plugin_crash",
            ScenarioFaultKind::PartialFleetResponse => "partial_fleet_response",
        }
    }

    /// Parses a slot `kind` string (exact match; anything else is a named
    /// error at the slot validator).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.name() == name)
    }

    /// The pre-registered expected platform response for the kind — the
    /// EXTENSION row of the P4.31 table (scenario-level specifics; the
    /// P4.31 reference travels with the plan spec, not here).
    pub const fn expected_response(self) -> &'static str {
        match self {
            // D-P4.16-4: a delivery inside a drop window has no acceptance —
            // the transport-timeout mapping; the command FAILED (P4.31 F1's
            // "no loss, never silently dropped" is the OUTBOX row; the
            // scenario-level delivery outcome is the timeout).
            ScenarioFaultKind::DroppedMessage => "failed",
            // P2.25 expected_quality(out_of_order) = GOOD, "the held frame is
            // stored late; arrival order never changes the stored quality"
            // (P2.09); P4.31 F2 is the receive-point analog.
            ScenarioFaultKind::ReorderedMessage => "hold",
            // P2.25 expected_quality(duplicate) = GOOD, "the exact duplicate
            // is acknowledged (P2.09); exactly one stored row per tick";
            // P4.31 F1b is the outbox-key analog.
            ScenarioFaultKind::DuplicatedMessage => "hold",
            // P2.25 expected_quality(conflicting_duplicate) = GOOD, "the
            // conflicting copy is quarantined with a reason (P2.09); the
            // original row stands".
            ScenarioFaultKind::ConflictingDuplicate => "hold",
            // D-P4.16-4: source_invalid ⇒ NACK at issue; stored BAD (P2.25).
            ScenarioFaultKind::InvalidSource => "reject",
            // D-P4.16-4: an out-of-tolerance tracking sample drives the
            // tracking verdict (deviation ⇒ FAILED); stored SUSPECT (P2.25).
            ScenarioFaultKind::OutOfRange => "failed",
            // P4.31 F4/F5: the issue is REFUSED / the sweep EXPIRES — expiry
            // wins, never a silent completion.
            ScenarioFaultKind::ClockSkew => "reject",
            // P4.31 F6: a STALE ramp baseline fails the feasibility gate
            // CLOSED with the trigger named.
            ScenarioFaultKind::StaleTelemetry => "reject",
            // P4.31 F8 shape at the scenario level: the asset dropout is
            // visible and the P3.24 rolling roll (the platform's fallback
            // response) supersedes the plan.
            ScenarioFaultKind::UnavailableAsset => "fallback",
            // P4.31 F9a: a solver timeout HOLDS the command with no
            // transition and no event (F9b: the reject-branch variant).
            ScenarioFaultKind::SolverTimeout => "hold",
            // P4.31 F10: the panic propagates fail-closed — no partial
            // transition, nothing issued; the command HELD.
            ScenarioFaultKind::PluginCrash => "hold",
            // P4.31 F11: per-asset acknowledgement, never portfolio-inferred.
            ScenarioFaultKind::PartialFleetResponse => "per_asset",
        }
    }

    /// The P4.31 (and, for source faults, P2.15/D-P4.16-4) reference the
    /// kind's expectation reuses. The FULL citation context is
    /// [`P4_31_FAULT_TABLE_SOURCE`].
    pub const fn p4_31_reference(self) -> &'static str {
        match self {
            ScenarioFaultKind::DroppedMessage => "P4.31 F1 (outbox drop; scenario delivery outcome via D-P4.16-4)",
            ScenarioFaultKind::ReorderedMessage => "P4.31 F2 (receive-point refusal) + P2.15 out_of_order + P2.25 expected_quality",
            ScenarioFaultKind::DuplicatedMessage => "P4.31 F1b (duplicate key refusal) + P2.15 duplicate + P2.25 expected_quality",
            ScenarioFaultKind::ConflictingDuplicate => "P4.31 F1b + P2.15 conflicting_duplicate + P2.25 expected_quality",
            ScenarioFaultKind::InvalidSource => "P4.31 §2 (no F-row; D-P4.16-4 source_invalid → NACK) + P2.25 expected_quality",
            ScenarioFaultKind::OutOfRange => "P4.31 §2 (no F-row; D-P4.16-4 out_of_range → out-of-tolerance sample) + P2.25 expected_quality",
            ScenarioFaultKind::ClockSkew => "P4.31 F4 (refused at issue) + F5 (expiry wins over completion)",
            ScenarioFaultKind::StaleTelemetry => "P4.31 F6 (fail-closed with the trigger named) + P2.15 delay",
            ScenarioFaultKind::UnavailableAsset => "P4.31 F8 (unavailable asset ⇒ FAILED + fallback) + P2.15 drop + P3.24 roll",
            ScenarioFaultKind::SolverTimeout => "P4.31 F9a (hold, no transition) + F9b (reject-branch variant)",
            ScenarioFaultKind::PluginCrash => "P4.31 F10 (SIMULATED — no plugin kernel exists; panic propagates)",
            ScenarioFaultKind::PartialFleetResponse => "P4.31 F11 (per-asset, never portfolio-inferred)",
        }
    }
}

// --- The plan ---------------------------------------------------------------------

/// Where a plan spec came from (the journal renders the origin so an
/// operator can tell slot-declared from derived from cross-referenced).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultSpecOrigin {
    /// Declared in the scenario's `scenario_faults` v3 slot.
    Slot,
    /// Derived 1:1 from the scenario's validated P2.15 fault plan.
    DerivedFromP2FaultPlan,
    /// A §19.2 layer 6 fault with NO scenario-level injection surface in
    /// this composition — cross-referenced to the P4.31 Phase 4 suite
    /// instead of duplicated (the P5.21 ledger step 4 rule).
    CrossReference,
}

/// One scenario-level fault spec (the validated plan row).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioFaultSpec {
    /// The stable fault id (slot-declared verbatim; derived ids are
    /// `p2-<kind>-<asset>-<from_ms>` — deterministic, never random).
    pub fault_id: String,
    /// The fault kind.
    pub kind: ScenarioFaultKind,
    /// The composition leg the injection applies at (`telemetry_source`,
    /// `dispatch_transport`, `composition_clock`, `grid_feasibility`,
    /// `fleet_probe`).
    pub injection_leg: String,
    /// The target asset key, when the fault targets one asset.
    pub target_asset_key: Option<String>,
    /// The injection window start (scenario clock, ms; `i64::MIN` for
    /// always).
    pub from_ms: i64,
    /// The injection window end, exclusive (scenario clock, ms; `i64::MAX`
    /// for always).
    pub until_ms: i64,
    /// The pre-registered expected response (`reject`/`hold`/`fallback`/
    /// `failed`/`per_asset`).
    pub expected_response: String,
    /// The pre-registered expected outcome detail (what "matching" means
    /// for this fault at the scenario level — written BEFORE any run).
    pub expected_detail: String,
    /// The reused expectation's reference (P4.31 row + P2.15/P2.25 where
    /// applicable).
    pub pre_registered_reference: String,
    /// Where the spec came from.
    pub origin: FaultSpecOrigin,
}

/// The validated scenario fault plan: the runner's injection configuration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioFaultPlan {
    /// The slot schema in effect (`flexcore-scenario-faults/v3`), or the
    /// derivation tag when the plan came from the P2.15 fault plan.
    pub schema: String,
    /// True when at least one fault is active for this run (a disabled
    /// P2.15 plan with no slot renders an INERT plan — no fault invented).
    pub enabled: bool,
    /// The validated specs (empty when inert).
    pub specs: Vec<ScenarioFaultSpec>,
    /// The drift notes (forward-compat fields ignored, mapping decisions)
    /// — recorded, never swallowed.
    pub drift_notes: Vec<String>,
    /// The statement the journal and the decision record render.
    pub statement: String,
}

impl ScenarioFaultPlan {
    /// Builds the plan from a scenario artefact: the `scenario_faults` v3
    /// slot when present (defensively validated), else the 1:1 derivation
    /// from the validated P2.15 fault plan. All-or-nothing: any slot
    /// validation error rejects the plan (never a partial fault set).
    pub fn from_scenario_json(scenario_json: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(scenario_json)
            .map_err(|error| format!("the scenario is not valid JSON: {error}"))?;
        let scenario = load_scenario(scenario_json)
            .map_err(|error| error.to_string())?;

        let portfolio_keys: Vec<String> = scenario
            .sites
            .iter()
            .flat_map(|site| site.assets.iter().map(|asset| asset.asset_key.clone()))
            .collect();

        let mut drift_notes: Vec<String> = Vec::new();

        if let Some(slot) = value.get("scenario_faults") {
            return Self::from_slot(slot, &portfolio_keys, &mut drift_notes);
        }

        // No slot: derive from the validated P2.15 fault plan.
        let mut specs = Vec::new();
        if scenario.faults.enabled {
            for injection in &scenario.faults.injections {
                specs.push(derived_spec(injection));
            }
        }
        let enabled = !specs.is_empty();
        Ok(Self {
            schema: if enabled {
                "derived:flexcore-scenario-fault-plan-p2.15".to_string()
            } else {
                "inert".to_string()
            },
            enabled,
            specs,
            drift_notes,
            statement: if enabled {
                "the fault plan is DERIVED 1:1 from the scenario's validated P2.15 \
                 fault configuration (no scenario_faults v3 slot present); the \
                 expected outcomes reuse the P2.25 expected-quality plan and the \
                 P4.31 fault matrix"
                    .to_string()
            } else {
                "no fault is configured for this run (the P2.15 fault plan ships \
                 disabled and no scenario_faults v3 slot exists) — the plan is \
                 inert and no fault is injected or invented"
                    .to_string()
            },
        })
    }

    /// The slot validator (defensive: unknown schema/kind/asset are named
    /// errors; unknown extra fields become drift notes).
    fn from_slot(
        slot: &Value,
        portfolio_keys: &[String],
        drift_notes: &mut Vec<String>,
    ) -> Result<Self, String> {
        let schema = slot
            .get("schema")
            .and_then(Value::as_str)
            .ok_or_else(|| "the scenario_faults slot carries no 'schema' string".to_string())?;
        if schema != SCENARIO_FAULTS_SLOT_SCHEMA {
            return Err(format!(
                "the scenario_faults slot schema '{schema}' is not '{}' — refusing \
                 to interpret an unknown fault-slot schema (fail-closed, never a \
                 silent guess)",
                SCENARIO_FAULTS_SLOT_SCHEMA
            ));
        }
        let enabled = slot
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or_else(|| "the scenario_faults slot carries no 'enabled' boolean".to_string())?;
        if let Some(note) = slot.get("note") {
            drift_notes.push(format!("slot note carried verbatim: {note}"));
        }
        let injections = slot
            .get("injections")
            .and_then(Value::as_array)
            .ok_or_else(|| "the scenario_faults slot carries no 'injections' array".to_string())?;
        let mut specs = Vec::new();
        for (index, injection) in injections.iter().enumerate() {
            // Unknown extra fields are forward-compat: counted, never silent.
            let known = ["fault_id", "kind", "leg", "asset_key", "from_ms", "until_ms", "note"];
            for key in injection.as_object().map(|o| o.keys()).into_iter().flatten() {
                if !known.contains(&key.as_str()) {
                    drift_notes.push(format!(
                        "injection {index}: unknown slot field '{key}' ignored (forward-compat)"
                    ));
                }
            }
            let fault_id = injection
                .get("fault_id")
                .and_then(Value::as_str)
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| {
                    format!("injection {index}: the slot injection carries no non-empty 'fault_id'")
                })?
                .to_string();
            let kind_name = injection
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("injection '{fault_id}': no 'kind' string"))?;
            let kind = ScenarioFaultKind::parse(kind_name).ok_or_else(|| {
                format!(
                    "injection '{fault_id}': unknown fault kind '{kind_name}' — the v3 \
                     vocabulary is closed; refusing to guess"
                )
            })?;
            let injection_leg = injection
                .get("leg")
                .and_then(Value::as_str)
                .unwrap_or_else(|| default_leg(kind))
                .to_string();
            let target_asset_key = match injection.get("asset_key") {
                None | Some(Value::Null) => None,
                Some(Value::String(key)) => {
                    if !portfolio_keys.contains(key) {
                        return Err(format!(
                            "injection '{fault_id}': asset key '{key}' is not a scenario \
                             portfolio asset (the loader validates injection keys against \
                             the portfolio, P2.15 discipline)"
                        ));
                    }
                    Some(key.clone())
                }
                Some(other) => {
                    return Err(format!(
                        "injection '{fault_id}': 'asset_key' must be a string or null, got {other}"
                    ))
                }
            };
            let from_ms = injection
                .get("from_ms")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("injection '{fault_id}': no integer 'from_ms'"))?;
            let until_ms = injection
                .get("until_ms")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("injection '{fault_id}': no integer 'until_ms'"))?;
            if until_ms < from_ms {
                return Err(format!(
                    "injection '{fault_id}': the window until_ms ({until_ms}) precedes \
                     from_ms ({from_ms})"
                ));
            }
            specs.push(ScenarioFaultSpec {
                fault_id,
                kind,
                injection_leg,
                target_asset_key,
                from_ms,
                until_ms,
                expected_response: kind.expected_response().to_string(),
                expected_detail: expected_detail(kind, from_ms, until_ms),
                pre_registered_reference: kind.p4_31_reference().to_string(),
                origin: FaultSpecOrigin::Slot,
            });
        }
        Ok(Self {
            schema: SCENARIO_FAULTS_SLOT_SCHEMA.to_string(),
            enabled: enabled && !specs.is_empty(),
            specs,
            drift_notes: drift_notes.clone(),
            statement: "the fault plan is the scenario's scenario_faults v3 slot \
                 (flexcore-scenario-faults/v3), validated fail-closed; the expected \
                 outcomes reuse the P4.31 fault matrix extended with the scenario-level \
                 specifics (decision record 21_24_faults_visibility.md §2)"
                .to_string(),
        })
    }

    /// The clock-skew specs (the composition probe's input; empty when the
    /// plan carries none).
    pub fn clock_skew_specs(&self) -> Vec<&ScenarioFaultSpec> {
        self.specs
            .iter()
            .filter(|spec| spec.kind == ScenarioFaultKind::ClockSkew)
            .collect()
    }

    /// The partial-fleet specs (the fleet probe's input; empty when the
    /// plan carries none).
    pub fn partial_fleet_specs(&self) -> Vec<&ScenarioFaultSpec> {
        self.specs
            .iter()
            .filter(|spec| spec.kind == ScenarioFaultKind::PartialFleetResponse)
            .collect()
    }

    /// The cross-reference-only kinds (solver timeout, plugin crash): the
    /// §19.2 layer 6 faults with no scenario-level injection surface in
    /// this composition. They ALWAYS appear in the journal — as
    /// cross-references with no fabricated outcome (the P5.21 ledger
    /// step-4 rule: platform behaviour tests stay in Phase 4).
    pub fn cross_reference_specs(&self) -> Vec<ScenarioFaultSpec> {
        self.specs
            .iter()
            .filter(|spec| {
                matches!(
                    spec.kind,
                    ScenarioFaultKind::SolverTimeout | ScenarioFaultKind::PluginCrash
                )
            })
            .cloned()
            .collect()
    }
}

/// The default composition leg per kind (used when a slot injection omits
/// `leg`; the decision record's table documents these).
fn default_leg(kind: ScenarioFaultKind) -> &'static str {
    match kind {
        ScenarioFaultKind::DroppedMessage => "dispatch_transport",
        ScenarioFaultKind::ReorderedMessage
        | ScenarioFaultKind::DuplicatedMessage
        | ScenarioFaultKind::ConflictingDuplicate
        | ScenarioFaultKind::InvalidSource
        | ScenarioFaultKind::OutOfRange
        | ScenarioFaultKind::StaleTelemetry
        | ScenarioFaultKind::UnavailableAsset => "telemetry_source",
        ScenarioFaultKind::ClockSkew => "composition_clock",
        ScenarioFaultKind::SolverTimeout | ScenarioFaultKind::PluginCrash => "grid_feasibility",
        ScenarioFaultKind::PartialFleetResponse => "fleet_probe",
    }
}

/// The pre-registered expected-outcome detail per kind (the scenario-level
/// extension of the P4.31 table; written BEFORE any run exists — this is
/// the pre-registration, not a post-hoc fit).
fn expected_detail(kind: ScenarioFaultKind, from_ms: i64, until_ms: i64) -> String {
    let window = format!("window [{from_ms}, {until_ms}) ms");
    match kind {
        ScenarioFaultKind::DroppedMessage => format!(
            "a delivery attempted inside {window} has NO acceptance (the \
             D-P4.16-4 transport mapping): the command FAILED with the \
             transport-timeout reason; the outbox row stays pending with its \
             attempt count (P4.31 F1) — never a silent pass"
        ),
        ScenarioFaultKind::ReorderedMessage => format!(
            "the held frame is released after a later tick's frame inside \
             {window}: the stored quality stays GOOD (P2.25 expected_quality: \
             'arrival order never changes the stored quality', P2.09) — the \
             receive-point sequence refusal (P4.31 F2) is the PLATFORM-level \
             analog, tested in Phase 4"
        ),
        ScenarioFaultKind::DuplicatedMessage => format!(
            "the identical re-emission inside {window} produces exactly ONE \
             effective stored row per tick (P2.25 expected_quality: GOOD, \
             P2.09 acknowledgement of the exact duplicate; P4.31 F1b is the \
             outbox-key analog)"
        ),
        ScenarioFaultKind::ConflictingDuplicate => format!(
            "the conflicting copy inside {window} is quarantined with a reason \
             (P2.25 expected_quality: GOOD, 'the conflicting copy is \
             quarantined with a reason (P2.09); the original row stands')"
        ),
        ScenarioFaultKind::InvalidSource => format!(
            "frames inside {window} carry the source-invalid code: stored BAD \
             (P2.25); a delivery attempted in the window is NACKed at issue \
             (D-P4.16-4) — never a coerced usable measurement"
        ),
        ScenarioFaultKind::OutOfRange => format!(
            "frames inside {window} carry the out-of-range code: stored SUSPECT \
             (P2.25); a tracking poll consuming the window's samples sees an \
             out-of-tolerance value (D-P4.16-4) and the tracking verdict \
             follows the criterion's evaluation order — never a silent pass"
        ),
        ScenarioFaultKind::ClockSkew => format!(
            "a command issued with the clock skewed past its validity inside \
             {window} is REFUSED at issue (P4.31 F4: ExpiredCommandRefused — \
             the adapter never expires, the sweep does) or expires during \
             tracking (F5: expiry wins over completion) — never a silent \
             reproduction of the old result"
        ),
        ScenarioFaultKind::StaleTelemetry => format!(
            "frames released late inside {window} age past the freshness limit: \
             stored STALE (P2.25); a feasibility gate instant falling inside \
             the window fails CLOSED with the stale-telemetry trigger named \
             (P4.31 F6, the D-P4.04-3 policy) — never a pass"
        ),
        ScenarioFaultKind::UnavailableAsset => format!(
            "no frames exist for the asset inside {window}: the asset is \
             UNAVAILABLE (the P5.14 visibility rule), the gap is an EXPECTED \
             gap (P2.25: MISSING/expected_gap), and the P3.24 rolling roll — \
             when a prior-schedule fixture is configured — supersedes the \
             plan WITHOUT the dropped asset (the P4.31 F8 fallback shape at \
             the scenario level)"
        ),
        ScenarioFaultKind::SolverTimeout => format!(
            "the P4.31 F9a/F9b seam outcomes HOLD the command (F9a: no \
             transition, no event) or REJECT fail-closed (F9b); this \
             composition has no solver-timeout injection surface in 0.3.0, \
             so the scenario level records the CROSS-REFERENCE only — the \
             platform tests stay in Phase 4 (P5.21 ledger step 4)"
        ),
        ScenarioFaultKind::PluginCrash => format!(
            "SIMULATED ONLY — no plugin kernel exists (v0.5.0, Stage 7): the \
             P4.31 F10 shape (a panicking proposal source whose panic \
             propagates fail-closed) is the Phase 4 seam test; the scenario \
             level records the CROSS-REFERENCE only, never a fabricated \
             crash outcome"
        ),
        ScenarioFaultKind::PartialFleetResponse => format!(
            "the fleet probe issues one command per target asset: the outcomes \
             are recorded PER ASSET (P4.31 F11: never inferred from the first \
             responder); a target inside an enabled drop window fails with \
             the transport timeout while the others accept — the mixed outcome \
             IS the expected result, not a failure of the probe"
        ),
    }
}

/// The canonical snake_case label of a P2.15 fault kind (the simulator
/// crate's serde spelling, restated here because `FaultKind` exposes no
/// name method — the label is only used inside derived fault ids).
fn p2_kind_label(kind: FaultKind) -> &'static str {
    match kind {
        FaultKind::Drop => "drop",
        FaultKind::Delay => "delay",
        FaultKind::SourceInvalid => "source_invalid",
        FaultKind::Duplicate => "duplicate",
        FaultKind::OutOfOrder => "out_of_order",
        FaultKind::ConflictingDuplicate => "conflicting_duplicate",
        FaultKind::OutOfRange => "out_of_range",
    }
}

/// The 1:1 derivation of a plan spec from a validated P2.15 fault injection.
fn derived_spec(injection: &flexcore_simulator::scenario::FaultSpec) -> ScenarioFaultSpec {
    let kind = match injection.kind {
        FaultKind::Drop => ScenarioFaultKind::UnavailableAsset,
        FaultKind::Delay => ScenarioFaultKind::StaleTelemetry,
        FaultKind::SourceInvalid => ScenarioFaultKind::InvalidSource,
        FaultKind::Duplicate => ScenarioFaultKind::DuplicatedMessage,
        FaultKind::OutOfOrder => ScenarioFaultKind::ReorderedMessage,
        FaultKind::ConflictingDuplicate => ScenarioFaultKind::ConflictingDuplicate,
        FaultKind::OutOfRange => ScenarioFaultKind::OutOfRange,
    };
    ScenarioFaultSpec {
        fault_id: format!(
            "p2-{}-{}-{}",
            p2_kind_label(injection.kind),
            injection.asset_key,
            injection.from_ms
        ),
        kind,
        injection_leg: default_leg(kind).to_string(),
        target_asset_key: Some(injection.asset_key.clone()),
        from_ms: injection.from_ms,
        until_ms: injection.until_ms,
        expected_response: kind.expected_response().to_string(),
        expected_detail: expected_detail(kind, injection.from_ms, injection.until_ms),
        pre_registered_reference: kind.p4_31_reference().to_string(),
        origin: FaultSpecOrigin::DerivedFromP2FaultPlan,
    }
}

// --- The typed slot (the ScenarioRecord v3 `faults` field) ------------------------

/// The v3 fault slot's TYPED form: the additive optional `faults` field of
/// the ScenarioRecord v3 (`replay_record.rs`, drift-noted there). The
/// untyped slot inside a telemetry scenario JSON and this typed record
/// field share ONE validation path: [`ScenarioFaultPlan::from_slot_struct`]
/// serializes the struct and runs the same fail-closed validator.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScenarioFaultsSlot {
    /// The schema tag — must be [`SCENARIO_FAULTS_SLOT_SCHEMA`].
    pub schema: String,
    /// Whether the slot is active.
    pub enabled: bool,
    /// The free-text note (optional).
    #[serde(default)]
    pub note: Option<String>,
    /// The declared injections (validated fail-closed).
    pub injections: Vec<ScenarioFaultSlotInjection>,
}

/// One typed slot injection (the same fields as the untyped slot's
/// injection object).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScenarioFaultSlotInjection {
    /// The stable fault id.
    pub fault_id: String,
    /// The kind (a [`ScenarioFaultKind::name`] value; unknown = error).
    pub kind: String,
    /// The composition leg (optional; defaults per kind).
    #[serde(default)]
    pub leg: Option<String>,
    /// The target asset key (optional; validated against the portfolio).
    #[serde(default)]
    pub asset_key: Option<String>,
    /// The window start, scenario-clock ms (`i64::MIN` = always).
    #[serde(default)]
    pub from_ms: i64,
    /// The window end, exclusive, scenario-clock ms (`i64::MAX` = always).
    #[serde(default)]
    pub until_ms: i64,
    /// The free-text note (optional).
    #[serde(default)]
    pub note: Option<String>,
}

impl ScenarioFaultPlan {
    /// Validates a TYPED slot (the ScenarioRecord v3 `faults` field) and
    /// builds the plan. The portfolio keys come from the pinned telemetry
    /// scenario the record pins (the caller loads it — the plan validates
    /// asset keys against it, the P2.15 discipline).
    pub fn from_slot_struct(
        slot: &ScenarioFaultsSlot,
        portfolio_keys: &[String],
    ) -> Result<Self, String> {
        let value = serde_json::to_value(slot)
            .map_err(|error| format!("the typed fault slot could not be rendered: {error}"))?;
        Self::from_slot(&value, portfolio_keys, &mut Vec::new())
    }
}

// --- The journal ------------------------------------------------------------------

/// One run-record fault-journal entry (the P5.22 display contract: fault
/// id, injection point, observed effect, platform response + evidence ref).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FaultJournalEntry {
    /// The fault id (the plan spec's).
    pub fault_id: String,
    /// The fault kind (the closed vocabulary name).
    pub kind: String,
    /// The injection point: the composition leg + target + window, one
    /// string so the console renders it as-is.
    pub injection_point: String,
    /// The scenario-clock instant the observation is anchored at (the run
    /// instant of the evidence, or the window start when unobserved).
    pub at_unix_millis: i64,
    /// What THIS RUN actually showed — quoted from the run-record facts or
    /// the explicit probe outcome. Never inferred, never a fabricated
    /// effect.
    pub observed_effect: String,
    /// The platform's response: `reject` / `hold` / `fallback` / `failed` /
    /// `per_asset` when evidenced by this run, else
    /// [`RESPONSE_NOT_OBSERVED`] or [`RESPONSE_CROSS_REFERENCED`].
    pub platform_response: String,
    /// The evidence reference: the run-record field the effect was quoted
    /// from (e.g. `availability_change:<asset>@<t>`,
    /// `command:<id>:issue_outcome`, `quality_counts:STALE`,
    /// `grid_check:<attempt>:reason`), or the P4.31 test name for a
    /// cross-reference.
    pub evidence_reference: String,
    /// The reused expectation's reference (P4.31 row + P2.15/P2.25).
    pub pre_registered_reference: String,
}

/// The built fault journal (the additive run-record field).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FaultJournal {
    /// One entry per plan spec, in plan order (deterministic).
    pub entries: Vec<FaultJournalEntry>,
    /// How many faults this run actually observed (response != the
    /// not-observed / cross-referenced markers).
    pub observed_count: usize,
    /// How many specs were cross-referenced to the Phase 4 suites instead
    /// of injected here.
    pub cross_referenced_count: usize,
    /// The verbatim statement (rendered by the console, never paraphrased).
    pub statement: String,
}

/// An explicit probe outcome the caller supplies for a slot-declared fault
/// it executed at the composition level (the clock-skew and partial-fleet
/// probes). The journal builder folds these into the matching entries —
/// the builder stays pure and the probes stay factual.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeOutcome {
    /// The plan spec's fault id the probe executed.
    pub fault_id: String,
    /// What the probe actually showed (the refusal text, the per-asset
    /// outcomes) — quoted, never interpreted into a pass.
    pub observed_effect: String,
    /// The evidenced response (`reject` / `per_asset` / ...).
    pub platform_response: String,
    /// The evidence reference (the probe's command id(s) / error text).
    pub evidence_reference: String,
}

/// Builds the fault journal for a run: one factual entry per plan spec.
/// The observed effects are mined from THIS run's record facts (and the
/// caller's probe outcomes); a fault with no observable effect in this run
/// records [`RESPONSE_NOT_OBSERVED`] with the reason — the never-silent-
/// pass rule applied to the journal.
pub fn build_fault_journal(
    plan: &ScenarioFaultPlan,
    record: &ScenarioRunRecord,
    probe_outcomes: &[ProbeOutcome],
) -> FaultJournal {
    let mut entries = Vec::new();
    for spec in &plan.specs {
        // An explicit probe outcome wins (it is the direct observation).
        if let Some(probe) = probe_outcomes
            .iter()
            .find(|probe| probe.fault_id == spec.fault_id)
        {
            entries.push(FaultJournalEntry {
                fault_id: spec.fault_id.clone(),
                kind: spec.kind.name().to_string(),
                injection_point: injection_point(spec),
                at_unix_millis: spec.from_ms,
                observed_effect: probe.observed_effect.clone(),
                platform_response: probe.platform_response.clone(),
                evidence_reference: probe.evidence_reference.clone(),
                pre_registered_reference: spec.pre_registered_reference.clone(),
            });
            continue;
        }
        // Cross-reference-only faults: no scenario-level injection surface.
        if matches!(
            spec.kind,
            ScenarioFaultKind::SolverTimeout | ScenarioFaultKind::PluginCrash
        ) {
            entries.push(FaultJournalEntry {
                fault_id: spec.fault_id.clone(),
                kind: spec.kind.name().to_string(),
                injection_point: injection_point(spec),
                at_unix_millis: spec.from_ms,
                observed_effect: format!(
                    "not injected at the scenario level in this run — no \
                     composition-level injection surface exists for '{}' in 0.3.0; \
                     the pre-registered platform outcomes live in the P4.31 suite \
                     (the P5.21 ledger step-4 rule: cross-reference, do not \
                     duplicate)",
                    spec.kind.name()
                ),
                platform_response: RESPONSE_CROSS_REFERENCED.to_string(),
                evidence_reference: spec.pre_registered_reference.clone(),
                pre_registered_reference: spec.pre_registered_reference.clone(),
            });
            continue;
        }
        entries.push(mined_entry(spec, record));
    }
    let observed_count = entries
        .iter()
        .filter(|entry| {
            entry.platform_response != RESPONSE_NOT_OBSERVED
                && entry.platform_response != RESPONSE_CROSS_REFERENCED
        })
        .count();
    let cross_referenced_count = entries
        .iter()
        .filter(|entry| entry.platform_response == RESPONSE_CROSS_REFERENCED)
        .count();
    FaultJournal {
        entries,
        observed_count,
        cross_referenced_count,
        statement: FAULT_JOURNAL_STATEMENT.to_string(),
    }
}

/// The injection-point rendering (leg + target + window, one string).
fn injection_point(spec: &ScenarioFaultSpec) -> String {
    let target = spec
        .target_asset_key
        .as_deref()
        .unwrap_or("(composition-wide)");
    format!(
        "leg={}, target={}, window [{}, {}) ms scenario-clock",
        spec.injection_leg, target, spec.from_ms, spec.until_ms
    )
}

/// Mines the record facts for one derived/slot spec. Every observed effect
/// quotes the run record; absence is recorded as not-observed WITH the
/// reason (never "passed", never blank).
fn mined_entry(spec: &ScenarioFaultSpec, record: &ScenarioRunRecord) -> FaultJournalEntry {
    let base = FaultJournalEntry {
        fault_id: spec.fault_id.clone(),
        kind: spec.kind.name().to_string(),
        injection_point: injection_point(spec),
        at_unix_millis: spec.from_ms,
        observed_effect: String::new(),
        platform_response: RESPONSE_NOT_OBSERVED.to_string(),
        evidence_reference: String::new(),
        pre_registered_reference: spec.pre_registered_reference.clone(),
    };
    match spec.kind {
        ScenarioFaultKind::UnavailableAsset => {
            let change = record.availability_changes.iter().find(|change| {
                change.evidence == "fault_plan_drop_window"
                    && Some(change.asset_key.as_str()) == spec.target_asset_key.as_deref()
            });
            match change {
                Some(change) => {
                    let rolled = record
                        .roll_plan
                        .as_ref()
                        .map(|plan| {
                            !plan.active_asset_keys.iter().any(|key| {
                                Some(key.as_str()) == spec.target_asset_key.as_deref()
                            })
                        })
                        .unwrap_or(false);
                    FaultJournalEntry {
                        observed_effect: format!(
                            "no frames exist for the window: the run record carries the \
                             availability change at {} ms with evidence '{}' and {} \
                             recorded dropout(s){}",
                            change.at_unix_millis,
                            change.evidence,
                            record.dropouts.len(),
                            if record.roll_plan.is_some() {
                                " (a P3.24 roll plan was produced)"
                            } else {
                                " (no roll plan: no prior-schedule fixture was configured)"
                            }
                        ),
                        platform_response: if rolled {
                            "fallback".to_string()
                        } else {
                            format!(
                                "{RESPONSE_NOT_OBSERVED} (the P3.24 roll fires only with a \
                                 prior-schedule fixture configured — provide the roll \
                                 options to observe the fallback response)"
                            )
                        },
                        evidence_reference: format!(
                            "availability_change:{}@{}",
                            change.asset_key, change.at_unix_millis
                        ),
                        ..base
                    }
                }
                None => FaultJournalEntry {
                    observed_effect: format!(
                        "{RESPONSE_NOT_OBSERVED}: no drop-window availability change for \
                         this asset appears in the run record (the window may lie outside \
                         the run's recorded horizon)"
                    ),
                    ..base
                },
            }
        }
        ScenarioFaultKind::StaleTelemetry => {
            let stale = record
                .quality_counts
                .get("STALE")
                .copied()
                .unwrap_or(0usize);
            if stale > 0 {
                let rejected = record.grid_checks.iter().any(|check| {
                    check.verdict == "fail_closed"
                        && check
                            .reason
                            .as_deref()
                            .map(|reason| reason.contains("STALE"))
                            .unwrap_or(false)
                });
                FaultJournalEntry {
                    observed_effect: format!(
                        "{stale} ingested sample(s) carried the STALE quality state \
                         (quality_counts)"
                    ),
                    platform_response: if rejected {
                        "reject".to_string()
                    } else {
                        format!(
                            "{RESPONSE_NOT_OBSERVED} (a STALE ramp baseline fails the \
                             feasibility gate closed only when a gate instant falls inside \
                             the window — P4.31 F6; no such gate instant occurred in this \
                             run)"
                        )
                    },
                    evidence_reference: "quality_counts:STALE".to_string(),
                    ..base
                }
            } else {
                FaultJournalEntry {
                    observed_effect: format!(
                        "{RESPONSE_NOT_OBSERVED}: no STALE sample appears in the run \
                         record's quality accounting (the window may lie outside the run's \
                         recorded horizon)"
                    ),
                    evidence_reference: "quality_counts".to_string(),
                    ..base
                }
            }
        }
        ScenarioFaultKind::InvalidSource
        | ScenarioFaultKind::DroppedMessage
        | ScenarioFaultKind::OutOfRange => {
            let command = record.commands.iter().find(|command| {
                Some(command.asset_key.as_str()) == spec.target_asset_key.as_deref()
            });
            match command {
                Some(command) => {
                    let (effect, response) = match spec.kind {
                        ScenarioFaultKind::InvalidSource => (
                            format!(
                                "the command's issue outcome is '{}' (a delivery in the \
                                 window is NACKed at issue, D-P4.16-4); the window's frames \
                                 carry BAD (P2.25 expected_quality)",
                                command.issue_outcome
                            ),
                            if command.issue_outcome.contains("nack")
                                || command.issue_outcome.starts_with("rejected")
                                || command.final_state == "FAILED"
                            {
                                "reject".to_string()
                            } else {
                                format!(
                                    "{RESPONSE_NOT_OBSERVED} (the delivery instant did not \
                                     fall inside the window — the NACK fires only then)"
                                )
                            },
                        ),
                        ScenarioFaultKind::DroppedMessage => (
                            format!(
                                "the command's issue outcome is '{}' (a delivery in the \
                                 window has no acceptance — the transport-timeout mapping, \
                                 D-P4.16-4)",
                                command.issue_outcome
                            ),
                            if command.final_state == "FAILED" {
                                "failed".to_string()
                            } else {
                                format!(
                                    "{RESPONSE_NOT_OBSERVED} (the delivery instant did not \
                                     fall inside the window — the timeout fires only then)"
                                )
                            },
                        ),
                        _ => (
                            format!(
                                "the command's tracking verdict is {:?} (the window's \
                                 out-of-range samples are the out-of-tolerance tracking \
                                 input, D-P4.16-4); the window's frames carry SUSPECT \
                                 (P2.25 expected_quality)",
                                command.tracking_verdict
                            ),
                            match command.tracking_verdict.as_deref() {
                                Some("deviation") | Some("failed") => "failed".to_string(),
                                _ => format!(
                                    "{RESPONSE_NOT_OBSERVED} (no tracking poll consumed the \
                                     window's samples in this run)"
                                ),
                            },
                        ),
                    };
                    FaultJournalEntry {
                        observed_effect: effect,
                        platform_response: response,
                        evidence_reference: format!(
                            "command:{}:issue_outcome",
                            command.command_id
                        ),
                        ..base
                    }
                }
                None => FaultJournalEntry {
                    observed_effect: format!(
                        "{RESPONSE_NOT_OBSERVED}: no dispatch command targeted this \
                         fault's asset in this run (add a slot spec naming the asset or \
                         run the composition probes to observe it)"
                    ),
                    ..base
                },
            }
        }
        // Source-level faults whose pre-registered outcome is the STORED
        // quality state (verified by the P2.25 expected-quality accounting
        // in the replay/coverage path — not observable in the scenario-run
        // record, and never claimed from it).
        ScenarioFaultKind::ReorderedMessage
        | ScenarioFaultKind::DuplicatedMessage
        | ScenarioFaultKind::ConflictingDuplicate => FaultJournalEntry {
            observed_effect: format!(
                "{RESPONSE_NOT_OBSERVED}: the pre-registered outcome is the STORED \
                 quality state (P2.25 expected_quality for this fault), verified by the \
                 Phase 2 replay/coverage accounting — not by the scenario-run record; \
                 the scenario-run record carries no per-fault stored-state claim"
            ),
            evidence_reference: "P2.25 expected_quality (replay coverage accounting)".to_string(),
            ..base
        },
        // Clock skew and partial fleet reach here only when the caller did
        // not run the probes.
        ScenarioFaultKind::ClockSkew | ScenarioFaultKind::PartialFleetResponse => {
            FaultJournalEntry {
                observed_effect: format!(
                    "{RESPONSE_NOT_OBSERVED}: the composition probe for '{}' was not run \
                     in this invocation (declare the spec in the scenario_faults v3 slot \
                     and run the runner with the probe enabled)",
                    spec.kind.name()
                ),
                ..base
            }
        }
        // Solver timeout / plugin crash never reach here (handled above).
        ScenarioFaultKind::SolverTimeout | ScenarioFaultKind::PluginCrash => base,
    }
}

// --- The P5.21 composition-level fault probes --------------------------------------

/// The horizon end of a scenario (the same formula the harness's private
/// `horizon_end_millis` uses — start + total_ticks × cadence, overflow as
/// a named error).
pub fn probe_horizon_end_millis(scenario: &Scenario) -> Result<i64, ScenarioRunError> {
    i64::try_from(scenario.total_ticks)
        .ok()
        .and_then(|ticks| {
            ticks.checked_mul(scenario.cadence_seconds.saturating_mul(1_000))
        })
        .and_then(|span| scenario.start_ms.checked_add(span))
        .ok_or_else(|| {
            ScenarioRunError::Scenario(
                "the scenario horizon overflows the scenario clock".to_string(),
            )
        })
}

/// Runs the composition-level fault probes for the slot-declared specs and
/// records each outcome FACTUALLY into `out`. The probes are real
/// injection controls at the composition level (P5.21 step 1):
///
/// - **clock_skew** — the probe issues a command at an instant past the
///   command's validity (horizon end + 1 ms): the pre-registered outcome
///   is a fail-closed refusal (P4.31 F4 `ExpiredCommandRefused` at the
///   adapter, or a named gate rejection — whichever fires first is what
///   the journal records). A silent acceptance would be a defect, and the
///   journal would record `not_observed_in_this_run` rather than a pass.
/// - **partial_fleet_response** — the probe issues ONE command per
///   scenario asset at the spec's declared instant and records the
///   per-asset outcomes (P4.31 F11: never portfolio-inferred). A target
///   inside an enabled fault window fails while the others accept — the
///   mixed outcome IS the expected result.
///
/// A probe error is recorded as an outcome, never swallowed (the journal
/// rule: every observation is quoted, and absence is named).
pub fn run_fault_probes(
    harness: &ScenarioRunHarness,
    plan: &ScenarioFaultPlan,
    base_config: &CommandRunConfig,
    schedule_id: &ScheduleId,
    revision: u64,
    out: &mut Vec<ProbeOutcome>,
) {
    let scenario = harness.scenario();
    let horizon_end = match probe_horizon_end_millis(scenario) {
        Ok(end) => end,
        Err(error) => {
            // A broken horizon ends the probe set with the named error
            // recorded — never a silently shortened probe set.
            out.push(ProbeOutcome {
                fault_id: "(probe-set)".to_string(),
                observed_effect: format!("the probe set could not start: {error}"),
                platform_response: "not_observed_in_this_run".to_string(),
                evidence_reference: "probe_horizon_end_millis".to_string(),
            });
            return;
        }
    };
    let cadence_millis = scenario.cadence_seconds.max(1) as i64 * 1_000;

    // The clock-skew probes (slot-declared specs).
    for spec in plan.clock_skew_specs() {
        let command_id = CommandId::new(flexcore_sim_runner::scenario_run::deterministic_uuid(
            &format!("flexcore-sim-runner/probe/clock-skew/{}", spec.fault_id),
        ));
        // Issue one instant past the command's validity window (the window
        // ends at the horizon end, per the composition's slice construction).
        let issue_at = horizon_end + 1;
        let observed = harness.run_dispatch_slice(
            base_config,
            command_id,
            schedule_id,
            revision,
            issue_at,
            issue_at,
        );
        let outcome = match observed {
            Err(error) => ProbeOutcome {
                fault_id: spec.fault_id.clone(),
                observed_effect: format!(
                    "the probe's issue attempt at {issue_at} ms (past the validity window \
                     ending {horizon_end} ms) was refused fail-closed: {error}"
                ),
                platform_response: "reject".to_string(),
                evidence_reference: format!("probe:clock-skew:{}", command_id),
            },
            Ok(slice) => {
                let response = if slice.command.issue_outcome.starts_with("rejected")
                    || slice.command.final_state == "REJECTED"
                    || slice.command.final_state == "EXPIRED"
                {
                    "reject".to_string()
                } else {
                    format!(
                        "{} (the probe's command was NOT refused — this is recorded as an \
                         UNOBSERVED refusal, never a pass)",
                        "not_observed_in_this_run"
                    )
                };
                ProbeOutcome {
                    fault_id: spec.fault_id.clone(),
                    observed_effect: format!(
                        "the probe's command ended in state {} with issue outcome '{}'",
                        slice.command.final_state, slice.command.issue_outcome
                    ),
                    platform_response: response,
                    evidence_reference: format!("probe:clock-skew:{}", command_id),
                }
            }
        };
        out.push(outcome);
    }

    // The partial-fleet probes (slot-declared specs): one command per
    // scenario asset, at the spec's declared instant.
    for spec in plan.partial_fleet_specs() {
        let issue_at = if spec.from_ms > i64::MIN {
            spec.from_ms
        } else {
            scenario.start_ms + cadence_millis
        };
        let mut asset_keys = Vec::new();
        for site in &scenario.sites {
            for asset in &site.assets {
                asset_keys.push(asset.asset_key.clone());
            }
        }
        let mut per_asset: Vec<String> = Vec::new();
        for asset_key in &asset_keys {
            let config = CommandRunConfig {
                asset_key: asset_key.clone(),
                ..base_config.clone()
            };
            let command_id = CommandId::new(
                flexcore_sim_runner::scenario_run::deterministic_uuid(&format!(
                    "flexcore-sim-runner/probe/fleet/{}/{}",
                    spec.fault_id, asset_key
                )),
            );
            let observed = harness.run_dispatch_slice(
                &config,
                command_id,
                schedule_id,
                revision,
                issue_at,
                issue_at + cadence_millis,
            );
            match observed {
                Ok(slice) => per_asset.push(format!(
                    "{}: state {}, issue {}",
                    asset_key, slice.command.final_state, slice.command.issue_outcome
                )),
                Err(error) => per_asset.push(format!("{asset_key}: probe error {error}")),
            }
        }
        let mixed = per_asset
            .iter()
            .map(|line| line.split(':').nth(1).unwrap_or("").trim())
            .collect::<Vec<_>>();
        let outcome_uniform = mixed.windows(2).all(|window| window[0] == window[1]);
        out.push(ProbeOutcome {
            fault_id: spec.fault_id.clone(),
            observed_effect: format!(
                "the fleet probe issued one command per scenario asset ({}) at {} ms; \
                 per-asset outcomes: [{}]",
                asset_keys.len(),
                issue_at,
                per_asset.join("; ")
            ),
            platform_response: "per_asset".to_string(),
            evidence_reference: if outcome_uniform {
                format!("probe:fleet:{}:uniform-outcomes (recorded per asset ANYWAY — \
                         never portfolio-inferred, P4.31 F11)", spec.fault_id)
            } else {
                format!("probe:fleet:{}:mixed-outcomes", spec.fault_id)
            },
        });
    }
}
