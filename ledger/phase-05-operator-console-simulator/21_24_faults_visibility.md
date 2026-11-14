# Decision record — P5.21 Failure injection in the simulator + P5.22 Failure-injection evidence and operator visibility (§19.2 layer 6, §20.2, §23 Stage 5)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, run, or executed** — no `cargo build/check/test/clippy`, no npm
install, no vitest run (loop bans; AGENTS.md working agreement; the wave
ENTRY_NOTE). Every test below is written and pre-registered UNRUN; the
human build/test pass is the gate. "Pre-registered" means the expectations
were written together with the suites BEFORE any execution exists — none
has run.

## 1. What was built

| Artefact | File | Content |
|---|---|---|
| Fault-injection layer (NEW) | `apps/sim-runner/src/fault_injection.rs` | The §19.2 layer 6 vocabulary ([`ScenarioFaultKind`], 12 kinds), the `scenario_faults` v3 slot validator (fail-closed), the 1:1 derivation from the validated P2.15 fault plan, the composition-level probes (clock skew, partial fleet), and the run-record fault journal builder whose entries are mined FROM THE RUN'S OWN FACTS. The expected outcomes REUSE the P4.31 fault matrix (`31_fault_shadow.md` §2) and the P2.25 expected-quality plan — no second platform-behaviour table is created. |
| Run-record extension (additive) | `apps/sim-runner/src/scenario_run.rs` | `ScenarioRunRecord` gains `fault_journal: Option<FaultJournal>` + `timings: Option<ScenarioRunTimings>` (both `skip_serializing_if` — old consumers unaffected; `record()` unchanged, values attached by the builders). |
| ScenarioRecord v3 extension (additive, drift-noted) | `apps/sim-runner/src/replay_record.rs` | `ScenarioRecordV3` gains `faults: Option<ScenarioFaultsSlot>` (`#[serde(default)]`; the drift note is on the field). The input hash is unaffected (it covers inputs/backend/seeds/tolerances/controls only). The typed slot shares ONE validation path with the untyped scenario-level slot. |
| Timing surface (NEW) | `apps/sim-runner/src/metrics.rs` | The P5.23 per-leg wall-clock timing block (`ScenarioRunTimings`): measured legs only, untimed legs absent, duplicate/non-finite figures refused — the same P3.27 `MeasuredField` discipline; the measurement clock never touches the run's scenario-clock STATE. |
| Load harness (NEW) | `apps/sim-runner/src/load_run.rs` | The P5.24 `load-run` harness: N telemetry-ingest iterations through the real replay pipeline and/or M sequential HTTP GET round trips (std-only, http:// only), emitting a `LoadMeasurementRecord` with the hardware block, scope notes, and fail-loud `not_measured` fields. |
| CLI wiring (additive) | `apps/sim-runner/src/main.rs` | `scenario-run` always builds the fault journal; `--fault-probes` opts in to the composition-level probes; per-leg timings are measured and attached; the new `load-run` subcommand emits the `LoadMeasurementRecord`. |
| Fault-injection suite (NEW) | `apps/sim-runner/tests/fault_injection_p5_21.rs` | 8 tests: slot parsing + pre-registration, fail-closed validator, P2.15 derivation, inert plan, the clock-skew probe refusal, the per-asset fleet probe, the never-silent-pass journal, cross-reference/probe-outcome precedence. |
| Metrics/load suites (NEW) | `apps/sim-runner/tests/metrics_p5_23.rs` | 6 tests: timings honesty (untimed legs absent, no zero-fill), additive record serialization, load-record schema + scope notes, spec validator fail-loud, nearest-rank percentile, a produced ingest-only record's measured/not-measured split. |
| Simulation Lab (replaces the P5.01 stub) | `console/src/workspaces/SimulationLab.tsx` | The visibility layer: run list with the FAULT-INJECTED tag, per-fault injected/caused/response columns with evidence refs, the failure journal of failed runs, the honest P5.31 alarm mapping, and the "clear injections and re-run clean" affordance. Export name unchanged. |
| Lab data module (NEW) | `console/src/workspaces/simulationLabData.ts` | The verbatim statements, the closed-vocabulary fault→alarm mapping, and the pure derivation helpers (failure rows, injected detection). |
| Run-record client (NEW) | `console/src/api/scenarioRuns.ts` | The display-contract client (`GET /api/v1/scenarios/runs`) + the wire mirrors of the run record / fault journal. |
| Lab tests (NEW) | `console/src/workspaces/SimulationLab.test.tsx` | 11 tests (UNRUN): the reviewer test rendered — injected/caused/response visibility, verbatim responses, failure journal, alarm mapping, clean re-run affordance, clean-run honesty, the serving gap, empty/loading/error states. |

No other console workspace, no `apps/control-plane/**`, and no `crates/**`
source was modified (the alarm model and shadow evaluation are read-only
consumption; the frozen §15.3 event vocabulary is not extended and no
alarm code is added — the mapping consumes ALARM-MODEL-0.1.0 as-is).

## 2. The per-fault table (the P5.21 deliverable — REUSES the P4.31 matrix)

**Source of truth for the platform-behaviour expectations:**
`ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md` §2 (the
P4.31 pre-registered fault matrix). The "scenario-level" column below is
the EXTENSION for the sim-runner composition; it is pre-registered here,
before any run. The closed kind vocabulary lives in `fault_injection.rs`
(`ScenarioFaultKind`, `name()` values are the wire spellings).

| Kind (wire name) | §19.2 layer 6 fault | P4.31 reference (REUSED) | P2.15/P2.25 source mapping | Injection leg | Pre-registered scenario-level expected outcome | Implemented as |
|---|---|---|---|---|---|---|
| `dropped_message` | dropped messages | F1 (outbox drop; no loss, never silently dropped) | — (slot-declared; the `drop` window realizes the same effect at the transport) | `dispatch_transport` | a delivery inside the window has NO acceptance (D-P4.16-4): the command FAILED with the transport-timeout reason; the outbox row stays pending | slot spec; journal mines the command outcome; probeable via the fleet probe |
| `reordered_message` | reordered messages | F2 (receive-point refusal) | `out_of_order` (hold_ticks ≥ 1) | `telemetry_source` | the held frame is stored late; the stored quality stays GOOD (P2.25: "arrival order never changes the stored quality", P2.09) | derived from the P2.15 plan; the journal names WHERE the outcome is verified (the P2.25 coverage accounting, not the run record) |
| `duplicated_message` | duplicated messages | F1b (duplicate key refusal) | `duplicate` | `telemetry_source` | exactly ONE effective stored row per tick (P2.25: GOOD, "the exact duplicate is acknowledged") | derived; same journal honesty as reordered |
| `conflicting_duplicate` | duplicated (conflicting) | F1b | `conflicting_duplicate` | `telemetry_source` | the conflicting copy is quarantined with a reason; the original row stands (P2.25) | derived |
| `invalid_source` | (source faults) | no F-row; D-P4.16-4 (source_invalid → NACK) | `source_invalid` | `telemetry_source` | stored BAD (P2.25); a delivery in the window is NACKed at issue — never a coerced usable measurement | derived; journal maps the command outcome |
| `out_of_range` | (source faults) | no F-row; D-P4.16-4 (out-of-tolerance tracking sample) | `out_of_range` (factor > 1) | `telemetry_source` | stored SUSPECT (P2.25); a tracking poll consuming the window's samples drives the verdict per the criterion's evaluation order | derived; journal maps the tracking verdict |
| `clock_skew` | clock skew | F4 (refused at issue) + F5 (expiry wins over completion) | — (slot-declared) | `composition_clock` | a command issued past its validity is REFUSED fail-closed (`ExpiredCommandRefused` at the adapter, or a named gate rejection — whichever fires first is what the journal records); never a silent acceptance | the composition-level probe (`run_fault_probes`, issues at horizon-end + 1 ms) |
| `stale_telemetry` | stale telemetry | F6 (fail-closed with the trigger named) | `delay` (delay_ticks > 0) | `telemetry_source` | stored STALE (P2.25); a feasibility gate instant inside the window fails CLOSED with the stale-telemetry trigger named (the D-P4.04-3 policy) | derived; the journal quotes the quality counts and the fail-closed reason when a gate instant fell inside |
| `unavailable_asset` | unavailable assets | F8 (FAILED + fallback activation) | `drop` window | `telemetry_source` | no frames for the window: the asset is UNAVAILABLE (the P5.14 visibility rule), the gap is an EXPECTED gap (P2.25: MISSING/expected_gap), and the P3.24 roll — when a prior-schedule fixture is configured — supersedes the plan without the asset | derived (the P5.14 dropout path); journal records the fallback response ONLY when a roll plan was produced |
| `solver_timeout` | solver timeout | F9a (hold, no transition, no event) + F9b (reject-branch variant) | — (no P2.15 mapping) | `grid_feasibility` | CROSS-REFERENCE ONLY at the scenario level: this composition has no solver-timeout injection surface in 0.3.0; the pre-registered outcomes live in the P4.31 suite (the P5.21 ledger step-4 rule: cross-reference, do not duplicate) | the journal carries the cross-reference entry with NO fabricated outcome |
| `plugin_crash` | plugin crash (SIMULATED — no kernel, v0.5.0) | F10 (panic propagates fail-closed) | — (no P2.15 mapping) | `grid_feasibility` | CROSS-REFERENCE ONLY; the scenario level never fabricates a crash outcome | same as solver_timeout |
| `partial_fleet_response` | partial fleet response | F11 (per-asset, never portfolio-inferred) | — (slot-declared) | `fleet_probe` | the probe issues ONE command per scenario asset at the spec's instant and records the per-asset outcomes; a target inside an enabled fault window fails while the others accept — the mixed outcome IS the expected result | the composition-level fleet probe (`run_fault_probes`) |

Coverage statement: every §19.2 layer 6 fault from the ledger objective
(dropped/reordered/duplicated messages, clock skew, stale telemetry,
unavailable assets, solver timeout, plugin crash, partial fleet response)
is injectable through this layer — the eight with a P2.15/slot surface are
injected at the composition or source level; the two cross-referenced
kinds (solver timeout, plugin crash) have NO scenario-level injection
surface in 0.3.0 and are recorded as cross-references to the Phase 4
suites rather than duplicated or fabricated. Outcomes for all of them are
pre-registered (this table), and the journal NEVER records a silent pass:
an unobserved fault carries `not_observed_in_this_run` WITH its reason.

## 3. The scenario fault slot (v3) — shape and the ScenarioRecord drift note

The wave-4a ScenarioRecord v3 (`replay_record.rs`) landed MID-WAVE: the
authoring started before it existed, so the slot shape was defined here
first and implemented defensively; when the record schema landed it was
extended additively per the task order.

**The slot shape** (validated fail-closed by `ScenarioFaultPlan::from_slot`
/ `from_slot_struct`):

```json
"scenario_faults": {
  "schema": "flexcore-scenario-faults/v3",
  "enabled": true,
  "note": "free text, optional",
  "injections": [
    {
      "fault_id":  "SF-<short-tag>-<n>",
      "kind":      "<one ScenarioFaultKind::name() value>",
      "leg":       "telemetry_source | dispatch_transport | composition_clock | grid_feasibility | fleet_probe",
      "asset_key": "<optional; validated against the scenario portfolio>",
      "from_ms":   0,
      "until_ms":  0,
      "note":      "free text, optional"
    }
  ]
}
```

Validator rules (each a named error, never a silent guess): unknown
`schema`; unknown `kind` (the 12-kind vocabulary is closed); `asset_key`
not in the portfolio; `until_ms < from_ms`; missing `fault_id`/`from_ms`/
`until_ms`. Unknown extra fields are ignored forward-compat and counted as
drift notes on the plan. With no slot, the plan derives 1:1 from the
validated P2.15 `faults` plan. With the P2.15 plan disabled and no slot,
the plan is INERT — no fault invented.

**Drift note (recorded on the `ScenarioRecordV3.faults` field):** the
record field is `Option<ScenarioFaultsSlot>`, `#[serde(default)]` —
pre-existing records deserialize unchanged; the input hash is unaffected
(it covers inputs/backend/seeds/tolerances/controls only). RECORDED
HONESTLY: the `replay-run` executor does NOT yet consume the field — the
operative injection surfaces in this phase are the telemetry scenario's
own `faults` plan and the top-level `scenario_faults` slot the
`scenario-run` path reads. Declaring a fault slot in a record is
configuration for the injection controls, never a claim that a fault was
injected.

## 4. The fault journal (the P5.22 run-record contract)

Every run whose scenario configures any fault carries a `FaultJournal`
(additive field on `ScenarioRunRecord`); each entry carries exactly the
contract fields:

| Field | Rule |
|---|---|
| `fault_id` | the plan spec's stable id (slot-declared verbatim; derived ids `p2-<kind>-<asset>-<from_ms>` — deterministic, never random) |
| `injection_point` | the composition leg + target + window, one string |
| `observed_effect` | quoted FROM THIS RUN's record facts (availability changes, quality counts, command outcomes, grid checks) or from an explicit probe outcome — never inferred, never a fabricated effect |
| `platform_response` | `reject` / `hold` / `fallback` / `failed` / `per_asset` when evidenced; else `not_observed_in_this_run` (WITH the reason) or `cross_referenced_to_phase_4_suite` |
| `evidence_reference` | the run-record field the effect was quoted from (`availability_change:<asset>@<t>`, `command:<id>:issue_outcome`, `quality_counts:STALE`, `grid_check:<attempt>:reason`, `probe:…`) or the P4.31 test reference for a cross-reference |
| `pre_registered_reference` | the reused expectation's citation (P4.31 row + P2.15/P2.25) |

The journal carries the verbatim statement (`FAULT_JOURNAL_STATEMENT`)
and the counts (`observed_count`, `cross_referenced_count`); the console
renders the statement verbatim.

## 5. Operator visibility coverage (P5.22 steps 1–4 → the workspace)

| P5.22 requirement | Where implemented |
|---|---|
| Step 1: active injections visible (fault type, target, timing); an operator can always tell a run is fault-injected | the `[FAULT-INJECTED]` tag is rendered structurally on any run whose journal carries entries (`isFaultInjected`); the fault table renders fault id + kind + injection point (leg/target/window) per fault |
| Step 2: the platform's response per fault (rejected/held/failed/fallback) with its evidence link | the response column renders the journal's `platform_response` verbatim; the evidence column renders `evidence_reference` + the pre-registered reference |
| Step 3: "clear injections and re-run clean" | the affordance renders the exact CLI command (`flexcore-sim-runner scenario-run … --seed <recorded seed>`) and states the recorded-inputs rule (P5.18/P5.19) AND the honest gap: the console-side trigger API is not served yet — a documented action, not a button that could not do anything |
| Step 4: reviewer test — fault, affected assets, response readable from the display alone | pinned by the UNRUN tests (`fault-injected-tag`, `fault-response-<id>`, `evidence` column, the failure journal); the observed effect and injection point name the asset |
| "never a silent blank screen" | failed runs render their failure journal (per-command and per-grid-check rows from `failureRows`); the alarms panel renders the P5.31 honest empty/error states; the run-record serving gap renders when the read fails |
| Alarms (the P5.31 model) surfaced where the run record maps them | the workspace embeds the `AlarmsPanel` (derived-on-read GET /api/v1/alarms) and maps journal responses onto the CLOSED vocabulary: `failed` → `DISPATCH_FAILURE`, `fallback` → `DISPATCH_FALLBACK`; every other response maps to NO alarm with the explicit no-invented-alarm note |

Honest gap, recorded: **the run-record serving API does not exist yet in
this phase.** The ScenarioRunRecord is a sim-runner CLI artefact; no
control-plane route serves it. The console client
(`console/src/api/scenarioRuns.ts`) pins `GET /api/v1/scenarios/runs` as
the display contract; in a real deployment today the read 404s and the
workspace renders `RUN_RECORD_API_GAP` — no run is fabricated. The
serving surface is a run-history matter (P5.08/P5.30). The UNRUN tests
therefore render from mocked records (the MarketPrograms test pattern).

## 6. Decisions (all DEFAULT unless noted)

| ID | Decision | Label |
|---|---|---|
| D-P5.21-1 | The fault vocabulary is ONE closed enum (12 kinds) spanning the §19.2 layer 6 faults AND the P2.15 source faults, so the journal, the slot, and the console share one vocabulary; the mapping table (§2 above) is the single pre-registration. | DEFAULT |
| D-P5.21-2 | The P2.15 `drop` window maps to `unavailable_asset` (the P5.14 recorded semantics) and the same window realizes "dropped messages" at the transport via D-P4.16-4 — ONE spec per P2.15 injection (1:1, no double counting); a dedicated `dropped_message` without unavailability is slot-declarable. | DEFAULT |
| D-P5.21-3 | Solver timeout and plugin crash are CROSS-REFERENCED, not re-implemented at the scenario level: no composition injection surface exists in 0.3.0 and the platform seam tests stay in Phase 4 (the P5.21 ledger step-4 rule). The journal carries the cross-reference with no fabricated outcome. | DEFAULT |
| D-P5.21-4 | The clock-skew probe issues at horizon-end + 1 ms (past the slice's validity window) and pre-registers a fail-closed refusal WITHOUT pinning WHICH refusal fires first (the adapter's `ExpiredCommandRefused` or a named gate rejection) — the journal records whichever actually happened, factually. A silent acceptance would be recorded `not_observed_in_this_run`, never a pass. | DEFAULT |
| D-P5.21-5 | The partial-fleet probe issues one command per scenario asset at the slot spec's declared instant and records per-asset outcomes (the P4.31 F11 shape); derived plans carry no fleet spec, so the probe runs only when the v3 slot declares it. | DEFAULT |
| D-P5.21-6 | The ScenarioRecord v3 `faults` field is additive with `#[serde(default)]` and a drift note; the `replay-run` executor does not consume it yet (recorded on the field, never silently assumed). | DEFAULT |
| D-P5.22-1 | The alarm mapping consumes the CLOSED ALARM-MODEL-0.1.0 vocabulary as-is: only `failed` → `DISPATCH_FAILURE` and `fallback` → `DISPATCH_FALLBACK` map; everything else maps to an explicit "no alarm code exists — none invented". No alarm code is added and the frozen §15.3 event vocabulary is not extended. | DEFAULT |
| D-P5.22-2 | The "clear injections and re-run clean" affordance renders the CLI command + the gap instead of a non-functional button: a control that does nothing would violate §20.2 honesty (the same discipline as P5.07's empty states). | DEFAULT |
| D-P5.22-3 | The run-record client pins the display-contract path (`/api/v1/scenarios/runs`) although no server route exists: a 404 renders the honest gap, and the tests mock the records (the MarketPrograms fake-client pattern) — the console never fabricates runs. | DEFAULT |

## 7. Honest limitations

1. **Everything is UNRUN.** No Rust test, no CLI run, no vitest run has
   executed. The expectations are pre-registered, not passing. The P5.21
   acceptance criterion "every listed fault is injectable with a
   pre-registered expected outcome; outcomes match" is implemented-and-
   unverified; the human build/test pass is the gate.
2. **The journal observes what the run record exposes.** The
   source-level stored-quality outcomes (reordered/duplicated/
   conflicting-duplicate) are verified by the P2.25 expected-quality
   accounting in the replay/coverage path — the scenario-run record does
   not carry per-fault stored-state claims, and the journal says exactly
   that instead of inventing an effect.
3. **The probes share the harness state.** The composition probes run on
   the same in-process harness after the main slice and horizon sweep
   (deterministic, controlled clock); a probe's command ids are
   deterministic and distinct, so no store conflict arises — but the
   probe outcomes are NOT folded back into the run record's command
   summaries (they live in the journal only). This is recorded, not
   hidden.
4. **The clock-skew probe's exact refusal point is pre-registered as a
   disjunction** (adapter expiry refusal or named gate rejection),
   because reading the code cannot prove which fires first — the test
   accepts both and rejects only a silent acceptance.
5. **The fleet probe instant comes from the slot spec.** A fleet spec
   without an explicit window probes at the scenario start + one cadence.
   The probe is single-harness and sequential; it does not model
   concurrent fleet dispatch.
6. **The console run-record surface is a display contract.** No server
   route serves run records in this phase; the workspace renders the gap
   honestly. Wiring the serving route is out of this subtask's file
   ownership (control-plane) and is recorded for P5.08/P5.30.
7. **Concurrent-wave edit note:** the wave-4a agent landed
   `replay_record.rs`/`reproducibility.rs`/`feasibility_consumption.rs`
   and the `replay-run` subcommand WHILE this subtask was in progress.
   The extension of `ScenarioRecordV3` (§3) and the `main.rs` additions
   were made additively against the landed state; the human build pass
   should confirm the combined tree compiles.

## 8. Tool runs performed in this subtask (the only executions allowed)

`python tools/verify_docs.py`, `python tools/validate_agent.py`, and
`python -I tools/check_dependency_rule.py` — their actual output is
quoted in the final task report. No cargo, npm, docker, database, or
broker command was run; nothing was installed.
