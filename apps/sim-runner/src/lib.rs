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
//!
//! Everything this crate produces is synthetic, simulation-only telemetry
//! (the entire 0.x line): no field read, no field write, no control
//! authority, no safety or certification implication.

pub mod as_of;
pub mod benchmark;
pub mod compare;
pub mod report;
pub mod replay;
