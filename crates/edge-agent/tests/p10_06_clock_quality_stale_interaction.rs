// P10.06 — Stale/missing telemetry handling verification (release-gate
//         slice, edge crate): the STALE-telemetry × clock-drift interaction
//         — the hold-on-drift rule on the edge command path (report §22.3
//         gate 5; §10.3–§10.4; §17.4; §17.5 rule 5; §14.3; ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md` P10.06;
//         record:
//         `ledger/phase-10-v1-release-hardening/06_telemetry_handling_tests.md`).
//
// WHAT THIS FILE PROVES (over `crates/edge-agent/src/clock_quality.rs`,
// P6.31 — the edge clock-quality owner; the P6.15 verifier consumes its
// status as a supplied parameter):
//
// 1. STALE ARRIVES AS STALE. A communication-loss source state maps to the
//   canonical `STALE` at the adapter boundary (`flexcore-contracts::
//   map_source_state`) — the edge never upgrades or derives quality
//   (P6.08: no default quality, no inferred GOOD anywhere on the path).
// 2. THE HOLD-ON-DRIFT RULE. Expiry checks are trusted ONLY on
//   `Synchronized`. On `Degraded` (drift beyond tolerance), `Unsynchronized`
//   (source quiet past the sync timeout, or a noted outage) and `Unknown`
//   (no observation yet — never healthy-by-default), the edge does NOT
//   trust expiry checks: commands are rejected/held fail-closed and the
//   condition is AUDITED (`audited_expiry_check` records
//   `EDGE_CLOCK_HOLD_ON_DRIFT`; `ClockQualityMonitor::expiry_check_allowed`
//   returns the named reason).
// 3. THE INTERACTION. A stale-telemetry window and a degraded clock are
//   INDEPENDENT fail-closed surfaces: staleness is judged on event_time
//   age (contracts time model), the clock hold on the sync-source offset
//   estimate — a command consumed in a window where the telemetry went
//   STALE AND the clock is Degraded hits BOTH gates; neither substitutes,
//   neither passes. The health record renders the clock Degraded as
//   `DEGRADED_CLOCK_DRIFT` while ingest keeps flowing (the COMMAND path is
//   what fails closed — through the verifier seam, not through health).
// 4. HAND-COMPUTED BOUNDARIES: tolerance inclusive at exactly the
//   configured offset (pilot default 100 ms), Degraded one millis beyond;
//   sync-timeout boundary exactly at the configured window (pilot default
//   30 000 ms) still trusted, one millis past → Unsynchronized; a corrupt
//   sample pair (offset ≥ one day) is refused, never estimated, and leaves
//   the state untouched.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed; the human build/test gate owns:
// `cargo test -p flexcore-edge-agent --test p10_06_clock_quality_stale_interaction`).
// Expected strings are the module's own constants. No P9.18 freshness-limit
// value is invented here: freshness limits are per-use-case configuration,
// proven in the application-crate P10.06 file.

use std::sync::Arc;

use flexcore_contracts::quality::{map_source_state, SourceQualityState, TelemetryQuality};
use flexcore_edge_agent::clock_quality::{
    audited_expiry_check, ClockQualityError, ClockQualityMonitor, ClockQualityState,
    SyncSourceConfig, TimeSamplePair, TimeSyncProfile, DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS,
    DEFAULT_PILOT_TOLERANCE_MILLIS, MAX_ABS_OFFSET_MILLIS,
};
use flexcore_ports::fakes::FixedClock;

const BASE: i64 = 1_700_000_000_000;

fn monitor() -> ClockQualityMonitor {
    let config =
        SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "site-time-service-1".to_string());
    ClockQualityMonitor::new(
        config,
        Arc::new(FixedClock::at_unix_millis(BASE)),
        Arc::new(flexcore_edge_agent::ports::InMemoryAuditSink::new()),
    )
    .expect("valid config")
}

/// A time-sample pair whose offset is `edge - reference` (positive = the
/// edge clock runs ahead).
fn pair(offset_millis: i64, at: i64) -> TimeSamplePair {
    TimeSamplePair {
        reference_unix_millis: at - offset_millis,
        edge_unix_millis: at,
        observed_at_unix_millis: at,
    }
}

// --- 1. STALE arrives as STALE (the adapter-boundary mapping) --------------------------

#[test]
fn communication_loss_maps_to_stale_and_no_source_state_upgrades_to_good() {
    // The §10.4 mapping table (P0.08): COMMUNICATION_LOSS → STALE; none of
    // the source states ever maps to GOOD — a stale value keeps its flag
    // on the edge path (P6.08 non-silencing).
    assert_eq!(
        map_source_state(SourceQualityState::CommunicationLoss),
        TelemetryQuality::Stale
    );
    for source in SourceQualityState::ALL {
        assert_ne!(
            map_source_state(source),
            TelemetryQuality::Good,
            "quality is never upgraded on the edge path"
        );
    }
}

// --- 2. The hold-on-drift rule ----------------------------------------------------------

#[test]
fn expiry_checks_are_trusted_only_on_synchronized() {
    // The closed four-state vocabulary, each with its documented trust:
    let states = [
        (ClockQualityState::Unknown, false),        // never healthy-by-default
        (ClockQualityState::Synchronized, true),    // the ONLY trusted state
        (ClockQualityState::Degraded, false),       // drift beyond tolerance
        (ClockQualityState::Unsynchronized, false), // source unreachable/quiet
    ];
    for (state, trusted) in states {
        assert_eq!(state.is_expiry_trusted(), trusted, "state {}", state.name());
    }
    // The live monitor agrees state-by-state.
    let monitor = monitor();
    // Unknown before any observation: hold.
    assert_eq!(monitor.quality_at(BASE), ClockQualityState::Unknown);
    assert!(monitor.expiry_check_allowed(BASE).is_err());
    // Synchronized after an in-tolerance pair: trusted.
    monitor.observe_pair(pair(80, BASE)).expect("pilot tolerance 100 ms");
    assert_eq!(monitor.quality_at(BASE), ClockQualityState::Synchronized);
    assert!(monitor.expiry_check_allowed(BASE).is_ok());
}

#[test]
fn a_degraded_clock_holds_commands_and_the_hold_is_audited() {
    let monitor = monitor();
    let audit = flexcore_edge_agent::ports::InMemoryAuditSink::new();
    // Drift 900 ms beyond the 100 ms pilot tolerance → Degraded.
    let state = monitor
        .observe_pair(pair(900, BASE))
        .expect("a valid (non-corrupt) pair");
    assert_eq!(state, ClockQualityState::Degraded);
    // THE HOLD: expiry checks are not trusted on a non-synchronized clock.
    let reason = monitor
        .expiry_check_allowed(BASE)
        .expect_err("Degraded holds commands");
    assert!(
        reason.contains("DEGRADED") && reason.contains("NOT trusted"),
        "the named hold reason: {reason}"
    );
    // The composed hold + audit: the hold event is recorded with the
    // dedicated event name leading the detail.
    let error = audited_expiry_check(&monitor, &audit, BASE).expect_err("hold");
    assert_eq!(error, reason);
    let events = audit.events();
    assert_eq!(events.len(), 1);
    assert!(
        events[0].detail.starts_with("EDGE_CLOCK_HOLD_ON_DRIFT"),
        "the hold is journaled: {}",
        events[0].detail
    );
    // The health record renders Degraded as DEGRADED_CLOCK_DRIFT (ingest
    // keeps flowing; the COMMAND path is what fails closed).
    let record = monitor.health_record(BASE);
    assert_eq!(record.status_name(), "DEGRADED_CLOCK_DRIFT");
    assert!(record.detail.contains("BEYOND"));
    // The P6.15 verifier seam maps Degraded → ClockQuality::Unsynchronized
    // (a true statement in the contracts vocabulary: NOT synchronized
    // within tolerance), which the verifier fails closed.
    assert_eq!(
        monitor.p6_15_clock_quality(BASE),
        flexcore_contracts::time_model::ClockQuality::Unsynchronized
    );
}

#[test]
fn an_outage_or_quiet_source_holds_until_a_fresh_in_tolerance_pair_recovers() {
    let monitor = monitor();
    monitor.observe_pair(pair(10, BASE)).expect("in-tolerance pair");
    // Explicit outage: Unsynchronized, held.
    monitor.note_source_unreachable(BASE + 1_000);
    assert_eq!(monitor.quality_at(BASE + 2_000), ClockQualityState::Unsynchronized);
    assert!(monitor.expiry_check_allowed(BASE + 2_000).is_err());
    // A quiet source ages into Unsynchronized past the configured timeout
    // (pilot default 30 000 ms): the boundary is hand-computed.
    assert_eq!(
        monitor.quality_at(BASE + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS),
        ClockQualityState::Synchronized,
        "exactly AT the timeout is still the derived state"
    );
    assert_eq!(
        monitor.quality_at(BASE + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1),
        ClockQualityState::Unsynchronized,
        "one millis past the timeout, the source is quiet → hold"
    );
    // Recovery: a fresh in-tolerance pair restores trust.
    monitor.observe_pair(pair(10, BASE + 35_000)).expect("recovery pair");
    assert!(monitor.expiry_check_allowed(BASE + 35_000).is_ok());
}

// --- 3. The interaction: STALE telemetry × Degraded clock are independent holds ---------

#[test]
fn stale_telemetry_and_a_degraded_clock_hold_independently_and_never_substitute() {
    let monitor = monitor();
    // The telemetry surface: a communication-loss sample arrives STALE —
    // its quality is exactly one canonical state and it is not upgraded.
    let stale = map_source_state(SourceQualityState::CommunicationLoss);
    assert_eq!(stale, TelemetryQuality::Stale);
    assert_ne!(stale, TelemetryQuality::Good);
    // The clock surface: drift beyond tolerance → Degraded → hold, audited.
    let audit = flexcore_edge_agent::ports::InMemoryAuditSink::new();
    monitor.observe_pair(pair(900, BASE)).expect("drift pair");
    assert_eq!(monitor.quality_at(BASE), ClockQualityState::Degraded);
    assert!(monitor.expiry_check_allowed(BASE).is_err());
    audited_expiry_check(&monitor, &audit, BASE).expect_err("the clock hold is recorded");
    assert_eq!(audit.events().len(), 1);
    // The joint condition: in a window where the telemetry went STALE and
    // the clock is Degraded, a command consumer hits BOTH gates — the
    // stale sample's age (judged on event_time, per the contracts time
    // model) exceeds any use-case freshness limit, and the expiry check is
    // untrusted. Neither surface substitutes a last-known value: the
    // sample stays STALE (no re-labelling) and the command stays held.
    let sample_state = stale;
    let clock_state = monitor.quality_at(BASE);
    let sample_is_substitutable = matches!(sample_state, TelemetryQuality::Good);
    let clock_is_trusted = clock_state.is_expiry_trusted();
    assert!(!sample_is_substitutable, "STALE is never substituted");
    assert!(!clock_is_trusted, "a Degraded clock never trusts expiry checks");
}

// --- 4. Hand-computed boundaries ---------------------------------------------------------

#[test]
fn the_tolerance_boundary_is_inclusive_and_a_corrupt_pair_is_refused() {
    let monitor = monitor();
    // Exactly AT the pilot tolerance (100 ms): inside → Synchronized.
    let state = monitor.observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS, BASE)).expect("pair");
    assert_eq!(state, ClockQualityState::Synchronized);
    // One millisecond beyond: Degraded (hold).
    let state = monitor
        .observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS + 1, BASE + 1_000))
        .expect("pair");
    assert_eq!(state, ClockQualityState::Degraded);
    // A corrupt pair (offset ≥ one day) is REFUSED, never estimated, and
    // leaves the state untouched.
    let error = monitor
        .observe_pair(pair(MAX_ABS_OFFSET_MILLIS + 1, BASE + 2_000))
        .expect_err("a corrupt pair is refused");
    assert!(matches!(error, ClockQualityError::InvalidSample { .. }));
    assert_eq!(
        monitor.quality_at(BASE + 2_000),
        ClockQualityState::Degraded,
        "the corrupt refusal left the derived state unchanged"
    );
}

#[test]
fn the_unknown_state_renders_degraded_unverified_never_healthy() {
    // No observation yet: the health record is DEGRADED_UNVERIFIED — never
    // healthy-by-default (P0.09 §4: unknown is recorded, never assumed).
    let monitor = monitor();
    let record = monitor.health_record(BASE);
    assert_eq!(record.status_name(), "DEGRADED_UNVERIFIED");
    assert!(record.detail.contains("UNKNOWN"));
    assert_eq!(record.adapter_id, "edge-clock");
    assert_eq!(monitor.p6_15_clock_quality(BASE), flexcore_contracts::time_model::ClockQuality::Unknown);
}
