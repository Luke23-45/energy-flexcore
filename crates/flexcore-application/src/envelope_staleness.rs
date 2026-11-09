// P3.13 — Envelope staleness and recomputation triggers (§11.6, §10.4).
//
// Implements ledger P3.13 ("Envelope staleness and recomputation
// triggers", report §11.6, §10.4, §15.3). Decision record:
// `ledger/phase-03-flexibility-scheduling/12_13_envelope.md`.
//
// THE POLICY, DECIDED ONCE:
//
//   **A STALE envelope is never used for a new schedule.** The scheduler
//   recomputes the envelope or fails loudly — it never silently proceeds
//   with a stale envelope. A schedule referencing a superseded envelope
//   version is rejected or re-derived. The pure checks are exposed here;
//   their enforcement wiring into the schedule store and the lifecycle
//   lands in P3.22/P3.24.
//
// THE TRIGGERS (ledger step 1), each carried by
// [`EnvelopeTriggerEvent`] and evaluated by the pure function
// `evaluate_envelope_trigger` over the event plus the current envelope
// metadata:
//
//   | Trigger | Invalidation rule |
//   |---|---|
//   | `TelemetryQualityChanged` (§15.3, §10.4) | the event's asset is in the envelope's scope AND the event's tenant matches → STALE. DECIDED: ANY quality transition on a scoped asset invalidates — the envelope's recorded computation consumed a telemetry basis whose quality just changed, in either direction; the non-silencing rule (§10.4: a quality flag is never silently converted) applies to improvements too. |
//   | `AssetAvailabilityChanged` (§15.3) | the event's asset is in scope (tenant matches) → STALE. |
//   | `AssetCapabilityUpdated` (§15.3) | the event's asset is in scope (tenant matches) → STALE. |
//   | `ForecastPublished` (§15.3) | the published forecast version differs from the envelope's input-snapshot forecast version → STALE; the SAME version → fresh (a republished fixture version dedupes — D-FX-9 records that the frozen payload cannot carry the version, so the caller reads it from the fixture store via the event's scope, per that decision). |
//   | Horizon rollover | a new horizon starting at or after the envelope's horizon end → STALE (the envelope cannot serve a horizon it does not cover). |
//   | Maximum age (configured) | `now − computed_at > max_age` → STALE. The age bound is CONFIGURATION with the recorded default `DEFAULT_MAX_AGE_MILLIS` (1 hour). The convention matches P2.08 staleness: age == limit is NOT stale; limit + 1 ms is. |
//
// A foreign-tenant event never invalidates (tenant scoping, P0.16 rule 5).
//
// WHAT EVERY ENVELOPE CARRIES (ledger step 2): the `EnvelopeMetadata`
// (defined with the envelope in `flexibility_envelope`) carries the
// envelope version, the computation timestamp, and the input-snapshot
// reference; schedules reference the envelope version they used through
// `ScheduleEnvelopeReference` (exposed here for P3.21), checked by
// `check_schedule_envelope_reference` and gated by
// `assert_usable_for_new_schedule`.
//
// DEBOUNCING (ledger risk note, recorded as CONFIGURATION): recomputing on
// every telemetry tick is a recorded performance risk. The configuration
// field `StalenessConfig::min_recompute_interval_millis` (default 0 = no
// debounce) turns a triggered recomputation ADVICE from `Immediate` into
// `Deferred { not_before }` while the debounce window is open.
//
//   **Which guarantees debounce CANNOT weaken (the recorded invariant):**
//   debounce applies ONLY to the recomputation advice. It never changes
//   `EnvelopeFreshness`, and both hard gates — `assert_usable_for_new_
//   schedule` (a stale envelope is never used for a new schedule) and
//   `check_schedule_envelope_reference` (a superseded envelope version is
//   rejected) — ignore the debounce configuration entirely. A deferred
//   recompute therefore leaves the envelope STALE in the meantime: the
//   scheduler still recomputes-or-fails, just not necessarily instantly.
//
// Control status: pure evaluation functions for the simulation-only 0.x
// line. No I/O, no store, no scheduler wiring, no field-write path.

use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::TenantId;
use flexcore_events::events::{
    AssetAvailabilityChangedPayload, AssetCapabilityUpdatedPayload, TelemetryQualityChangedPayload,
};

use crate::flexibility_envelope::{EnvelopeMetadata, EnvelopeVersion};

/// The recorded default maximum envelope age: one hour. A CONFIGURATION
/// default (the ledger requires the bound to be configured and the default
/// recorded), not a report requirement.
pub const DEFAULT_MAX_AGE_MILLIS: i64 = 3_600_000;

/// The recorded policy statement (ledger P3.13 step 4).
pub const STALE_ENVELOPE_POLICY: &str = "A STALE envelope is never used for a new schedule: the scheduler recomputes the envelope or fails loudly — it never silently proceeds. A schedule referencing a superseded envelope version is rejected or re-derived. The checks are pure functions here; the enforcement wiring lands in P3.22/P3.24.";

/// The recorded debounce-guarantee statement (ledger P3.13 risk note).
pub const DEBOUNCE_GUARANTEE_STATEMENT: &str = "Debouncing delays only the recommended recomputation (RecomputationAdvice::Deferred). It never changes EnvelopeFreshness, and the hard gates — assert_usable_for_new_schedule and check_schedule_envelope_reference — ignore the debounce configuration entirely. Guarantees debounce cannot weaken: (1) a stale envelope is never used for a new schedule, and (2) a schedule referencing a superseded envelope version is rejected — in both cases regardless of the debounce window.";

// --- Configuration -----------------------------------------------------------

/// The staleness configuration. Both values are configuration (P3.13 step
/// 1 and the recorded risk note); the defaults are the recorded ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StalenessConfig {
    /// The maximum envelope age in milliseconds beyond which the envelope
    /// is stale regardless of events. `age == max_age` is not stale;
    /// `age > max_age` is (the P2.08 staleness convention).
    pub max_age_millis: i64,
    /// The debounce window in milliseconds: within this window after the
    /// last recomputation, a triggered recomputation is advised as
    /// `Deferred` instead of `Immediate`. `0` disables debouncing. This
    /// value NEVER affects the staleness verdict (see
    /// `DEBOUNCE_GUARANTEE_STATEMENT`).
    pub min_recompute_interval_millis: i64,
}

impl Default for StalenessConfig {
    /// The recorded defaults: the one-hour maximum age (recorded default)
    /// and no debouncing (the conservative starting point).
    fn default() -> Self {
        Self {
            max_age_millis: DEFAULT_MAX_AGE_MILLIS,
            min_recompute_interval_millis: 0,
        }
    }
}

// --- Triggers ----------------------------------------------------------------

/// One staleness trigger event. The three frozen §15.3 payloads are taken
/// verbatim from flexcore-events; `ForecastPublished` carries the forecast
/// version explicitly because the frozen payload cannot (the recorded
/// D-FX-9 gap: the caller reads the version from the fixture store via the
/// event's scope and key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeTriggerEvent {
    /// A telemetry value's quality state changed (§15.3, §10.4).
    TelemetryQualityChanged(TelemetryQualityChangedPayload),
    /// An asset's availability changed (§15.3).
    AssetAvailabilityChanged(AssetAvailabilityChangedPayload),
    /// An asset's capability set was updated (§15.3).
    AssetCapabilityUpdated(AssetCapabilityUpdatedPayload),
    /// A forecast was published (§15.3) with the observed forecast version
    /// (read from the fixture store; the frozen payload cannot carry it).
    ForecastPublished { forecast_version: String },
    /// A horizon rollover: a new horizon starts at the given start-inclusive
    /// instant (UTC millis).
    HorizonRollover { new_horizon_start_millis: i64 },
}

/// Why an envelope is stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StalenessReason {
    /// A telemetry quality change on a scoped asset.
    TelemetryQualityChanged { asset_id: String },
    /// An availability change on a scoped asset.
    AssetAvailabilityChanged { asset_id: String },
    /// A capability update on a scoped asset.
    AssetCapabilityUpdated { asset_id: String },
    /// A newer forecast version was published.
    ForecastSuperseded {
        /// The forecast version the envelope consumed.
        envelope_forecast_version: String,
        /// The published forecast version.
        published_forecast_version: String,
    },
    /// A new horizon the envelope does not cover.
    HorizonRollover,
    /// The configured maximum age was exceeded.
    MaxAgeExceeded {
        /// The envelope's age at evaluation time, millis.
        age_millis: i64,
        /// The configured bound.
        max_age_millis: i64,
    },
}

/// Whether an envelope is fresh or stale (the staleness verdict's first
/// half; the recomputation advice is separate — see the debounce
/// guarantee).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeFreshness {
    /// No trigger fired and the age bound holds.
    Fresh,
    /// The envelope is stale: it must not back a new schedule.
    Stale { reason: StalenessReason },
}

/// The recomputation advice belonging to a stale verdict. Debouncing turns
/// `Immediate` into `Deferred` — and nothing else (the recorded guarantee).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecomputationAdvice {
    /// Recompute now.
    Immediate,
    /// The debounce window is open: recompute no earlier than this UTC
    /// instant (millis). The envelope stays STALE until recomputation
    /// completes — deferral never re-validates it.
    Deferred {
        /// The earliest UTC millis the recomputation should run at.
        not_before_millis: i64,
    },
}

/// The full verdict of one trigger evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalenessVerdict {
    /// Fresh or stale (event trigger and the age bound).
    pub freshness: EnvelopeFreshness,
    /// The recomputation advice (debounce-aware; meaningless while fresh,
    /// `Immediate` by convention).
    pub recomputation: RecomputationAdvice,
}

// --- Schedule envelope reference (P3.21 consumes this) -----------------------

/// The envelope version a schedule was computed against. P3.21 attaches one
/// to every schedule record; the checks below gate its use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleEnvelopeReference {
    /// The envelope version the schedule used.
    pub envelope_version: EnvelopeVersion,
}

impl ScheduleEnvelopeReference {
    /// Wraps a validated envelope version into a schedule reference.
    pub fn new(envelope_version: EnvelopeVersion) -> Self {
        Self { envelope_version }
    }
}

/// Errors of the schedule-reference and usability checks (P3.13 step 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeReferenceError {
    /// The schedule references an envelope version that is not the current
    /// one: superseded. The schedule is rejected or re-derived — never
    /// silently used.
    EnvelopeSuperseded {
        /// The version the schedule referenced.
        referenced: String,
        /// The current envelope version.
        current: String,
    },
    /// The envelope is stale: a new schedule may not use it. The scheduler
    /// recomputes or fails loudly (never silently proceeds).
    StaleEnvelopeRejected { reason: StalenessReason },
}

impl std::fmt::Display for EnvelopeReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeReferenceError::EnvelopeSuperseded { referenced, current } => write!(
                f,
                "the schedule references envelope version {referenced}, superseded by {current}; reject or re-derive"
            ),
            EnvelopeReferenceError::StaleEnvelopeRejected { .. } => write!(
                f,
                "the envelope is stale; a new schedule may not use it — recompute or fail loudly"
            ),
        }
    }
}

impl std::error::Error for EnvelopeReferenceError {}

// --- The pure evaluation -----------------------------------------------------

/// Whether the event's asset is in the envelope's scope.
fn in_scope(metadata: &EnvelopeMetadata, asset_id: flexcore_domain::ids::AssetId) -> bool {
    metadata.scoped_asset_ids.contains(&asset_id)
}

/// Whether the event's tenant matches the envelope's tenant (a
/// foreign-tenant event never invalidates; P0.16 rule 5).
fn tenant_matches(metadata: &EnvelopeMetadata, payload_tenant: TenantId) -> bool {
    payload_tenant == metadata.tenant_id
}

/// The pure max-age evaluation (P3.13 step 1, the configured bound).
/// Convention: `age == max_age` is NOT stale; `age > max_age` is (P2.08).
/// A clock read before the computation timestamp (age <= 0) is not stale.
pub fn evaluate_max_age(
    metadata: &EnvelopeMetadata,
    config: &StalenessConfig,
    now: EventTime,
) -> EnvelopeFreshness {
    let age = now.unix_millis() - metadata.computed_at.unix_millis();
    if age > i128::from(config.max_age_millis) {
        EnvelopeFreshness::Stale {
            reason: StalenessReason::MaxAgeExceeded {
                age_millis: i64::try_from(age).unwrap_or(i64::MAX),
                max_age_millis: config.max_age_millis,
            },
        }
    } else {
        EnvelopeFreshness::Fresh
    }
}

/// Evaluates one trigger event against the current envelope metadata: the
/// pure, event-driven invalidation function (P3.13 step 3). The verdict
/// ALSO applies the configured maximum age bound — this is the one
/// function a runtime calls per event. Debouncing affects only the
/// recomputation advice, never the freshness (the recorded guarantee).
pub fn evaluate_envelope_trigger(
    metadata: &EnvelopeMetadata,
    event: &EnvelopeTriggerEvent,
    config: &StalenessConfig,
    now: EventTime,
    last_recompute_at: Option<EventTime>,
) -> StalenessVerdict {
    // The event trigger, if any.
    let triggered: Option<StalenessReason> = match event {
        EnvelopeTriggerEvent::TelemetryQualityChanged(payload) => {
            if tenant_matches(metadata, payload.tenant_id) && in_scope(metadata, payload.asset_id)
            {
                Some(StalenessReason::TelemetryQualityChanged {
                    asset_id: payload.asset_id.to_string(),
                })
            } else {
                None
            }
        }
        EnvelopeTriggerEvent::AssetAvailabilityChanged(payload) => {
            if tenant_matches(metadata, payload.tenant_id) && in_scope(metadata, payload.asset_id)
            {
                Some(StalenessReason::AssetAvailabilityChanged {
                    asset_id: payload.asset_id.to_string(),
                })
            } else {
                None
            }
        }
        EnvelopeTriggerEvent::AssetCapabilityUpdated(payload) => {
            if tenant_matches(metadata, payload.tenant_id) && in_scope(metadata, payload.asset_id)
            {
                Some(StalenessReason::AssetCapabilityUpdated {
                    asset_id: payload.asset_id.to_string(),
                })
            } else {
                None
            }
        }
        EnvelopeTriggerEvent::ForecastPublished { forecast_version } => {
            if *forecast_version != metadata.forecast_version {
                Some(StalenessReason::ForecastSuperseded {
                    envelope_forecast_version: metadata.forecast_version.clone(),
                    published_forecast_version: forecast_version.clone(),
                })
            } else {
                // The same forecast version republished: no invalidation
                // (durable dedupe happens in the outbox on the key).
                None
            }
        }
        EnvelopeTriggerEvent::HorizonRollover {
            new_horizon_start_millis,
        } => {
            // A new horizon the envelope cannot serve: its start lies at or
            // beyond the envelope's end-exclusive horizon end.
            if *new_horizon_start_millis >= metadata.horizon_end_millis {
                Some(StalenessReason::HorizonRollover)
            } else {
                None
            }
        }
    };

    // The freshness verdict: the event trigger first, then the age bound.
    let freshness = match triggered {
        Some(reason) => EnvelopeFreshness::Stale { reason },
        None => evaluate_max_age(metadata, config, now),
    };

    // The debounce-aware recomputation advice (freshness-independent).
    let recomputation = match &freshness {
        EnvelopeFreshness::Fresh => RecomputationAdvice::Immediate,
        EnvelopeFreshness::Stale { .. } => {
            match last_recompute_at {
                Some(last) if config.min_recompute_interval_millis > 0 => {
                    let since = now.unix_millis() - last.unix_millis();
                    if since < i128::from(config.min_recompute_interval_millis) {
                        let not_before = last.unix_millis()
                            + i128::from(config.min_recompute_interval_millis);
                        RecomputationAdvice::Deferred {
                            not_before_millis: i64::try_from(not_before)
                                .unwrap_or(i64::MAX),
                        }
                    } else {
                        RecomputationAdvice::Immediate
                    }
                }
                _ => RecomputationAdvice::Immediate,
            }
        }
    };

    StalenessVerdict {
        freshness,
        recomputation,
    }
}

/// The hard gate for a NEW schedule (P3.13 step 4): the envelope must be
/// fresh at `now`. A stale envelope is never used for a new schedule — the
/// scheduler recomputes or fails loudly; this function never reads the
/// debounce configuration (the recorded guarantee). The enforcement wiring
/// lands in P3.22/P3.24.
pub fn assert_usable_for_new_schedule(
    metadata: &EnvelopeMetadata,
    config: &StalenessConfig,
    now: EventTime,
) -> Result<(), EnvelopeReferenceError> {
    match evaluate_max_age(metadata, config, now) {
        EnvelopeFreshness::Fresh => Ok(()),
        EnvelopeFreshness::Stale { reason } => {
            Err(EnvelopeReferenceError::StaleEnvelopeRejected { reason })
        }
    }
}

/// The superseded-version gate (P3.13 step 4): a schedule referencing an
/// envelope version other than the current one is rejected (or re-derived
/// by the caller). The enforcement wiring lands in P3.22/P3.24.
pub fn check_schedule_envelope_reference(
    reference: &ScheduleEnvelopeReference,
    current: &EnvelopeMetadata,
) -> Result<(), EnvelopeReferenceError> {
    if reference.envelope_version != current.envelope_version {
        return Err(EnvelopeReferenceError::EnvelopeSuperseded {
            referenced: reference.envelope_version.as_str().to_string(),
            current: current.envelope_version.as_str().to_string(),
        });
    }
    Ok(())
}
