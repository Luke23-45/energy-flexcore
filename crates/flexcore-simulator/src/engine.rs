// P2.14/P2.15 — The deterministic generation engine.
//
// The engine turns the validated scenario into a deterministic stream of
// raw **frames** (the simulator's own wire format): one frame = one asset's
// complete sample set at one scenario-clock tick. Frames are what the
// `TelemetrySource` implementation (source.rs) pumps and decodes; the frame
// format exists so the port's decode step is a real decode, not a
// pass-through.
//
// Determinism rules encoded here (P2.13 §3; P2.14):
//
// - The engine is a pure function of (scenario, seed, code). There is no
//   wall-clock input anywhere: the scenario clock advances one cadence tick
//   per generated tick, and `released_at_unix_ms` (which becomes
//   `ingest_time`) is the scenario-clock instant of the tick at which the
//   frame is released from the engine — the only clock a deterministic
//   simulator may read. This keeps `ingest_time >= event_time` true by
//   construction and leaves `ingest_time` outside the replay comparison
//   (P2.13/P2.24).
// - Per-asset state (SOC bookkeeping, seeded RNG, sequence counter)
//   advances in a fixed order (sites in scenario order, assets in listed
//   order, one tick at a time). Faults act on **generated frames**, never
//   on generator state: enabling a fault changes only which frames are
//   emitted, delayed, duplicated, or re-labelled — no other tick's values
//   change.
// - Sequence numbering (P2.04 §8): `source_sequence` is assigned at
//   generation, monotonically per asset, gap-free from 1 in a fault-free
//   run. Dropped frames consume no sequence number (the absence stays a
//   pure event-time gap, which is exactly what the pipeline's gap detector
//   needs to synthesize `MISSING`); duplicates and conflicting duplicates
//   re-use the original frame's sequence (that shared identity is the
//   point); delayed and held frames keep their original number, which is
//   why a release-after-later-tick shows up as an arrival-order violation.
// - The site meter aggregates the **profile** values of its site's assets
//   (pre-fault). Faults apply per asset afterwards and do not propagate
//   into the meter sum — the meter is a consistency fixture of the
//   profiles, not a fault-propagation model.
// - Frames still held in a delay/out-of-order buffer when the horizon ends
//   are dropped: the run ends at the horizon. Injections are validated to
//   lie within the horizon; a hold that would release past the last tick
//   simply never emits.
//
// Fault semantics (P2.15): the fault plan is scenario configuration
// (`scenario.rs` validated it); the engine only evaluates the plan. The
// seven kinds map to SOURCE-level outcomes:
//
// | kind                  | engine behaviour                                     |
// |-----------------------|------------------------------------------------------|
// | drop                  | no frame (no sequence consumed)                      |
// | delay                 | frame held `delay_ticks`, released later             |
// | out_of_range          | active power scaled past rated, code OUT_OF_RANGE    |
// | source_invalid        | code SOURCE_INVALID on the whole frame               |
// | duplicate             | frame emitted twice, identical                       |
// | out_of_order          | frame held `hold_ticks`, released after later tick   |
// | conflicting_duplicate | frame twice, same identity, different value          |
//
// These are simulator faults for testing the platform's handling — not
// claims about real device failure modes.

use crate::profiles;
use crate::rng::derive_asset_seed;
use crate::scenario::{BehaviourParams, FaultKind, Scenario};
use serde::Serialize;
use std::collections::VecDeque;

/// The frame format tag. The decode step rejects any other value.
pub(crate) const FRAME_FORMAT: &str = "flexcore-sim-frame";
/// The frame format version.
pub(crate) const FRAME_VERSION: u32 = 1;

/// The source-side quality codes the simulator can emit (P0.08 §3: richer
/// source codes mapped onto the canonical six via the declared quality
/// mapping — see the provider declaration in `source.rs`).
pub(crate) mod quality_codes {
    pub(crate) const OUT_OF_RANGE: &str = "OUT_OF_RANGE";
    pub(crate) const SOURCE_INVALID: &str = "SOURCE_INVALID";
}

/// The wire quantity names (P2.04 §4 quantity keys). The decode step maps
/// these onto the frozen `TelemetryQuantity` vocabulary.
pub(crate) mod quantity_names {
    pub(crate) const ACTIVE_POWER: &str = "active_power";
    pub(crate) const REACTIVE_POWER: &str = "reactive_power";
    pub(crate) const SOC: &str = "soc";
    pub(crate) const ENERGY: &str = "energy";
    pub(crate) const AVAILABILITY: &str = "availability";
    pub(crate) const OPERATING_MODE: &str = "operating_mode";
    pub(crate) const HEALTH: &str = "health";
    pub(crate) const CONNECTION_STATE: &str = "connection_state";
    pub(crate) const BASELINE_REFERENCE: &str = "baseline_reference";
}

// --- The frame wire format ----------------------------------------------------

/// One asset's sample set at one scenario tick: the unit of emission and of
/// decode. Serialized to JSON by the source; parsed back by `decode`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct WireFrame {
    pub format: &'static str,
    pub version: u32,
    pub asset_key: String,
    /// Scenario-clock instant the measurement occurred.
    pub event_time_unix_ms: i64,
    /// Scenario-clock instant the frame is released from the engine
    /// (becomes `ingest_time`; excluded from replay comparison).
    pub released_at_unix_ms: i64,
    pub source_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_quality_code: Option<&'static str>,
    pub samples: Vec<WireSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct WireSample {
    pub quantity: &'static str,
    pub value: WireValue,
    /// Present exactly on reactive-power samples: the site's declared
    /// convention (P0.07 §3; P2.04 D-T4 — the convention travels on the
    /// record; the sign is never interpreted without it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactive_sign_convention: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum WireValue {
    Number(f64),
    Flag(bool),
    State(String),
}

fn convention_name(
    convention: flexcore_contracts::units::ReactiveSignConvention,
) -> &'static str {
    use flexcore_contracts::units::ReactiveSignConvention as C;
    match convention {
        C::InjectionPositive => "INJECTION_POSITIVE",
        C::ConsumptionPositive => "CONSUMPTION_POSITIVE",
    }
}

// --- Engine state -------------------------------------------------------------

/// The fault data the engine needs for one asset tick, copied out of the
/// plan so fault evaluation never borrows the generator state it mutates.
#[derive(Debug, Clone, Copy)]
struct ActiveFault {
    kind: FaultKind,
    delay_ticks: u64,
    hold_ticks: u64,
    factor: f64,
}

/// Per-asset runtime state, in flat generation order.
struct AssetEngine {
    asset_key: String,
    site_index: usize,
    spec: crate::scenario::AssetSpec,
    /// SOC bookkeeping state (BESS and EVSE only).
    soc: Option<f64>,
    /// The asset's derived RNG stream (D-SC4). Profiles without randomness
    /// never draw from it.
    rng: crate::rng::SplitMix64,
    /// The next `source_sequence` value (assigned at generation).
    next_sequence: u64,
    /// The asset's validated injections (empty when the plan is disabled).
    faults: Vec<crate::scenario::FaultSpec>,
}

/// A held frame awaiting release (the `delay` / `out_of_order` mechanism).
struct HeldFrame {
    /// The tick at which the frame releases.
    release_tick: u64,
    /// The tick that generated the frame (deterministic release tiebreak).
    generated_tick: u64,
    frame: WireFrame,
}

/// The mutable engine state.
#[derive(Default)]
struct EngineState {
    next_tick: u64,
    queue: VecDeque<WireFrame>,
    held: Vec<HeldFrame>,
}

/// The deterministic generation engine over one validated scenario.
pub(crate) struct Engine {
    scenario: Scenario,
    assets: Vec<AssetEngine>,
    state: EngineState,
}

impl Engine {
    /// Builds the engine. All state is derived from the scenario (SOC
    /// initial values, per-asset seeds via the recorded derivation rule);
    /// generation starts at tick 0 (the horizon start).
    pub(crate) fn new(scenario: Scenario) -> Self {
        let plan_enabled = scenario.faults.enabled;
        let master_seed = scenario.master_seed;
        let mut assets = Vec::new();
        for (site_index, site) in scenario.sites.iter().enumerate() {
            for asset in &site.assets {
                let soc = match &asset.params {
                    BehaviourParams::Bess(p) => Some(p.initial_soc),
                    BehaviourParams::Evse(p) => Some(p.initial_soc),
                    _ => None,
                };
                let faults = if plan_enabled {
                    scenario
                        .faults
                        .injections
                        .iter()
                        .filter(|f| f.asset_key == asset.asset_key)
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                assets.push(AssetEngine {
                    asset_key: asset.asset_key.clone(),
                    site_index,
                    spec: asset.clone(),
                    soc,
                    rng: crate::rng::SplitMix64::new(derive_asset_seed(
                        master_seed,
                        &asset.asset_key,
                    )),
                    next_sequence: 1,
                    faults,
                });
            }
        }
        Self {
            scenario,
            assets,
            state: EngineState::default(),
        }
    }

    /// The scenario clock of the last generated tick, in ms (the engine's
    /// current instant).
    pub(crate) fn scenario_clock_ms(&self) -> i64 {
        self.scenario
            .tick_event_ms(self.state.next_tick.saturating_sub(1))
    }

    /// Pops the next frame to emit, generating ticks as needed. Returns
    /// `None` once the horizon is exhausted (all generated and held frames
    /// released). One call yields at most one frame — the bounded-emission
    /// unit; the caller pulls, which is the backpressure mechanism.
    pub(crate) fn next_frame(&mut self) -> Option<WireFrame> {
        loop {
            if let Some(frame) = self.state.queue.pop_front() {
                return Some(frame);
            }
            if self.state.next_tick >= self.scenario.total_ticks {
                // The horizon is exhausted; frames still held never release
                // (documented engine boundary).
                return None;
            }
            let tick = self.state.next_tick;
            self.generate_tick(tick);
            self.state.next_tick += 1;
            // Held frames release AFTER the current tick's own frames: a
            // held frame's arrival then lands after a strictly later tick's
            // frame, which is what makes `out_of_order` observable.
            self.release_due(tick);
        }
    }

    /// Generates one tick: per site, all non-meter assets first (their net
    /// power sums into the meter's aggregate), then every asset's frame is
    /// built and disposed per the fault plan.
    fn generate_tick(&mut self, tick: u64) {
        let event_ms = self.scenario.tick_event_ms(tick);
        let cadence_s = self.scenario.cadence_seconds;
        let sod = profiles::seconds_of_day(event_ms);
        let total = self.assets.len();
        let mut flat = 0usize;
        while flat < total {
            let site_index = self.assets[flat].site_index;
            let mut end = flat;
            while end < total && self.assets[end].site_index == site_index {
                end += 1;
            }
            let convention = self.scenario.sites[site_index].reactive_convention;

            // Pass 1: compute every non-meter asset's tick values (RNG and
            // SOC advance here, in flat order, whether or not a fault later
            // drops the frame), summing net power for the meter.
            let mut site_sum = 0.0;
            let mut pending: Vec<Option<profiles::AssetTick>> = Vec::with_capacity(end - flat);
            for i in flat..end {
                if matches!(self.assets[i].spec.params, BehaviourParams::Meter(_)) {
                    pending.push(None);
                    continue;
                }
                let asset = &mut self.assets[i];
                let values = profiles::asset_tick(
                    &asset.spec,
                    &mut asset.soc,
                    &mut asset.rng,
                    cadence_s,
                    sod,
                    site_sum,
                );
                site_sum += values.active_power_mw;
                pending.push(Some(values));
            }
            // Pass 2: the site meter emits the aggregated profile sum
            // (losses = 0, non-modelled load = 0 — a consistency fixture).
            for (slot, i) in pending.iter_mut().zip(flat..end) {
                if slot.is_none() {
                    let q = match &self.assets[i].spec.params {
                        BehaviourParams::Meter(p) => p.q_constant_mvar,
                        _ => unreachable!("only the meter takes an empty slot"),
                    };
                    *slot = Some(profiles::meter_tick(site_sum, q));
                }
            }
            // Pass 3: dispose each frame in listed order.
            for (i, slot) in (flat..end).zip(pending.into_iter()) {
                let values = slot.expect("every asset slot is filled by passes 1–2");
                self.dispose_frame(i, event_ms, tick, values, convention);
            }
            flat = end;
        }
    }

    /// Builds the wire samples for one asset tick.
    fn build_samples(
        values: &profiles::AssetTick,
        convention: flexcore_contracts::units::ReactiveSignConvention,
    ) -> Vec<WireSample> {
        use quantity_names as q;
        let mut samples = Vec::new();
        samples.push(WireSample {
            quantity: q::ACTIVE_POWER,
            value: WireValue::Number(values.active_power_mw),
            reactive_sign_convention: None,
        });
        if let Some(mvar) = values.reactive_power_mvar {
            samples.push(WireSample {
                quantity: q::REACTIVE_POWER,
                value: WireValue::Number(mvar),
                reactive_sign_convention: Some(convention_name(convention)),
            });
        }
        if let Some(soc) = values.soc {
            samples.push(WireSample {
                quantity: q::SOC,
                value: WireValue::Number(soc),
                reactive_sign_convention: None,
            });
        }
        if let Some(energy) = values.energy_mwh {
            samples.push(WireSample {
                quantity: q::ENERGY,
                value: WireValue::Number(energy),
                reactive_sign_convention: None,
            });
        }
        if let Some(flag) = values.availability {
            samples.push(WireSample {
                quantity: q::AVAILABILITY,
                value: WireValue::Flag(flag),
                reactive_sign_convention: None,
            });
        }
        if let Some(flag) = values.connection_state {
            samples.push(WireSample {
                quantity: q::CONNECTION_STATE,
                value: WireValue::Flag(flag),
                reactive_sign_convention: None,
            });
        }
        if let Some(mode) = &values.operating_mode {
            samples.push(WireSample {
                quantity: q::OPERATING_MODE,
                value: WireValue::State(mode.clone()),
                reactive_sign_convention: None,
            });
        }
        if let Some(health) = &values.health {
            samples.push(WireSample {
                quantity: q::HEALTH,
                value: WireValue::State(health.clone()),
                reactive_sign_convention: None,
            });
        }
        if let Some(baseline) = values.baseline_reference_mw {
            samples.push(WireSample {
                quantity: q::BASELINE_REFERENCE,
                value: WireValue::Number(baseline),
                reactive_sign_convention: None,
            });
        }
        samples
    }

    /// Applies the fault plan to one generated asset tick and disposes the
    /// resulting frame(s). Faults apply in listed order.
    fn dispose_frame(
        &mut self,
        flat: usize,
        event_ms: i64,
        tick: u64,
        values: profiles::AssetTick,
        convention: flexcore_contracts::units::ReactiveSignConvention,
    ) {
        // Copy out everything the fault evaluation needs, so no borrow of
        // the generator state survives into the disposition.
        let (asset_key, rated, sequence) = {
            let asset = &self.assets[flat];
            (
                asset.asset_key.clone(),
                asset.spec.rated_power_mw,
                asset.next_sequence,
            )
        };
        let active: Vec<ActiveFault> = self.assets[flat]
            .faults
            .iter()
            .filter(|f| f.from_ms <= event_ms && event_ms < f.until_ms)
            .map(|f| ActiveFault {
                kind: f.kind,
                delay_ticks: f.delay_ticks,
                hold_ticks: f.hold_ticks,
                factor: f.factor,
            })
            .collect();

        // drop → no frame, no sequence consumed: the absence stays a pure
        // event-time gap (the pipeline's detector synthesizes MISSING).
        if active.iter().any(|f| f.kind == FaultKind::Drop) {
            return;
        }

        let mut samples = Self::build_samples(&values, convention);
        let mut code: Option<&'static str> = None;
        for fault in &active {
            match fault.kind {
                FaultKind::OutOfRange => {
                    // Scale the active power past the rated limit,
                    // preserving the profile's sign direction.
                    for sample in &mut samples {
                        if sample.quantity == quantity_names::ACTIVE_POWER {
                            if let WireValue::Number(p) = sample.value {
                                let sign = if p < 0.0 { -1.0 } else { 1.0 };
                                sample.value = WireValue::Number(sign * fault.factor * rated);
                            }
                        }
                    }
                    code = Some(quality_codes::OUT_OF_RANGE);
                }
                FaultKind::SourceInvalid => {
                    code = Some(quality_codes::SOURCE_INVALID);
                }
                _ => {}
            }
        }

        self.assets[flat].next_sequence = sequence + 1;
        let frame = WireFrame {
            format: FRAME_FORMAT,
            version: FRAME_VERSION,
            asset_key,
            event_time_unix_ms: event_ms,
            released_at_unix_ms: event_ms, // updated below when held
            source_sequence: sequence,
            source_quality_code: code,
            samples,
        };

        // Disposition: holds re-release later; duplicates emit immediately.
        let mut held_by: Option<u64> = None;
        let mut queue_clones: Vec<WireFrame> = Vec::new();
        for fault in &active {
            match fault.kind {
                FaultKind::Delay => held_by = Some(tick + fault.delay_ticks),
                FaultKind::OutOfOrder => held_by = Some(tick + fault.hold_ticks),
                FaultKind::Duplicate => queue_clones.push(frame.clone()),
                FaultKind::ConflictingDuplicate => {
                    let mut conflicting = frame.clone();
                    for sample in &mut conflicting.samples {
                        if sample.quantity == quantity_names::ACTIVE_POWER {
                            if let WireValue::Number(p) = sample.value {
                                // A deterministically different value under
                                // the same identity: the dedup conflict.
                                sample.value = WireValue::Number(if p == 0.0 {
                                    rated * 0.5
                                } else {
                                    p * 1.5
                                });
                            }
                        }
                    }
                    queue_clones.push(conflicting);
                }
                _ => {}
            }
        }
        for clone in queue_clones {
            self.state.queue.push_back(clone);
        }
        match held_by {
            Some(release_tick) => {
                let mut held = frame;
                held.released_at_unix_ms = self.scenario.tick_event_ms(release_tick);
                self.state.held.push(HeldFrame {
                    release_tick,
                    generated_tick: tick,
                    frame: held,
                });
            }
            None => self.state.queue.push_back(frame),
        }
    }

    /// Moves every held frame due at or before `tick` into the queue, in a
    /// deterministic order (release tick, generation tick, event time).
    fn release_due(&mut self, tick: u64) {
        let mut due: Vec<HeldFrame> = Vec::new();
        self.state.held.retain(|held| {
            if held.release_tick <= tick {
                due.push(HeldFrame {
                    release_tick: held.release_tick,
                    generated_tick: held.generated_tick,
                    frame: held.frame.clone(),
                });
                false
            } else {
                true
            }
        });
        due.sort_by_key(|h| (h.release_tick, h.generated_tick, h.frame.event_time_unix_ms));
        for held in due {
            self.state.queue.push_back(held.frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::load_scenario;

    pub(super) fn scenario_json() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/synthetic-portfolio/scenario-v1.json"
        ))
    }

    fn enabled_faults(json: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        value["faults"]["enabled"] = serde_json::Value::Bool(true);
        value.to_string()
    }

    fn drain(engine: &mut Engine) -> Vec<WireFrame> {
        std::iter::from_fn(|| engine.next_frame()).collect()
    }

    #[test]
    fn two_engines_produce_identical_frame_streams() {
        let scenario = load_scenario(scenario_json()).unwrap();
        let mut a = Engine::new(scenario);
        let mut b = Engine::new(load_scenario(scenario_json()).unwrap());
        let frames_a = drain(&mut a);
        let frames_b = drain(&mut b);
        assert_eq!(frames_a, frames_b);
        // 10 assets x 1440 ticks.
        assert_eq!(frames_a.len(), 14_400);
    }

    #[test]
    fn sequences_are_gap_free_and_monotonic_per_asset_in_a_fault_free_run() {
        let mut engine = Engine::new(load_scenario(scenario_json()).unwrap());
        let mut last: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for frame in drain(&mut engine) {
            let entry = last.entry(frame.asset_key.clone()).or_insert(0);
            assert_eq!(
                frame.source_sequence,
                *entry + 1,
                "sequences must be gap-free from 1 per asset"
            );
            *entry = frame.source_sequence;
        }
        assert_eq!(last.len(), 10);
        assert!(last.values().all(|&s| s == 1440));
    }

    #[test]
    fn drop_removes_frames_without_disturbing_the_sequence_or_other_values() {
        let mut clean = Engine::new(load_scenario(scenario_json()).unwrap());
        let mut faulted = Engine::new(load_scenario(&enabled_faults(scenario_json())).unwrap());
        let clean: Vec<WireFrame> = drain(&mut clean);
        let faulted: Vec<WireFrame> = drain(&mut faulted);
        // site-north-bess-1 is the drop target (02:00–02:10, 10 ticks).
        let dropped: Vec<&WireFrame> = clean
            .iter()
            .filter(|f| f.asset_key == "site-north-bess-1")
            .collect();
        let kept: Vec<&WireFrame> = faulted
            .iter()
            .filter(|f| f.asset_key == "site-north-bess-1")
            .collect();
        assert_eq!(dropped.len() - kept.len(), 10);
        // Sequence numbering is untouched by the drop.
        assert_eq!(
            dropped.last().unwrap().source_sequence,
            kept.last().unwrap().source_sequence
        );
        // Every surviving frame of the asset is value-identical to the
        // clean run (faults act on frames, not generator state).
        let kept_by_seq: std::collections::HashMap<u64, &WireFrame> =
            kept.iter().map(|f| (f.source_sequence, *f)).collect();
        for frame in &dropped {
            if let Some(k) = kept_by_seq.get(&frame.source_sequence) {
                assert_eq!(frame.samples, k.samples);
                assert_eq!(frame.event_time_unix_ms, k.event_time_unix_ms);
            }
        }
    }

    #[test]
    fn duplicates_and_conflicts_share_identity_and_conflicts_differ_in_value() {
        let mut engine = Engine::new(load_scenario(&enabled_faults(scenario_json())).unwrap());
        let frames = drain(&mut engine);
        let start = load_scenario(scenario_json()).unwrap().start_ms;
        // duplicate: site-north-evse-1, 06:00–06:10.
        let dup_window = (start + 6 * 3600 * 1000, start + 6 * 3600 * 1000 + 60 * 1000);
        let dup: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| {
                f.asset_key == "site-north-evse-1"
                    && f.event_time_unix_ms >= dup_window.0
                    && f.event_time_unix_ms < dup_window.1
            })
            .collect();
        assert!(!dup.is_empty());
        assert_eq!(dup.len() % 2, 0, "duplicates arrive in identical pairs");
        for pair in dup.chunks(2) {
            assert_eq!(pair[0].source_sequence, pair[1].source_sequence);
            assert_eq!(pair[0].samples, pair[1].samples);
        }
        // conflicting_duplicate: site-south-load-1, 08:00–08:10.
        let conflict_window = (start + 8 * 3600 * 1000, start + 8 * 3600 * 1000 + 60 * 1000);
        let conflicts: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| {
                f.asset_key == "site-south-load-1"
                    && f.event_time_unix_ms >= conflict_window.0
                    && f.event_time_unix_ms < conflict_window.1
            })
            .collect();
        assert!(!conflicts.is_empty());
        assert_eq!(conflicts.len() % 2, 0);
        let active = |f: &WireFrame| {
            f.samples
                .iter()
                .find(|s| s.quantity == quantity_names::ACTIVE_POWER)
                .unwrap()
                .value
                .clone()
        };
        for pair in conflicts.chunks(2) {
            assert_eq!(pair[0].source_sequence, pair[1].source_sequence);
            assert_eq!(pair[0].event_time_unix_ms, pair[1].event_time_unix_ms);
            assert_ne!(
                active(pair[0]),
                active(pair[1]),
                "a conflicting duplicate must differ in value under the same identity"
            );
        }
    }

    #[test]
    fn held_frames_release_after_a_later_tick_and_carry_their_later_release_time() {
        let mut engine = Engine::new(load_scenario(&enabled_faults(scenario_json())).unwrap());
        let frames = drain(&mut engine);
        // out_of_order: site-south-pv-1, 07:00–07:10, hold_ticks 1: its
        // arrival order must show a decrease.
        let pv: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| f.asset_key == "site-south-pv-1")
            .collect();
        let mut saw_decrease = false;
        for pair in pv.windows(2) {
            if pair[1].event_time_unix_ms < pair[0].event_time_unix_ms {
                saw_decrease = true;
                // The held frame was released one cadence tick after its
                // event tick — later than the frame before it arrived.
                assert!(pair[1].released_at_unix_ms > pair[1].event_time_unix_ms);
            }
        }
        assert!(
            saw_decrease,
            "the out_of_order fault must show an arrival-order violation"
        );
        // delay: site-south-bess-1, 03:00–03:10, 120 s → released 2 ticks late.
        let bess: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| f.asset_key == "site-south-bess-1")
            .collect();
        let delayed: Vec<&&WireFrame> = bess
            .iter()
            .filter(|f| f.released_at_unix_ms > f.event_time_unix_ms)
            .collect();
        assert!(!delayed.is_empty());
        for frame in delayed {
            assert_eq!(frame.released_at_unix_ms - frame.event_time_unix_ms, 120_000);
        }
    }

    #[test]
    fn out_of_range_and_source_invalid_frames_carry_their_codes() {
        let mut engine = Engine::new(load_scenario(&enabled_faults(scenario_json())).unwrap());
        let frames = drain(&mut engine);
        let start = load_scenario(scenario_json()).unwrap().start_ms;
        // out_of_range: site-north-pv-1, 10:00–10:10.
        let oor: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| {
                f.asset_key == "site-north-pv-1"
                    && f.event_time_unix_ms >= start + 10 * 3600 * 1000
                    && f.event_time_unix_ms < start + 10 * 3600 * 1000 + 60 * 1000
            })
            .collect();
        assert!(!oor.is_empty());
        for frame in &oor {
            assert_eq!(frame.source_quality_code, Some(quality_codes::OUT_OF_RANGE));
            let p = match &frame
                .samples
                .iter()
                .find(|s| s.quantity == quantity_names::ACTIVE_POWER)
                .unwrap()
                .value
            {
                WireValue::Number(p) => *p,
                _ => panic!("active power must be numeric"),
            };
            assert!(p.abs() > 1.5, "the faulted value must exceed the rated power");
        }
        // source_invalid: site-north-load-1, 05:00–05:10.
        let invalid: Vec<&WireFrame> = frames
            .iter()
            .filter(|f| {
                f.asset_key == "site-north-load-1"
                    && f.event_time_unix_ms >= start + 5 * 3600 * 1000
                    && f.event_time_unix_ms < start + 5 * 3600 * 1000 + 60 * 1000
            })
            .collect();
        assert!(!invalid.is_empty());
        assert!(invalid
            .iter()
            .all(|f| f.source_quality_code == Some(quality_codes::SOURCE_INVALID)));
    }
}
