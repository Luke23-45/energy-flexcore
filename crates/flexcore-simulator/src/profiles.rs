// P2.14 — The behaviour profiles: the per-tick synthetic value models for
// the four exit-gate asset types plus the site meter (P2.13 §5).
//
// **Every profile here is synthetic** — shaped data with stated assumptions
// (each stated in the scenario file and below), NOT a validated physical
// model and not calibrated against measured data. They exist to produce
// deterministic, canonical, plausible-shape telemetry for platform testing;
// no result derived from them is evidence about real device behaviour.
//
// Profile boundaries (stated, not claimed):
// - BESS: rectangular-efficiency daily dispatch; no electrochemical,
//   temperature, degradation, or state-of-health model; constant efficiency
//   (the round-trip efficiency split symmetrically across charge and
//   discharge as sqrt(round_trip_efficiency)); no auxiliary consumption;
//   instantaneous transitions; SOC clamped into the configured operating
//   band and, belt-and-braces, into [0,1] — the generator never emits a
//   step whose bookkeeping leaves [0,1] (P2.13 §5.1).
// - PV: synthetic clear-sky cosine bell, zero at night; NO weather model;
//   no derating beyond the peak factor and the rated-power cap; curtailment
//   is noted as a capability reference but NOT exercised in Phase 2
//   (P2.13 §5.2).
// - Flexible load: a declared baseline shape (morning and evening cosine
//   peaks over a near-flat night) — NO baseline *method* is computed
//   (D1/D2 open); seeded jitter only. The reducible range travels as
//   scenario data for Phase 3.
// - EVSE: declared connection windows (which may wrap midnight), one
//   aggregate EV, constant charging efficiency, no V2G. Synthetic
//   assumption recorded once here: the aggregate EV battery holds
//   `charging_power_mw × EV_FULL_CHARGE_HOURS` of energy (2 hours at
//   rated charge) — the scenario does not parameterise battery size, and
//   this assumption is deterministic and stated, not measured.
// - Site meter: the aggregated SUM of the site's assets' net active power
//   — a consistency fixture. **Losses = 0 and non-modelled load = 0 by
//   construction**; a real PCC meter would include both (P2.13 §5.5).
//
// Sign rule everywhere (P0.07 §2): P > 0 = export at the point of
// connection — load consumption and EVSE charging are negative. Reactive
// power carries no meaning without the site's declared convention
// (P0.07 §3; P2.04 D-T4); the convention travels on the record.

use crate::scenario::{AssetSpec, BehaviourParams, BessParams, EvseParams, LoadParams, PvParams};

/// The synthetic aggregate-EV battery assumption: full battery = 2 hours at
/// rated charge power (see module header).
pub(crate) const EV_FULL_CHARGE_HOURS: f64 = 2.0;

/// The synthetic EVSE charging efficiency (constant, no model behind it).
pub(crate) const EV_CHARGING_EFFICIENCY: f64 = 0.9;

/// Seconds-of-day (UTC) of a scenario-clock instant.
pub(crate) fn seconds_of_day(unix_ms: i64) -> u32 {
    (unix_ms.div_euclid(1000).rem_euclid(86_400)) as u32
}

/// Whether a seconds-of-day instant lies in `[start, end)`, allowing the
/// midnight wrap (end <= start).
pub(crate) fn in_window(sod: u32, window: (u32, u32)) -> bool {
    let (start, end) = window;
    if start < end {
        sod >= start && sod < end
    } else if start > end {
        // Wrap: e.g. 17:30 → 08:00 covers [17:30, 24:00) ∪ [00:00, 08:00).
        sod >= start || sod < end
    } else {
        // start == end: an empty window.
        false
    }
}

/// The per-tick values of one asset, before canonical-record encoding. A
/// `None` field means the profile does not emit that quantity (P2.04 §4).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AssetTick {
    pub active_power_mw: f64,
    pub reactive_power_mvar: Option<f64>,
    pub soc: Option<f64>,
    pub energy_mwh: Option<f64>,
    pub availability: Option<bool>,
    pub connection_state: Option<bool>,
    pub operating_mode: Option<String>,
    pub health: Option<String>,
    pub baseline_reference_mw: Option<f64>,
}

/// A symmetric cosine bump of height 1 centred on `center` with half-width
/// `half_width` seconds; 0 outside.
fn cosine_bump(sod: u32, center: f64, half_width_s: f64) -> f64 {
    let delta = (f64::from(sod) - center).abs();
    if delta < half_width_s {
        0.5 * (1.0 + (std::f64::consts::PI * delta / half_width_s).cos())
    } else {
        0.0
    }
}

/// BESS: fixed daily charge/discharge cycle with rectangular-efficiency SOC
/// bookkeeping (P2.13 §5.1). `soc` is the running state (updated in place);
/// the emitted power is scaled down on the step that would leave the
/// configured operating band, so the emitted P and the SOC delta always
/// agree.
pub(crate) fn bess_tick(
    soc: &mut f64,
    params: &BessParams,
    rated_energy_mwh: f64,
    cadence_s: i64,
    sod: u32,
) -> AssetTick {
    let eff = params.round_trip_efficiency.sqrt();
    let hours = cadence_s as f64 / 3600.0;
    let (p_net, mode) = if in_window(sod, params.charge_window) {
        // Charging: import → negative P. Energy into the cell includes the
        // (constant, synthetic) efficiency.
        let mut p_mag = params.charge_power_mw;
        let mut soc_new = *soc + p_mag * hours * eff / rated_energy_mwh;
        if soc_new > params.soc_ceiling {
            // Scale the step so SOC lands exactly on the ceiling; a step
            // that would leave [0,1] is never emitted (P2.13 §5.1).
            let headroom = params.soc_ceiling - *soc;
            if headroom <= 0.0 {
                p_mag = 0.0;
                soc_new = *soc;
            } else {
                p_mag = headroom * rated_energy_mwh / (hours * eff);
                soc_new = params.soc_ceiling;
            }
        }
        *soc = soc_new;
        let mode = if p_mag > 0.0 { 0 } else { 2 };
        (-p_mag, mode)
    } else if in_window(sod, params.discharge_window) {
        // Discharging: export → positive P. Energy out of the cell costs
        // 1/efficiency at the terminal.
        let mut p_mag = params.discharge_power_mw;
        let mut soc_new = *soc - p_mag * hours / (eff * rated_energy_mwh);
        if soc_new < params.soc_floor {
            let headroom = *soc - params.soc_floor;
            if headroom <= 0.0 {
                p_mag = 0.0;
                soc_new = *soc;
            } else {
                p_mag = headroom * rated_energy_mwh * eff / hours;
                soc_new = params.soc_floor;
            }
        }
        *soc = soc_new;
        let mode = if p_mag > 0.0 { 1 } else { 2 };
        (p_mag, mode)
    } else {
        (0.0, 2)
    };
    // Belt-and-braces clamp: SOC stays in [0,1] by construction.
    *soc = soc.clamp(0.0, 1.0);
    AssetTick {
        active_power_mw: p_net,
        reactive_power_mvar: Some(0.0),
        soc: Some(*soc),
        energy_mwh: None,
        availability: Some(true),
        connection_state: None,
        operating_mode: Some(params.operating_mode_states[mode].clone()),
        health: Some(params.health_state.clone()),
        baseline_reference_mw: None,
    }
}

/// PV: synthetic clear-sky cosine bell, zero at night (P2.13 §5.2).
/// Curtailment is noted but NOT exercised — nothing reduces the shape.
pub(crate) fn pv_tick(params: &PvParams, rated_power_mw: f64, sod: u32) -> AssetTick {
    let p = if sod >= params.sunrise_sod && sod < params.sunset_sod {
        let span = f64::from(params.sunset_sod - params.sunrise_sod);
        let x = (f64::from(sod) - f64::from(params.sunrise_sod)) / span; // [0, 1)
        let bell = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * x).cos());
        (rated_power_mw * params.peak_factor * bell).min(rated_power_mw)
    } else {
        0.0
    };
    AssetTick {
        active_power_mw: p,
        reactive_power_mvar: Some(0.0),
        soc: None,
        energy_mwh: None,
        availability: Some(true),
        connection_state: None,
        operating_mode: None,
        health: None,
        baseline_reference_mw: None,
    }
}

/// Flexible load: the declared baseline shape (morning + evening cosine
/// peaks over a near-flat night) plus seeded jitter, emitted as
/// consumption (negative). The baseline reference — when configured — is
/// the declared shape itself, without jitter (P2.13 §5.3).
pub(crate) fn load_tick(
    rng: &mut crate::rng::SplitMix64,
    params: &LoadParams,
    rated_peak_mw: f64,
    sod: u32,
) -> AssetTick {
    let night = 0.25 * rated_peak_mw;
    let morning = 0.45 * rated_peak_mw * cosine_bump(sod, 7.5 * 3600.0, 2.5 * 3600.0);
    let evening = 0.50 * rated_peak_mw * cosine_bump(sod, 19.0 * 3600.0, 3.0 * 3600.0);
    let baseline = night + morning + evening; // peaks never overlap; stays <= 0.75 * rated_peak
    let jitter = rng.f64_in(-params.jitter_fraction, params.jitter_fraction) * rated_peak_mw;
    let consumption = (baseline + jitter).min(rated_peak_mw);
    AssetTick {
        active_power_mw: -consumption,
        reactive_power_mvar: None,
        soc: None,
        energy_mwh: None,
        availability: None,
        connection_state: None,
        operating_mode: None,
        health: None,
        baseline_reference_mw: params
            .emit_baseline_reference
            .then_some(-baseline),
    }
}

/// EVSE: declared connection windows, constant-power charging that fills
/// the configured EV SOC band, per-interval delivered energy (P2.13 §5.4).
/// `soc` is the running EV SOC state (updated in place).
pub(crate) fn evse_tick(
    soc: &mut f64,
    rng: &mut crate::rng::SplitMix64,
    params: &EvseParams,
    cadence_s: i64,
    sod: u32,
) -> AssetTick {
    let connected = params.connection_windows.iter().any(|w| in_window(sod, *w));
    let battery_mwh = params.charging_power_mw * EV_FULL_CHARGE_HOURS;
    let hours = cadence_s as f64 / 3600.0;
    let (p, energy) = if connected && *soc < params.soc_target {
        // Seeded jitter only — connections follow the declared windows, not
        // arrival-time distributions.
        let scale = 1.0 + rng.f64_in(-params.jitter_fraction, params.jitter_fraction);
        let mut p_mag = params.charging_power_mw * scale;
        let mut e_grid = p_mag * hours;
        let soc_new = *soc + e_grid * EV_CHARGING_EFFICIENCY / battery_mwh;
        if soc_new >= params.soc_target {
            // Land exactly on the target; the EV holds there while
            // connected (no V2G in Phase 2).
            let need = (params.soc_target - *soc) * battery_mwh / EV_CHARGING_EFFICIENCY;
            e_grid = need;
            p_mag = need / hours;
            *soc = params.soc_target;
        } else {
            *soc = soc_new;
        }
        (-p_mag, e_grid)
    } else {
        (0.0, 0.0)
    };
    *soc = soc.clamp(0.0, 1.0);
    AssetTick {
        active_power_mw: p,
        reactive_power_mvar: None,
        soc: Some(*soc),
        energy_mwh: params.emit_energy.then_some(energy),
        availability: None,
        connection_state: Some(connected),
        operating_mode: None,
        health: None,
        baseline_reference_mw: None,
    }
}

/// Site meter: the aggregated SUM of the site's assets' net active power
/// (losses = 0, non-modelled load = 0 — a consistency fixture), and a
/// configured constant reactive power under the site's declared convention
/// (P2.13 §5.5). `site_net_power_mw` is the pre-fault profile sum computed
/// by the engine for the same tick.
pub(crate) fn meter_tick(site_net_power_mw: f64, q_constant_mvar: f64) -> AssetTick {
    AssetTick {
        active_power_mw: site_net_power_mw,
        reactive_power_mvar: Some(q_constant_mvar),
        soc: None,
        energy_mwh: None,
        availability: None,
        connection_state: None,
        operating_mode: None,
        health: None,
        baseline_reference_mw: None,
    }
}

/// Dispatches the per-tick generation for one asset, updating its running
/// state where the profile has one. `rng` is the asset's derived stream
/// (only the jitter-bearing profiles draw from it); `site_net_power_mw` is
/// required for the site meter (the engine passes its per-tick sum).
pub(crate) fn asset_tick(
    asset: &AssetSpec,
    soc: &mut Option<f64>,
    rng: &mut crate::rng::SplitMix64,
    cadence_s: i64,
    sod: u32,
    site_net_power_mw: f64,
) -> AssetTick {
    match (&asset.params, &mut *soc) {
        (BehaviourParams::Bess(p), Some(state)) => {
            bess_tick(state, p, asset.rated_energy_mwh.unwrap_or_default(), cadence_s, sod)
        }
        (BehaviourParams::Pv(p), _) => pv_tick(p, asset.rated_power_mw, sod),
        (BehaviourParams::Load(p), _) => load_tick(rng, p, asset.rated_power_mw, sod),
        (BehaviourParams::Evse(p), Some(state)) => evse_tick(state, rng, p, cadence_s, sod),
        (BehaviourParams::Meter(p), _) => meter_tick(site_net_power_mw, p.q_constant_mvar),
        // A profile without SOC state paired with a state slot is a caller
        // bug; the engine only allocates SOC state for SOC-bearing profiles.
        (BehaviourParams::Bess(_) | BehaviourParams::Evse(_), None) => {
            unreachable!("SOC-bearing profiles always carry SOC state")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bess_params() -> BessParams {
        BessParams {
            charge_window: (3600, 5 * 3600),          // 01:00–05:00
            discharge_window: (18 * 3600, 21 * 3600), // 18:00–21:00
            charge_power_mw: 1.0,
            discharge_power_mw: 1.5,
            initial_soc: 0.5,
            soc_floor: 0.05,
            soc_ceiling: 0.95,
            round_trip_efficiency: 0.9,
            operating_mode_states: [
                "CHARGING".to_string(),
                "DISCHARGING".to_string(),
                "IDLE".to_string(),
            ],
            health_state: "NOMINAL".to_string(),
        }
    }

    #[test]
    fn windows_wrap_midnight() {
        assert!(in_window(18 * 3600, (17 * 3600 + 1800, 8 * 3600)));
        assert!(in_window(3600, (17 * 3600 + 1800, 8 * 3600)));
        assert!(!in_window(12 * 3600, (17 * 3600 + 1800, 8 * 3600)));
        assert!(in_window(3600, (3600, 5 * 3600)));
        assert!(!in_window(3600, (3600, 3600)), "start == end is an empty window");
    }

    #[test]
    fn soc_stays_in_the_operating_band_over_a_full_day() {
        let params = bess_params();
        let mut soc = params.initial_soc;
        let cadence: i64 = 60;
        for tick in 0..1440u32 {
            let sod = tick * 60;
            let t = bess_tick(&mut soc, &params, 4.0, cadence, sod);
            assert!(
                (0.0..=1.0).contains(&soc),
                "SOC left [0,1] at tick {tick}: {soc}"
            );
            assert!(
                soc >= params.soc_floor - 1e-12 && soc <= params.soc_ceiling + 1e-12,
                "SOC left the operating band at tick {tick}: {soc}"
            );
            assert!(t.active_power_mw.abs() <= params.discharge_power_mw + 1e-12);
            // Sign rule: charging is import (negative), discharging export.
            if in_window(sod, params.charge_window) && t.active_power_mw != 0.0 {
                assert!(t.active_power_mw < 0.0);
            }
            if in_window(sod, params.discharge_window) && t.active_power_mw != 0.0 {
                assert!(t.active_power_mw > 0.0);
            }
        }
    }

    #[test]
    fn pv_is_zero_at_night_and_capped_in_daylight() {
        let params = PvParams {
            sunrise_sod: 5 * 3600 + 1800,
            sunset_sod: 21 * 3600,
            peak_factor: 0.85,
        };
        assert_eq!(pv_tick(&params, 1.5, 0).active_power_mw, 0.0);
        assert_eq!(pv_tick(&params, 1.5, 22 * 3600).active_power_mw, 0.0);
        let noon = pv_tick(&params, 1.5, 12 * 3600).active_power_mw;
        assert!(noon > 0.0 && noon <= 1.5);
        // The bell peaks at solar noon between sunrise and sunset.
        let peak_sod = (params.sunrise_sod + params.sunset_sod) / 2;
        let peak = pv_tick(&params, 1.5, peak_sod).active_power_mw;
        assert!((peak - 1.5 * 0.85).abs() < 1e-9);
    }

    #[test]
    fn load_baseline_is_negative_consumption_and_capped() {
        let params = LoadParams {
            reducible_range_mw: 0.3,
            emit_baseline_reference: true,
            jitter_fraction: 0.0,
        };
        let mut rng = crate::rng::SplitMix64::new(42);
        for tick in 0..1440u32 {
            let t = load_tick(&mut rng, &params, 0.8, tick * 60);
            assert!(t.active_power_mw <= 0.0, "load consumption must be negative");
            assert!(t.active_power_mw >= -0.8);
            let baseline = t.baseline_reference_mw.unwrap();
            assert!(baseline <= 0.0 && baseline >= -0.8);
        }
        // Morning peak present at 07:30, near-flat at 03:00.
        let mut rng = crate::rng::SplitMix64::new(42);
        let morning = load_tick(&mut rng, &params, 0.8, 7 * 3600 + 1800).active_power_mw;
        let night = load_tick(&mut rng, &params, 0.8, 3 * 3600).active_power_mw;
        assert!(morning < night, "morning peak must exceed the night floor");
    }

    #[test]
    fn evse_charges_only_when_connected_and_reaches_its_target() {
        let params = EvseParams {
            connection_windows: vec![(17 * 3600 + 1800, 8 * 3600)], // wraps midnight
            charging_power_mw: 0.05,
            initial_soc: 0.3,
            soc_target: 0.9,
            emit_energy: true,
            jitter_fraction: 0.0,
        };
        let mut soc = params.initial_soc;
        let mut rng = crate::rng::SplitMix64::new(7);
        let mut ever_charged = false;
        for tick in 0..1440u32 {
            let sod = tick * 60;
            let t = evse_tick(&mut soc, &mut rng, &params, 60, sod);
            assert!((0.0..=1.0).contains(&soc));
            assert_eq!(t.connection_state, Some(in_window(sod, params.connection_windows[0])));
            if t.active_power_mw < 0.0 {
                ever_charged = true;
                assert!(t.energy_mwh.unwrap() > 0.0);
            }
            if !t.connection_state.unwrap() {
                assert_eq!(t.active_power_mw, 0.0);
            }
        }
        assert!(ever_charged);
        assert!((soc - params.soc_target).abs() < 1e-9, "the EV must reach its target");
    }

    #[test]
    fn meter_is_the_site_sum_with_its_constant_reactive() {
        let t = meter_tick(-1.7, 0.1);
        assert_eq!(t.active_power_mw, -1.7);
        assert_eq!(t.reactive_power_mvar, Some(0.1));
    }
}
