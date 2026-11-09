// P2.27 — The reusable `TelemetrySource` provider contract suite (report
// §19.2 test layer 3; ledger P2.11 step 6, P2.27 step 3).
//
// What this module is: ONE suite that every `TelemetrySource` provider must
// pass — the "every adapter/provider passes the same interface suite" gate.
// It is generic over the port ([`TelemetrySource`]): the suite never names a
// concrete provider, a wire format, or a technology, so any future adapter
// (MQTT, Modbus, OpenEMS — later phases) plugs in by instantiating it
// against its own implementation. In Phase 2 the provider set is exactly
// one (the simulator, P2.14).
//
// THE ARRANGEMENT (recorded, ledger P2.27 step 3): the ports crate cannot
// depend on flexcore-simulator (the port-only dependency rule enforced by
// tools/check_dependency_rule.py — flexcore-simulator depends on ports, so
// the edge back would be a cycle), so the SIMULATOR-SIDE instantiation of
// this suite lives OUTSIDE this crate:
//
// - NOW: CI (workflow job `telemetry-contract`) runs the suite through
//   `crates/flexcore-ports/tests/telemetry_source_contract.rs`, which drives
//   it against an in-memory deterministic stand-in provider that mirrors the
//   simulator's declared contract (P2.14/P2.15 declaration shape: a
//   `sim://`-namespaced source ID, the declared quality-state mapping with
//   the original code travelling as `source_quality_code`,
//   `ClockQuality::Synchronized`, scenario-cadence event times, seed-derived
//   per-asset streams, `ingest_time` stamped at the release tick).
// - FOLLOW-UP (owned by the sim-runner/simulator owners, recorded in
//   `ledger/phase-02-asset-telemetry/27_ci_gate_inventory.md`): instantiate
//   the same suite against `flexcore-simulator`'s `TelemetrySource`
//   implementation from a crate that legally depends on both (a
//   `flexcore-sim-runner contract` subcommand or a dev-dependency test
//   there). The suite itself needs no change — that is the point of making
//   it generic.
//
// The checks (each one a named violation below, so a failure states WHICH
// contract rule broke, not just that something did):
//
// 1. [`check_provider_declaration`] — the provider rule (P2.11 step 4):
//    non-empty, non-reserved source ID, positive cadence, a non-empty
//    quality mapping, and `source_id()` agreeing with the declaration.
// 2. [`check_canonical_schema`] — every decoded record is a complete
//    canonical record (P2.04 §3; P2.11 step 3): tenant from the query, asset
//    and quantity within the subscribed scope, `event_time` inside the
//    query range, the provider's own `source_id`, value totality
//    (present exactly on measured records), the quantity/value pairing,
//    and the clock quality the declaration promised.
// 3. [`check_quality_closure`] — the six-state closure (P0.08): the emitted
//    quality state is one of the frozen six (structural: an exhaustive
//    match with no wildcard arm — a seventh state fails to COMPILE here),
//    and every retained `source_quality_code` is declared in the provider's
//    mapping with the canonical state the record actually carries (P0.08 §3).
// 4. [`check_sequence_monotonicity`] — `source_sequence` is monotonically
//    increasing per (asset, source) across the run (P0.09 §1; P2.04 §8);
//    records sharing one frame (one multi-quantity sample, P2.04 D-T2) may
//    share the sequence value of that frame.
// 5. [`check_event_time_on_clock`] — every record's `event_time` is the
//    provided clock's time for its tick (P0.09; the provider reports when
//    the measurement occurred on the clock the caller supplied, never a
//    private wall clock).
// 6. [`check_ingest_time_at_emission`] — `ingest_time` is set at emission:
//    it satisfies the `ingest_time >= event_time` ordering rule (P0.09) and
//    never goes backwards across the run (a monotone emission clock).
// 7. [`check_same_seed_reproduction`] — two independent provider instances
//    driven with the same frames reproduce the frozen replay comparison
//    tuple `(asset_id, event_time, quantity, value, quality,
//    source_sequence)` exactly (P2.13 §3; P2.24) — `ingest_time` is
//    excluded from the comparison, as recorded for replay determinism.
//
// The entry point is [`run_contract_suite`]; every check is also public so
// a provider embedding can run subsets. [`ContractViolation`] names the
// broken rule with a human-readable detail; [`ContractOutcome`] reports
// pass/fail with the full violation list.
//
// Scope and control status: a test-side contract for simulation-only
// telemetry providers (the entire 0.x line). Nothing here touches a
// database, a broker, a device, or a live grid, and nothing here implies
// certification, safety, or field-control capability.

use std::collections::BTreeMap;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime, validate_ingest_after_event};
use flexcore_domain::ids::AssetId;

use crate::telemetry_source::{
    NormalizedTelemetry, QuantityValue, TelemetryQuery, TelemetrySource, TelemetrySourceError,
    TelemetrySubscription, TelemetryQuantity,
};

// --- Violations -------------------------------------------------------------

/// One broken contract rule, naming the rule and the evidence. The suite
/// never fails silently: every violation renders a human-readable detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolation {
    /// The provider declaration failed the provider rule (P2.11 step 4).
    ProviderDeclaration(String),
    /// The specification handed to the suite is malformed (the suite cannot
    /// run it honestly).
    Spec(String),
    /// `subscribe` or `decode` failed, or a successful decode returned an
    /// empty record set (the port's no-empty-set rule, P2.11 step 3).
    Decode(String),
    /// A decoded record is not a complete canonical record in the subscribed
    /// scope (P2.04 §3; P2.11 step 3).
    CanonicalSchema(String),
    /// A record's quality is outside the declared six-state closure or
    /// disagrees with the declared source-code mapping (P0.08).
    QualityClosure(String),
    /// `source_sequence` is not monotone per (asset, source) (P0.09 §1).
    SequenceMonotonicity(String),
    /// `event_time` is not the provided clock's time for the tick (P0.09).
    EventTimeOnClock(String),
    /// `ingest_time` was not set at emission (ordering or monotonicity
    /// broken, P0.09).
    IngestTimeAtEmission(String),
    /// Two same-frames provider instances do not reproduce the frozen
    /// replay comparison tuple (P2.13 §3; P2.24).
    SameSeedReproduction(String),
}

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractViolation::ProviderDeclaration(detail) => {
                write!(f, "provider declaration violated: {detail}")
            }
            ContractViolation::Spec(detail) => write!(f, "suite specification rejected: {detail}"),
            ContractViolation::Decode(detail) => {
                write!(f, "subscribe/decode contract violated: {detail}")
            }
            ContractViolation::CanonicalSchema(detail) => {
                write!(f, "canonical record schema violated: {detail}")
            }
            ContractViolation::QualityClosure(detail) => {
                write!(f, "quality-state closure violated: {detail}")
            }
            ContractViolation::SequenceMonotonicity(detail) => {
                write!(f, "source-sequence monotonicity violated: {detail}")
            }
            ContractViolation::EventTimeOnClock(detail) => {
                write!(f, "event-time-on-clock contract violated: {detail}")
            }
            ContractViolation::IngestTimeAtEmission(detail) => {
                write!(f, "ingest-time-at-emission contract violated: {detail}")
            }
            ContractViolation::SameSeedReproduction(detail) => {
                write!(f, "same-seed reproduction violated: {detail}")
            }
        }
    }
}

/// The suite's result: pass/fail plus every violation, so a failing
/// provider sees ALL its broken rules at once, not just the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractOutcome {
    /// True exactly when no violation was recorded.
    pub passed: bool,
    /// Every violation, in check order.
    pub violations: Vec<ContractViolation>,
}

// --- The specification ------------------------------------------------------

/// What the suite needs from the embedding side to drive one provider:
///
/// - `make_source` builds an INDEPENDENT provider instance. It is called
///   once for the main run and twice more for the reproduction check — the
///   provider must be cheap to construct and stateless between instances
///   (the same property replay determinism needs, P2.13).
/// - `query` is the tenant-scoped subscription the suite registers (built
///   through `TelemetryQuery::new` — there is no other constructor, P0.16).
/// - `frames` are the opaque raw payloads the suite feeds to `decode`, one
///   entry per tick, in emission order. They must be a deterministic
///   function of the provider's own inputs (seed/scenario) — the same bytes
///   are reused for both reproduction runs.
/// - `expected_event_times` is the provided clock's time per tick (same
///   indexing as `frames`); check 5 holds the provider to it.
pub struct ContractSpec<'a> {
    /// Factory for independent provider instances (see above).
    pub make_source: &'a dyn Fn() -> Box<dyn TelemetrySource>,
    /// The tenant-scoped query the suite subscribes with.
    pub query: TelemetryQuery,
    /// One opaque payload per tick, in emission order.
    pub frames: &'a [Vec<u8>],
    /// The provided clock's `event_time` per tick (same indexing).
    pub expected_event_times: &'a [EventTime],
}

/// One full subscription run: the decoded record batches, in emission
/// order, plus the provider identity every record must carry.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractRun {
    /// The provider's `source_id()` for this run.
    pub source_id: String,
    /// One batch per tick: the complete record set `decode` returned for
    /// that tick's frame (non-empty, by the port's rule).
    pub per_tick: Vec<Vec<NormalizedTelemetry>>,
}

/// Subscribes and decodes every frame, enforcing the subscription/decode
/// contract on the way (errors and empty sets are violations, not panics).
pub fn run_subscription(
    source: &dyn TelemetrySource,
    spec: &ContractSpec<'_>,
) -> Result<ContractRun, ContractViolation> {
    let subscription: TelemetrySubscription = source
        .subscribe(&spec.query)
        .map_err(|error| ContractViolation::Decode(format!("subscribe failed: {error}")))?;
    let mut per_tick: Vec<Vec<NormalizedTelemetry>> = Vec::with_capacity(spec.frames.len());
    for (tick, frame) in spec.frames.iter().enumerate() {
        let records: Vec<NormalizedTelemetry> = source
            .decode(&subscription, frame)
            .map_err(|error: TelemetrySourceError| {
                ContractViolation::Decode(format!("tick {tick}: decode failed: {error}"))
            })?;
        if records.is_empty() {
            return Err(ContractViolation::Decode(format!(
                "tick {tick}: decode returned an empty record set; a successful decode is the \
                 complete non-empty record set the frame carries (P2.11 step 3)"
            )));
        }
        per_tick.push(records);
    }
    Ok(ContractRun {
        source_id: source.source_id().to_string(),
        per_tick,
    })
}

// --- Check 1: the provider declaration --------------------------------------

/// The provider rule (P2.11 step 4), re-checked from the outside: a
/// non-empty, non-reserved source ID, a positive cadence, a non-empty
/// quality mapping, and `source_id()` agreeing with the declaration.
pub fn check_provider_declaration(source: &dyn TelemetrySource) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let declaration = source.provider_declaration();
    if declaration.source_id().is_empty() {
        violations.push(ContractViolation::ProviderDeclaration(
            "source_id is empty".to_string(),
        ));
    }
    if declaration.source_id().starts_with("flexcore:") {
        violations.push(ContractViolation::ProviderDeclaration(
            "a measured provider must not declare the reserved 'flexcore:' namespace (P2.04 D-T6)"
                .to_string(),
        ));
    }
    if source.source_id() != declaration.source_id() {
        violations.push(ContractViolation::ProviderDeclaration(format!(
            "source_id() ({}) disagrees with the declared source_id ({})",
            source.source_id(),
            declaration.source_id()
        )));
    }
    if declaration.cadence().is_zero() {
        violations.push(ContractViolation::ProviderDeclaration(
            "declared cadence is zero; a provider must declare its expected emission interval"
                .to_string(),
        ));
    }
    if declaration.quality_mapping().mappings().is_empty() {
        violations.push(ContractViolation::ProviderDeclaration(
            "the quality mapping is empty; a provider must declare at least one source-code \
             mapping (P0.08 §3)"
                .to_string(),
        ));
    }
    violations
}

// --- Check 2: the canonical record schema -----------------------------------

/// Every decoded record is complete and canonical within the subscribed
/// scope (P2.04 §3; P2.11 step 3). The record constructor already rejects
/// incomplete records — this check re-asserts the contract from the OUTSIDE
/// so a provider emitting records built by any other path is caught too.
pub fn check_canonical_schema(
    source: &dyn TelemetrySource,
    run: &ContractRun,
    spec: &ContractSpec<'_>,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let declared_clock_quality = source.provider_declaration().clock_quality();
    for (tick, batch) in run.per_tick.iter().enumerate() {
        for record in batch {
            let where_ = format!("tick {tick}, sequence {}", record.source_sequence());
            if record.tenant_id() != spec.query.tenant_id() {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: record tenant {:?} is not the subscribed tenant {:?}",
                    record.tenant_id(),
                    spec.query.tenant_id()
                )));
            }
            if !spec.query.assets().contains(&record.asset_id()) {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: asset {} is outside the subscribed asset set",
                    record.asset_id()
                )));
            }
            if !spec.query.quantities().contains(&record.quantity()) {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: quantity {:?} is outside the subscribed quantity set",
                    record.quantity()
                )));
            }
            if record.event_time() < spec.query.from() || record.event_time() > spec.query.until()
            {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: event_time {:?} is outside the subscribed range",
                    record.event_time()
                )));
            }
            if record.source_id() != run.source_id {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: record source_id '{}' is not the provider identity '{}'",
                    record.source_id(),
                    run.source_id
                )));
            }
            // Value totality (P2.04 §3): present exactly on measured records.
            match (record.value(), record.quality()) {
                (Some(_), TelemetryQuality::Missing) => {
                    violations.push(ContractViolation::CanonicalSchema(format!(
                        "{where_}: a MISSING gap marker must not carry a value"
                    )));
                }
                (None, TelemetryQuality::Missing) => {}
                (None, _) => {
                    violations.push(ContractViolation::CanonicalSchema(format!(
                        "{where_}: a measured record must carry a value (no partial records)"
                    )));
                }
                (Some(value), _) => {
                    if !record.quantity().accepts(value) {
                        violations.push(ContractViolation::CanonicalSchema(format!(
                            "{where_}: the value kind does not match quantity {:?} (P2.04 §4 \
                             pairing)",
                            record.quantity()
                        )));
                    }
                }
            }
            if record.clock_quality() != declared_clock_quality {
                violations.push(ContractViolation::CanonicalSchema(format!(
                    "{where_}: record clock quality {:?} disagrees with the declared clock \
                     behaviour {:?}",
                    record.clock_quality(),
                    declared_clock_quality
                )));
            }
        }
    }
    violations
}

// --- Check 3: the quality-state closure -------------------------------------

/// The six-state closure (P0.08), written WITHOUT a wildcard arm: a seventh
/// quality state added to the vocabulary fails to compile here, keeping the
/// closure structural rather than a runtime checklist.
fn quality_state_is_canonical(quality: TelemetryQuality) -> bool {
    match quality {
        TelemetryQuality::Good => true,
        TelemetryQuality::Suspect => true,
        TelemetryQuality::Bad => true,
        TelemetryQuality::Missing => true,
        TelemetryQuality::Estimated => true,
        TelemetryQuality::Stale => true,
    }
}

/// Every emitted quality state is one of the frozen six, and every retained
/// `source_quality_code` is declared in the provider's mapping with exactly
/// the canonical state the record carries (P0.08 §3 — the original code
/// travels as diagnostic metadata, the canonical vocabulary never grows).
pub fn check_quality_closure(
    source: &dyn TelemetrySource,
    run: &ContractRun,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let mapping = source.provider_declaration().quality_mapping();
    for (tick, batch) in run.per_tick.iter().enumerate() {
        for record in batch {
            let where_ = format!(
                "tick {tick}, sequence {}, quantity {:?}",
                record.source_sequence(),
                record.quantity()
            );
            if !quality_state_is_canonical(record.quality()) {
                violations.push(ContractViolation::QualityClosure(format!(
                    "{where_}: quality state is outside the frozen six-state vocabulary"
                )));
            }
            if let Some(code) = record.source_quality_code() {
                match mapping.canonical_for(code) {
                    None => {
                        violations.push(ContractViolation::QualityClosure(format!(
                            "{where_}: source quality code '{code}' is not declared in the \
                             provider's mapping (P0.08 §3)"
                        )));
                    }
                    Some(canonical) if canonical != record.quality() => {
                        violations.push(ContractViolation::QualityClosure(format!(
                            "{where_}: source quality code '{code}' is declared as {canonical:?} \
                             but the record carries {:?}",
                            record.quality()
                        )));
                    }
                    Some(_) => {}
                }
            }
        }
    }
    violations
}

// --- Check 4: sequence monotonicity -----------------------------------------

/// `source_sequence` is monotonically increasing per (asset, source) across
/// the run in decode order (P0.09 §1; P2.04 §8). Records returned by ONE
/// decode call share one frame — one multi-quantity sample (P2.04 D-T2) —
/// and may share that frame's sequence value; the sequence must strictly
/// increase between frames of the same asset.
pub fn check_sequence_monotonicity(run: &ContractRun) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    // Key: (asset, source). Value: the last observed (tick, sequence).
    let mut last_seen: BTreeMap<(AssetId, String), (usize, u64)> = BTreeMap::new();
    for (tick, batch) in run.per_tick.iter().enumerate() {
        for record in batch {
            let key = (record.asset_id(), record.source_id().to_string());
            let sequence = record.source_sequence();
            if let Some(&(previous_tick, previous_sequence)) = last_seen.get(&key) {
                let monotone = if tick == previous_tick {
                    // Same frame: the sequence may repeat, never regress.
                    sequence >= previous_sequence
                } else {
                    sequence > previous_sequence
                };
                if !monotone {
                    violations.push(ContractViolation::SequenceMonotonicity(format!(
                        "asset {} from '{}': sequence {sequence} at tick {tick} does not \
                         increase over {previous_sequence} at tick {previous_tick}",
                        record.asset_id(),
                        record.source_id()
                    )));
                }
            }
            // Always advance: one reported violation per regression, no
            // cascading repeats of the same divergence.
            last_seen.insert(key, (tick, sequence));
        }
    }
    violations
}

// --- Check 5: event_time on the provided clock ------------------------------

/// Every record's `event_time` equals the provided clock's time for its
/// tick (P0.09): the provider reports WHEN the measurement occurred on the
/// clock the caller supplied — never a private or shifted timeline.
pub fn check_event_time_on_clock(
    run: &ContractRun,
    expected_event_times: &[EventTime],
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    for (tick, batch) in run.per_tick.iter().enumerate() {
        let Some(expected) = expected_event_times.get(tick) else {
            violations.push(ContractViolation::EventTimeOnClock(format!(
                "the specification provides no expected event_time for tick {tick}"
            )));
            continue;
        };
        for record in batch {
            if record.event_time() != *expected {
                violations.push(ContractViolation::EventTimeOnClock(format!(
                    "asset {}, sequence {}: event_time {:?} is not the provided clock's time \
                     {expected:?} for tick {tick}",
                    record.asset_id(),
                    record.source_sequence(),
                    record.event_time()
                )));
            }
        }
    }
    violations
}

// --- Check 6: ingest_time set at emission -----------------------------------

/// `ingest_time` is set at emission (P0.09): every record satisfies the
/// `ingest_time >= event_time` ordering rule, and the emission clock never
/// goes backwards across the run in decode order. `ingest_time` itself
/// remains excluded from any comparison use (P2.13 §3) — this check only
/// asserts the emission contract, it does not feed it into matching.
pub fn check_ingest_time_at_emission(run: &ContractRun) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let mut previous_ingest: Option<IngestTime> = None;
    for (tick, batch) in run.per_tick.iter().enumerate() {
        for record in batch {
            let where_ = format!(
                "tick {tick}, asset {}, sequence {}",
                record.asset_id(),
                record.source_sequence()
            );
            if validate_ingest_after_event(record.event_time(), record.ingest_time()).is_err() {
                violations.push(ContractViolation::IngestTimeAtEmission(format!(
                    "{where_}: ingest_time {:?} precedes event_time {:?} (P0.09 ordering rule)",
                    record.ingest_time(),
                    record.event_time()
                )));
            }
            if let Some(previous) = previous_ingest {
                if record.ingest_time() < previous {
                    violations.push(ContractViolation::IngestTimeAtEmission(format!(
                        "{where_}: ingest_time {:?} goes backwards relative to an earlier \
                         emission {previous:?}",
                        record.ingest_time()
                    )));
                }
            }
            previous_ingest = Some(record.ingest_time());
        }
    }
    violations
}

// --- Check 7: deterministic same-seed reproduction --------------------------

/// The frozen replay comparison tuple (P2.24 §2.2): exactly
/// `(asset_id, event_time, quantity, value, quality, source_sequence)` —
/// NEVER `ingest_time`. There is no field on this tuple a comparison could
/// use to touch `ingest_time`.
#[derive(Debug, Clone, PartialEq)]
struct ReplayTuple {
    asset_id: AssetId,
    event_time: EventTime,
    quantity: TelemetryQuantity,
    value: Option<QuantityValue>,
    quality: TelemetryQuality,
    source_sequence: u64,
}

impl ReplayTuple {
    fn of(record: &NormalizedTelemetry) -> Self {
        Self {
            asset_id: record.asset_id(),
            event_time: record.event_time(),
            quantity: record.quantity(),
            value: record.value().cloned(),
            quality: record.quality(),
            source_sequence: record.source_sequence(),
        }
    }

    /// A total sort key over the tuple: the multiset comparison (P2.24 §2.2)
    /// sorts both sides, so a provider that emits the same records in a
    /// different frame order still reproduces. Asset and event time are
    /// ordered types; the quantity gets a stable order from an exhaustive
    /// match (no wildcard — a new quantity fails to compile here).
    fn sort_key(&self) -> (AssetId, EventTime, u8, u64) {
        (self.asset_id, self.event_time, quantity_order(self.quantity), self.source_sequence)
    }

    fn render(&self) -> String {
        format!(
            "(asset {}, event_time {:?}, quantity {:?}, value {:?}, quality {:?}, sequence {})",
            self.asset_id,
            self.event_time,
            self.quantity,
            self.value,
            self.quality,
            self.source_sequence
        )
    }
}

/// A stable ordering of the frozen quantity vocabulary (P2.04 §4). No
/// wildcard arm: extending the vocabulary fails to compile here until the
/// order is declared explicitly.
fn quantity_order(quantity: TelemetryQuantity) -> u8 {
    match quantity {
        TelemetryQuantity::ActivePower => 0,
        TelemetryQuantity::ReactivePower => 1,
        TelemetryQuantity::Soc => 2,
        TelemetryQuantity::Energy => 3,
        TelemetryQuantity::Availability => 4,
        TelemetryQuantity::OperatingMode => 5,
        TelemetryQuantity::Health => 6,
        TelemetryQuantity::ConnectionState => 7,
        TelemetryQuantity::BaselineReference => 8,
    }
}

/// Two INDEPENDENT provider instances (fresh `make_source` calls) driven
/// with the same frames reproduce the same multiset of frozen comparison
/// tuples (P2.13 §3; P2.24): a generation change or a seed dependency in
/// the provider shows up as a named divergence with both sides rendered.
pub fn check_same_seed_reproduction(spec: &ContractSpec<'_>) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let source_a = (spec.make_source)();
    let run_a = run_subscription(source_a.as_ref(), spec);
    let source_b = (spec.make_source)();
    let run_b = run_subscription(source_b.as_ref(), spec);
    let (run_a, run_b) = match (run_a, run_b) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(error), _) | (_, Err(error)) => {
            violations.push(error);
            return violations;
        }
    };
    let mut tuples_a: Vec<ReplayTuple> = run_a
        .per_tick
        .iter()
        .flatten()
        .map(ReplayTuple::of)
        .collect();
    let mut tuples_b: Vec<ReplayTuple> = run_b
        .per_tick
        .iter()
        .flatten()
        .map(ReplayTuple::of)
        .collect();
    tuples_a.sort_by_key(ReplayTuple::sort_key);
    tuples_b.sort_by_key(ReplayTuple::sort_key);
    if tuples_a.len() != tuples_b.len() {
        violations.push(ContractViolation::SameSeedReproduction(format!(
            "record counts differ between the two runs: {} vs {}",
            tuples_a.len(),
            tuples_b.len()
        )));
        return violations;
    }
    for (index, (a, b)) in tuples_a.iter().zip(tuples_b.iter()).enumerate() {
        if a != b {
            violations.push(ContractViolation::SameSeedReproduction(format!(
                "run A tuple {index} {} does not reproduce run B tuple {index} {} (the \
                 comparison tuple excludes ingest_time by construction, P2.24)",
                a.render(),
                b.render()
            )));
            break;
        }
    }
    violations
}

// --- The suite --------------------------------------------------------------

/// Runs the full contract suite against one provider. Every check runs even
/// after an earlier one failed, so a failing provider sees all its broken
/// rules in one run — except that a run that cannot even subscribe/decode
/// stops the record-level checks (there is nothing to check without
/// records) and the outcome states that honestly.
pub fn run_contract_suite(spec: &ContractSpec<'_>) -> ContractOutcome {
    let mut violations: Vec<ContractViolation> = Vec::new();

    // Spec sanity first: the suite must not pretend to check a malformed
    // specification.
    if spec.frames.is_empty() {
        violations.push(ContractViolation::Spec(
            "the frame list is empty; the suite needs at least one tick to drive".to_string(),
        ));
    }
    if spec.frames.len() != spec.expected_event_times.len() {
        violations.push(ContractViolation::Spec(format!(
            "frames ({}) and expected event times ({}) must have the same length (one clock \
             time per tick)",
            spec.frames.len(),
            spec.expected_event_times.len()
        )));
    }
    let spec_usable = violations.is_empty();
    if !spec_usable {
        return ContractOutcome {
            passed: false,
            violations,
        };
    }

    let source = (spec.make_source)();
    violations.extend(check_provider_declaration(source.as_ref()));
    let run = match run_subscription(source.as_ref(), spec) {
        Ok(run) => run,
        Err(error) => {
            violations.push(error);
            return ContractOutcome {
                passed: false,
                violations,
            };
        }
    };
    violations.extend(check_canonical_schema(source.as_ref(), &run, spec));
    violations.extend(check_quality_closure(source.as_ref(), &run));
    violations.extend(check_sequence_monotonicity(&run));
    violations.extend(check_event_time_on_clock(&run, spec.expected_event_times));
    violations.extend(check_ingest_time_at_emission(&run));
    violations.extend(check_same_seed_reproduction(spec));

    let passed = violations.is_empty();
    ContractOutcome { passed, violations }
}

// --- Suite self-tests ---------------------------------------------------------
//
// The suite is itself testable code: a scripted fake drives each violation
// path so the suite is proven to DETECT every rule it claims to check, not
// just to pass a conforming provider (the ledger's P2.27 risk — a gate that
// always passes — is excluded here at the suite level).

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::ClockQuality;
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_domain::ids::TenantId;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
    use crate::telemetry_source::{
        NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
        SourceStateMapping, TelemetrySourceHealth,
    };
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::Duration;

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
    const ASSET_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
    const ASSET_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";

    fn tenant_id() -> TenantId {
        TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap())
    }

    fn asset_a() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_A_UUID).unwrap())
    }

    fn asset_b() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_B_UUID).unwrap())
    }

    fn tenant_context() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant_id(),
                subject: "user:contract-suite".to_string(),
            },
        ))
    }

    fn event(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).unwrap()
    }

    fn ingest(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).unwrap()
    }

    fn scripted_declaration() -> ProviderDeclaration {
        ProviderDeclaration::new(
            "sim://scripted".to_string(),
            Duration::from_secs(1),
            QualityMappingDeclaration::new(vec![SourceStateMapping {
                source_code: "GOOD".to_string(),
                canonical: TelemetryQuality::Good,
            }])
            .unwrap(),
            ClockQuality::Synchronized,
        )
        .unwrap()
    }

    /// One record per tick: asset A, active power, sequence tick+1,
    /// event_time on the scripted clock, ingest_time one millisecond after
    /// the event (a monotone emission clock), all-GOOD.
    fn scripted_record(tick: usize) -> NormalizedTelemetry {
        let event_time = event(1_000 + 1_000 * tick as i64);
        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: tenant_id(),
            asset_id: asset_a(),
            quantity: TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
            event_time,
            ingest_time: ingest((event_time.unix_millis() + 1) as i64),
            source_sequence: tick as u64 + 1,
            source_id: "sim://scripted".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .unwrap()
    }

    /// The conforming script: three ticks, one record each, everything on
    /// contract. `mutate` lets a test break exactly one rule.
    fn scripted_batches(
        mutate: impl Fn(usize, &mut NormalizedTelemetryParts),
    ) -> Vec<Vec<NormalizedTelemetry>> {
        (0..3)
            .map(|tick| {
                let event_time = event(1_000 + 1_000 * tick as i64);
                let mut parts = NormalizedTelemetryParts {
                    tenant_id: tenant_id(),
                    asset_id: asset_a(),
                    quantity: TelemetryQuantity::ActivePower,
                    value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
                    event_time,
                    ingest_time: ingest((event_time.unix_millis() + 1) as i64),
                    source_sequence: tick as u64 + 1,
                    source_id: "sim://scripted".to_string(),
                    quality: TelemetryQuality::Good,
                    source_quality_code: None,
                    clock_quality: ClockQuality::Synchronized,
                };
                mutate(tick, &mut parts);
                vec![NormalizedTelemetry::new(parts).unwrap()]
            })
            .collect()
    }

    /// A provider that returns one scripted batch per decode call (and an
    /// empty set once the script is exhausted — which the suite flags, so a
    /// test must script exactly as many batches as it feeds frames).
    struct ScriptedSource {
        declaration: ProviderDeclaration,
        subscription: TelemetrySubscription,
        scripts: Mutex<VecDeque<Vec<NormalizedTelemetry>>>,
        fail_decode: bool,
    }

    impl ScriptedSource {
        fn new(batches: Vec<Vec<NormalizedTelemetry>>) -> Self {
            Self {
                declaration: scripted_declaration(),
                subscription: TelemetrySubscription::from_handle(1),
                scripts: Mutex::new(batches.into()),
                fail_decode: false,
            }
        }

        fn failing() -> Self {
            let mut source = Self::new(Vec::new());
            source.fail_decode = true;
            source
        }
    }

    impl TelemetrySource for ScriptedSource {
        fn provider_declaration(&self) -> &ProviderDeclaration {
            &self.declaration
        }
        fn subscribe(
            &self,
            _query: &TelemetryQuery,
        ) -> Result<TelemetrySubscription, TelemetrySourceError> {
            Ok(self.subscription)
        }
        fn decode(
            &self,
            subscription: &TelemetrySubscription,
            _payload: &[u8],
        ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
            if *subscription != self.subscription {
                return Err(TelemetrySourceError::NotSubscribed);
            }
            if self.fail_decode {
                return Err(TelemetrySourceError::DecodeFailed {
                    reason: "scripted decode failure".to_string(),
                });
            }
            let mut scripts = self.scripts.lock().unwrap();
            Ok(scripts.pop_front().unwrap_or_default())
        }
        fn health(&self) -> TelemetrySourceHealth {
            TelemetrySourceHealth::Healthy
        }
    }

    fn suite_query() -> TelemetryQuery {
        TelemetryQuery::new(
            &tenant_context(),
            vec![asset_a()],
            event(1_000),
            event(3_000),
            vec![TelemetryQuantity::ActivePower],
        )
        .unwrap()
    }

    fn suite_frames() -> Vec<Vec<u8>> {
        vec![b"tick-0".to_vec(), b"tick-1".to_vec(), b"tick-2".to_vec()]
    }

    fn suite_clock() -> Vec<EventTime> {
        vec![event(1_000), event(2_000), event(3_000)]
    }

    /// Builds a spec over locally owned frames/clock with the given source
    /// factory (the spec borrows them, so the locals outlive it in tests).
    fn suite_spec<'a>(
        make_source: &'a dyn Fn() -> Box<dyn TelemetrySource>,
        query: &TelemetryQuery,
        frames: &'a [Vec<u8>],
        clock: &'a [EventTime],
    ) -> ContractSpec<'a> {
        ContractSpec {
            make_source,
            query: query.clone(),
            frames,
            expected_event_times: clock,
        }
    }

    #[test]
    fn the_suite_passes_a_conforming_provider_with_zero_violations() {
        let make = || -> Box<dyn TelemetrySource> {
            Box::new(ScriptedSource::new(scripted_batches(|_, _| {})))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(
            outcome.passed,
            "the conforming provider must pass the suite; violations: {:?}",
            outcome.violations
        );
        assert!(outcome.violations.is_empty());
    }

    #[test]
    fn the_suite_flags_a_sequence_regression_per_asset() {
        let make = || -> Box<dyn TelemetrySource> {
            // Sequence goes 3, 2, 1 across the ticks: a regression the
            // monotonicity check must name.
            Box::new(ScriptedSource::new(scripted_batches(|tick, parts| {
                parts.source_sequence = 3 - tick as u64;
            })))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::SequenceMonotonicity(_))),
            "expected a SequenceMonotonicity violation, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_flags_an_event_time_off_the_provided_clock() {
        let make = || -> Box<dyn TelemetrySource> {
            // Tick 1 reports an event_time 500 ms ahead of the provided
            // clock: a private/shifted timeline.
            Box::new(ScriptedSource::new(scripted_batches(|tick, parts| {
                if tick == 1 {
                    parts.event_time = event(2_500);
                }
            })))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::EventTimeOnClock(_))),
            "expected an EventTimeOnClock violation, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_flags_an_undeclared_source_quality_code() {
        let make = || -> Box<dyn TelemetrySource> {
            // The record retains a richer source code the provider never
            // declared in its mapping (P0.08 §3).
            Box::new(ScriptedSource::new(scripted_batches(|tick, parts| {
                if tick == 0 {
                    parts.source_quality_code = Some("UNDECLARED_CODE".to_string());
                }
            })))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::QualityClosure(_))),
            "expected a QualityClosure violation, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_flags_a_mapping_disagreement_between_code_and_state() {
        let make = || -> Box<dyn TelemetrySource> {
            // The declared mapping says GOOD -> Good, but the record claims
            // Suspect while retaining the GOOD code.
            Box::new(ScriptedSource::new(scripted_batches(|tick, parts| {
                if tick == 2 {
                    parts.quality = TelemetryQuality::Suspect;
                    parts.source_quality_code = Some("GOOD".to_string());
                }
            })))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        let flagged = outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::QualityClosure(_)));
        assert!(flagged, "expected a QualityClosure violation, got: {outcome:?}");
    }

    #[test]
    fn the_suite_flags_a_record_outside_the_subscribed_scope() {
        let make = || -> Box<dyn TelemetrySource> {
            // Tick 1 emits a record for an asset the query never named.
            Box::new(ScriptedSource::new(scripted_batches(|tick, parts| {
                if tick == 1 {
                    parts.asset_id = asset_b();
                }
            })))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::CanonicalSchema(_))),
            "expected a CanonicalSchema violation, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_flags_an_empty_record_set_and_a_decode_failure() {
        // An empty set where a complete record set is required (P2.11
        // step 3: success is the COMPLETE non-empty set or an error).
        let make_empty = || -> Box<dyn TelemetrySource> {
            Box::new(ScriptedSource::new(vec![Vec::new(), Vec::new(), Vec::new()]))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make_empty, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::Decode(_))),
            "expected a Decode violation for the empty set, got: {outcome:?}"
        );

        // A hard decode failure is a violation too, never a silent skip.
        let make_failing = || -> Box<dyn TelemetrySource> {
            Box::new(ScriptedSource::failing())
        };
        let spec = suite_spec(&make_failing, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert!(
            outcome
                .violations
                .iter()
                .any(|v| matches!(v, ContractViolation::Decode(_))),
            "expected a Decode violation for the failing decode, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_flags_a_provider_that_does_not_reproduce_its_own_records() {
        // The suite calls the factory once for the main run and twice for
        // the reproduction check; the third instance emits different values
        // — the recorded same-seed divergence (P2.13/P2.24).
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let make = || -> Box<dyn TelemetrySource> {
            let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call == 2 {
                Box::new(ScriptedSource::new(scripted_batches(|_, parts| {
                    parts.value = Some(QuantityValue::Power(ActivePowerMW::new(2.5).unwrap()));
                })))
            } else {
                Box::new(ScriptedSource::new(scripted_batches(|_, _| {})))
            }
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        let reproduction = outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::SameSeedReproduction(_)));
        assert!(
            reproduction,
            "expected a SameSeedReproduction violation, got: {outcome:?}"
        );
    }

    #[test]
    fn the_suite_rejects_a_malformed_specification_instead_of_pretending_to_check() {
        let make = || -> Box<dyn TelemetrySource> {
            Box::new(ScriptedSource::new(scripted_batches(|_, _| {})))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = vec![event(1_000), event(2_000)]; // one short: malformed
        let spec = suite_spec(&make, &query, &frames, &clock);
        let outcome = run_contract_suite(&spec);
        assert!(!outcome.passed);
        assert_eq!(
            outcome.violations,
            vec![ContractViolation::Spec(
                "frames (3) and expected event times (2) must have the same length (one clock \
                 time per tick)"
                    .to_string()
            )]
        );
    }

    #[test]
    fn the_reproduction_comparison_is_a_sorted_multiset_over_the_frozen_tuple() {
        // The same records in a different emission order still reproduce:
        // the comparison sorts both sides on the frozen tuple (P2.24 §2.2
        // multiset semantics), so emission ORDER between independent runs
        // is not part of the determinism contract. The suite calls the
        // factory once for the main run and twice for the reproduction
        // check; the second reproduction instance emits the SAME records in
        // the REVERSE order, so a positional comparison would fail here.
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let make = || -> Box<dyn TelemetrySource> {
            let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut batches = scripted_batches(|_, _| {});
            if call != 1 {
                batches.reverse();
            }
            Box::new(ScriptedSource::new(batches))
        };
        let query = suite_query();
        let frames = suite_frames();
        let clock = suite_clock();
        let spec = suite_spec(&make, &query, &frames, &clock);
        let violations = check_same_seed_reproduction(&spec);
        assert!(
            violations.is_empty(),
            "reordered but identical records must reproduce; got: {violations:?}"
        );
    }

    #[test]
    fn the_frozen_comparison_tuple_has_no_ingest_time_field() {
        // Structural restatement of P2.24 §2.2: the tuple is built from the
        // record and contains no ingest_time, so a comparison that touches
        // it cannot compile. Pinned by constructing the tuple and asserting
        // its fields.
        let record = scripted_record(0);
        let tuple = ReplayTuple::of(&record);
        assert_eq!(tuple.asset_id, asset_a());
        assert_eq!(tuple.event_time, event(1_000));
        assert_eq!(tuple.quantity, TelemetryQuantity::ActivePower);
        assert_eq!(tuple.quality, TelemetryQuality::Good);
        assert_eq!(tuple.source_sequence, 1);
        // The exclusion itself: ingest_time is never read into the tuple.
        assert!(
            !format!("{tuple:?}").contains("ingest"),
            "the rendered tuple must not carry ingest_time"
        );
    }
}
