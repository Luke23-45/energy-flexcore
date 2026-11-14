//! Neryva FlexCore — the deterministic synthetic telemetry simulator
//! (ledger P2.14/P2.15): the Phase 2 `TelemetrySource` provider.
//!
//! | Module | Content |
//! |---|---|
//! | [`scenario`] | The typed, validated scenario model (P2.13): portfolio, horizon, cadence, seeds, behaviour profiles, and the **fault-injection plan** — all scenario configuration, loaded all-or-nothing |
//! | [`rng`] | The recorded RNG and seed handling: splitmix64 streams, one per asset, derived as `master_seed XOR splitmix64(FNV-1a-64(asset_key))` (D-SC4) |
//! | [`profiles`] | The per-tick synthetic behaviour profiles (BESS daily cycle with [0,1]-clamped SOC bookkeeping, PV clear-sky bell, flexible-load baseline, EVSE connection windows, site-meter aggregation) — stated assumptions, never validated physical models |
//! | [`engine`] | The deterministic tick engine: frame generation, per-asset sequence numbering, and the seven fault kinds (drop, delay, out-of-range, source-invalid, duplicate, out-of-order, conflicting duplicate) applied to frames, never to generator state |
//! | [`source`] | The [`TelemetrySource`](flexcore_ports::telemetry_source::TelemetrySource) implementation: provider declaration (cadence, quality mapping, clock behaviour), tenant-scoped subscriptions, run control (start/pause/resume/stop), bounded pull-based emission, and the real frame decode into canonical [`NormalizedTelemetry`](flexcore_ports::telemetry_source::NormalizedTelemetry) records |
//!
//! # Rules this crate encodes
//!
//! - **Determinism scope (P2.13 §3):** same scenario version + same seed +
//!   same code version ⇒ identical records on `(asset, event_time,
//!   quantity, value, quality, source_sequence)`. `ingest_time` is the
//!   wall-clock moment of emission and is **excluded from replay
//!   comparison by construction** — the simulator stamps it from the
//!   scenario clock at the frame's release tick, the only clock a
//!   deterministic simulator may read, which also keeps
//!   `ingest_time >= event_time` true by construction (P0.09).
//! - **Canonical output only (P2.04):** canonical units via the
//!   `flexcore-contracts` newtypes, `P > 0` = export, SOC a fraction in
//!   [0,1], exactly one quality state per value, `source_sequence`
//!   monotonic per asset, and the provider identity
//!   `sim://<scenario_id>` — never the reserved `flexcore:` namespace
//!   (P2.04 D-T6: a provider is a measured source; `MISSING` gap markers
//!   are platform-derived, which is why the `drop` fault emits *absence*).
//! - **Faults are configuration, not code branches (P2.15 step 2):** the
//!   fault plan lives in the scenario's versioned `faults` section and is
//!   parsed and validated like every other scenario element; with
//!   `enabled: false` the plan is inert. Fault tests enable it by flipping
//!   that one flag on the parsed configuration.
//! - **Fault-injection scope (P2.15 step 4, recorded):** these are
//!   simulator faults for testing the platform's handling of missing,
//!   stale, suspect, bad, duplicate, out-of-order, and conflicting values
//!   — they are **not claims about real device failure modes**. All fault
//!   arithmetic is scenario-clock arithmetic; no fault path reads the wall
//!   clock.
//! - **Bounded, backpressure-aware emission (P2.14 step 5):** generation
//!   is pull-driven — nothing is produced unless the consumer pulls a
//!   frame, each pull yields at most one frame (one asset, one tick), and
//!   pause/stop halt the stream at tick boundaries.
//! - **The port is the only integration point:** this crate implements
//!   `TelemetrySource` and nothing else about the ingestion path — no
//!   database, no broker, no pipeline internals. It depends on
//!   flexcore-contracts, flexcore-domain, flexcore-ports, serde,
//!   serde_json, and time; nothing infrastructure.
//!
//! # Placement decision (P2.14 step 1, recorded once)
//!
//! The simulator is a `crates/` provider crate rather than a
//! `plugins/builtin/asset-simulator/` tree: it is built-in Phase 2 code
//! (the plugin SDK is v0.5.0), implements the `TelemetrySource` port and
//! nothing else, and the composition-side dependency policy therefore
//! applies (see the crate manifest header and README for the workspace
//! member line to add).
//!
//! # Scope and control status
//!
//! Everything this crate produces is **synthetic, simulation-only
//! telemetry** (the entire 0.x line). No behaviour profile is a validated
//! physical model; no value is measured data; nothing here carries field
//! write authority or implies any safety, certification, or
//! control-authority property. FlexCore is a supervisory orchestration
//! layer and performs no primary frequency response, synthetic inertia,
//! relay protection, inverter inner-loop control, or sub-cycle control.

pub mod scenario;
pub mod source;

mod engine;
mod profiles;
mod rng;

pub use scenario::{load_scenario, FaultKind, Scenario, ScenarioError};
pub use source::{RunStatus, SimulatorTelemetrySource};

/// The simulator's own version, as recorded by the P5.14 run record
/// (P5.19 reproducibility input). It is the crate's workspace version
/// (`Cargo.toml`), read at compile time — never a hard-coded string that
/// can drift from the artefact actually compiled. The P5.14
/// `ScenarioRunRecord` carries it per run together with the scenario
/// version and the code version, so a replay names every generator-side
/// version that produced the records (P2.13 §4 discipline, extended to
/// the adapter integration).
///
/// Recording a version is a provenance statement only: it does NOT claim
/// certification, validation, or any physical-model fidelity for the
/// simulator (§19.1 — the profiles are stated synthetic assumptions).
pub const SIMULATOR_CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
