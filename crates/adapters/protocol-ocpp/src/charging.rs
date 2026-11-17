// P17.05 — The SetChargingProfile payload mapping: the domain composer's
// `ChargingProfilePlan` (flexcore_domain::ev_charging_composer) onto the
// wire `SetChargingProfile` request (crate::model), the
// GetCompositeSchedule reconciliation surface, and the lifecycle-typed
// relay gate.
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/05_p17_05_smart_charging_composer.md`.
//
// THE COMMAND-RELAY-ONLY RULE, ENFORCED BY TYPING (phase-17 context §3
// constraint 1; plan §7.4): a charging profile is a COMMAND. It enters the
// command lifecycle at PROPOSED and passes the validation, feasibility,
// and authorization gates like any other candidate. The wire payload can
// be built ONLY from an ISSUED lifecycle state
// ([`ChargingProfilePlan::into_relay_request`] takes the frozen
// `CommandState` and refuses everything else) — an adapter that constructs
// APPROVED or ISSUED from external input is a defect, not a shortcut, and
// this surface makes the defect unrepresentable.
//
// The "Accepted" honesty rule ([S-124]; record 05 §6): the charge point's
// Accepted response means the profile was STORED, not applied. The
// adapter records the status against the command lifecycle's
// ACKNOWLEDGED-class evidence and reconciles application through
// GetCompositeSchedule
// ([`parse_composite_schedule_response`] +
// [`flexcore_domain::ev_charging_composer::reconcile_composite_schedule`]);
// a mismatch is a named record surfaced to the operator — the adapter
// NEVER retries or re-issues on its own authority.
//
// Module-level UNVERIFIED register: the payload field spellings and the
// composite-schedule response shape are the model.rs register's (the OCA
// 2.0.1 Part 2 smart-charging text [S-124][S-141] is the
// implementation-time check).

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::{
    ChargingProfilePlan, CompositeScheduleObservation, PlanPeriod, RateUnit,
};

use crate::model::{
    ChargingProfileKind, ChargingProfilePurpose, ChargingRateUnit, ChargingSchedule,
    ChargingSchedulePeriod, SetChargingProfileRequest, SetChargingProfileStatus,
};

/// Charging-surface errors (named).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChargingError {
    /// The plan maps onto the wire payload only from an ISSUED lifecycle
    /// state (the command-relay-only rule; the found state is named).
    NotIssued {
        found: &'static str,
    },
    /// The plan's stack level exceeds the sanity bound carried from the
    /// model (i32 range on the wire — UNVERIFIED bound; a negative level
    /// is refused by the composer and re-checked here).
    InvalidStackLevel {
        value: i32,
    },
    /// The composite-schedule response could not be decoded (the shape
    /// did not match the recorded UNVERIFIED expectation — a refusal,
    /// never a guessed reconstruction).
    InvalidCompositeResponse {
        reason: &'static str,
    },
    /// The plan could not be encoded into the wire payload (a model
    /// validation failure re-raised; carries the model message).
    Model(String),
}

impl std::fmt::Display for ChargingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChargingError::NotIssued { found } => {
                write!(f, "the wire payload is built only from an ISSUED lifecycle state (found {found}); the command-relay-only rule refuses every other state")
            }
            ChargingError::InvalidStackLevel { value } => {
                write!(f, "the plan's stack level {value} is outside the permitted range")
            }
            ChargingError::InvalidCompositeResponse { reason } => {
                write!(f, "the composite-schedule response did not decode: {reason} (a refusal, never a guessed reconstruction)")
            }
            ChargingError::Model(message) => {
                write!(f, "the plan failed the wire-payload validation: {message}")
            }
        }
    }
}

impl std::error::Error for ChargingError {}

/// Maps a composed plan + an ISSUED lifecycle state onto the wire
/// SetChargingProfile request. EVERY other lifecycle state is refused
/// (the command-relay-only rule, enforced by typing): the plan is a
/// payload candidate produced upstream of the gates; this surface is
/// downstream of ISSUED only.
///
/// `charging_profile_id` is the CSMS-side profile id (declared by the
/// caller; the dispatch command id travels alongside the command, not
/// inside the OCPP payload).
pub fn into_relay_request(
    plan: &ChargingProfilePlan,
    lifecycle_state: CommandState,
    charging_profile_id: i64,
) -> Result<SetChargingProfileRequest, ChargingError> {
    // The lifecycle gate: ISSUED only (the adapter relays, never authorizes
    // and never issues).
    if lifecycle_state != CommandState::Issued {
        return Err(ChargingError::NotIssued {
            found: lifecycle_state.name(),
        });
    }
    if plan.stack_level < 0 {
        return Err(ChargingError::InvalidStackLevel {
            value: plan.stack_level,
        });
    }
    let purpose = match plan.purpose {
        flexcore_domain::ev_charging_composer::PlanPurpose::ChargingStationExternalConstraints => {
            ChargingProfilePurpose::ChargingStationExternalConstraints
        }
        flexcore_domain::ev_charging_composer::PlanPurpose::ChargingStationMaxProfile => {
            ChargingProfilePurpose::ChargingStationMaxProfile
        }
        flexcore_domain::ev_charging_composer::PlanPurpose::TxDefaultProfile => {
            ChargingProfilePurpose::TxDefaultProfile
        }
        flexcore_domain::ev_charging_composer::PlanPurpose::TxProfile => {
            ChargingProfilePurpose::TxProfile
        }
    };
    let rate_unit = match plan.rate_unit {
        RateUnit::W => ChargingRateUnit::W,
        RateUnit::A => ChargingRateUnit::A,
    };
    let request = SetChargingProfileRequest {
        charging_profile_id,
        evse_id: plan.evse_wire_id,
        is_grid_critical: plan.is_grid_critical,
        transaction_id: plan.transaction_id.clone(),
        stack_level: plan.stack_level,
        charging_profile_purpose: purpose,
        // The composer's declared default kind (Absolute — anchored to the
        // plan's start instant; the model register item 6 carries the
        // UNVERIFIED note).
        charging_profile_kind: ChargingProfileKind::Absolute,
        charging_schedule: ChargingSchedule {
            duration_s: None,
            start_schedule_millis: Some(plan.start_millis),
            charging_rate_unit: rate_unit,
            min_charging_rate: plan.min_charging_rate,
            charging_schedule_period: plan
                .periods
                .iter()
                .map(|period| ChargingSchedulePeriod {
                    start_period_s: period.start_offset_s,
                    limit: period.limit,
                    number_phases: period.number_phases,
                })
                .collect(),
        },
    };
    request
        .validate()
        .map_err(|error| ChargingError::Model(error.to_string()))?;
    Ok(request)
}

/// Records the charge point's SetChargingProfile response against the
/// stored-not-applied honesty rule. The return value pairs the status
/// with the statement that must travel with it (record 05 §6): an
/// `Accepted` is STORAGE evidence, never application evidence; application
/// evidence comes only from the reconciliation. No lifecycle transition is
/// performed here — the lifecycle engine owns its edges; this function is
/// the honest record the caller attaches to the command's tracking
/// evidence.
pub fn record_accepted_status(
    status: SetChargingProfileStatus,
) -> (&'static str, SetChargingProfileStatus) {
    (
        SetChargingProfileStatus::STORED_NOT_APPLIED_STATEMENT,
        status,
    )
}

/// Decodes the GetCompositeSchedule response payload into the domain
/// observation shape (the reconciliation input). The recorded UNVERIFIED
/// shape: `{"evseId": n, "schedule": {"duration": n, "startSchedule":
/// "...", "chargingRateUnit": "W"|"A", "chargingSchedulePeriod":
/// [{"startPeriod": n, "limit": x, "numberPhases": n?}]}}`. A mismatched
/// shape is a named refusal — never a guessed reconstruction; the unit
/// must MATCH the plan's (like-with-like comparison, the domain
/// reconciliation rule).
pub fn parse_composite_schedule_response(
    payload: &serde_json::Value,
    plan_rate_unit: RateUnit,
) -> Result<CompositeScheduleObservation, ChargingError> {
    let schedule = payload
        .get("schedule")
        .ok_or(ChargingError::InvalidCompositeResponse {
            reason: "the 'schedule' object is missing",
        })?;
    let unit_name = schedule
        .get("chargingRateUnit")
        .and_then(|value| value.as_str())
        .ok_or(ChargingError::InvalidCompositeResponse {
            reason: "the schedule's chargingRateUnit is missing",
        })?;
    let observed_unit = match unit_name {
        "W" => RateUnit::W,
        "A" => RateUnit::A,
        _ => {
            return Err(ChargingError::InvalidCompositeResponse {
                reason: "the schedule's chargingRateUnit is not W or A",
            })
        }
    };
    if observed_unit != plan_rate_unit {
        return Err(ChargingError::InvalidCompositeResponse {
            reason: "the schedule's rate unit does not match the plan's (like-with-like \
                     comparison only; the conversion belongs to the caller's declared mapping)",
        });
    }
    let periods_json = schedule
        .get("chargingSchedulePeriod")
        .and_then(|value| value.as_array())
        .ok_or(ChargingError::InvalidCompositeResponse {
            reason: "the schedule's chargingSchedulePeriod array is missing",
        })?;
    let mut periods: Vec<PlanPeriod> = Vec::with_capacity(periods_json.len());
    let mut previous: Option<u32> = None;
    for item in periods_json {
        let start_period_s = item
            .get("startPeriod")
            .and_then(|value| value.as_u64())
            .ok_or(ChargingError::InvalidCompositeResponse {
                reason: "a period's startPeriod is missing or not an unsigned integer",
            })?;
        let start_period_s = u32::try_from(start_period_s).map_err(|_| {
            ChargingError::InvalidCompositeResponse {
                reason: "a period's startPeriod exceeds the u32 range",
            }
        })?;
        let limit = item
            .get("limit")
            .and_then(|value| value.as_f64())
            .ok_or(ChargingError::InvalidCompositeResponse {
                reason: "a period's limit is missing or not a number",
            })?;
        if limit < 0.0 {
            return Err(ChargingError::InvalidCompositeResponse {
                reason: "a period's limit is negative",
            });
        }
        if let Some(prev) = previous {
            if start_period_s <= prev {
                return Err(ChargingError::InvalidCompositeResponse {
                    reason: "the observed periods are not strictly ascending",
                });
            }
        }
        previous = Some(start_period_s);
        periods.push(PlanPeriod {
            start_offset_s: start_period_s,
            limit,
            number_phases: item
                .get("numberPhases")
                .and_then(|value| value.as_u64())
                .and_then(|value| u8::try_from(value).ok()),
        });
    }
    Ok(CompositeScheduleObservation {
        rate_unit: observed_unit,
        periods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_domain::ev_charging_composer::{
        compose_external_constraints_profile, compose_tx_profile, DeviceRateConstraints,
        DispatchEnvelope, DispatchPeriod, SupplyContext,
    };

    fn envelope() -> DispatchEnvelope {
        DispatchEnvelope {
            start_millis: 1_700_000_000_000,
            periods: vec![
                DispatchPeriod {
                    start_offset_s: 0,
                    limit_mw: 0.1,
                },
                DispatchPeriod {
                    start_offset_s: 900,
                    limit_mw: 0.0,
                },
            ],
        }
    }

    fn plan() -> ChargingProfilePlan {
        compose_external_constraints_profile(
            &envelope(),
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0,
            },
            &DeviceRateConstraints {
                min_charging_rate: None,
            },
            10,
            true,
        )
        .expect("composed")
    }

    /// The command-relay-only rule: every non-ISSUED state is refused,
    /// named.
    #[test]
    fn relay_is_issued_only() {
        let plan = plan();
        for state in CommandState::ALL {
            if state == CommandState::Issued {
                continue;
            }
            let error =
                into_relay_request(&plan, state, 1).expect_err("non-issued refused");
            assert!(
                error.to_string().contains(state.name()),
                "the error names the found state: {error}"
            );
        }
        // External input cannot construct APPROVED or ISSUED here: the
        // function takes the frozen state enum, and no path exists from an
        // external payload to a lifecycle state.
        let request = into_relay_request(&plan, CommandState::Issued, 42).expect("issued");
        assert_eq!(request.charging_profile_id, 42);
        assert_eq!(request.evse_id, 0);
        assert_eq!(request.is_grid_critical, Some(true));
        assert_eq!(
            request.charging_profile_purpose,
            ChargingProfilePurpose::ChargingStationExternalConstraints
        );
        assert_eq!(request.charging_profile_kind, ChargingProfileKind::Absolute);
        assert_eq!(
            request.charging_schedule.start_schedule_millis,
            Some(1_700_000_000_000)
        );
        assert_eq!(request.charging_schedule.charging_rate_unit, ChargingRateUnit::W);
        assert_eq!(request.charging_schedule.charging_schedule_period.len(), 2);
        assert_eq!(request.charging_schedule.charging_schedule_period[0].limit, 100_000.0);
        assert_eq!(request.charging_schedule.charging_schedule_period[1].limit, 0.0);
    }

    #[test]
    fn tx_profile_relay_carries_the_transaction_and_refuses_station_scope() {
        let plan = compose_tx_profile(
            &envelope(),
            RateUnit::A,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0,
            },
            &DeviceRateConstraints {
                min_charging_rate: None,
            },
            3,
            7,
            "txn-9",
        )
        .expect("composed");
        let request = into_relay_request(&plan, CommandState::Issued, 7).expect("issued");
        assert_eq!(request.transaction_id.as_deref(), Some("txn-9"));
        assert_eq!(request.evse_id, 3);
        assert_eq!(request.charging_schedule.charging_rate_unit, ChargingRateUnit::A);
        request.validate().expect("wire-valid");
    }

    #[test]
    fn accepted_status_travels_with_the_stored_not_applied_statement() {
        for status in [
            SetChargingProfileStatus::Accepted,
            SetChargingProfileStatus::Rejected,
        ] {
            let (statement, carried) = record_accepted_status(status);
            assert_eq!(carried, status);
            assert!(statement.contains("STORED, not applied"));
        }
        // No lifecycle transition happens here: the function returns
        // records, never states.
    }

    #[test]
    fn composite_response_decodes_and_refuses_mismatched_shapes() {
        let payload = serde_json::json!({
            "evseId": 0,
            "schedule": {
                "duration": 1800,
                "chargingRateUnit": "W",
                "chargingSchedulePeriod": [
                    {"startPeriod": 0, "limit": 100000.0},
                    {"startPeriod": 900, "limit": 0.0}
                ]
            }
        });
        let observation = parse_composite_schedule_response(&payload, RateUnit::W)
            .expect("decoded");
        assert_eq!(observation.rate_unit, RateUnit::W);
        assert_eq!(observation.periods.len(), 2);
        assert_eq!(observation.periods[0].limit, 100_000.0);

        // A unit mismatch is refused (like-with-like only).
        assert!(parse_composite_schedule_response(&payload, RateUnit::A).is_err());

        // Missing schedule object.
        assert!(parse_composite_schedule_response(&serde_json::json!({}), RateUnit::W).is_err());
        // Unknown rate unit.
        let bad = serde_json::json!({"schedule": {"chargingRateUnit": "kW", "chargingSchedulePeriod": []}});
        assert!(parse_composite_schedule_response(&bad, RateUnit::W).is_err());
        // Negative limit.
        let bad = serde_json::json!({"schedule": {"chargingRateUnit": "W", "chargingSchedulePeriod": [{"startPeriod": 0, "limit": -1.0}]}});
        assert!(parse_composite_schedule_response(&bad, RateUnit::W).is_err());
        // Non-ascending observed periods.
        let bad = serde_json::json!({"schedule": {"chargingRateUnit": "W", "chargingSchedulePeriod": [
            {"startPeriod": 900, "limit": 0.0}, {"startPeriod": 900, "limit": 1.0}]}});
        assert!(parse_composite_schedule_response(&bad, RateUnit::W).is_err());
        // Missing limit.
        let bad = serde_json::json!({"schedule": {"chargingRateUnit": "W", "chargingSchedulePeriod": [{"startPeriod": 0}]}});
        assert!(parse_composite_schedule_response(&bad, RateUnit::W).is_err());
    }

    #[test]
    fn end_to_end_plan_relay_reconcile_cycle_is_honest() {
        // Compose -> relay (ISSUED only) -> Accepted (stored) -> observe
        // the composite -> reconcile.
        let plan = plan();
        let request = into_relay_request(&plan, CommandState::Issued, 1).expect("issued");
        let payload = serde_json::to_value(&request).expect("serializes");

        // The station reports Accepted: storage evidence only.
        let (statement, _) = record_accepted_status(SetChargingProfileStatus::Accepted);
        assert!(statement.contains("GetCompositeSchedule reconciliation"));

        // The station's composite matches: aligned (application evidence
        // CONSISTENT — never a delivery guarantee).
        let observation =
            parse_composite_schedule_response(&payload, plan.rate_unit).expect("decoded");
        let outcome = flexcore_domain::ev_charging_composer::reconcile_composite_schedule(
            &plan,
            &observation,
        );
        assert_eq!(
            outcome,
            flexcore_domain::ev_charging_composer::ReconciliationOutcome::Aligned
        );

        // A station that composes another profile on top surfaces a named
        // mismatch — never a silent pass and never an adapter-side retry.
        let drifted = serde_json::json!({
            "schedule": {
                "chargingRateUnit": "W",
                "chargingSchedulePeriod": [
                    {"startPeriod": 0, "limit": 50000.0}
                ]
            }
        });
        let observation = parse_composite_schedule_response(&drifted, plan.rate_unit)
            .expect("decoded");
        let outcome = flexcore_domain::ev_charging_composer::reconcile_composite_schedule(
            &plan,
            &observation,
        );
        match outcome {
            flexcore_domain::ev_charging_composer::ReconciliationOutcome::Mismatch {
                details,
            } => assert!(!details.is_empty(), "the mismatch is named"),
            other => panic!("expected a named mismatch, got {other:?}"),
        }
        // The plan carries the precedence statement verbatim.
        assert_eq!(plan.precedence_statement, flexcore_domain::ev_charging_composer::STACK_PRECEDENCE_STATEMENT);
    }
}
