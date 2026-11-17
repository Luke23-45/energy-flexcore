// P17.08 — Dual-source state-of-charge arbitration (pure rules, no I/O).
//
// Implements ledger P17.08: the arbitration between the two SoC sources of
// an EV fleet — the OCPP MeterValues SoC (authoritative WHILE PLUGGED) and
// the vehicle-telematics API seam (a PORT, see below). Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 23_p17_08_soc_arbitration.md`.
//
// # The arbitration policy, decided once
//
// 1. **Plug-state is the participation ground truth** [S-139][S-140]. A
//    vehicle that is not physically connected to an EVSE dispatches
//    nothing, whatever its telematics say; a connection loss is NOT asset
//    unavailability. When no plug-state observation exists, the arbitration
//    refuses to pick a source (it never assumes "plugged" or "unplugged").
// 2. **OCPP MeterValues SoC is authoritative while the vehicle is plugged**
//    [S-137][S-140]: the charger meter reads the vehicle through the
//    physical charge link. While plugged and fresh, it outranks the
//    telematics value regardless of which source is newer.
// 3. **Telematics are the fallback when plugged and the OCPP SoC is stale
//    or absent** — a PORT seam, vendor-agnostic (P17-E-3, DEFAULT (a):
//    vendor-agnostic port first, named vendor adapters later; the decision
//    is registered OPEN and is NEVER closed by an agent — conventions §4).
// 4. **Staleness is per-source and uses the frozen quality vocabulary**: a
//    chosen observation whose age exceeds its source's freshness limit
//    yields quality `STALE` (the six-state vocabulary of
//    `flexcore-contracts::quality`, consumed — NEVER redefined, no seventh
//    state). Freshness limits are declared per source; none is hard-coded.
// 5. **Telematics go stale when the vehicle sleeps** [S-138]: a telematics
//    observation that the source reported while the vehicle was sleeping
//    (or whose source declared a sleep gap covering it) is carried with
//    quality `STALE` even when its age is within the freshness limit — the
//    value is real but the vehicle was not communicating. The
//    sleeping-flag is declared data from the telematics port, never
//    inferred here.
// 6. **Tesla 500 ms telemetry is the outlier, not the baseline** [S-137]:
//    the recorded note
//    ([`TESLA_500MS_TELEMETRY_OUTLIER_NOTE`]) documents that
//    high-frequency push telemetry exists in the market but is NOT the
//    freshness assumption any default carries; a vendor adapter that can
//    sustain a sub-second cadence declares its own (smaller) freshness
//    limit explicitly at composition.
// 7. **Non-silencing (P0.08)**: the arbitration never upgrades a value.
//    The output quality is the WORSE of the input record's own quality and
//    any staleness verdict — a SUSPECT input stays SUSPECT even when fresh.
//    No function here produces `GOOD` from anything but a GOOD-input
//    observation that is fresh; a mapped source degradation always
//    survives.
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// telematics vendor is named as integrated, no charge point is connected,
// no SoC value has ever been observed by this platform (the workspace is
// WRITTEN-UNRUN); every rule below is written against the port contracts,
// not against a device.

use crate::quality_rules::{staleness, FreshnessLimit, Staleness};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, TimeModelError};
use flexcore_contracts::units::StateOfCharge;

/// The recorded P17-E-3 status (the E-decision this module depends on —
/// agents never close E-decisions, conventions §4).
pub const P17_E_3_STATUS: &str =
    "P17-E-3 (telematics approach) is OPEN with DEFAULT (a): a vendor-agnostic \
telematics PORT first (staleness rules specified at the port; no vendor \
named), named vendor adapters later on demand. Vendor APIs carry commercial \
terms that are human-gate inputs; no terms of service are accepted by any \
planning artifact. The decision is closed by the human owner naming the \
first fleet vendor whose API terms are accepted.";

/// The recorded plug-state rule [S-139][S-140] (decided once).
pub const PLUG_STATE_GROUND_TRUTH_STATEMENT: &str =
    "Plug-state (whether a vehicle is physically connected to an EVSE) is the \
participation ground truth: a disconnected vehicle dispatches nothing, \
whatever telematics say. Connection loss is not asset unavailability — it \
removes the vehicle from the dispatchable set, never from the registry.";

/// The recorded telematics staleness rule [S-138] (decided once).
pub const TELEMATICS_STALE_WHEN_SLEEPING_STATEMENT: &str =
    "Telematics SoC values go stale when the vehicle sleeps: an observation \
the source reported during a sleep gap is carried with quality STALE even \
when its age is within the freshness limit, because the vehicle was not \
communicating. The sleeping flag is declared data from the telematics \
port, never inferred by the platform.";

/// The recorded Tesla-outlier note [S-137] (decided once).
pub const TESLA_500MS_TELEMETRY_OUTLIER_NOTE: &str =
    "Tesla Fleet Telemetry's ~500 ms push cadence is the market OUTLIER, not \
the baseline [S-137]. No default freshness limit in this module assumes \
sub-second telemetry; a vendor adapter that can sustain a faster cadence \
declares its own smaller freshness limit explicitly at composition.";

/// The recorded OCPP-authoritative rule [S-137][S-140] (decided once).
pub const OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED_STATEMENT: &str =
    "OCPP MeterValues SoC is authoritative while the vehicle is plugged: the \
charger meter reads through the physical charge link. While plugged and \
fresh it outranks the telematics value; when stale or absent the fresh \
telematics value is the fallback (and an aged one carries STALE).";

/// The two SoC sources, in precedence-inspection order (the precedence
/// itself is the plug-state-first policy above, not this enum's order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocSource {
    /// The OCPP MeterValues SoC (the CSMS adapter path; authoritative while
    /// plugged).
    OcppMeterValues,
    /// The vendor-agnostic telematics port (P17-E-3 DEFAULT (a) — no vendor
    /// named; adapters implement the port later).
    TelematicsPort,
}

impl SocSource {
    /// All sources, in frozen vocabulary order.
    pub const ALL: [SocSource; 2] = [SocSource::OcppMeterValues, SocSource::TelematicsPort];

    /// The canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            SocSource::OcppMeterValues => "ocpp_meter_values",
            SocSource::TelematicsPort => "telematics_port",
        }
    }
}

/// Errors of the arbitration. Every variant names what was refused — the
/// arbitration refuses to guess, it never defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocArbitrationError {
    /// No plug-state observation was provided. The policy refuses to
    /// assume a plug state (plug-state is the participation ground truth —
    /// an assumed state would fabricate participation).
    NoPlugStateObservation,
    /// The plug-state observation itself carried a non-measurable quality
    /// state (`MISSING` markers cannot arrive here — a gap marker is not
    /// an observation of a plug state; `ESTIMATED` plug states are refused:
    /// participation is never model-derived).
    PlugStateQualityUnusable {
        /// The refused quality state.
        quality: TelemetryQuality,
    },
    /// A timestamp was outside the representable UTC range (the contracts
    /// time-model bound) while building the staleness evaluation.
    TimestampOutOfRange {
        /// The offending timestamp in UTC ms.
        value: i128,
    },
    /// A freshness limit was not positive (the P2.08 rule: limits are
    /// positive; zero or negative limits are configuration errors).
    NonPositiveFreshnessLimit {
        /// The offending limit in ms.
        value: i64,
    },
}

impl std::fmt::Display for SocArbitrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocArbitrationError::NoPlugStateObservation => write!(
                f,
                "no plug-state observation was provided; the arbitration refuses to \
                 assume a plug state (plug-state is the participation ground truth)"
            ),
            SocArbitrationError::PlugStateQualityUnusable { quality } => write!(
                f,
                "the plug-state observation carries quality {quality:?}; participation is \
                 never decided from a MISSING marker or an ESTIMATED (model-derived) plug state"
            ),
            SocArbitrationError::TimestampOutOfRange { value } => {
                write!(f, "the timestamp {value} is outside the representable UTC range")
            }
            SocArbitrationError::NonPositiveFreshnessLimit { value } => {
                write!(f, "the freshness limit must be positive in ms (found {value})")
            }
        }
    }
}

impl std::error::Error for SocArbitrationError {}

/// One SoC observation from one source, as the port delivered it. The
/// quality is one of the frozen six states (consumed, never redefined);
/// the source-reported degradation travels in the quality field.
#[derive(Debug, Clone, PartialEq)]
pub struct SocObservation {
    /// Which source produced the observation.
    pub source: SocSource,
    /// The observed SoC fraction in [0,1] (the contracts unit type).
    pub soc: StateOfCharge,
    /// When the measurement occurred (UTC ms, event time).
    pub event_time_unix_ms: i64,
    /// The observation's own quality state (one of the frozen six; the
    /// `MISSING`/`ESTIMATED` states arrive only as platform-derived
    /// records upstream and are refused here — an arbitration input is a
    /// measured observation).
    pub quality: TelemetryQuality,
    /// Telematics-port declared data: the source reported the vehicle was
    /// SLEEPING for this observation (a sleep-gap coverage flag, [S-138]).
    /// Always `false` for OCPP MeterValues observations (the charge link
    /// keeps the vehicle awake while energy flows; recorded convention).
    pub reported_sleeping: bool,
}

impl SocObservation {
    /// Refuses the quality states an arbitration input cannot carry: a
    /// `MISSING` marker is a gap marker, not an observation, and an
    /// `ESTIMATED` SoC is model-derived (estimation of a participation
    /// input without explicit approval is disallowed by the Phase 0
    /// estimation rule).
    fn measured_quality(&self) -> Result<TelemetryQuality, SocArbitrationError> {
        match self.quality {
            TelemetryQuality::Missing | TelemetryQuality::Estimated => {
                Err(SocArbitrationError::PlugStateQualityUnusable {
                    quality: self.quality,
                })
            }
            other => Ok(other),
        }
    }
}

/// The observed plug state (the participation ground truth [S-139][S-140]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlugStateObservation {
    /// Whether the vehicle is physically connected to an EVSE.
    pub connected: bool,
    /// When the plug-state observation occurred (UTC ms, event time).
    pub event_time_unix_ms: i64,
    /// The observation's own quality state. `MISSING` and `ESTIMATED` are
    /// refused at arbitration time (participation is never gap-marked or
    /// model-derived; a missing plug state means the arbitration refuses,
    /// it never guesses).
    pub quality: TelemetryQuality,
}

/// The arbitration outcome: the chosen SoC, its source, and its quality
/// (exactly one of the frozen six — the non-silencing rule holds: the
/// output quality is the worse of the input quality and the staleness
/// verdict).
#[derive(Debug, Clone, PartialEq)]
pub struct ArbitratedSoc {
    /// The chosen SoC fraction in [0,1].
    pub soc: StateOfCharge,
    /// The source the value came from.
    pub source: SocSource,
    /// The output quality state (frozen six; never upgraded).
    pub quality: TelemetryQuality,
    /// Why this source won (human-safe; audit-safe).
    pub reason: String,
}

impl ArbitratedSoc {
    /// Whether the arbitrated SoC is usable for scheduling participation
    /// under the frozen vocabulary (GOOD or SUSPECT are live values; STALE
    /// is a last-known value; BAD is not trusted). The per-use-case
    /// acceptability set is the consumer's decision — this helper only
    /// names the conservative default.
    pub const fn is_live_value(&self) -> bool {
        matches!(
            self.quality,
            TelemetryQuality::Good | TelemetryQuality::Suspect
        )
    }
}

/// Combines an input quality with a staleness verdict WITHOUT upgrading:
/// a fresh BAD/SUSPECT stays BAD/SUSPECT; a fresh GOOD stays GOOD; a stale
/// GOOD/SUSPECT becomes STALE; a stale BAD stays BAD (a hard-invalid value
/// is worse than stale — the P2.08 severity intuition, recorded here once).
fn worse_with_staleness(input: TelemetryQuality, verdict: Staleness) -> TelemetryQuality {
    match (input, verdict.stale_quality()) {
        (_, None) => input,
        (TelemetryQuality::Bad, Some(_)) => TelemetryQuality::Bad,
        (_, Some(TelemetryQuality::Stale)) => TelemetryQuality::Stale,
        // The only remaining case is an input already STALE combined with a
        // fresh verdict — staleness never upgrades, STALE stays STALE.
        (other, _) => other,
    }
}

fn event_time(unix_ms: i64) -> Result<EventTime, SocArbitrationError> {
    EventTime::from_unix_millis(unix_ms).map_err(|e: TimeModelError| {
        let _ = e;
        SocArbitrationError::TimestampOutOfRange {
            value: unix_ms as i128,
        }
    })
}

/// The dual-source SoC arbitration (the policy above, as one pure
/// function). `now_unix_ms` is the caller-supplied evaluation instant (the
/// injected-clock rule: no clock is read here).
///
/// Refusals (never defaults): no plug-state observation; a plug-state
/// observation carrying `MISSING` or `ESTIMATED`; a non-positive freshness
/// limit; an SoC observation carrying `MISSING` or `ESTIMATED`.
pub fn arbitrate_soc(
    plug_state: Option<&PlugStateObservation>,
    ocpp_observation: Option<&SocObservation>,
    telematics_observation: Option<&SocObservation>,
    ocpp_freshness_limit: FreshnessLimit,
    telematics_freshness_limit: FreshnessLimit,
    now_unix_ms: i64,
) -> Result<Option<ArbitratedSoc>, SocArbitrationError> {
    // Rule 1: plug-state is the ground truth. No observation → refuse (no
    // assumed participation, either direction).
    let plug = plug_state.ok_or(SocArbitrationError::NoPlugStateObservation)?;
    match plug.quality {
        TelemetryQuality::Missing | TelemetryQuality::Estimated => {
            return Err(SocArbitrationError::PlugStateQualityUnusable {
                quality: plug.quality,
            });
        }
        _ => {}
    }
    if !plug.connected {
        // Disconnected: the vehicle participates in nothing; no SoC is
        // arbitrated (telematics may still report a battery level — that
        // is fleet information, NOT a participation SoC).
        return Ok(None);
    }

    // Both freshness limits are caller-declared; validate them here so a
    // configuration error surfaces at arbitration, not downstream.
    if ocpp_freshness_limit.millis() <= 0 {
        return Err(SocArbitrationError::NonPositiveFreshnessLimit {
            value: ocpp_freshness_limit.millis(),
        });
    }
    if telematics_freshness_limit.millis() <= 0 {
        return Err(SocArbitrationError::NonPositiveFreshnessLimit {
            value: telematics_freshness_limit.millis(),
        });
    }

    let now = event_time(now_unix_ms)?;

    // Rule 2: OCPP MeterValues SoC is authoritative while plugged.
    if let Some(ocpp) = ocpp_observation {
        let input_quality = ocpp.measured_quality()?;
        let event = event_time(ocpp.event_time_unix_ms)?;
        let verdict = staleness(event, now, ocpp_freshness_limit);
        let quality = worse_with_staleness(input_quality, verdict);
        let reason = if verdict.is_stale() {
            format!(
                "ocpp meter-values SoC chosen while plugged, but its age exceeds the \
                 declared OCPP freshness limit ({} ms) — carried as STALE",
                ocpp_freshness_limit.millis()
            )
        } else {
            "ocpp meter-values SoC chosen while plugged (authoritative source)".to_string()
        };
        return Ok(Some(ArbitratedSoc {
            soc: ocpp.soc,
            source: SocSource::OcppMeterValues,
            quality,
            reason,
        }));
    }

    // Rule 3: telematics fallback when plugged and the OCPP SoC is absent
    // (a stale-but-present OCPP value still wins — the charge link is the
    // authoritative path; its STALE quality marks the age honestly).
    if let Some(telematics) = telematics_observation {
        let input_quality = telematics.measured_quality()?;
        let event = event_time(telematics.event_time_unix_ms)?;
        let age_verdict = staleness(event, now, telematics_freshness_limit);
        // Rule 5: telematics go stale when the vehicle sleeps — the
        // declared sleeping flag forces STALE regardless of age.
        let quality = if telematics.reported_sleeping {
            TelemetryQuality::Stale
        } else {
            worse_with_staleness(input_quality, age_verdict)
        };
        let reason = if telematics.reported_sleeping {
            "telematics SoC chosen while plugged (OCPP SoC absent); the source \
                 reported the vehicle sleeping — carried as STALE [S-138]"
                .to_string()
        } else if age_verdict.is_stale() {
            format!(
                "telematics SoC chosen while plugged (OCPP SoC absent), but its age \
                 exceeds the declared telematics freshness limit ({} ms) — carried as STALE",
                telematics_freshness_limit.millis()
            )
        } else {
            "telematics SoC chosen while plugged (OCPP SoC absent; the vendor-agnostic \
                 telematics port is the fallback source)"
                .to_string()
        };
        return Ok(Some(ArbitratedSoc {
            soc: telematics.soc,
            source: SocSource::TelematicsPort,
            quality,
            reason,
        }));
    }

    // Plugged, and neither source has an observation: refuse to invent a
    // value (the P3.02 unknown-narrows rule — an unknown SoC narrows the
    // feasible set downstream, it is never filled).
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soc(value: f64) -> StateOfCharge {
        StateOfCharge::new(value).unwrap()
    }

    fn plug(connected: bool) -> PlugStateObservation {
        PlugStateObservation {
            connected,
            event_time_unix_ms: 1_700_000_000_000,
            quality: TelemetryQuality::Good,
        }
    }

    // The `age_ms` helpers place the event `age_ms` BEFORE the base
    // instant (T0), so `age = now - event_time` is the declared age —
    // staleness verdicts evaluate against real elapsed time.
    fn ocpp(value: f64, age_ms: i64) -> SocObservation {
        SocObservation {
            source: SocSource::OcppMeterValues,
            soc: soc(value),
            event_time_unix_ms: 1_700_000_000_000 - age_ms,
            quality: TelemetryQuality::Good,
            reported_sleeping: false,
        }
    }

    fn telematics(value: f64, age_ms: i64, sleeping: bool) -> SocObservation {
        SocObservation {
            source: SocSource::TelematicsPort,
            soc: soc(value),
            event_time_unix_ms: 1_700_000_000_000 - age_ms,
            quality: TelemetryQuality::Good,
            reported_sleeping: sleeping,
        }
    }

    // The declared freshness limits of the test (deployment configuration,
    // never hard-coded into the module): OCPP 60 s, telematics 10 min.
    fn ocpp_limit() -> FreshnessLimit {
        FreshnessLimit::from_seconds(60).unwrap()
    }

    fn telematics_limit() -> FreshnessLimit {
        FreshnessLimit::from_seconds(600).unwrap()
    }

    #[test]
    fn the_policy_statements_are_recorded() {
        assert!(P17_E_3_STATUS.contains("OPEN with DEFAULT (a)"));
        assert!(P17_E_3_STATUS.contains("vendor-agnostic"));
        assert!(PLUG_STATE_GROUND_TRUTH_STATEMENT.contains("participation ground truth"));
        assert!(OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED_STATEMENT.contains("authoritative while"));
        assert!(TELEMATICS_STALE_WHEN_SLEEPING_STATEMENT.contains("sleep"));
        assert!(TESLA_500MS_TELEMETRY_OUTLIER_NOTE.contains("OUTLIER"));
    }

    #[test]
    fn disconnected_vehicle_arbitrates_no_participation_soc() {
        // Rule 1: whatever telematics say, a disconnected vehicle
        // dispatches nothing — the arbitration returns None.
        let out = arbitrate_soc(
            Some(&plug(false)),
            Some(&ocpp(0.7, 1_000)),
            Some(&telematics(0.5, 1_000, false)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_002,
        )
        .unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn missing_plug_state_is_refused_never_assumed() {
        let err = arbitrate_soc(
            None,
            Some(&ocpp(0.7, 1_000)),
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_002,
        )
        .unwrap_err();
        assert_eq!(err, SocArbitrationError::NoPlugStateObservation);
    }

    #[test]
    fn a_model_derived_or_gap_marked_plug_state_is_refused() {
        // Participation is never ESTIMATED and never decided from a
        // MISSING gap marker.
        for quality in [TelemetryQuality::Missing, TelemetryQuality::Estimated] {
            let bad = PlugStateObservation {
                connected: true,
                event_time_unix_ms: 1_700_000_000_000,
                quality,
            };
            let err = arbitrate_soc(
                Some(&bad),
                Some(&ocpp(0.7, 1_000)),
                None,
                ocpp_limit(),
                telematics_limit(),
                1_700_000_000_002,
            )
            .unwrap_err();
            assert_eq!(
                err,
                SocArbitrationError::PlugStateQualityUnusable { quality }
            );
        }
    }

    #[test]
    fn while_plugged_the_ocpp_value_is_authoritative_over_a_newer_telematics_value() {
        // Rule 2: OCPP wins even when telematics is NEWER — the charge link
        // is the authoritative path while plugged.
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&ocpp(0.72, 5_000)),
            Some(&telematics(0.60, 1_000, false)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_010,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.source, SocSource::OcppMeterValues);
        assert_eq!(out.soc, soc(0.72));
        assert_eq!(out.quality, TelemetryQuality::Good);
        assert!(out.reason.contains("authoritative"));
    }

    #[test]
    fn an_aged_ocpp_value_is_chosen_but_carries_stale() {
        // Age 61 s against a 60 s limit: age > limit → STALE (the frozen
        // boundary; age == limit would still be fresh).
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&ocpp(0.72, 61_000)),
            Some(&telematics(0.60, 1_000, false)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_000,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.source, SocSource::OcppMeterValues);
        assert_eq!(out.quality, TelemetryQuality::Stale);
    }

    #[test]
    fn an_ocpp_value_exactly_at_the_freshness_limit_is_not_stale() {
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&ocpp(0.72, 60_000)),
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_000,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.quality, TelemetryQuality::Good);
    }

    #[test]
    fn telematics_is_the_fallback_when_the_ocpp_soc_is_absent() {
        let out = arbitrate_soc(
            Some(&plug(true)),
            None,
            Some(&telematics(0.55, 30_000, false)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_030,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.source, SocSource::TelematicsPort);
        assert_eq!(out.soc, soc(0.55));
        assert_eq!(out.quality, TelemetryQuality::Good);
        assert!(out.reason.contains("fallback"));
    }

    #[test]
    fn a_stale_but_present_ocpp_value_still_outranks_fresh_telematics() {
        // The policy is authoritative-while-plugged, not newest-wins: a
        // stale OCPP value is chosen and honestly marked STALE; the fresh
        // telematics value is not silently preferred.
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&ocpp(0.72, 120_000)),
            Some(&telematics(0.60, 1_000, false)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_120,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.source, SocSource::OcppMeterValues);
        assert_eq!(out.quality, TelemetryQuality::Stale);
    }

    #[test]
    fn a_sleeping_vehicle_s_telematics_value_is_stale_even_when_fresh() {
        // Rule 5 [S-138]: the declared sleeping flag forces STALE regardless
        // of age.
        let out = arbitrate_soc(
            Some(&plug(true)),
            None,
            Some(&telematics(0.55, 1_000, true)),
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_002,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.source, SocSource::TelematicsPort);
        assert_eq!(out.quality, TelemetryQuality::Stale);
        assert!(out.reason.contains("sleeping"));
    }

    #[test]
    fn a_suspect_input_stays_suspect_even_when_fresh() {
        // Non-silencing: a source-reported degradation survives fresh
        // arbitration.
        let mut suspect = ocpp(0.72, 1_000);
        suspect.quality = TelemetryQuality::Suspect;
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&suspect),
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_002,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.quality, TelemetryQuality::Suspect);
    }

    #[test]
    fn a_bad_input_stays_bad_even_when_stale() {
        // A hard-invalid value is worse than stale: BAD survives.
        let mut bad = ocpp(0.72, 120_000);
        bad.quality = TelemetryQuality::Bad;
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&bad),
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_120,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.quality, TelemetryQuality::Bad);
    }

    #[test]
    fn gap_marked_and_estimated_observations_are_refused() {
        for quality in [TelemetryQuality::Missing, TelemetryQuality::Estimated] {
            let mut marker = ocpp(0.7, 1_000);
            marker.quality = quality;
            let err = arbitrate_soc(
                Some(&plug(true)),
                Some(&marker),
                None,
                ocpp_limit(),
                telematics_limit(),
                1_700_000_000_002,
            )
            .unwrap_err();
            assert!(matches!(
                err,
                SocArbitrationError::PlugStateQualityUnusable { .. }
            ));
        }
    }

    #[test]
    fn plugged_with_no_observation_at_all_yields_none_never_an_invented_value() {
        let out = arbitrate_soc(
            Some(&plug(true)),
            None,
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_002,
        )
        .unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn non_positive_freshness_limits_are_refused_at_construction() {
        // The FreshnessLimit constructor refuses non-positive limits, so an
        // invalid limit can never reach the arbitration through the typed
        // parameter (the arbitration's own millis<=0 guard is
        // defense-in-depth against a future untyped path).
        assert!(FreshnessLimit::try_new(0).is_err());
        assert!(FreshnessLimit::try_new(-1_000).is_err());
        assert!(FreshnessLimit::from_seconds(0).is_err());
    }

    #[test]
    fn a_future_dated_event_time_is_fresh_not_stale() {
        // Clock skew is a separate fault (P2.08); staleness never fires on
        // a negative age: the event is 500 s in the FUTURE of `now`.
        let future = SocObservation {
            source: SocSource::OcppMeterValues,
            soc: soc(0.72),
            event_time_unix_ms: 1_700_000_000_000 + 500_000,
            quality: TelemetryQuality::Good,
            reported_sleeping: false,
        };
        let out = arbitrate_soc(
            Some(&plug(true)),
            Some(&future),
            None,
            ocpp_limit(),
            telematics_limit(),
            1_700_000_000_000,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.quality, TelemetryQuality::Good);
    }

    #[test]
    fn the_live_value_helper_names_the_conservative_default() {
        let good = ArbitratedSoc {
            soc: soc(0.5),
            source: SocSource::OcppMeterValues,
            quality: TelemetryQuality::Good,
            reason: "test".to_string(),
        };
        let stale = ArbitratedSoc {
            quality: TelemetryQuality::Stale,
            ..good.clone()
        };
        let bad = ArbitratedSoc {
            quality: TelemetryQuality::Bad,
            ..good.clone()
        };
        assert!(good.is_live_value());
        assert!(!stale.is_live_value());
        assert!(!bad.is_live_value());
    }
}
