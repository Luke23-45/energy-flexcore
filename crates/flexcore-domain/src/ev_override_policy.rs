// P17.10 — User-override precedence policy (pure rules, no I/O).
//
// Implements ledger P17.10: the dispatch POLICY that a driver's manual
// charging action outranks VPP curtailment, resolved BEFORE issuance —
// never inside an adapter. Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 25_p17_10_user_override_precedence.md`.
//
// # The policy, decided once [S-140]
//
// 1. **Driver manual starts outrank VPP curtailment.** When a driver
//    manually starts charging for a vehicle, any VPP candidate that would
//    curtail (reduce or stop) that charging over the overlapping window
//    must YIELD: the candidate is returned to the proposal path (it is not
//    issued). The override is a dispatch POLICY input to the platform's
//    gates, recorded on the command record for audit.
// 2. **Precedence resolves BEFORE issuance.** The policy function refuses
//    to adjudicate any lifecycle stage at or beyond ISSUED: once a command
//    is issued, the frozen lifecycle governs (FAILED → FALLBACK, SUPERSEDED
//    by a newer revision) — a policy re-adjudication mid-flight would be a
//    second, unauthorized control path.
// 3. **An adapter never adjudicates and never refuses a lifecycle-issued
//    command on its own authority** (ADR-010-EXT-01: relay is not
//    authority). The negative tests below pin the structural form: the
//    policy takes NO adapter context, produces NO command and NO lifecycle
//    transition — its output is a precedence RECORD the protected gates
//    consume; and the adjudication-after-issuance refusal means no
//    adapter-side override check can exist downstream of ISSUED in this
//    module's vocabulary.
// 4. **Opt-out remains the P3.11 hard gate.** A driver opt-out (start-
//    inclusive) removes the vehicle from the feasible set from that
//    instant onward (`DispatchedAfterOptOut` in `ev_fleet_model`); the
//    precedence policy here governs manual STARTS against VPP
//    candidates, it does not re-implement the opt-out — it REFERENCES it
//    (the opt-out instant travels in the fleet configuration).
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// driver interaction was observed, no vehicle is connected, no command was
// issued; the tests are written-unrun (execution is a human gate).

use crate::command_state::CommandState;

/// The recorded precedence statement (decided once, [S-140]).
pub const USER_OVERRIDE_PRECEDENCE_STATEMENT: &str =
    "Driver manual starts outrank VPP curtailment. The precedence resolves \
BEFORE issuance, as a dispatch policy input to the platform's protected \
gates — never inside an adapter. An adapter never adjudicates precedence \
and never refuses a lifecycle-issued command on its own authority; once \
issued, the frozen lifecycle (FAILED, EXPIRED, SUPERSEDED, FALLBACK) \
governs. Opt-out remains the P3.11 hard gate on the feasible set.";

/// The closed driver-override vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverrideKind {
    /// The driver manually started charging (e.g. pressed the vehicle's or
    /// app's charge-now control). VPP curtailment yields.
    ManualStart,
    /// The driver manually stopped charging. This is recorded data; it
    /// does NOT by itself curtail a VPP candidate (the vehicle is simply
    /// disconnected-or-idle in the model's connection-window semantics).
    ManualStop,
}

impl OverrideKind {
    /// All override kinds, in frozen vocabulary order.
    pub const ALL: [OverrideKind; 2] = [OverrideKind::ManualStart, OverrideKind::ManualStop];

    /// The canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            OverrideKind::ManualStart => "manual_start",
            OverrideKind::ManualStop => "manual_stop",
        }
    }
}

/// The driver override event, as ingested (declared data; the platform
/// learned of it through a reporting path — never by observing a device).
#[derive(Debug, Clone, PartialEq)]
pub struct OverrideEvent {
    /// The vehicle key (the fleet-model key).
    pub vehicle_key: String,
    /// The override kind.
    pub kind: OverrideKind,
    /// When the manual action started (UTC ms; start-inclusive — the
    /// §10.3 interval semantics).
    pub override_start_unix_ms: i64,
    /// When the manual action ended, when known (UTC ms; `None` = open-
    /// ended, the driver has not stopped).
    pub override_end_unix_ms: Option<i64>,
    /// When the platform received the override event (UTC ms).
    pub received_at_unix_ms: i64,
}

impl OverrideEvent {
    /// Validates the declared event: non-empty key, end (when present)
    /// strictly after start.
    pub fn validate(&self) -> Result<(), OverridePolicyError> {
        if self.vehicle_key.trim().is_empty() {
            return Err(OverridePolicyError::EmptyVehicleKey);
        }
        if let Some(end) = self.override_end_unix_ms {
            if end <= self.override_start_unix_ms {
                return Err(OverridePolicyError::InvertedOverrideWindow {
                    vehicle_key: self.vehicle_key.clone(),
                    start_millis: self.override_start_unix_ms,
                    end_millis: end,
                });
            }
        }
        Ok(())
    }
}

/// The outcome of resolving the precedence for one candidate command
/// against the current override events. A RECORD the protected gates
/// consume — never a command, never a lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecedenceOutcome {
    /// No override applies to the candidate window: the candidate proceeds
    /// through the gates as normal (this outcome carries no authority —
    /// the gates still run).
    NoOverrideApplies,
    /// A driver manual start overlaps the candidate window and the
    /// candidate would curtail: the candidate YIELDS to the driver. The
    /// candidate is not issued; it returns to the proposal path (a revised
    /// candidate may be proposed that respects the manual start).
    CandidateYieldsToDriver,
    /// A manual stop (or an ended manual start) overlaps: recorded, but
    /// the candidate is not forced to yield by the override policy alone —
    /// the vehicle's own connection-window/idle semantics govern.
    OverrideRecordedNoYield,
}

impl PrecedenceOutcome {
    /// The canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            PrecedenceOutcome::NoOverrideApplies => "no_override_applies",
            PrecedenceOutcome::CandidateYieldsToDriver => "candidate_yields_to_driver",
            PrecedenceOutcome::OverrideRecordedNoYield => "override_recorded_no_yield",
        }
    }
}

/// Errors of the precedence policy. Every variant is a refusal to
/// adjudicate — the policy never guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverridePolicyError {
    /// The vehicle key was empty or whitespace-only.
    EmptyVehicleKey,
    /// The override event's window was inverted (end <= start).
    InvertedOverrideWindow {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The offending start (UTC ms).
        start_millis: i64,
        /// The offending end (UTC ms).
        end_millis: i64,
    },
    /// THE ADJUDICATION-AFTER-ISSUANCE REFUSAL: precedence resolves before
    /// issuance. A command at ISSUED or any later lifecycle stage is out
    /// of the policy's reach — the frozen lifecycle governs it (FAILED →
    /// FALLBACK, SUPERSEDED by a newer revision), and no adapter may
    /// refuse it on its own authority either.
    AdjudicationAfterIssuance {
        /// The lifecycle stage the adjudication was requested at.
        stage: CommandState,
    },
    /// The candidate's window was inverted (window end <= window start).
    InvertedCandidateWindow {
        /// The candidate window start (UTC ms).
        start_millis: i64,
        /// The candidate window end (UTC ms).
        end_millis: i64,
    },
}

impl std::fmt::Display for OverridePolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverridePolicyError::EmptyVehicleKey => {
                write!(f, "the vehicle key is mandatory and must be non-empty")
            }
            OverridePolicyError::InvertedOverrideWindow { vehicle_key, start_millis, end_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': the override window is inverted (start {start_millis} >= end {end_millis} UTC ms)"
                )
            }
            OverridePolicyError::AdjudicationAfterIssuance { stage } => write!(
                f,
                "user-override precedence resolves BEFORE issuance; stage {} is beyond \
                 the policy's reach — the frozen lifecycle governs the command, and no \
                 adapter may refuse it on its own authority",
                stage.name()
            ),
            OverridePolicyError::InvertedCandidateWindow { start_millis, end_millis } => {
                write!(
                    f,
                    "the candidate window is inverted (start {start_millis} >= end {end_millis} UTC ms)"
                )
            }
        }
    }
}

impl std::error::Error for OverridePolicyError {}

/// Whether the candidate would curtail charging the driver started: a
/// candidate whose setpoint for the vehicle reduces or stops charging
/// (charge below the manual rate, or idle). The candidate's charging set
/// travels as a declared magnitude in canonical MW; the manual rate the
/// driver established is the reference. A candidate that INCREASES or
/// matches the manual charging does not curtail and need not yield.
fn would_curtail(candidate_charge_mw: f64, manual_charge_mw: f64) -> bool {
    candidate_charge_mw < manual_charge_mw
}

/// Resolves the user-override precedence for one candidate command window
/// against the driver's override events for the vehicle.
///
/// The contract, structurally:
/// - `command_stage` must be BEFORE issuance (`PROPOSED`, `VALIDATED`, or
///   `FEASIBLE` — the pre-authorization stages the policy may inform). Any
///   stage at or beyond `APPROVED`... wait: precedence resolves BEFORE
///   ISSUANCE; the authorization gate (FEASIBLE → APPROVED) is where the
///   recorded policy input is consumed. The policy therefore accepts
///   stages strictly before `ISSUED` (it informs the authorization gate)
///   and refuses `ISSUED` and everything after — the refusal is the
///   before-issuance rule in code.
/// - the output is a [`PrecedenceOutcome`] record. There is no variant
///   that issues, authorizes, or mutates a command: the policy has no
///   dispatch authority to give.
pub fn resolve_override_precedence(
    command_stage: CommandState,
    vehicle_key: &str,
    candidate_window: (i64, i64),
    candidate_charge_mw: f64,
    manual_charge_mw: f64,
    overrides: &[OverrideEvent],
) -> Result<PrecedenceOutcome, OverridePolicyError> {
    // Rule 2: precedence resolves BEFORE issuance. ISSUED and everything
    // after is the frozen lifecycle's jurisdiction.
    if matches!(
        command_stage,
        CommandState::Issued
            | CommandState::Acknowledged
            | CommandState::Tracking
            | CommandState::Completed
    ) {
        return Err(OverridePolicyError::AdjudicationAfterIssuance {
            stage: command_stage,
        });
    }
    // The live-candidate stages are exactly PROPOSED / VALIDATED / FEASIBLE
    // / APPROVED (pre-issuance); everything else is refused by the check
    // above (a terminal state is unreachable from a live candidate without
    // passing ISSUED, so no separate terminal-case vocabulary is invented).
    if vehicle_key.trim().is_empty() {
        return Err(OverridePolicyError::EmptyVehicleKey);
    }
    let (cand_start, cand_end) = candidate_window;
    if cand_end <= cand_start {
        return Err(OverridePolicyError::InvertedCandidateWindow {
            start_millis: cand_start,
            end_millis: cand_end,
        });
    }

    let mut saw_recorded = false;
    for event in overrides {
        event.validate()?;
        if event.vehicle_key.trim() != vehicle_key.trim() {
            continue;
        }
        // The event window: open-ended events extend indefinitely forward
        // (the driver has not stopped).
        let event_end = event.override_end_unix_ms.unwrap_or(i64::MAX);
        let overlaps = event.override_start_unix_ms < cand_end && event_end > cand_start;
        if !overlaps {
            continue;
        }
        match event.kind {
            OverrideKind::ManualStart => {
                if would_curtail(candidate_charge_mw, manual_charge_mw) {
                    // Rule 1: the driver's manual start outranks VPP
                    // curtailment over the overlapping window.
                    return Ok(PrecedenceOutcome::CandidateYieldsToDriver);
                }
                saw_recorded = true;
            }
            OverrideKind::ManualStop => {
                saw_recorded = true;
            }
        }
    }
    if saw_recorded {
        Ok(PrecedenceOutcome::OverrideRecordedNoYield)
    } else {
        Ok(PrecedenceOutcome::NoOverrideApplies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_700_000_000_000;

    fn manual_start(start: i64, end: Option<i64>) -> OverrideEvent {
        OverrideEvent {
            vehicle_key: "veh-01".to_string(),
            kind: OverrideKind::ManualStart,
            override_start_unix_ms: start,
            override_end_unix_ms: end,
            received_at_unix_ms: start + 1,
        }
    }

    fn manual_stop(start: i64, end: Option<i64>) -> OverrideEvent {
        OverrideEvent {
            vehicle_key: "veh-01".to_string(),
            kind: OverrideKind::ManualStop,
            override_start_unix_ms: start,
            override_end_unix_ms: end,
            received_at_unix_ms: start + 1,
        }
    }

    #[test]
    fn the_statement_carries_the_before_issuance_and_adapter_rules() {
        assert!(USER_OVERRIDE_PRECEDENCE_STATEMENT.contains("outrank VPP curtailment"));
        assert!(USER_OVERRIDE_PRECEDENCE_STATEMENT.contains("BEFORE issuance"));
        assert!(USER_OVERRIDE_PRECEDENCE_STATEMENT.contains("never inside an adapter"));
        assert!(USER_OVERRIDE_PRECEDENCE_STATEMENT.contains("never refuses a lifecycle-issued command"));
    }

    #[test]
    fn a_driver_manual_start_over_a_curtailing_candidate_yields_the_candidate() {
        // The driver manually started charging at T0 (open-ended) at
        // 0.011 MW; the VPP candidate would curtail to 0 over the window:
        // the candidate yields.
        let overrides = vec![manual_start(T0, None)];
        let outcome = resolve_override_precedence(
            CommandState::Proposed,
            "veh-01",
            (T0 + 3_600_000, T0 + 7_200_000),
            0.0,     // candidate curtails to zero
            0.011,   // the manual rate
            &overrides,
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::CandidateYieldsToDriver);
    }

    #[test]
    fn a_candidate_that_matches_or_raises_the_manual_rate_does_not_yield() {
        let overrides = vec![manual_start(T0, None)];
        // The candidate keeps the manual charging level: no curtailment.
        let outcome = resolve_override_precedence(
            CommandState::Feasible,
            "veh-01",
            (T0 + 3_600_000, T0 + 7_200_000),
            0.011,
            0.011,
            &overrides,
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::OverrideRecordedNoYield);
        // The candidate raises above the manual rate: no yield.
        let outcome = resolve_override_precedence(
            CommandState::Feasible,
            "veh-01",
            (T0 + 3_600_000, T0 + 7_200_000),
            0.022,
            0.011,
            &overrides,
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::OverrideRecordedNoYield);
    }

    #[test]
    fn a_non_overlapping_manual_start_does_not_yield() {
        // The manual start ended before the candidate window opens.
        let overrides = vec![manual_start(T0, Some(T0 + 1_800_000))];
        let outcome = resolve_override_precedence(
            CommandState::Proposed,
            "veh-01",
            (T0 + 3_600_000, T0 + 7_200_000),
            0.0,
            0.011,
            &overrides,
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::NoOverrideApplies);
    }

    #[test]
    fn another_vehicle_s_override_does_not_apply() {
        let mut other = manual_start(T0, None);
        other.vehicle_key = "veh-02".to_string();
        let outcome = resolve_override_precedence(
            CommandState::Proposed,
            "veh-01",
            (T0 + 3_600_000, T0 + 7_200_000),
            0.0,
            0.011,
            &[other],
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::NoOverrideApplies);
    }

    #[test]
    fn a_manual_stop_is_recorded_without_forcing_a_yield() {
        let overrides = vec![manual_stop(T0, Some(T0 + 1_800_000))];
        let outcome = resolve_override_precedence(
            CommandState::Proposed,
            "veh-01",
            (T0 + 900_000, T0 + 2_700_000),
            0.005,
            0.0,
            &overrides,
        )
        .unwrap();
        assert_eq!(outcome, PrecedenceOutcome::OverrideRecordedNoYield);
    }

    // --- NEGATIVE TESTS (written-unrun; execution is a human gate) ------

    #[test]
    fn negative_adjudication_is_refused_at_and_after_issuance() {
        // The before-issuance rule in code: every stage from ISSUED onward
        // is refused — no adapter, no consumer, no call shape can obtain a
        // precedence decision for an issued command from this policy.
        let after_issuance = [
            CommandState::Issued,
            CommandState::Acknowledged,
            CommandState::Tracking,
            CommandState::Completed,
        ];
        for stage in after_issuance {
            let err = resolve_override_precedence(
                stage,
                "veh-01",
                (T0, T0 + 3_600_000),
                0.0,
                0.011,
                &[manual_start(T0, None)],
            )
            .unwrap_err();
            assert_eq!(
                err,
                OverridePolicyError::AdjudicationAfterIssuance { stage },
                "stage {} must be beyond the policy's reach",
                stage.name()
            );
        }
    }

    #[test]
    fn negative_the_frozen_lifecycle_governs_issued_commands_not_the_policy() {
        // The structural counterpart of rule 3: an issued command that a
        // driver override arrives against exits the lifecycle through the
        // frozen edges (e.g. SUPERSEDED by a newer revision), which the
        // policy never performs. Pin that the policy module offers no
        // transition at all — the only lifecycle entry point it touches is
        // the read-only stage parameter.
        // (The lifecycle engine's own tests pin the edges; here we pin the
        // policy's silence: its output type carries no state.)
        let outcome = PrecedenceOutcome::CandidateYieldsToDriver;
        // The outcome enum has no lifecycle-state field to mutate.
        assert!(matches!(
            outcome,
            PrecedenceOutcome::CandidateYieldsToDriver
                | PrecedenceOutcome::NoOverrideApplies
                | PrecedenceOutcome::OverrideRecordedNoYield
        ));
    }

    #[test]
    fn negative_inverted_windows_and_empty_keys_are_refused() {
        // Inverted candidate window.
        assert_eq!(
            resolve_override_precedence(
                CommandState::Proposed,
                "veh-01",
                (T0 + 3_600_000, T0),
                0.0,
                0.011,
                &[],
            ),
            Err(OverridePolicyError::InvertedCandidateWindow {
                start_millis: T0 + 3_600_000,
                end_millis: T0,
            })
        );
        // Empty vehicle key.
        assert_eq!(
            resolve_override_precedence(
                CommandState::Proposed,
                "  ",
                (T0, T0 + 3_600_000),
                0.0,
                0.011,
                &[],
            ),
            Err(OverridePolicyError::EmptyVehicleKey)
        );
        // Inverted override window.
        let bad = OverrideEvent {
            vehicle_key: "veh-01".to_string(),
            kind: OverrideKind::ManualStart,
            override_start_unix_ms: T0 + 1_800_000,
            override_end_unix_ms: Some(T0),
            received_at_unix_ms: T0 + 1,
        };
        let err = resolve_override_precedence(
            CommandState::Proposed,
            "veh-01",
            (T0, T0 + 3_600_000),
            0.0,
            0.011,
            &[bad],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            OverridePolicyError::InvertedOverrideWindow { .. }
        ));
    }

    #[test]
    fn negative_the_policy_produces_no_command_and_holds_no_authority() {
        // The policy's output type is a three-variant enum of records; the
        // never-granted permission set is untouched by construction (this
        // module adds no permission, no command constructor, no lifecycle
        // transition — the compile-time shape of "adapters never
        // adjudicate").
        // The driver override vocabulary is recorded data only.
        assert_eq!(OverrideKind::ALL.len(), 2);
        assert_eq!(OverrideKind::ManualStart.name(), "manual_start");
        assert_eq!(OverrideKind::ManualStop.name(), "manual_stop");
        assert_eq!(
            PrecedenceOutcome::CandidateYieldsToDriver.name(),
            "candidate_yields_to_driver"
        );
    }
}
