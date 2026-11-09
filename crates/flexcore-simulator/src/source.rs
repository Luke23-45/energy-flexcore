// P2.14/P2.15 — The `TelemetrySource` implementation: the simulator as the
// Phase 2 telemetry provider.
//
// What this module adds on top of the engine:
//
// - **The port, and only the port.** This crate implements
//   `TelemetrySource` (P2.11; report §9.1) and nothing else about the
//   ingestion path — no database, no broker, no pipeline internals
//   (`cargo tree` shows flexcore-contracts, flexcore-domain,
//   flexcore-ports, serde, serde_json, time and nothing infrastructure).
// - **Run control**: `start` / `pause` / `resume` / `stop` over the
//   scenario clock, plus `pull_frame` — the adapter-side frame pump. The
//   port is synchronous and pull-based: `pull_frame` returns at most one
//   frame (one asset, one tick), and nothing is generated unless pulled,
//   which is the bounded, backpressure-aware emission rule (P2.14 step 5):
//   a consumer that stops pulling stops the stream, and a paused or
//   stopped source emits nothing. The port's `decode` stays a real decode:
//   it parses the opaque frame bytes into the complete, canonical record
//   set the frame carries.
// - **The provider rule** (P2.11 step 4): the declaration is structural —
//   source ID `sim://<scenario_id>` (never the reserved `flexcore:`
//   namespace; D-T6), cadence = the scenario default, the quality-state
//   mapping (`GOOD`→`GOOD`, `OUT_OF_RANGE`→`SUSPECT`,
//   `SOURCE_INVALID`→`BAD`; the original code travels as
//   `source_quality_code`), and `ClockQuality::Synchronized` (the scenario
//   clock is the simulation's time reference by construction; recorded,
//   never assumed).
// - **Tenant scoping is structural** (P0.16): `subscribe` takes the
//   tenant-scoped `TelemetryQuery` (the tenant comes only from its
//   `TenantContext`) and records carry that tenant. An asset ID outside
//   the registered mapping is rejected with `InvalidQuery`.
// - **Asset identity**: the scenario speaks stable asset keys; the mapping
//   key → registry `AssetId` happens at registration (P2.25 step 2), so it
//   is supplied to the constructor. Unmapped scenario assets are simply
//   not subscribable (the site meter's aggregation is unaffected — it sums
//   profile values, not identities).
//
// `ingest_time` is stamped from the scenario clock at the frame's release
// tick — the only clock a deterministic simulator may read. It therefore
// satisfies `ingest_time >= event_time` by construction and stays outside
// the replay comparison (P2.13 §3; P2.24). All fault arithmetic is
// scenario-clock arithmetic; no path here reads the wall clock (P2.15).

use crate::engine::{
    quality_codes, quantity_names, Engine, FRAME_FORMAT, FRAME_VERSION, WireFrame,
};
use crate::scenario::{load_scenario, Scenario, ScenarioError};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    SourceStateMapping, TelemetryQuery, TelemetrySource, TelemetrySourceError,
    TelemetrySourceHealth, TelemetrySubscription,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// The simulator's run status, reported through [`SimulatorTelemetrySource::status`]
/// and [`TelemetrySource::health`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// Created but not started: no emission.
    Idle,
    /// Emitting on pull, from the current scenario-clock position.
    Running,
    /// Paused at a tick boundary: pulls return no frames until resumed.
    Paused,
    /// Stopped for good: no further emission (the run cannot be restarted;
    /// a fresh source replays deterministically).
    Stopped,
}

/// What one subscription registered interest in. The tenant comes only
/// from the query's `TenantContext` (P0.16 rules 1 and 5).
struct SubscriptionInfo {
    tenant_id: TenantId,
    assets: HashSet<AssetId>,
    quantities: HashSet<flexcore_ports::telemetry_source::TelemetryQuantity>,
}

/// The mutable source state behind one mutex (the port is sync; the
/// simulator is a single-threaded-friendly pull source).
struct SourceState {
    engine: Engine,
    status: RunStatus,
    subscriptions: HashMap<u64, SubscriptionInfo>,
    next_subscription: u64,
    /// The registration mapping: scenario asset key → registry asset ID.
    asset_ids: HashMap<String, AssetId>,
}

/// The deterministic synthetic telemetry simulator — the Phase 2
/// `TelemetrySource` provider (P2.14) with scenario-driven fault injection
/// (P2.15). See the crate README for the determinism, RNG, and fault-scope
/// records.
pub struct SimulatorTelemetrySource {
    declaration: ProviderDeclaration,
    state: Mutex<SourceState>,
}

impl SimulatorTelemetrySource {
    /// Loads the scenario (all-or-nothing validation) and builds the source
    /// with the registration mapping `asset_key → AssetId`. Every mapping
    /// key must exist in the scenario; keys must be unique. The engine
    /// starts in [`RunStatus::Idle`] — call [`start`](Self::start).
    pub fn new(
        scenario_json: &str,
        asset_ids: Vec<(String, AssetId)>,
    ) -> Result<Self, ScenarioError> {
        Self::from_scenario(load_scenario(scenario_json)?, asset_ids)
    }

    /// Builds the source from an already-loaded [`Scenario`] plus the
    /// registration mapping. Useful for tests and tooling that manipulate
    /// the scenario configuration (e.g. enabling the fault plan) without
    /// re-reading the file.
    pub fn from_scenario(
        scenario: Scenario,
        asset_ids: Vec<(String, AssetId)>,
    ) -> Result<Self, ScenarioError> {
        if asset_ids.is_empty() {
            return Err(ScenarioError::new(
                "at least one asset key → AssetId mapping is required",
            ));
        }
        let valid_keys: HashSet<&str> = scenario
            .sites
            .iter()
            .flat_map(|s| s.assets.iter().map(|a| a.asset_key.as_str()))
            .collect();
        let mut mapping: HashMap<String, AssetId> = HashMap::new();
        for (key, id) in asset_ids {
            if !valid_keys.contains(key.as_str()) {
                return Err(ScenarioError::new(format!(
                    "asset mapping names '{key}', which is not an asset of scenario '{}'",
                    scenario.meta.scenario_id
                )));
            }
            if mapping.insert(key.clone(), id).is_some() {
                return Err(ScenarioError::new(format!(
                    "asset mapping lists '{key}' twice"
                )));
            }
        }
        // The provider rule (P2.11 step 4), declared structurally. Neither
        // construction can fail: the source ID is non-empty and outside the
        // reserved namespace, and the mapping codes are distinct non-empty.
        let source_id = format!("sim://{}", scenario.meta.scenario_id);
        let declaration = ProviderDeclaration::new(
            source_id,
            Duration::from_secs(scenario.cadence_seconds as u64),
            QualityMappingDeclaration::new(vec![
                SourceStateMapping {
                    source_code: "GOOD".to_string(),
                    canonical: TelemetryQuality::Good,
                },
                SourceStateMapping {
                    source_code: quality_codes::OUT_OF_RANGE.to_string(),
                    canonical: TelemetryQuality::Suspect,
                },
                SourceStateMapping {
                    source_code: quality_codes::SOURCE_INVALID.to_string(),
                    canonical: TelemetryQuality::Bad,
                },
            ])
            .expect("the simulator's quality mapping is statically valid"),
            ClockQuality::Synchronized,
        )
        .expect("the simulator's source ID is statically valid");
        let engine = Engine::new(scenario);
        Ok(Self {
            declaration,
            state: Mutex::new(SourceState {
                engine,
                status: RunStatus::Idle,
                subscriptions: HashMap::new(),
                next_subscription: 1,
                asset_ids: mapping,
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, SourceState> {
        // A poisoned mutex means a panic mid-emission; the state is still
        // structurally valid, so continue rather than cascade the poison.
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // --- Run control (P2.14 step 5) -------------------------------------

    /// Starts (or resumes) emission from the current scenario-clock
    /// position.
    pub fn start(&self) {
        self.lock().status = RunStatus::Running;
    }

    /// Pauses emission at the current tick boundary. Pulls return no
    /// frames until [`resume`](Self::resume); no state is lost.
    pub fn pause(&self) {
        let mut st = self.lock();
        if st.status == RunStatus::Running {
            st.status = RunStatus::Paused;
        }
    }

    /// Resumes a paused simulation (a stopped simulation cannot be
    /// restarted — build a fresh source to replay).
    pub fn resume(&self) {
        let mut st = self.lock();
        if st.status == RunStatus::Paused {
            st.status = RunStatus::Running;
        }
    }

    /// Stops the simulation for good. Pulls return no frames afterwards.
    pub fn stop(&self) {
        self.lock().status = RunStatus::Stopped;
    }

    /// The current run status.
    pub fn status(&self) -> RunStatus {
        self.lock().status
    }

    /// The scenario clock of the last generated tick, or `None` before the
    /// first tick is generated.
    pub fn scenario_clock(&self) -> Option<EventTime> {
        let st = self.lock();
        if st.status == RunStatus::Idle {
            return None;
        }
        EventTime::from_unix_millis(st.engine.scenario_clock_ms()).ok()
    }

    /// Pulls the next raw frame for a subscription: at most one frame per
    /// call (the bounded-emission unit), generated only on demand (the
    /// backpressure mechanism), and `Ok(None)` when paused, stopped, not
    /// yet started, or past the horizon. Frames outside the subscription's
    /// asset set are consumed and skipped — the simulator is a single
    /// deterministic stream; the recorded usage is one pipeline
    /// subscription covering the portfolio (P2.10), and a subscription must
    /// cover the assets it decodes.
    pub fn pull_frame(
        &self,
        subscription: &TelemetrySubscription,
    ) -> Result<Option<Vec<u8>>, TelemetrySourceError> {
        let mut st = self.lock();
        let Some(info) = st.subscriptions.get(&subscription.handle()) else {
            return Err(TelemetrySourceError::NotSubscribed);
        };
        loop {
            if st.status != RunStatus::Running {
                return Ok(None);
            }
            let Some(frame) = st.engine.next_frame() else {
                return Ok(None);
            };
            let asset_subscribed = st
                .asset_ids
                .get(&frame.asset_key)
                .is_some_and(|id| info.assets.contains(id));
            let quantity_subscribed = frame.samples.iter().any(|s| {
                quantity_from_name(s.quantity)
                    .is_some_and(|q| info.quantities.contains(&q))
            });
            if asset_subscribed && quantity_subscribed {
                let bytes = serde_json::to_vec(&frame)
                    .expect("frame serialization over owned data cannot fail");
                return Ok(Some(bytes));
            }
            // Not this subscription's frame: consumed and skipped (the
            // stream is global; nothing is buffered for another reader).
        }
    }
}

/// Maps a wire quantity name onto the frozen `TelemetryQuantity` vocabulary
/// (P2.04 §4). Unknown names are a decode failure, never a silent skip.
fn quantity_from_name(
    name: &str,
) -> Option<flexcore_ports::telemetry_source::TelemetryQuantity> {
    use flexcore_ports::telemetry_source::TelemetryQuantity as Q;
    match name {
        quantity_names::ACTIVE_POWER => Some(Q::ActivePower),
        quantity_names::REACTIVE_POWER => Some(Q::ReactivePower),
        quantity_names::SOC => Some(Q::Soc),
        quantity_names::ENERGY => Some(Q::Energy),
        quantity_names::AVAILABILITY => Some(Q::Availability),
        quantity_names::OPERATING_MODE => Some(Q::OperatingMode),
        quantity_names::HEALTH => Some(Q::Health),
        quantity_names::CONNECTION_STATE => Some(Q::ConnectionState),
        quantity_names::BASELINE_REFERENCE => Some(Q::BaselineReference),
        _ => None,
    }
}

fn decode_failed(reason: impl Into<String>) -> TelemetrySourceError {
    TelemetrySourceError::DecodeFailed {
        reason: reason.into(),
    }
}

/// Builds the canonical `QuantityValue` for one sample, per the
/// quantity/value pairing rule (P2.04 §3–§4). The reactive convention is a
/// required part of the reactive sample — the sign is never interpreted
/// without it (P0.07 §3).
fn build_value(
    quantity: flexcore_ports::telemetry_source::TelemetryQuantity,
    raw: &serde_json::Value,
    convention_raw: Option<&serde_json::Value>,
) -> Result<flexcore_ports::telemetry_source::QuantityValue, TelemetrySourceError> {
    use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity as Q};
    let number = || {
        raw.as_f64()
            .ok_or_else(|| decode_failed("this quantity's value must be a JSON number"))
    };
    let unit_error = |e: flexcore_contracts::units::UnitsError| {
        decode_failed(format!("the frame carried a non-canonical value: {e}"))
    };
    let value = match quantity {
        Q::ActivePower | Q::BaselineReference => {
            QuantityValue::Power(ActivePowerMW::new(number()?).map_err(unit_error)?)
        }
        Q::ReactivePower => {
            let name = convention_raw.and_then(|v| v.as_str()).ok_or_else(|| {
                decode_failed(
                    "a reactive-power sample must carry its declared sign convention \
                     (the sign is never interpreted without it, P0.07 §3)",
                )
            })?;
            let convention = match name {
                "INJECTION_POSITIVE" => ReactiveSignConvention::InjectionPositive,
                "CONSUMPTION_POSITIVE" => ReactiveSignConvention::ConsumptionPositive,
                other => {
                    return Err(decode_failed(format!(
                        "unknown reactive sign convention '{other}'"
                    )))
                }
            };
            QuantityValue::Reactive {
                power: ReactivePowerMVAr::new(number()?).map_err(unit_error)?,
                convention,
            }
        }
        Q::Soc => QuantityValue::Fraction(StateOfCharge::new(number()?).map_err(unit_error)?),
        Q::Energy => QuantityValue::Energy(EnergyMWh::new(number()?).map_err(unit_error)?),
        Q::Availability | Q::ConnectionState => QuantityValue::Flag(raw.as_bool().ok_or_else(
            || decode_failed("this quantity's value must be a JSON boolean"),
        )?),
        Q::OperatingMode | Q::Health => QuantityValue::State(
            raw.as_str()
                .ok_or_else(|| {
                    decode_failed("this quantity's value must be a JSON string")
                })?
                .to_string(),
        ),
    };
    Ok(value)
}

impl TelemetrySource for SimulatorTelemetrySource {
    fn provider_declaration(&self) -> &ProviderDeclaration {
        &self.declaration
    }

    fn subscribe(
        &self,
        query: &TelemetryQuery,
    ) -> Result<TelemetrySubscription, TelemetrySourceError> {
        let mut st = self.lock();
        for asset in query.assets() {
            if !st.asset_ids.values().any(|id| id == asset) {
                return Err(TelemetrySourceError::InvalidQuery {
                    reason: format!(
                        "asset {asset} is not part of this simulator's registered asset set"
                    ),
                });
            }
        }
        let handle = st.next_subscription;
        st.next_subscription += 1;
        st.subscriptions.insert(
            handle,
            SubscriptionInfo {
                tenant_id: query.tenant_id(),
                assets: query.assets().iter().copied().collect(),
                quantities: query.quantities().iter().copied().collect(),
            },
        );
        Ok(TelemetrySubscription::from_handle(handle))
    }

    fn decode(
        &self,
        subscription: &TelemetrySubscription,
        payload: &[u8],
    ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
        let st = self.lock();
        let Some(info) = st.subscriptions.get(&subscription.handle()) else {
            return Err(TelemetrySourceError::NotSubscribed);
        };
        let parsed: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| decode_failed(format!("the frame is not valid JSON: {e}")))?;
        let obj = parsed
            .as_object()
            .ok_or_else(|| decode_failed("a frame must be a JSON object"))?;
        if obj.get("format").and_then(|v| v.as_str()) != Some(FRAME_FORMAT) {
            return Err(decode_failed(format!(
                "unknown frame format (expected '{FRAME_FORMAT}')"
            )));
        }
        if obj.get("version").and_then(|v| v.as_u64()) != Some(u64::from(FRAME_VERSION)) {
            return Err(decode_failed(format!(
                "unknown frame version (expected {FRAME_VERSION})"
            )));
        }
        let asset_key = obj
            .get("asset_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| decode_failed("the frame must name its asset"))?;
        let asset_id = st.asset_ids.get(asset_key).copied().ok_or_else(|| {
            decode_failed(format!(
                "the frame references asset '{asset_key}', which is not part of this provider"
            ))
        })?;
        if !info.assets.contains(&asset_id) {
            return Err(decode_failed(
                "the frame's asset is not part of this subscription",
            ));
        }
        let event_ms = obj
            .get("event_time_unix_ms")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| decode_failed("the frame must carry event_time_unix_ms"))?;
        let released_ms = obj
            .get("released_at_unix_ms")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| decode_failed("the frame must carry released_at_unix_ms"))?;
        let sequence = obj
            .get("source_sequence")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| decode_failed("the frame must carry source_sequence"))?;
        let code = obj.get("source_quality_code").and_then(|v| v.as_str());
        // The declared quality mapping (P2.11 step 4) is the only path from
        // the source code to the canonical state; an undeclared code maps
        // to GOOD with the code still retained — the non-silencing rule
        // keeps the original as `source_quality_code` either way.
        let quality = code
            .and_then(|c| self.declaration.quality_mapping().canonical_for(c))
            .unwrap_or(TelemetryQuality::Good);
        let event_time = EventTime::from_unix_millis(event_ms)
            .map_err(|e| decode_failed(format!("the frame's event_time is not canonical: {e}")))?;
        let ingest_time = IngestTime::from_unix_millis(released_ms).map_err(|e| {
            decode_failed(format!("the frame's ingest_time is not canonical: {e}"))
        })?;
        let samples = obj
            .get("samples")
            .and_then(|v| v.as_array())
            .ok_or_else(|| decode_failed("the frame must carry a samples array"))?;
        let mut records = Vec::with_capacity(samples.len());
        for sample in samples {
            let sample = sample
                .as_object()
                .ok_or_else(|| decode_failed("each sample must be a JSON object"))?;
            let name = sample
                .get("quantity")
                .and_then(|v| v.as_str())
                .ok_or_else(|| decode_failed("each sample must name its quantity"))?;
            let quantity = quantity_from_name(name)
                .ok_or_else(|| decode_failed(format!("unknown wire quantity '{name}'")))?;
            if !info.quantities.contains(&quantity) {
                continue;
            }
            let raw = sample
                .get("value")
                .ok_or_else(|| decode_failed("each sample must carry a value"))?;
            let value = build_value(quantity, raw, sample.get("reactive_sign_convention"))?;
            let record = NormalizedTelemetry::new(NormalizedTelemetryParts {
                tenant_id: info.tenant_id,
                asset_id,
                quantity,
                value: Some(value),
                event_time,
                ingest_time,
                source_sequence: sequence,
                source_id: self.declaration.source_id().to_string(),
                quality,
                source_quality_code: code.map(str::to_string),
                clock_quality: self.declaration.clock_quality(),
            })
            .map_err(|e| {
                decode_failed(format!("the frame produced a non-canonical record: {e}"))
            })?;
            records.push(record);
        }
        if records.is_empty() {
            // The no-empty-success rule (P2.11): a decoded frame carries at
            // least one record for its subscription. `pull_frame` never
            // hands out a frame that would violate this.
            return Err(decode_failed(
                "the frame carries no records for this subscription",
            ));
        }
        Ok(records)
    }

    fn health(&self) -> TelemetrySourceHealth {
        match self.lock().status {
            RunStatus::Running => TelemetrySourceHealth::Healthy,
            RunStatus::Paused => TelemetrySourceHealth::Degraded {
                reason: "the simulation is paused; emission resumes on demand".to_string(),
            },
            RunStatus::Idle => TelemetrySourceHealth::Unavailable {
                reason: "the simulation has not been started".to_string(),
            },
            RunStatus::Stopped => TelemetrySourceHealth::Unavailable {
                reason: "the simulation has been stopped".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::WireValue;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::quality_rules::{staleness, FreshnessLimit};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
    use flexcore_ports::telemetry_source::{NormalizedTelemetry, QuantityValue, TelemetryQuantity};
    use std::collections::BTreeMap;

    const SCENARIO_V1: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/synthetic-portfolio/scenario-v1.json"
    ));

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    /// The ten stable asset keys of the exit-gate scenario, in scenario
    /// order, with fixed test UUIDs (the real key → ID mapping happens at
    /// registration, P2.25 step 2).
    const ASSET_KEYS: [&str; 10] = [
        "site-north-bess-1",
        "site-north-pv-1",
        "site-north-load-1",
        "site-north-evse-1",
        "site-north-meter-1",
        "site-south-bess-1",
        "site-south-pv-1",
        "site-south-load-1",
        "site-south-evse-1",
        "site-south-meter-1",
    ];

    fn asset_mapping() -> Vec<(String, AssetId)> {
        ASSET_KEYS
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let uuid = format!("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a{:02x}", 0x8c + i);
                (key.to_string(), AssetId::new(FlexUuid::parse(&uuid).unwrap()))
            })
            .collect()
    }

    fn asset_id(key: &str) -> AssetId {
        let index = ASSET_KEYS.iter().position(|k| *k == key).unwrap();
        let uuid = format!("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a{:02x}", 0x8c + index);
        AssetId::new(FlexUuid::parse(&uuid).unwrap())
    }

    fn tenant_context() -> flexcore_domain::tenant_context::TenantContext {
        flexcore_domain::tenant_context::TenantContext::from_authenticated_claims(
            AuthenticatedClaims::from_verified(VerifiedClaims {
                tenant_id: TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap()),
                subject: "user:simulator-test".to_string(),
            }),
        )
    }

    fn all_quantities() -> Vec<TelemetryQuantity> {
        vec![
            TelemetryQuantity::ActivePower,
            TelemetryQuantity::ReactivePower,
            TelemetryQuantity::Soc,
            TelemetryQuantity::Energy,
            TelemetryQuantity::Availability,
            TelemetryQuantity::OperatingMode,
            TelemetryQuantity::Health,
            TelemetryQuantity::ConnectionState,
            TelemetryQuantity::BaselineReference,
        ]
    }

    fn make_source(scenario_json: &str) -> SimulatorTelemetrySource {
        let source = SimulatorTelemetrySource::new(scenario_json, asset_mapping())
            .expect("the exit-gate scenario plus the test mapping must build");
        source.start();
        source
    }

    fn subscribe_all(source: &SimulatorTelemetrySource) -> TelemetrySubscription {
        let assets: Vec<AssetId> = ASSET_KEYS.iter().map(|k| asset_id(k)).collect();
        source
            .subscribe(&TelemetryQuery::new(
                &tenant_context(),
                assets,
                EventTime::from_unix_millis(0).unwrap(),
                EventTime::from_unix_millis(i64::MAX / 2).unwrap(),
                all_quantities(),
            ))
            .unwrap()
    }

    /// Pulls and decodes the whole horizon, in arrival order.
    fn drain(source: &SimulatorTelemetrySource) -> Vec<NormalizedTelemetry> {
        let subscription = subscribe_all(source);
        let mut records = Vec::new();
        while let Some(frame) = source.pull_frame(&subscription).unwrap() {
            records.extend(source.decode(&subscription, &frame).unwrap());
        }
        records
    }

    /// The replay-comparison tuple (P2.13 §3 / P2.24): everything BUT
    /// `ingest_time`.
    fn replay_key(record: &NormalizedTelemetry) -> (AssetId, i128, TelemetryQuantity, QuantityValue, TelemetryQuality, u64) {
        (
            record.asset_id(),
            record.event_time().unix_millis(),
            record.quantity(),
            record.value().cloned().expect("measured records carry values"),
            record.quality(),
            record.source_sequence(),
        )
    }

    fn event_window_ms(hours: u64) -> (i128, i128) {
        let start = load_scenario(SCENARIO_V1).unwrap().start_ms as i128;
        let base = start + hours as i128 * 3600 * 1000;
        (base, base + 10 * 60 * 1000)
    }

    // --- P2.14 core tests ------------------------------------------------

    #[test]
    fn two_runs_are_identical_on_the_replay_fields() {
        let a = drain(&make_source(SCENARIO_V1));
        let b = drain(&make_source(SCENARIO_V1));
        assert_eq!(a.len(), b.len());
        assert!(!a.is_empty());
        for (ra, rb) in a.iter().zip(b.iter()) {
            assert_eq!(replay_key(ra), replay_key(rb));
        }
        // The full record set is 1440 ticks x 10 assets x their quantities.
        // Sanity-check the scale and that ingest_time is present but NOT
        // part of the comparison above.
        let per_asset: HashSet<u64> = a
            .iter()
            .filter(|r| r.asset_id() == asset_id("site-north-bess-1"))
            .map(|r| r.source_sequence())
            .collect();
        assert_eq!(per_asset.len(), 1440);
    }

    #[test]
    fn soc_stays_in_unit_range_and_power_within_rated_limits_over_24h() {
        let records = drain(&make_source(SCENARIO_V1));
        let mut rated: HashMap<AssetId, f64> = HashMap::new();
        for (key, mw) in [
            ("site-north-bess-1", 2.0),
            ("site-north-pv-1", 1.5),
            ("site-north-load-1", 0.8),
            ("site-north-evse-1", 0.05),
            ("site-north-meter-1", 3.0),
            ("site-south-bess-1", 1.0),
            ("site-south-pv-1", 2.5),
            ("site-south-load-1", 1.2),
            ("site-south-evse-1", 0.15),
            ("site-south-meter-1", 3.5),
        ] {
            rated.insert(asset_id(key), mw);
        }
        let mut soc_checks = 0;
        for record in &records {
            match (record.quantity(), record.value()) {
                (TelemetryQuantity::Soc, Some(QuantityValue::Fraction(soc))) => {
                    assert!(
                        (0.0..=1.0).contains(&soc.value()),
                        "SOC left [0,1]: {}",
                        soc.value()
                    );
                    soc_checks += 1;
                }
                (TelemetryQuantity::ActivePower, Some(QuantityValue::Power(p))) => {
                    let limit = rated[&record.asset_id()];
                    assert!(
                        p.value().abs() <= limit + 1e-9,
                        "P {} exceeded the rated {} MW",
                        p.value(),
                        limit
                    );
                }
                _ => {}
            }
        }
        // Two BESS + two EVSE carry SOC at every tick.
        assert_eq!(soc_checks, 4 * 1440);
    }

    #[test]
    fn sequences_are_monotonic_per_asset_in_arrival_order() {
        let records = drain(&make_source(SCENARIO_V1));
        let mut last: BTreeMap<AssetId, u64> = BTreeMap::new();
        for record in &records {
            let entry = last.entry(record.asset_id()).or_insert(0);
            assert!(
                record.source_sequence() > *entry,
                "sequence must strictly increase per asset in arrival order"
            );
            *entry = record.source_sequence();
        }
        assert_eq!(last.len(), 10);
    }

    #[test]
    fn canonical_records_carry_the_provider_identity_and_scoped_tenant() {
        let source = make_source(SCENARIO_V1);
        // The provider declaration (P2.11 step 4), structurally present.
        assert_eq!(source.source_id(), "sim://synthetic-portfolio-exit-gate");
        assert_eq!(source.provider_declaration().cadence(), Duration::from_secs(60));
        assert_eq!(
            source
                .provider_declaration()
                .quality_mapping()
                .canonical_for("OUT_OF_RANGE"),
            Some(TelemetryQuality::Suspect)
        );
        assert_eq!(
            source
                .provider_declaration()
                .quality_mapping()
                .canonical_for("SOURCE_INVALID"),
            Some(TelemetryQuality::Bad)
        );
        assert_eq!(
            source.provider_declaration().clock_quality(),
            ClockQuality::Synchronized
        );
        assert!(!source.source_id().starts_with("flexcore:"));
        let records = drain(&source);
        for record in &records {
            assert_eq!(record.source_id(), "sim://synthetic-portfolio-exit-gate");
            assert_eq!(record.tenant_id(), tenant_context().tenant_id());
            assert_eq!(record.clock_quality(), ClockQuality::Synchronized);
            assert_eq!(record.quality(), TelemetryQuality::Good);
            assert!(record.source_quality_code().is_none());
            assert!(record.ingest_time().unix_millis() >= record.event_time().unix_millis());
        }
    }

    #[test]
    fn the_site_meter_equals_the_sum_of_its_site_assets() {
        let records = drain(&make_source(SCENARIO_V1));
        let meter_power = |records: &[NormalizedTelemetry], tick_ms: i128| -> f64 {
            records
                .iter()
                .find(|r| {
                    r.asset_id() == asset_id("site-north-meter-1")
                        && r.event_time().unix_millis() == tick_ms
                        && r.quantity() == TelemetryQuantity::ActivePower
                })
                .and_then(|r| match r.value() {
                    Some(QuantityValue::Power(p)) => Some(p.value()),
                    _ => None,
                })
                .unwrap()
        };
        let start = load_scenario(SCENARIO_V1).unwrap().start_ms as i128;
        let tick = start + 12 * 3600 * 1000 + 7 * 60 * 1000;
        let member_keys = [
            "site-north-bess-1",
            "site-north-pv-1",
            "site-north-load-1",
            "site-north-evse-1",
        ];
        let sum: f64 = member_keys
            .iter()
            .map(|key| {
                records
                    .iter()
                    .find(|r| {
                        r.asset_id() == asset_id(key)
                            && r.event_time().unix_millis() == tick
                            && r.quantity() == TelemetryQuantity::ActivePower
                    })
                    .and_then(|r| match r.value() {
                        Some(QuantityValue::Power(p)) => Some(p.value()),
                        _ => None,
                    })
                    .unwrap()
            })
            .sum();
        assert!((meter_power(&records, tick) - sum).abs() < 1e-9);
    }

    // --- Run control, bounded emission, port errors ----------------------

    #[test]
    fn run_control_pause_and_stop_bound_the_emission() {
        let source = make_source(SCENARIO_V1);
        let subscription = subscribe_all(&source);
        assert_eq!(source.status(), RunStatus::Running);
        assert_eq!(source.health(), TelemetrySourceHealth::Healthy);

        // Bounded emission: each pull is at most one frame, and each decoded
        // frame is a small, complete record set.
        let frame = source.pull_frame(&subscription).unwrap().unwrap();
        let records = source.decode(&subscription, &frame).unwrap();
        assert!(!records.is_empty() && records.len() <= 6);

        source.pause();
        assert_eq!(source.status(), RunStatus::Paused);
        assert!(matches!(
            source.health(),
            TelemetrySourceHealth::Degraded { .. }
        ));
        assert!(source.pull_frame(&subscription).unwrap().is_none());

        source.resume();
        assert_eq!(source.status(), RunStatus::Running);
        assert!(source.pull_frame(&subscription).unwrap().is_some());

        source.stop();
        assert_eq!(source.status(), RunStatus::Stopped);
        assert!(matches!(
            source.health(),
            TelemetrySourceHealth::Unavailable { .. }
        ));
        assert!(source.pull_frame(&subscription).unwrap().is_none());
    }

    #[test]
    fn the_source_rejects_unknown_assets_and_unknown_handles() {
        let source = make_source(SCENARIO_V1);
        let stranger = AssetId::new(
            FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a99").unwrap(),
        );
        let err = source
            .subscribe(&TelemetryQuery::new(
                &tenant_context(),
                vec![stranger],
                EventTime::from_unix_millis(0).unwrap(),
                EventTime::from_unix_millis(1000).unwrap(),
                vec![TelemetryQuantity::ActivePower],
            ))
            .unwrap_err();
        assert!(matches!(err, TelemetrySourceError::InvalidQuery { .. }));

        let subscription = subscribe_all(&source);
        let stranger_handle = TelemetrySubscription::from_handle(999_999);
        assert_eq!(
            source.pull_frame(&stranger_handle),
            Err(TelemetrySourceError::NotSubscribed)
        );
        assert_eq!(
            source.decode(&stranger_handle, b"{}"),
            Err(TelemetrySourceError::NotSubscribed)
        );
        let _ = subscription;
    }

    #[test]
    fn decode_failure_is_err_and_never_a_partial_record() {
        let source = make_source(SCENARIO_V1);
        let subscription = subscribe_all(&source);
        // Not JSON.
        assert!(matches!(
            source.decode(&subscription, b"not json"),
            Err(TelemetrySourceError::DecodeFailed { .. })
        ));
        // Wrong format tag.
        assert!(matches!(
            source.decode(&subscription, br#"{"format":"other"}"#),
            Err(TelemetrySourceError::DecodeFailed { .. })
        ));
        // Valid JSON shape but a non-canonical SOC (outside [0,1]) must be a
        // whole-record decode failure, never a degraded record.
        let frame = r#"{
            "format": "flexcore-sim-frame", "version": 1,
            "asset_key": "site-north-bess-1",
            "event_time_unix_ms": 1780272000000,
            "released_at_unix_ms": 1780272000000,
            "source_sequence": 1,
            "samples": [{"quantity": "soc", "value": 1.5}]
        }"#;
        let err = source.decode(&subscription, frame.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("non-canonical"));
        // A frame for an asset outside the subscription.
        let foreign = r#"{
            "format": "flexcore-sim-frame", "version": 1,
            "asset_key": "site-south-bess-1",
            "event_time_unix_ms": 1780272000000,
            "released_at_unix_ms": 1780272000000,
            "source_sequence": 1,
            "samples": [{"quantity": "active_power", "value": 0.0}]
        }"#;
        // Subscribe to only site-north assets.
        let north_only = source
            .subscribe(&TelemetryQuery::new(
                &tenant_context(),
                vec![asset_id("site-north-bess-1")],
                EventTime::from_unix_millis(0).unwrap(),
                EventTime::from_unix_millis(i64::MAX / 2).unwrap(),
                all_quantities(),
            ))
            .unwrap();
        assert!(matches!(
            source.decode(&north_only, foreign.as_bytes()),
            Err(TelemetrySourceError::DecodeFailed { .. })
        ));
    }

    #[test]
    fn a_scenario_with_an_unknown_mapping_key_is_rejected() {
        let mapping = vec![("not-in-scenario".to_string(), asset_id("site-north-bess-1"))];
        let err = SimulatorTelemetrySource::new(SCENARIO_V1, mapping).unwrap_err();
        assert!(err.to_string().contains("not-in-scenario"));
    }

    // --- P2.15 fault-injection tests -------------------------------------
    //
    // The fault plan is scenario configuration: the tests enable it by
    // flipping `faults.enabled` on the parsed scenario JSON — no fault is
    // reachable without scenario configuration (P2.15 step 2). The seven
    // injections live in the versioned scenario's `faults` section, on
    // distinct assets over distinct intervals of the same 24 h horizon.
    // All arithmetic is scenario-clock arithmetic; nothing reads the wall
    // clock. End-to-end pipeline outcomes (stored quality/quarantine) are
    // P2.23's; these assert the SOURCE-level outcomes.

    fn faults_enabled_source() -> SimulatorTelemetrySource {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["faults"]["enabled"] = serde_json::Value::Bool(true);
        make_source(&value.to_string())
    }

    #[test]
    fn fault_drop_leaves_a_pure_event_time_gap_with_intact_sequences() {
        let faulted = drain(&faults_enabled_source());
        let clean = drain(&make_source(SCENARIO_V1));
        let key = asset_id("site-north-bess-1");
        let (from, until) = event_window_ms(2); // drop: 02:00–02:10
        let in_window = |r: &NormalizedTelemetry| {
            let t = r.event_time().unix_millis();
            t >= from && t < until
        };
        assert!(
            faulted
                .iter()
                .filter(|r| r.asset_id() == key && in_window(r))
                .count()
                == 0,
            "the dropped interval must emit nothing (the pipeline's gap detector turns this into MISSING)"
        );
        assert!(
            clean
                .iter()
                .filter(|r| r.asset_id() == key && in_window(r))
                .count()
                > 0,
            "the clean run covers the interval"
        );
        // Sequence numbering is untouched: the first sequence after the gap
        // continues exactly where the last one before it stopped.
        let last_before = faulted
            .iter()
            .filter(|r| r.asset_id() == key && r.event_time().unix_millis() < from)
            .map(|r| r.source_sequence())
            .max()
            .unwrap();
        let first_after = faulted
            .iter()
            .filter(|r| r.asset_id() == key && r.event_time().unix_millis() >= until)
            .map(|r| r.source_sequence())
            .min()
            .unwrap();
        assert_eq!(first_after, last_before + 1);
    }

    #[test]
    fn fault_delay_pushes_the_age_past_the_freshness_limit() {
        let faulted = drain(&faults_enabled_source());
        let key = asset_id("site-south-bess-1");
        let (from, until) = event_window_ms(3); // delay: 03:00–03:10, 120 s
        let limit = FreshnessLimit::from_seconds(90).unwrap();
        let mut checked = 0;
        for record in faulted.iter().filter(|r| {
            r.asset_id() == key
                && r.quantity() == TelemetryQuantity::ActivePower
                && r.event_time().unix_millis() >= from
                && r.event_time().unix_millis() < until
        }) {
            // Age = ingest − event, both on the scenario clock (P2.08: the
            // age is computed from event_time against the consumer's now).
            let age = (record.ingest_time().unix_millis() - record.event_time().unix_millis()) as i64;
            assert!(age > limit.millis(), "the delayed record must be past the freshness limit");
            // The P2.08 staleness rule, reused (not re-implemented): with
            // the scenario now at the release instant, the value is STALE.
            let now = EventTime::from_unix_millis(
                (record.event_time().unix_millis() + i128::from(age)) as i64,
            )
            .unwrap();
            assert!(staleness(record.event_time(), now, limit).is_stale());
            assert_eq!(record.quality(), TelemetryQuality::Good);
            checked += 1;
        }
        assert!(checked >= 9, "the delayed interval covers ~10 ticks, minus hold edge");
    }

    #[test]
    fn fault_out_of_range_yields_suspect_with_the_source_code() {
        let faulted = drain(&faults_enabled_source());
        let key = asset_id("site-north-pv-1");
        let (from, until) = event_window_ms(10); // out_of_range: 10:00–10:10
        let mut checked = 0;
        for record in faulted.iter().filter(|r| {
            r.asset_id() == key
                && r.quantity() == TelemetryQuantity::ActivePower
                && r.event_time().unix_millis() >= from
                && r.event_time().unix_millis() < until
        }) {
            assert_eq!(record.quality(), TelemetryQuality::Suspect);
            assert_eq!(record.source_quality_code(), Some("OUT_OF_RANGE"));
            match record.value() {
                Some(QuantityValue::Power(p)) => {
                    assert!(p.value().abs() > 1.5, "the faulted value must exceed the rated 1.5 MW");
                }
                other => panic!("expected a power value, got {other:?}"),
            }
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn fault_source_invalid_yields_bad_with_the_source_code() {
        let faulted = drain(&faults_enabled_source());
        let key = asset_id("site-north-load-1");
        let (from, until) = event_window_ms(5); // source_invalid: 05:00–05:10
        let mut checked = 0;
        for record in faulted.iter().filter(|r| {
            r.asset_id() == key
                && r.event_time().unix_millis() >= from
                && r.event_time().unix_millis() < until
        }) {
            assert_eq!(record.quality(), TelemetryQuality::Bad);
            assert_eq!(record.source_quality_code(), Some("SOURCE_INVALID"));
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn fault_duplicate_emits_identical_records_under_one_identity() {
        let faulted = drain(&faults_enabled_source());
        let key = asset_id("site-north-evse-1");
        let (from, until) = event_window_ms(6); // duplicate: 06:00–06:10
        let mut groups: BTreeMap<(i128, u64, TelemetryQuantity), Vec<&QuantityValue>> =
            BTreeMap::new();
        for record in faulted.iter().filter(|r| {
            r.asset_id() == key
                && r.event_time().unix_millis() >= from
                && r.event_time().unix_millis() < until
        }) {
            groups
                .entry((
                    record.event_time().unix_millis(),
                    record.source_sequence(),
                    record.quantity(),
                ))
                .or_default()
                .push(record.value().unwrap());
        }
        assert!(!groups.is_empty());
        for (_, values) in groups {
            assert_eq!(values.len(), 2, "each duplicated tick arrives exactly twice");
            assert_eq!(values[0], values[1], "a duplicate is byte-identical in value");
        }
    }

    #[test]
    fn fault_out_of_order_violates_the_arrival_order() {
        let source = faults_enabled_source();
        let subscription = subscribe_all(&source);
        let key = asset_id("site-south-pv-1");
        // Collect only this asset's records in arrival (pull) order.
        let mut arrival: Vec<(i128, u64)> = Vec::new();
        while let Some(frame) = source.pull_frame(&subscription).unwrap() {
            for record in source.decode(&subscription, &frame).unwrap() {
                if record.asset_id() == key {
                    arrival.push((record.event_time().unix_millis(), record.source_sequence()));
                }
            }
        }
        let mut saw_event_decrease = false;
        let mut saw_sequence_decrease = false;
        for pair in arrival.windows(2) {
            if pair[1].0 < pair[0].0 {
                saw_event_decrease = true;
            }
            if pair[1].1 < pair[0].1 {
                saw_sequence_decrease = true;
            }
        }
        assert!(
            saw_event_decrease && saw_sequence_decrease,
            "the out_of_order fault must show event_time and sequence decreases in arrival order"
        );
    }

    #[test]
    fn fault_conflicting_duplicate_differs_in_value_under_one_identity() {
        let faulted = drain(&faults_enabled_source());
        let key = asset_id("site-south-load-1");
        let (from, until) = event_window_ms(8); // conflicting_duplicate: 08:00–08:10
        let mut groups: BTreeMap<(i128, u64, TelemetryQuantity), Vec<&QuantityValue>> =
            BTreeMap::new();
        for record in faulted.iter().filter(|r| {
            r.asset_id() == key
                && r.event_time().unix_millis() >= from
                && r.event_time().unix_millis() < until
        }) {
            groups
                .entry((
                    record.event_time().unix_millis(),
                    record.source_sequence(),
                    record.quantity(),
                ))
                .or_default()
                .push(record.value().unwrap());
        }
        assert!(!groups.is_empty());
        let mut conflicts = 0;
        for (_, values) in groups {
            assert_eq!(values.len(), 2);
            if values[0] != values[1] {
                conflicts += 1;
            }
        }
        assert!(
            conflicts > 0,
            "at least the active-power value must conflict under the shared identity"
        );
    }

    #[test]
    fn faults_are_inert_while_the_plan_is_disabled() {
        // The shipped v1 plan (enabled: false) must not change a single
        // record relative to a plan-free run — fault logic is reachable
        // only through scenario configuration.
        let scenario = load_scenario(SCENARIO_V1).unwrap();
        assert!(!scenario.faults.enabled);
        assert_eq!(scenario.faults.injections.len(), 7);
        let plain = drain(&make_source(SCENARIO_V1));
        assert!(plain
            .iter()
            .all(|r| r.quality() == TelemetryQuality::Good && r.source_quality_code().is_none()));
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["faults"]["injections"] = serde_json::Value::Array(Vec::new());
        let empty_plan = drain(&make_source(&value.to_string()));
        assert_eq!(plain.len(), empty_plan.len());
        for (a, b) in plain.iter().zip(empty_plan.iter()) {
            assert_eq!(replay_key(a), replay_key(b));
        }
    }

    // A compile-time note: the WireValue variants must round-trip through
    // JSON faithfully (number vs bool vs string) for the decode pairing.
    #[test]
    fn wire_values_survive_serialization_round_trip() {
        let frame = WireFrame {
            format: FRAME_FORMAT,
            version: FRAME_VERSION,
            asset_key: "site-north-bess-1".to_string(),
            event_time_unix_ms: 1_780_272_000_000,
            released_at_unix_ms: 1_780_272_000_000,
            source_sequence: 1,
            source_quality_code: None,
            samples: vec![
                crate::engine::WireSample {
                    quantity: quantity_names::ACTIVE_POWER,
                    value: WireValue::Number(-1.0),
                    reactive_sign_convention: None,
                },
                crate::engine::WireSample {
                    quantity: quantity_names::AVAILABILITY,
                    value: WireValue::Flag(true),
                    reactive_sign_convention: None,
                },
                crate::engine::WireSample {
                    quantity: quantity_names::OPERATING_MODE,
                    value: WireValue::State("CHARGING".to_string()),
                    reactive_sign_convention: None,
                },
            ],
        };
        let bytes = serde_json::to_vec(&frame).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["samples"][0]["value"], -1.0);
        assert_eq!(parsed["samples"][1]["value"], true);
        assert_eq!(parsed["samples"][2]["value"], "CHARGING");
    }
}
