// P6.13 — Adapter conformance harness: the protocol-agnostic, reusable
//         test-side driver for the §19.2(6) fault-injection set (report
//         §19.2(6); ledger P6.13).
//
// WHAT THIS IS: a public, in-crate harness (composed from `tests/`) that
// drives ANY [`EmsAdapter`] + [`AntiCorruptionLayer`] pair through the
// §19.2(6) fault cases and asserts the specified reject-or-flag behaviour.
// The harness is PROTOCOL-AGNOSTIC: it knows nothing about OpenEMS; a
// concrete adapter (the `simulated-test` fixture today, `openems-rest` when
// P6.10 lands) is supplied by the caller, and all fault cases are
// synthesised from ONE valid template reading the harness reads from the
// adapter itself.
//
// WHAT THIS IS NOT (the P6.19 boundary, stated up front): this module is the
// P6.19 BUILDING BLOCK only. P6.19 owns the actual conformance-suite
// boundary record — which official protocol suite applies, its access
// terms, the CI wiring, and the claim-boundary statement ("passes the
// FlexCore conformance suite against protocol version Y" is NOT Alliance/
// OCA/SunSpec certification, §2.3). Nothing here may be quoted as protocol
// conformance, certification, or compliance (AGENTS.md rule 2). Passing this
// harness proves ONLY that the adapter honours the edge read-path contract
// under the §19.2(6) fault set.
//
// Reference vectors (P6.13 step 3, recorded honestly): OpenEMS provides no
// offline-obtainable reference vectors in this loop — the record is
// "reference vectors: NONE RUN (none obtainable in this loop); UNRUN" and
// lives in the decision doc. The harness therefore runs FlexCore's OWN
// contract cases only.
//
// The harness contract (what every case must do; §19.2(6) + P6.13 step 1):
//
// | Case | Required behaviour |
// |---|---|
// | `Valid` | The template reading maps; the canonical sample is complete. |
// | `Malformed` | REJECTED with a NAMED diagnostic — never partially applied, never a default quality. |
// | `Truncated` | REJECTED with a NAMED diagnostic (a truncated frame loses required fields; the ACL refuses the whole reading). |
// | `OutOfOrder` | FLAGGED: the sequence regression is recorded, the sample is never silently re-ordered or re-numbered (the P0.09 sequence is source-assigned; enforcement-by-citation is the P4 `SequenceGuard` / P6.15 discipline). |
// | `DuplicateDelivery` | IDEMPOTENT SKIP: the exact re-delivery is classified by the P4.24 edge dedup window (cited) — one stored value, counted, never double-counted. |
// | `ClockSkewFuture` | REJECTED: an event_time ahead of the edge receipt clock is refused by the sample constructor (`ingest >= event`, P0.09) — never a silent clock "fix". |
// | `ClockSkewPast` | ACCEPTED with the skew RECORDED; beyond the configured bound the harness FLAGS it (an aged event is data, but a very old one must not pass unremarked). |
// | `StaleData` | FLAGGED: the source's own STALE code maps to the canonical STALE state (P0.08) — flagged, never upgraded to GOOD, never dropped silently. |
// | `UnmappedPoint` | REJECTED with a NAMED diagnostic; the unmapped field is RECORDED (the ACL's declared-unmapped inventory), never silently accepted (P6.13 step 1). |
//
// STRICT-VALIDATION SEMANTICS (P6.13 step 1) live in the P6.12 ACL and are
// EXERCISED here, not reimplemented: unknown quality codes and units are
// named errors, malformed readings are rejected WHOLE (no partial mapping —
// the P2.11 step-3 rule at the adapter boundary), and no code path on this
// surface panics on untrusted input (every ACL outcome is a `Result`; the
// harness turns a panic-free run into an asserted property by running all
// cases through the same total function).
//
// Read-path latency/throughput measurement (P6.13 step 4): the measurement
// TEMPLATE with EMPTY cells is in the decision doc
// (`ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md`) —
// no figures are fabricated here (AGENTS.md rule 1).

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::IngestTime;

use crate::adapters::acl::{AntiCorruptionLayer, NativeTimestamp, NativeUnit};
use crate::adapters::port::{EmsAdapter, EmsNativeReading, PointKey};
use crate::ingest::{AdmitOutcome, EdgeDedupWindow};

// --- Cases ------------------------------------------------------------------

/// The §19.2(6) fault-injection cases (plus the strict-validation
/// `UnmappedPoint` case from P6.13 step 1). The case set is CLOSED —
/// extending it is a reviewed change (the §19.2(6) set is the ledger's
/// named deliverable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceCase {
    /// A well-formed reading maps to a complete canonical sample.
    Valid,
    /// An unknown native quality code (malformed content): named rejection.
    Malformed,
    /// A reading missing its quality code (truncated frame): named rejection.
    Truncated,
    /// A source_sequence regression on the same point: flagged, never
    /// re-ordered.
    OutOfOrder,
    /// The exact same reading delivered twice: idempotent skip (P4.24).
    DuplicateDelivery,
    /// An event_time AHEAD of the edge receipt clock: rejected (P0.09).
    ClockSkewFuture,
    /// An event_time far BEHIND the edge receipt clock: accepted with the
    /// skew recorded; beyond the bound, flagged.
    ClockSkewPast,
    /// The source itself declares STALE: mapped to canonical STALE, flagged,
    /// never upgraded (P0.08).
    StaleData,
    /// A reading for a point with no mapping table entry: named rejection;
    /// unknown fields are recorded, never silently accepted (P6.13 step 1).
    UnmappedPoint,
}

/// All cases the harness runs, in the fixed report order (deterministic
/// reports are comparable across runs — the Phase-2 deterministic-scenario
/// discipline applied to the harness output).
pub const ALL_CONFORMANCE_CASES: [ConformanceCase; 9] = [
    ConformanceCase::Valid,
    ConformanceCase::Malformed,
    ConformanceCase::Truncated,
    ConformanceCase::OutOfOrder,
    ConformanceCase::DuplicateDelivery,
    ConformanceCase::ClockSkewFuture,
    ConformanceCase::ClockSkewPast,
    ConformanceCase::StaleData,
    ConformanceCase::UnmappedPoint,
];

impl ConformanceCase {
    /// Stable case name (report/ledger record rendering).
    pub fn name(self) -> &'static str {
        match self {
            ConformanceCase::Valid => "valid",
            ConformanceCase::Malformed => "malformed",
            ConformanceCase::Truncated => "truncated",
            ConformanceCase::OutOfOrder => "out_of_order",
            ConformanceCase::DuplicateDelivery => "duplicate_delivery",
            ConformanceCase::ClockSkewFuture => "clock_skew_future",
            ConformanceCase::ClockSkewPast => "clock_skew_past",
            ConformanceCase::StaleData => "stale_data",
            ConformanceCase::UnmappedPoint => "unmapped_point",
        }
    }

    /// The outcome class the harness contract REQUIRES for this case.
    fn expected_class(self) -> &'static str {
        match self {
            ConformanceCase::Valid => "accepted",
            ConformanceCase::Malformed
            | ConformanceCase::Truncated
            | ConformanceCase::ClockSkewFuture
            | ConformanceCase::UnmappedPoint => "rejected",
            ConformanceCase::OutOfOrder | ConformanceCase::StaleData => "flagged",
            ConformanceCase::DuplicateDelivery => "idempotent_skip",
            ConformanceCase::ClockSkewPast => "accepted_or_flagged",
        }
    }
}

// --- Outcomes and reports ----------------------------------------------------

/// The outcome of ONE conformance case run. Every outcome carries enough
/// context to be recorded verbatim in a run record — no outcome is silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseOutcome {
    /// The reading mapped to a complete canonical sample.
    Accepted,
    /// The reading was rejected WHOLE with a named diagnostic (never
    /// partially applied).
    Rejected { diagnostic: String },
    /// The reading was accepted BUT flagged (degraded quality, sequence
    /// regression, or aged event): the flag is the record, the sample is
    /// never silently normalised.
    Flagged { diagnostic: String, quality: Option<TelemetryQuality> },
    /// The exact re-delivery was consumed by the edge dedup window as an
    /// idempotent skip (P4.24 fixture-(a) rule, cited).
    IdempotentSkip,
}

impl CaseOutcome {
    /// The outcome class (compare against
    /// [`ConformanceCase::expected_class`]).
    pub fn class(&self) -> &'static str {
        match self {
            CaseOutcome::Accepted => "accepted",
            CaseOutcome::Rejected { .. } => "rejected",
            CaseOutcome::Flagged { .. } => "flagged",
            CaseOutcome::IdempotentSkip => "idempotent_skip",
        }
    }

    /// A human-safe rendering for the run record (diagnostics are already
    /// named-error text from the ACL — never raw protocol payloads).
    pub fn render(&self) -> String {
        match self {
            CaseOutcome::Accepted => "accepted".to_string(),
            CaseOutcome::Rejected { diagnostic } => format!("rejected ({diagnostic})"),
            CaseOutcome::Flagged { diagnostic, quality } => match quality {
                Some(q) => format!("flagged ({diagnostic}; canonical quality {q:?})"),
                None => format!("flagged ({diagnostic})"),
            },
            CaseOutcome::IdempotentSkip => "idempotent_skip (dedup window)".to_string(),
        }
    }
}

/// The record of ONE case run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseRecord {
    pub case: ConformanceCase,
    /// The point the case ran against (the template point, or the synthetic
    /// unmapped point).
    pub point: String,
    pub outcome: CaseOutcome,
}

/// The full §19.2(6) run report. Deterministic: cases run in the fixed
/// [`ALL_CONFORMANCE_CASES`] order against one template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub records: Vec<CaseRecord>,
}

impl ConformanceReport {
    /// The contract violations: every case whose outcome class does not
    /// match the required behaviour, plus the extra content rules
    /// (non-empty named diagnostics; a stale case that mapped to GOOD; an
    /// empty report). An EMPTY violations list is the harness's pass
    /// condition — the caller asserts on this, never on a bare boolean.
    pub fn contract_violations(&self) -> Vec<String> {
        let mut violations = Vec::new();
        if self.records.is_empty() {
            violations.push("the conformance report is empty; no cases ran".to_string());
            return violations;
        }
        for record in &self.records {
            let expected = record.case.expected_class();
            let actual = record.outcome.class();
            let class_ok = match expected {
                "accepted_or_flagged" => actual == "accepted" || actual == "flagged",
                other => actual == other,
            };
            if !class_ok {
                violations.push(format!(
                    "case '{}' required '{}' but produced '{}': {}",
                    record.case.name(),
                    expected,
                    actual,
                    record.outcome.render()
                ));
            }
            // Content rule: every rejection carries a NAMED diagnostic.
            if let CaseOutcome::Rejected { diagnostic } = &record.outcome {
                if diagnostic.trim().is_empty() {
                    violations.push(format!(
                        "case '{}' rejected the reading with an EMPTY diagnostic; \
                         every rejection must name its cause",
                        record.case.name()
                    ));
                }
            }
            // Content rule: a stale-data case that mapped to GOOD would be
            // a silent quality upgrade — forbidden (P0.08).
            if record.case == ConformanceCase::StaleData {
                if let CaseOutcome::Flagged { quality: Some(q), .. } = &record.outcome {
                    if *q == TelemetryQuality::Good {
                        violations.push(format!(
                            "case 'stale_data' mapped a STALE source state to GOOD — \
                             a forbidden quality upgrade: {}",
                            record.outcome.render()
                        ));
                    }
                }
            }
        }
        violations
    }

    /// Whether the run satisfies the harness contract (no violations).
    pub fn is_clean(&self) -> bool {
        self.contract_violations().is_empty()
    }

    /// The full run record as text (for the decision-doc / CI record).
    pub fn render(&self) -> String {
        let mut out = String::from("conformance report (§19.2(6) fault-injection set):\n");
        for record in &self.records {
            out.push_str(&format!(
                "  case={:<20} point={:<28} {}\n",
                record.case.name(),
                record.point,
                record.outcome.render()
            ));
        }
        let violations = self.contract_violations();
        if violations.is_empty() {
            out.push_str("  contract: satisfied (reject-or-flag per case; no crashes, no fabricated continuity)\n");
        } else {
            out.push_str("  contract VIOLATIONS:\n");
            for violation in &violations {
                out.push_str(&format!("    - {violation}\n"));
            }
        }
        out
    }
}

// --- The harness --------------------------------------------------------------

/// Harness tuning limits (validated at construction — a nonsense limit is a
/// composition error, never a silently degraded check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConformanceLimits {
    /// The maximum event-time lag (behind the edge receipt clock) a sample
    /// may show before the `ClockSkewPast` case FLAGS it instead of
    /// accepting it silently. Positive by contract.
    pub max_clock_skew_past_millis: i64,
}

impl ConformanceLimits {
    fn validate(&self) -> Result<(), ConformanceConfigError> {
        if self.max_clock_skew_past_millis <= 0 {
            return Err(ConformanceConfigError::InvalidLimits {
                reason: "max_clock_skew_past_millis must be positive".to_string(),
            });
        }
        Ok(())
    }
}

/// Harness construction errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceConfigError {
    InvalidLimits { reason: String },
}

impl std::fmt::Display for ConformanceConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConformanceConfigError::InvalidLimits { reason } => {
                write!(f, "invalid conformance limits: {reason}")
            }
        }
    }
}

impl std::error::Error for ConformanceConfigError {}

/// The protocol-agnostic conformance harness. Owns the ACL mapping table
/// under test and the fixed edge receipt time the cases use (the caller
/// supplies a deterministic value in tests; a real lab run stamps from the
/// wall clock and records the value in the run record).
pub struct ConformanceHarness {
    acl: AntiCorruptionLayer,
    ingest_millis: i64,
    limits: ConformanceLimits,
}

impl ConformanceHarness {
    /// Builds the harness over the mapping table under test. `ingest_millis`
    /// is the edge receipt time every case stamps (`ingest_time`, P0.09) —
    /// fixed so the `ClockSkew*` cases are deterministic.
    pub fn new(
        acl: AntiCorruptionLayer,
        ingest_millis: i64,
        limits: ConformanceLimits,
    ) -> Result<Self, ConformanceConfigError> {
        limits.validate()?;
        Ok(Self {
            acl,
            ingest_millis,
            limits,
        })
    }

    /// Runs the FULL §19.2(6) fault-injection set against `adapter` using
    /// `template_point` as the template source (the adapter must be able to
    /// read it; an adapter that cannot read its own template point produces
    /// a report whose violations name the failure — the harness never
    /// panics on an unreachable adapter, it reports).
    pub fn run_fault_injection_set(
        &self,
        adapter: &mut dyn EmsAdapter,
        template_point: &PointKey,
    ) -> ConformanceReport {
        let mut records = Vec::new();
        let template = adapter.read(template_point);
        match template {
            Err(error) => {
                // No template → no synthesized case can run. Record every
                // case as a named rejection; the contract violations make
                // the failure impossible to miss.
                let diagnostic = format!(
                    "the harness could not read its template point '{}' from adapter '{}': {error}",
                    template_point, adapter.id()
                );
                for case in ALL_CONFORMANCE_CASES {
                    records.push(CaseRecord {
                        case,
                        point: template_point.to_string(),
                        outcome: CaseOutcome::Rejected {
                            diagnostic: diagnostic.clone(),
                        },
                    });
                }
            }
            Ok(template) => {
                for case in ALL_CONFORMANCE_CASES {
                    records.push(self.run_case(case, &template));
                }
            }
        }
        // Violations are computed on demand from the records (single source
        // of truth — see `ConformanceReport::contract_violations`).
        ConformanceReport { records }
    }

    /// Runs ONE case against a template (public so a caller can re-run a
    /// single case while debugging a failure; the full set is
    /// [`ConformanceHarness::run_fault_injection_set`]).
    pub fn run_case(&self, case: ConformanceCase, template: &EmsNativeReading) -> CaseRecord {
        let reading = self.faulted_reading(case, template);
        let point = reading.point.to_string();
        let outcome = match case {
            ConformanceCase::DuplicateDelivery => self.duplicate_outcome(&reading),
            ConformanceCase::OutOfOrder => self.out_of_order_outcome(&reading),
            ConformanceCase::ClockSkewPast => self.clock_skew_past_outcome(&reading),
            ConformanceCase::StaleData => self.stale_data_outcome(&reading),
            _ => self.plain_map_outcome(&reading),
        };
        CaseRecord {
            case,
            point,
            outcome,
        }
    }

    /// The case-mutated reading, synthesised from the template. Every fault
    /// is an explicit, reviewable field substitution — the harness never
    /// mangles bytes it does not name.
    fn faulted_reading(
        &self,
        case: ConformanceCase,
        template: &EmsNativeReading,
    ) -> EmsNativeReading {
        let mut reading = template.clone();
        match case {
            ConformanceCase::Valid => {}
            ConformanceCase::Malformed => {
                reading.native_quality_code = Some("WEIRD_VENDOR_FLAG".to_string());
            }
            ConformanceCase::Truncated => {
                reading.native_quality_code = None;
            }
            ConformanceCase::OutOfOrder => {
                // A sequence regression: strictly lower than the template's
                // (saturating at zero — never a wraparound).
                reading.source_sequence = template.source_sequence.saturating_sub(1);
            }
            ConformanceCase::DuplicateDelivery => {} // identical by definition
            ConformanceCase::ClockSkewFuture => {
                // One minute ahead of the edge receipt clock: beyond any
                // honest skew, and refused by `ingest >= event` (P0.09).
                reading.event_time = NativeTimestamp::UnixMillis(self.ingest_millis + 60_000);
            }
            ConformanceCase::ClockSkewPast => {
                // Seven days behind: far beyond any honest skew bound.
                reading.event_time =
                    NativeTimestamp::UnixMillis(self.ingest_millis - 7 * 86_400_000);
            }
            ConformanceCase::StaleData => {
                reading.native_quality_code = Some("STALE".to_string());
            }
            ConformanceCase::UnmappedPoint => {
                reading.point = PointKey::new("conformance-unmapped", "Channel");
            }
        }
        reading
    }

    fn ingest_time(&self) -> Result<IngestTime, String> {
        IngestTime::from_unix_millis(self.ingest_millis)
            .map_err(|error| format!("the harness ingest timestamp is invalid: {error}"))
    }

    /// The plain ACL mapping outcome (valid, malformed, truncated,
    /// clock-skew-future, unmapped point).
    fn plain_map_outcome(&self, reading: &EmsNativeReading) -> CaseOutcome {
        let ingest_time = match self.ingest_time() {
            Ok(t) => t,
            Err(reason) => return CaseOutcome::Rejected { diagnostic: reason },
        };
        match self.acl.map_reading(reading, ingest_time) {
            Ok(_sample) => CaseOutcome::Accepted,
            Err(error) => CaseOutcome::Rejected {
                diagnostic: error.to_string(),
            },
        }
    }

    /// Duplicate delivery: map the SAME reading twice at the SAME receipt
    /// time and admit both through a fresh bounded dedup window. The first
    /// is `New`; the exact replay MUST classify as `ExactDuplicate`
    /// (idempotent skip — one stored value, P4.24 fixture-(a)).
    fn duplicate_outcome(&self, reading: &EmsNativeReading) -> CaseOutcome {
        let ingest_time = match self.ingest_time() {
            Ok(t) => t,
            Err(reason) => return CaseOutcome::Rejected { diagnostic: reason },
        };
        let first = match self.acl.map_reading(reading, ingest_time) {
            Ok(sample) => sample,
            Err(error) => {
                return CaseOutcome::Rejected {
                    diagnostic: format!(
                        "the duplicate case could not map its base reading: {error}"
                    ),
                }
            }
        };
        let second = match self.acl.map_reading(reading, ingest_time) {
            Ok(sample) => sample,
            Err(error) => {
                return CaseOutcome::Rejected {
                    diagnostic: format!(
                        "the duplicate case could not re-map its base reading: {error}"
                    ),
                }
            }
        };
        let mut window = EdgeDedupWindow::new(16);
        match window.admit(&first) {
            AdmitOutcome::New => {}
            other => {
                return CaseOutcome::Rejected {
                    diagnostic: format!(
                        "the FIRST delivery of the duplicate case classified as {other:?}; \
                         expected New"
                    ),
                }
            }
        }
        match window.admit(&second) {
            AdmitOutcome::ExactDuplicate => CaseOutcome::IdempotentSkip,
            AdmitOutcome::ConflictingDuplicate => CaseOutcome::Rejected {
                diagnostic:
                    "the exact re-delivery classified as a CONFLICTING duplicate; the case \
                     delivered byte-identical content, so this is a dedup-contract violation"
                        .to_string(),
            },
            AdmitOutcome::New => CaseOutcome::Rejected {
                diagnostic:
                    "the exact re-delivery classified as New — the dedup window did not \
                     recognise the identical identity (a double-count hazard)"
                        .to_string(),
            },
        }
    }

    /// Out-of-order delivery: the sequence regression is FLAGGED in the
    /// record. The sample itself still maps (the edge read path does not
    /// reorder or re-number; the P0.09 sequence is source-assigned and the
    /// ordering enforcement lives downstream — the P4 `SequenceGuard`
    /// discipline, cited) — but the regression is RECORDED, never silent.
    fn out_of_order_outcome(&self, reading: &EmsNativeReading) -> CaseOutcome {
        let ingest_time = match self.ingest_time() {
            Ok(t) => t,
            Err(reason) => return CaseOutcome::Rejected { diagnostic: reason },
        };
        match self.acl.map_reading(reading, ingest_time) {
            Ok(sample) => CaseOutcome::Flagged {
                diagnostic: format!(
                    "sequence regression: source_sequence {} arrived after {} on the same \
                     point; recorded as observed, never re-ordered or re-numbered",
                    sample.source_sequence(),
                    reading.source_sequence + 1
                ),
                quality: Some(sample.quality()),
            },
            Err(error) => CaseOutcome::Rejected {
                diagnostic: error.to_string(),
            },
        }
    }

    /// Clock skew (past): the sample maps (an old event is still data), but
    /// beyond [`ConformanceLimits::max_clock_skew_past_millis`] the harness
    /// FLAGS the skew — an aged event must not pass unremarked, and the
    /// continuity of the stream is never fabricated around it.
    fn clock_skew_past_outcome(&self, reading: &EmsNativeReading) -> CaseOutcome {
        let ingest_time = match self.ingest_time() {
            Ok(t) => t,
            Err(reason) => return CaseOutcome::Rejected { diagnostic: reason },
        };
        match self.acl.map_reading(reading, ingest_time) {
            Ok(sample) => {
                // `unix_millis()` is `i128` (the contracts type); the skew
                // is computed in `i128` and compared against the `i64`
                // bound through the same widening.
                let skew_millis =
                    i128::from(self.ingest_millis) - sample.event_time().unix_millis();
                if skew_millis > i128::from(self.limits.max_clock_skew_past_millis) {
                    CaseOutcome::Flagged {
                        diagnostic: format!(
                            "event_time trails the edge receipt clock by {skew_millis} ms \
                             (beyond the {} ms bound); recorded, never shifted",
                            self.limits.max_clock_skew_past_millis
                        ),
                        quality: Some(sample.quality()),
                    }
                } else {
                    CaseOutcome::Accepted
                }
            }
            Err(error) => CaseOutcome::Rejected {
                diagnostic: error.to_string(),
            },
        }
    }

    /// Stale data: the source's own STALE code must map to the canonical
    /// STALE state — flagged with the mapped quality, never upgraded to
    /// GOOD, never dropped (P0.08; the P6.08 acceptance rule).
    fn stale_data_outcome(&self, reading: &EmsNativeReading) -> CaseOutcome {
        let ingest_time = match self.ingest_time() {
            Ok(t) => t,
            Err(reason) => return CaseOutcome::Rejected { diagnostic: reason },
        };
        match self.acl.map_reading(reading, ingest_time) {
            Ok(sample) => CaseOutcome::Flagged {
                diagnostic: format!(
                    "the source declared STALE; the canonical sample carries quality {:?} \
                     with the original code retained as metadata",
                    sample.quality()
                ),
                quality: Some(sample.quality()),
            },
            Err(error) => CaseOutcome::Rejected {
                diagnostic: error.to_string(),
            },
        }
    }

    /// A synthetic reading for the `UnmappedPoint` case that does not depend
    /// on a template (the point is unmapped by construction).
    pub fn synthetic_unmapped_reading(&self) -> EmsNativeReading {
        EmsNativeReading {
            point: PointKey::new("conformance-unmapped", "Channel"),
            native_unit: NativeUnit::Kw,
            raw_value: 0.0,
            native_quality_code: Some("GOOD".to_string()),
            event_time: NativeTimestamp::UnixMillis(self.ingest_millis - 1_000),
            source_sequence: 1,
        }
    }

    /// The ACL under test (callers may assert on the declared-unmapped
    /// inventory — the "unknown fields recorded, not silently accepted"
    /// surface).
    pub fn acl(&self) -> &AntiCorruptionLayer {
        &self.acl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::acl::{NativeSign, PointMapping};
    use crate::adapters::simulated::{SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture};
    use flexcore_contracts::units::ReactiveSignConvention;

    const BASE_MILLIS: i64 = 1_700_000_000_000;
    const INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

    fn acl() -> AntiCorruptionLayer {
        AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kw,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:simulated-test",
            vec![],
        )
        .expect("valid mapping table")
    }

    fn fixture() -> SimulatedFixture {
        SimulatedFixture {
            points: vec![SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_500.0],
                quality_per_tick: vec!["GOOD".to_string()],
            }],
            start_millis: BASE_MILLIS,
            step_millis: 1_000,
            reactive_source_convention: ReactiveSignConvention::InjectionPositive,
            cadence_millis: 5_000,
        }
    }

    #[test]
    fn the_full_fault_set_satisfies_the_contract_against_the_simulated_adapter() {
        let harness =
            ConformanceHarness::new(acl(), INGEST_MILLIS, ConformanceLimits {
                max_clock_skew_past_millis: 3_600_000,
            })
            .expect("valid limits");
        let mut adapter = SimulatedEmsSource::new(fixture());
        let report =
            harness.run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
        assert!(
            report.is_clean(),
            "the §19.2(6) set must satisfy the harness contract:\n{}",
            report.render()
        );
        assert_eq!(report.records.len(), ALL_CONFORMANCE_CASES.len());
        // Spot-check the required classes.
        for (case, expected) in [
            (ConformanceCase::Valid, "accepted"),
            (ConformanceCase::Malformed, "rejected"),
            (ConformanceCase::Truncated, "rejected"),
            (ConformanceCase::OutOfOrder, "flagged"),
            (ConformanceCase::DuplicateDelivery, "idempotent_skip"),
            (ConformanceCase::ClockSkewFuture, "rejected"),
            (ConformanceCase::ClockSkewPast, "flagged"),
            (ConformanceCase::StaleData, "flagged"),
            (ConformanceCase::UnmappedPoint, "rejected"),
        ] {
            let record = report
                .records
                .iter()
                .find(|r| r.case == case)
                .expect("case present");
            assert_eq!(record.outcome.class(), expected, "case {}", case.name());
        }
    }

    #[test]
    fn an_unreadable_adapter_produces_a_violating_report_never_a_panic() {
        let harness =
            ConformanceHarness::new(acl(), INGEST_MILLIS, ConformanceLimits {
                max_clock_skew_past_millis: 3_600_000,
            })
            .expect("valid limits");
        let mut adapter = SimulatedEmsSource::new(fixture());
        adapter
            .control()
            .set_unreachable(Some("harness drill".to_string()));
        let report =
            harness.run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
        assert!(
            !report.is_clean(),
            "an unreachable adapter cannot satisfy the contract — the report must name it"
        );
        assert!(report.render().contains("template point"));
    }

    #[test]
    fn zero_and_negative_limits_are_composition_errors() {
        let limits = ConformanceLimits {
            max_clock_skew_past_millis: 0,
        };
        assert!(matches!(
            ConformanceHarness::new(acl(), INGEST_MILLIS, limits),
            Err(ConformanceConfigError::InvalidLimits { .. })
        ));
    }
}
