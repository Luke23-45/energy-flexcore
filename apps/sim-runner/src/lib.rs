// P2.24 — `flexcore-sim-runner` library surface: the deterministic-replay
// engine behind the CLI binary (`main.rs` is a thin argument shell over
// this library; the split exists so the replay behaviour is testable as a
// normal integration test target, `tests/replay.rs`).
//
// | Module | Content |
// |---|---|
// | [`replay`] | Running a scenario version + seed through the deterministic simulator and rendering the canonical replay records (`ReplayRecord` — structurally WITHOUT `ingest_time`, P2.13 §3) |
// | [`compare`] | The comparison on the frozen tuple `(asset_id, event_time, quantity, value, quality, source_sequence)`, with per-record diffs |
// | [`report`] | The replay report (JSON + human summary) and the recorded exit-code contract (pass 0 / mismatch 1 / usage 2) |
// | [`as_of`] | The P2.31 as-of (point-in-time) replay: re-deriving a past decision's input snapshot from the versioned record history and comparing it with what the decision actually used |
// | [`benchmark`] | The P3.27 benchmark harness: per-scenario × portfolio-size `BenchmarkRecord` (runtime + hardware/container capture, fail-loud unmeasured fields) and the deterministic ×2/×4 scaling-probe copies — baseline figures for later comparison, never targets (report §19.4) |
// | [`scenario_run`] | The P5.14/P5.15 scenario-run composition: the full §24 slice (telemetry ingest → schedules → grid feasibility → dispatch to SIMULATOR) against the REAL Phase 2 simulator and the REAL Phase 4 adapter chain, with the run record naming which adapters were real vs seam-level |
// | [`replay_record`] | The P5.18 ScenarioRecord v3 (pinned, replayable records — the additive evolution of the pinned-record pattern) and the `replay-run` execution path: pinned-input verification (fail-loud), the `ReplayComparisonRecord` (input-hash match, per-leg comparison, named diffs), and the P4.37 closed loop recorded on the run record |
// | [`reproducibility`] | The P5.19 reproducibility checker (§24 step 10): the pre-registered tolerance table (byte-identical scripted legs; 1e-9 relative DEFAULT for floating-point legs), the nondeterminism-source inventory with each neutralization, and the verdict builder (identical / within_tolerance / mismatch) |
// | [`feasibility_consumption`] | The P5.18 P4.37 closed loop at the scenario level: the captured pipeline evidence routed through the frozen D-P4.07-1 rule (`route_pipeline_outcome`) and fed to the frozen consumption contract (`plan_consumption`); the decision recorded on the run record; the re-scheduling consumer stays the P3.24 trigger path (the frozen trigger set is NOT modified) |
// | [`helics_orchestration`] | The P5.16 HELICS co-simulation orchestration (§12.4, §19.1): federation-config validation, the broker+federates launch plan, the fail-closed process execution over the P5.15 worker-process pattern, and the `HelicsCoSimRecord` the run record names the co-sim topology through. OPT-IN: the default scenario runs WITHOUT HELICS (direct adapters); no helics Rust binding exists or is added |
// | [`fault_injection`] | The P5.21/P5.22 scenario-level fault-injection layer (§19.2 layer 6): the `scenario_faults` v3 slot or the 1:1 P2.15 fault-plan derivation, the composition-level probe helpers, and the run-record fault journal whose entries the Simulation Lab renders — expectations REUSE the P4.31 fault matrix (Phase 4), never duplicate it |
// | [`metrics`] | The P5.23 additive per-leg timing surface: wall-clock leg durations recorded beside the run record (measurement clock, never the scenario state clock; P0.09) — baseline inputs for the benchmark template, never targets (§19.4) |
// | [`load_run`] | The P5.24 load-measurement harness (`load-run` subcommand): N telemetry-ingest iterations and/or M sequential query round trips against configured endpoints, emitting a `LoadMeasurementRecord` — single sequential client, console browser-side figures NOT measured (manual procedure) |
//!
//! Everything this crate produces is synthetic, simulation-only telemetry
//! (the entire 0.x line): no field read, no field write, no control
//! authority, no safety or certification implication.

pub mod as_of;
pub mod benchmark;
pub mod compare;
pub mod fault_injection;
pub mod feasibility_consumption;
pub mod helics_orchestration;
pub mod load_run;
pub mod metrics;
pub mod replay_record;
pub mod report;
pub mod replay;
pub mod reproducibility;
pub mod scenario_run;
