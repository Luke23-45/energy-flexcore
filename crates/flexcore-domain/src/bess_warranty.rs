// P15.10 — The BESS warranty object model (pure domain records, no I/O).
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 24_p15_10_warranty_object_model.md`.
//
// WHAT THIS MODULE IS: the object model for BESS warranty terms — the
// capacity-retention schedule, cycle and throughput limits, RTE and
// availability commitments, augmentation events — and the WARRANTY-HEADROOM
// computation: remaining allowance vs the consumed counters of the P15.07
// degradation ledger (`crate::bess_degradation`).
//
// REFERENCE STRUCTURES, NO VENDOR CONFORMANCE (the honesty line,
// [`WARRANTY_REFERENCE_STRUCTURE_STATEMENT`]): the Megapack CMA (Tesla)
// [S-150] and the Fluence Dispatchable Energy Guarantee class [S-151] are
// cited as the reference STRUCTURES for the field set (a capacity-retention
// schedule over years; cycle/throughput limits; augmentation as a recorded
// event). Their texts were NOT consulted; every semantic below is
// UNVERIFIED against those primary sources; nothing here claims conformance
// with any vendor's warranty, and no warranty number is invented — every
// term value is declared input with a citation.
//
// PROVENANCE (P3.02, inherited from P15.07): warranty terms carry provenance
// (a signed contract is `Configured` data with a citation; an unsourced term
// is `Unknown`). Headroom computed against counters or SoH observations that
// carry UNKNOWN provenance, or against a STALE SoH report (P15.09
// `soh_report_creditable`), is **NOT CREDITABLE** — unknown narrows, never
// widens; stale is never silently used. A negative headroom (allowance
// exceeded) is a RECORD for the operator, never a penalty, a claim, or a
// dispatch input (ADR-010, conventions §6.3).
//
// UNITS: energies in MWh, FEC dimensionless, SoH/retention/RTE/availability
// fractions in [0, 1] (canonical freeze untouched; `flexcore-contracts::
// units` is read-only). The warranty year is the Julian year (365.25 days)
// — a DECLARED convention, not physics, recorded in [`WARRANTY_YEAR_MILLIS`].
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// I/O, no solver coupling, no device-write path, no dispatch reach.

use flexcore_contracts::capability::Provenance;

use crate::bess_degradation::{BessDegradationLedger, DegradationLedgerError};
use crate::bess_soh::{soh_report_creditable, SohStalenessConfig};

// --- Recorded statements -----------------------------------------------------

/// The reference-structure statement (visible in exports, not a comment).
pub const WARRANTY_REFERENCE_STRUCTURE_STATEMENT: &str =
    "Reference structures only: the Megapack CMA (Tesla) [S-150] and the \
Fluence Dispatchable Energy Guarantee class [S-151] informed the FIELD SET \
of this object model (capacity-retention schedule, cycle/throughput limits, \
RTE/availability commitments, augmentation events). Their texts were not \
consulted; every semantic here is UNVERIFIED against those primary sources; \
FlexCore claims NO conformance with any vendor warranty, and no warranty \
value is invented — every term is declared input with a citation.";

/// The declared warranty-year convention: 365.25 days in milliseconds (the
/// Julian year). A declared convention for resolving the capacity-retention
/// schedule against elapsed time — not a calendar-accurate year and not
/// physics.
pub const WARRANTY_YEAR_MILLIS: i64 = 365 * 24 * 3_600_000 + 6 * 3_600_000;

/// The warranty-headroom honesty statement (carried on every report).
pub const WARRANTY_HEADROOM_STATEMENT: &str =
    "Warranty headroom is a RECORD computed from declared terms and the \
P15.07 consumed counters: remaining allowance vs consumed usage. A negative \
headroom (allowance exceeded) is recorded, never penalized, claimed, or \
dispatched upon. Headroom computed against UNKNOWN-provenance counters or a \
STALE SoH report is NOT CREDITABLE (P3.02 / P15.09 — unknown narrows, stale \
is never silently used). Headroom SUPPORTS human warranty review; it never \
asserts a claim (P15.11).";

// --- Errors ------------------------------------------------------------------

/// Errors of warranty-term construction and headroom evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum WarrantyError {
    /// A numeric datum was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be positive was zero or negative.
    NonPositiveValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A fraction was outside its declared inclusive range.
    ValueOutOfRange {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
        /// The inclusive lower bound.
        lower: f64,
        /// The inclusive upper bound.
        upper: f64,
    },
    /// A required text field was empty or whitespace.
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
        /// The recorded bound.
        max_chars: usize,
    },
    /// The warranty window was inverted or degenerate
    /// (`valid_from >= valid_until`).
    InvertedWindow {
        /// The declared start (unix millis).
        valid_from_unix_ms: i64,
        /// The declared end (unix millis).
        valid_until_unix_ms: i64,
    },
    /// The capacity-retention schedule was not strictly ascending in years.
    NonAscendingScheduleYear {
        /// The offending year index.
        year_index: u32,
    },
    /// The capacity-retention schedule's first point was not year 1.
    ScheduleMustStartAtYearOne,
    /// A structural error came from the P15.07 ledger.
    LedgerError(DegradationLedgerError),
}

impl std::fmt::Display for WarrantyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarrantyError::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite (found {value})")
            }
            WarrantyError::NonPositiveValue { field, value } => {
                write!(f, "{field} must be > 0 (found {value})")
            }
            WarrantyError::ValueOutOfRange { field, value, lower, upper } => write!(
                f,
                "{field} must be in [{lower}, {upper}] inclusive (found {value})"
            ),
            WarrantyError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            WarrantyError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
            WarrantyError::InvertedWindow {
                valid_from_unix_ms,
                valid_until_unix_ms,
            } => write!(
                f,
                "the warranty window is inverted or degenerate: valid_from \
                 {valid_from_unix_ms} ms must be strictly before valid_until \
                 {valid_until_unix_ms} ms"
            ),
            WarrantyError::NonAscendingScheduleYear { year_index } => write!(
                f,
                "the capacity-retention schedule years must be strictly ascending \
                 (offending year {year_index})"
            ),
            WarrantyError::ScheduleMustStartAtYearOne => {
                write!(f, "the capacity-retention schedule must start at year 1")
            }
            WarrantyError::LedgerError(error) => write!(f, "degradation ledger error: {error}"),
        }
    }
}

impl std::error::Error for WarrantyError {}

fn validate_text(field: &'static str, value: &str) -> Result<(), WarrantyError> {
    if value.trim().is_empty() {
        return Err(WarrantyError::EmptyField { field });
    }
    if value.len() > 256 {
        return Err(WarrantyError::FieldTooLong { field, max_chars: 256 });
    }
    Ok(())
}

// --- Warranty terms -----------------------------------------------------------

/// One point of the capacity-retention schedule: the minimum guaranteed
/// capacity retention (fraction in [0, 1]) at the end of warranty year
/// `year_index` (years are 1-based and strictly ascending; the schedule is
/// the reference structure's shape [S-150][S-151], values are declared data).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapacityRetentionPoint {
    /// The warranty year (1-based; strictly ascending across the schedule).
    pub year_index: u32,
    /// The minimum guaranteed capacity retention as a fraction in [0, 1].
    pub min_retention_fraction: f64,
}

/// The declared RTE commitment: the minimum round-trip efficiency (fraction
/// in (0, 1]) with the declared measurement basis. DECLARED DATA — no RTE
/// verdict is computed anywhere in this phase (the P15.07 ledger accumulates
/// no efficiency measurement; recorded limitation).
#[derive(Debug, Clone, PartialEq)]
pub struct RteCommitment {
    /// The minimum committed round-trip efficiency, fraction in (0, 1].
    pub min_round_trip_efficiency_fraction: f64,
    /// The declared measurement basis (recorded data; e.g. the metering
    /// point and averaging window the commitment references).
    pub measurement_basis: String,
}

/// The declared availability commitment: the minimum availability (fraction
/// in [0, 1]) with the declared measurement basis. DECLARED DATA — no
/// availability verdict is computed anywhere in this phase (recorded
/// limitation; availability KPIs are a wave-2 P15.12 concern).
#[derive(Debug, Clone, PartialEq)]
pub struct AvailabilityCommitment {
    /// The minimum committed availability, fraction in [0, 1].
    pub min_availability_fraction: f64,
    /// The declared measurement basis (recorded data).
    pub measurement_basis: String,
}

/// The warranty terms for one asset or system (P15.10). Every value is
/// declared input with provenance and a citation — no default warranty and
/// no invented number.
#[derive(Debug, Clone, PartialEq)]
pub struct WarrantyTerms {
    /// The warranty identifier (declared, non-empty).
    pub warranty_id: String,
    /// The warranty window start (unix millis, inclusive).
    pub valid_from_unix_ms: i64,
    /// The warranty window end (unix millis, exclusive; strictly after the
    /// start).
    pub valid_until_unix_ms: i64,
    /// The capacity-retention schedule (non-empty, years strictly ascending
    /// starting at 1).
    pub retention_schedule: Vec<CapacityRetentionPoint>,
    /// The maximum full equivalent cycles before coverage terms change
    /// (the throughput-warranty reference structure [S-152][S-153]);
    /// `None` records the dimension absent.
    pub max_full_equivalent_cycles: Option<f64>,
    /// The maximum cumulative (charge + discharge) throughput in MWh before
    /// coverage terms change ([S-152][S-153]); `None` records the dimension
    /// absent.
    pub max_cumulative_throughput_mwh: Option<f64>,
    /// The declared RTE commitment, if any.
    pub rte: Option<RteCommitment>,
    /// The declared availability commitment, if any.
    pub availability: Option<AvailabilityCommitment>,
    /// The source citation (the warranty document reference; required —
    /// an unsourced warranty term is not reviewed data).
    pub citation: String,
    /// The provenance of the terms.
    pub provenance: Provenance,
}

impl WarrantyTerms {
    /// Constructs and validates warranty terms.
    pub fn new(
        warranty_id: String,
        valid_from_unix_ms: i64,
        valid_until_unix_ms: i64,
        retention_schedule: Vec<CapacityRetentionPoint>,
        max_full_equivalent_cycles: Option<f64>,
        max_cumulative_throughput_mwh: Option<f64>,
        rte: Option<RteCommitment>,
        availability: Option<AvailabilityCommitment>,
        citation: String,
        provenance: Provenance,
    ) -> Result<Self, WarrantyError> {
        validate_text("warranty_id", &warranty_id)?;
        validate_text("citation", &citation)?;
        if valid_from_unix_ms >= valid_until_unix_ms {
            return Err(WarrantyError::InvertedWindow {
                valid_from_unix_ms,
                valid_until_unix_ms,
            });
        }
        if retention_schedule.is_empty() {
            return Err(WarrantyError::EmptyField {
                field: "retention_schedule",
            });
        }
        if retention_schedule[0].year_index != 1 {
            return Err(WarrantyError::ScheduleMustStartAtYearOne);
        }
        let mut previous_year: Option<u32> = None;
        for point in &retention_schedule {
            if let Some(previous) = previous_year {
                if point.year_index <= previous {
                    return Err(WarrantyError::NonAscendingScheduleYear {
                        year_index: point.year_index,
                    });
                }
            }
            if !point.min_retention_fraction.is_finite() {
                return Err(WarrantyError::NonFiniteValue {
                    field: "min_retention_fraction",
                    value: point.min_retention_fraction,
                });
            }
            if !(0.0..=1.0).contains(&point.min_retention_fraction) {
                return Err(WarrantyError::ValueOutOfRange {
                    field: "min_retention_fraction",
                    value: point.min_retention_fraction,
                    lower: 0.0,
                    upper: 1.0,
                });
            }
            previous_year = Some(point.year_index);
        }
        for (field, limit) in [
            (
                "max_full_equivalent_cycles",
                max_full_equivalent_cycles,
            ),
            (
                "max_cumulative_throughput_mwh",
                max_cumulative_throughput_mwh,
            ),
        ] {
            if let Some(value) = limit {
                if !value.is_finite() {
                    return Err(WarrantyError::NonFiniteValue { field, value });
                }
                if value <= 0.0 {
                    return Err(WarrantyError::NonPositiveValue { field, value });
                }
            }
        }
        if let Some(rte) = &rte {
            if !rte.min_round_trip_efficiency_fraction.is_finite()
                || rte.min_round_trip_efficiency_fraction <= 0.0
                || rte.min_round_trip_efficiency_fraction > 1.0
            {
                return Err(WarrantyError::ValueOutOfRange {
                    field: "min_round_trip_efficiency_fraction",
                    value: rte.min_round_trip_efficiency_fraction,
                    lower: f64::EPSILON,
                    upper: 1.0,
                });
            }
            validate_text("rte.measurement_basis", &rte.measurement_basis)?;
        }
        if let Some(availability) = &availability {
            if !availability.min_availability_fraction.is_finite()
                || !(0.0..=1.0).contains(&availability.min_availability_fraction)
            {
                return Err(WarrantyError::ValueOutOfRange {
                    field: "min_availability_fraction",
                    value: availability.min_availability_fraction,
                    lower: 0.0,
                    upper: 1.0,
                });
            }
            validate_text(
                "availability.measurement_basis",
                &availability.measurement_basis,
            )?;
        }
        Ok(Self {
            warranty_id,
            valid_from_unix_ms,
            valid_until_unix_ms,
            retention_schedule,
            max_full_equivalent_cycles,
            max_cumulative_throughput_mwh,
            rte,
            availability,
            citation,
            provenance,
        })
    }

    /// The scheduled minimum retention for the elapsed warranty year:
    /// the last schedule point whose `year_index` is at or below
    /// `elapsed_years` (the schedule holds flat between points — the
    /// reference structure's reading [S-150], UNVERIFIED). When
    /// `elapsed_years` falls short of the first point (year 1), the first
    /// point applies. When it runs beyond the schedule's coverage, the LAST
    /// point applies and the caller-visible flag
    /// [`RetentionResolution::beyond_schedule_coverage`] records the
    /// limitation — the schedule is never extrapolated beyond its last
    /// declared year with an invented stricter value.
    pub fn resolve_retention(&self, elapsed_years: f64) -> RetentionResolution {
        let mut resolved = &self.retention_schedule[0];
        for point in &self.retention_schedule {
            if (point.year_index as f64) <= elapsed_years {
                resolved = point;
            }
        }
        // Beyond-coverage means past the schedule's LAST declared year, not
        // past the resolved point: elapsed time between points still holds
        // the last point at or below it, inside coverage.
        let last_year = self
            .retention_schedule
            .last()
            .map(|point| point.year_index as f64)
            .unwrap_or(0.0);
        RetentionResolution {
            year_index: resolved.year_index,
            min_retention_fraction: resolved.min_retention_fraction,
            beyond_schedule_coverage: elapsed_years > last_year,
        }
    }
}

/// The retention-schedule resolution for one evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetentionResolution {
    /// The schedule year the resolution landed on.
    pub year_index: u32,
    /// The scheduled minimum retention for that year (fraction in [0, 1]).
    pub min_retention_fraction: f64,
    /// True when the elapsed time ran beyond the schedule's last declared
    /// year: the LAST point applied and the limitation is recorded — the
    /// schedule is never extrapolated.
    pub beyond_schedule_coverage: bool,
}

// --- Augmentation events ---------------------------------------------------------

/// A recorded augmentation event (P15.10): capacity added to restore a
/// degraded system within warranty terms (the reference structure's
/// augmentation concept [S-151], UNVERIFIED). A RECORD of what was declared
/// to have happened — never a physical action this platform took.
#[derive(Debug, Clone, PartialEq)]
pub struct AugmentationEvent {
    /// When the augmentation was recorded as effective (unix millis).
    pub at_unix_ms: i64,
    /// The added usable energy in MWh (strictly positive).
    pub added_usable_energy_mwh: f64,
    /// The recorded note (audit-safe: what the operator declared, references
    /// only — no internal paths).
    pub note: String,
    /// The provenance of the event record.
    pub provenance: Provenance,
}

impl AugmentationEvent {
    /// Constructs and validates an augmentation event.
    pub fn new(
        at_unix_ms: i64,
        added_usable_energy_mwh: f64,
        note: String,
        provenance: Provenance,
    ) -> Result<Self, WarrantyError> {
        if !added_usable_energy_mwh.is_finite() {
            return Err(WarrantyError::NonFiniteValue {
                field: "added_usable_energy_mwh",
                value: added_usable_energy_mwh,
            });
        }
        if added_usable_energy_mwh <= 0.0 {
            return Err(WarrantyError::NonPositiveValue {
                field: "added_usable_energy_mwh",
                value: added_usable_energy_mwh,
            });
        }
        validate_text("note", &note)?;
        Ok(Self {
            at_unix_ms,
            added_usable_energy_mwh,
            note,
            provenance,
        })
    }
}

// --- Headroom ----------------------------------------------------------------------

/// The status of one headroom dimension. Every variant is a RECORD — never
/// a penalty, a claim, or a dispatch input.
#[derive(Debug, Clone, PartialEq)]
pub enum HeadroomStatus {
    /// The warranty declares no limit for this dimension.
    NotConfigured,
    /// The dimension could not be evaluated from creditable data: the
    /// reason names exactly what was not creditable (UNKNOWN provenance,
    /// no fresh SoH, no SoH at all — P3.02 / P15.09).
    NotCreditable {
        /// The recorded reason (audit-safe).
        reason: String,
    },
    /// The consumed amount is within the allowance.
    Within {
        /// The remaining allowance (MWh or cycles per `unit`).
        remaining: f64,
        /// The unit of `remaining` ("MWh" or "cycles").
        unit: &'static str,
    },
    /// The consumed amount exceeded the allowance. A RECORD for the
    /// operator — never a penalty, a claim, or a dispatch input.
    Exceeded {
        /// The overage beyond the allowance (MWh or cycles per `unit`).
        overage: f64,
        /// The unit of `overage` ("MWh" or "cycles").
        unit: &'static str,
    },
    /// The capacity-retention dimension was evaluated: the full retention
    /// detail (the margin may be negative — a recorded below-schedule SoH,
    /// never a claim).
    Evaluated(CapacityRetentionHeadroom),
}

/// The capacity-retention headroom record: the scheduled minimum for the
/// elapsed year vs the latest creditable SoH observation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapacityRetentionHeadroom {
    /// The resolved schedule year.
    pub year_index: u32,
    /// The scheduled minimum retention (fraction in [0, 1]).
    pub scheduled_minimum_fraction: f64,
    /// The observed SoH (fraction in [0, 1]) from the latest creditable
    /// observation.
    pub observed_soh_fraction: f64,
    /// `observed_soh − scheduled_minimum`; negative means the observed SoH
    /// sits BELOW the scheduled minimum (a record, never a claim).
    pub margin_fraction: f64,
    /// True when the elapsed time ran beyond the schedule's last declared
    /// year (the schedule was NOT extrapolated; the last point applied).
    pub beyond_schedule_coverage: bool,
}

/// The warranty-window status at evaluation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowStatus {
    /// `valid_from <= now < valid_until`.
    Within,
    /// `now >= valid_until`.
    Expired,
    /// `now < valid_from`.
    NotYetEffective,
}

/// The full warranty-headroom report (P15.10). Every field is a RECORD;
/// nothing here asserts a warranty claim (P15.11 exports SUPPORT human
/// claims, never assert them).
#[derive(Debug, Clone, PartialEq)]
pub struct WarrantyHeadroomReport {
    /// The evaluation time (unix millis).
    pub evaluated_at_unix_ms: i64,
    /// The warranty window status at evaluation time.
    pub window: WindowStatus,
    /// The capacity-retention headroom, or the recorded reason it could not
    /// be evaluated.
    pub capacity_retention: HeadroomStatus,
    /// The cycle-limit headroom vs the P15.07 FEC counter.
    pub cycles: HeadroomStatus,
    /// The throughput-limit headroom vs the P15.07 cumulative throughput
    /// counter.
    pub throughput: HeadroomStatus,
    /// The resolved capacity-retention detail, present only when the
    /// capacity dimension was evaluated.
    pub capacity_detail: Option<CapacityRetentionHeadroom>,
    /// Whether the ledger's chemistry is inside the P15-E-3 DEFAULT scope
    /// (LFP/NMC first [S-147]); recorded for every report.
    pub chemistry_in_scope: bool,
    /// The recorded RTE commitment, if declared — DECLARED DATA only; no
    /// RTE verdict is computed in this phase (recorded limitation).
    pub rte_declared_only: Option<RteCommitment>,
    /// The declared availability commitment, if any — DECLARED DATA only;
    /// no availability verdict is computed in this phase (recorded
    /// limitation).
    pub availability_declared_only: Option<AvailabilityCommitment>,
}

impl WarrantyHeadroomReport {
    /// The recorded honesty statement ([`WARRANTY_HEADROOM_STATEMENT`]).
    pub fn statement(&self) -> &'static str {
        WARRANTY_HEADROOM_STATEMENT
    }
}

/// Computes the warranty headroom: the warranty's remaining allowances vs
/// the consumed counters of the P15.07 ledger, at `now_unix_millis`, with
/// SoH creditability gated by the P15.09 staleness configuration.
///
/// Pure function of declared data — no clock read, no I/O, no solver, no
/// dispatch reach.
pub fn compute_headroom(
    terms: &WarrantyTerms,
    ledger: &BessDegradationLedger,
    now_unix_millis: i64,
    soh_config: &SohStalenessConfig,
) -> Result<WarrantyHeadroomReport, WarrantyError> {
    let window = if now_unix_millis < terms.valid_from_unix_ms {
        WindowStatus::NotYetEffective
    } else if now_unix_millis >= terms.valid_until_unix_ms {
        WindowStatus::Expired
    } else {
        WindowStatus::Within
    };

    // --- Capacity retention ---------------------------------------------
    let capacity_retention = match latest_creditable_soh(ledger, now_unix_millis, soh_config) {
        SohSource::None => HeadroomStatus::NotCreditable {
            reason: "no SoH observation exists on the P15.07 ledger; capacity-retention \
                     headroom is not creditable (nothing is invented, P3.02)"
                .to_string(),
        },
        SohSource::NotCreditable(reason) => HeadroomStatus::NotCreditable { reason },
        SohSource::Creditable(soh_fraction) => {
            let elapsed_years =
                (now_unix_millis - terms.valid_from_unix_ms) as f64 / WARRANTY_YEAR_MILLIS as f64;
            let resolution = terms.resolve_retention(elapsed_years);
            let margin = soh_fraction - resolution.min_retention_fraction;
            HeadroomStatus::Evaluated(CapacityRetentionHeadroom {
                year_index: resolution.year_index,
                scheduled_minimum_fraction: resolution.min_retention_fraction,
                observed_soh_fraction: soh_fraction,
                margin_fraction: margin,
                beyond_schedule_coverage: resolution.beyond_schedule_coverage,
            })
        }
    };

    // --- Cycle limit vs the P15.07 FEC counter -----------------------------
    let cycles = match terms.max_full_equivalent_cycles {
        None => HeadroomStatus::NotConfigured,
        Some(limit) => {
            if !ledger.counters_creditable() {
                HeadroomStatus::NotCreditable {
                    reason: "the P15.07 cumulative counters carry UNKNOWN provenance or no \
                             observation; cycle headroom is not creditable (unknown narrows, \
                             never widens)"
                        .to_string(),
                }
            } else {
                headroom_status(limit, ledger.full_equivalent_cycles.value(), "cycles")
            }
        }
    };

    // --- Throughput limit vs the P15.07 cumulative throughput ---------------
    let throughput = match terms.max_cumulative_throughput_mwh {
        None => HeadroomStatus::NotConfigured,
        Some(limit) => {
            if !ledger.counters_creditable() {
                HeadroomStatus::NotCreditable {
                    reason: "the P15.07 cumulative counters carry UNKNOWN provenance or no \
                             observation; throughput headroom is not creditable (unknown \
                             narrows, never widens)"
                        .to_string(),
                }
            } else {
                headroom_status(limit, ledger.cumulative_throughput_mwh.value(), "MWh")
            }
        }
    };

    let capacity_detail = match &capacity_retention {
        HeadroomStatus::Evaluated(detail) => Some(*detail),
        _ => None,
    };

    Ok(WarrantyHeadroomReport {
        evaluated_at_unix_ms: now_unix_millis,
        window,
        capacity_retention,
        cycles,
        throughput,
        capacity_detail,
        chemistry_in_scope: ledger.chemistry.is_in_scope(),
        rte_declared_only: terms.rte.clone(),
        availability_declared_only: terms.availability.clone(),
    })
}

/// The latest SoH observation, classified for creditability (P3.02 +
/// P15.09): present, fresh, and known-provenance — or the recorded reason it
/// is not.
enum SohSource {
    None,
    NotCreditable(String),
    Creditable(f64),
}

fn latest_creditable_soh(
    ledger: &BessDegradationLedger,
    now_unix_millis: i64,
    soh_config: &SohStalenessConfig,
) -> SohSource {
    let Some(latest) = ledger.latest_soh() else {
        return SohSource::None;
    };
    let report = crate::bess_soh::BmsSohReport {
        soh_fraction: latest.soh_fraction,
        event_time_unix_ms: latest.observed_at_unix_ms,
        // The ledger observation carries no separate ingest stamp; the
        // observation time doubles as the ingest time here (recorded
        // convention: the P15.07 ledger records observations, not
        // transport receipts).
        ingest_time_unix_ms: latest.observed_at_unix_ms,
        provenance: latest.provenance,
        source: latest.source.clone(),
    };
    if !soh_report_creditable(&report, now_unix_millis, soh_config) {
        return SohSource::NotCreditable(
            "the latest SoH observation carries UNKNOWN provenance or is stale beyond the \
             configured maximum age (P15.09); capacity-retention headroom is not creditable"
                .to_string(),
        );
    }
    SohSource::Creditable(latest.soh_fraction)
}

/// Remaining-vs-consumed for a scalar limit.
fn headroom_status(limit: f64, consumed: f64, unit: &'static str) -> HeadroomStatus {
    let remaining = limit - consumed;
    if remaining >= 0.0 {
        HeadroomStatus::Within { remaining, unit }
    } else {
        HeadroomStatus::Exceeded {
            overage: -remaining,
            unit,
        }
    }
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bess_degradation::{
        DegradationChemistry, SohObservation as LedgerSohObservation,
    };
    use crate::bess_model::ProvenancedValue;
    use crate::ids::AssetId;
    use flexcore_contracts::ids::FlexUuid;

    const DAY: i64 = 24 * 3_600_000;

    fn asset() -> AssetId {
        AssetId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8d").unwrap())
    }

    fn retention_schedule() -> Vec<CapacityRetentionPoint> {
        vec![
            CapacityRetentionPoint { year_index: 1, min_retention_fraction: 0.96 },
            CapacityRetentionPoint { year_index: 5, min_retention_fraction: 0.90 },
            CapacityRetentionPoint { year_index: 10, min_retention_fraction: 0.80 },
        ]
    }

    fn terms() -> WarrantyTerms {
        WarrantyTerms::new(
            "warranty-2027-001".to_string(),
            0,
            10 * WARRANTY_YEAR_MILLIS,
            retention_schedule(),
            Some(6_000.0),
            Some(60_000.0),
            Some(RteCommitment {
                min_round_trip_efficiency_fraction: 0.86,
                measurement_basis: "declared metering basis".to_string(),
            }),
            Some(AvailabilityCommitment {
                min_availability_fraction: 0.97,
                measurement_basis: "declared basis".to_string(),
            }),
            "declared warranty citation".to_string(),
            Provenance::Configured,
        )
        .unwrap()
    }

    fn ledger(chemistry: DegradationChemistry) -> BessDegradationLedger {
        BessDegradationLedger::new(
            asset(),
            chemistry,
            ProvenancedValue::configured(10.0),
            vec![0.0, 0.5, 1.0],
            vec![0.0, 1.0],
            vec![0.0, 45.0],
            0.9,
        )
        .unwrap()
    }

    // --- terms construction ------------------------------------------------

    #[test]
    fn terms_validate_schedule_limits_and_text() {
        assert!(matches!(
            WarrantyTerms::new(
                "w".to_string(),
                100,
                100, // degenerate window
                retention_schedule(),
                None,
                None,
                None,
                None,
                "citation".to_string(),
                Provenance::Configured,
            ),
            Err(WarrantyError::InvertedWindow { .. })
        ));
        assert!(matches!(
            WarrantyTerms::new(
                "w".to_string(),
                0,
                1,
                vec![CapacityRetentionPoint { year_index: 2, min_retention_fraction: 0.9 }],
                None,
                None,
                None,
                None,
                "citation".to_string(),
                Provenance::Configured,
            ),
            Err(WarrantyError::ScheduleMustStartAtYearOne)
        ));
        assert!(matches!(
            WarrantyTerms::new(
                "w".to_string(),
                0,
                1,
                vec![
                    CapacityRetentionPoint { year_index: 1, min_retention_fraction: 0.9 },
                    CapacityRetentionPoint { year_index: 1, min_retention_fraction: 0.8 },
                ],
                None,
                None,
                None,
                None,
                "citation".to_string(),
                Provenance::Configured,
            ),
            Err(WarrantyError::NonAscendingScheduleYear { year_index: 1 })
        ));
        assert!(matches!(
            WarrantyTerms::new(
                "w".to_string(),
                0,
                1,
                retention_schedule(),
                Some(0.0), // non-positive limit
                None,
                None,
                None,
                "citation".to_string(),
                Provenance::Configured,
            ),
            Err(WarrantyError::NonPositiveValue { field: "max_full_equivalent_cycles", .. })
        ));
    }

    // --- retention resolution ------------------------------------------------

    #[test]
    fn retention_resolves_to_the_last_point_at_or_below_elapsed_years() {
        let terms = terms();
        // Half a year: the year-1 point applies.
        let half = terms.resolve_retention(0.5);
        assert_eq!(half.year_index, 1);
        assert!(!half.beyond_schedule_coverage);
        // Exactly year 5: the year-5 point applies.
        let five = terms.resolve_retention(5.0);
        assert_eq!(five.year_index, 5);
        assert_eq!(five.min_retention_fraction, 0.90);
        // Year 7 (between points): the year-5 point holds.
        let seven = terms.resolve_retention(7.0);
        assert_eq!(seven.year_index, 5);
        assert!(!seven.beyond_schedule_coverage);
    }

    #[test]
    fn retention_beyond_the_schedule_is_recorded_never_extrapolated() {
        let terms = terms();
        let twelve = terms.resolve_retention(12.0);
        assert_eq!(twelve.year_index, 10);
        assert_eq!(twelve.min_retention_fraction, 0.80);
        assert!(
            twelve.beyond_schedule_coverage,
            "beyond-coverage evaluation must be flagged, never silently extended"
        );
    }

    // --- headroom ------------------------------------------------------------

    #[test]
    fn headroom_computes_retention_cycle_and_throughput_margins() {
        let mut ledger = ledger(DegradationChemistry::Lfp);
        // Throughput: 1_000 cycles' worth of consumption on a 6_000-cycle
        // allowance; throughput 20_000 MWh of 60_000 MWh.
        for _ in 0..200 {
            ledger
                .record_throughput(50.0, 50.0, Provenance::Measured)
                .unwrap();
        }
        // FEC: 200 * 100 / (2 * 10) = 1_000.0 cycles; throughput 20_000 MWh.
        // A fresh, measured SoH at 0.94.
        ledger
            .record_soh_observation(
                LedgerSohObservation::new(0.94, DAY, Provenance::Measured, "bms-7".to_string())
                    .unwrap(),
            )
            .unwrap();
        let report = compute_headroom(&terms(), &ledger, 2 * DAY, &SohStalenessConfig::default())
            .unwrap();
        assert_eq!(report.window, WindowStatus::Within);
        assert!(report.chemistry_in_scope);
        match &report.cycles {
            HeadroomStatus::Within { remaining, unit } => {
                assert_eq!(*unit, "cycles");
                assert!((remaining - 5_000.0).abs() < 1e-9);
            }
            other => panic!("expected Within(cycles), got {other:?}"),
        }
        match &report.throughput {
            HeadroomStatus::Within { remaining, unit } => {
                assert_eq!(*unit, "MWh");
                assert!((remaining - 40_000.0).abs() < 1e-9);
            }
            other => panic!("expected Within(MWh), got {other:?}"),
        }
        let detail = report.capacity_detail.expect("capacity evaluated");
        assert_eq!(detail.year_index, 1);
        assert_eq!(detail.scheduled_minimum_fraction, 0.96);
        assert!((detail.margin_fraction - (0.94 - 0.96)).abs() < 1e-12);
    }

    #[test]
    fn an_exceeded_allowance_is_a_record_not_a_penalty() {
        let mut ledger = ledger(DegradationChemistry::Nmc);
        // 7_000 cycles' worth on a 6_000-cycle allowance:
        // 7_000 * 10 * 2 / 100 = 1_400 throughput MWh per 100 cycles... use
        // direct arithmetic: each record_throughput(500, 500) adds 1_000 MWh
        // throughput and 50 FEC; 140 records -> 7_000 FEC, 140_000 MWh.
        for _ in 0..140 {
            ledger
                .record_throughput(500.0, 500.0, Provenance::Measured)
                .unwrap();
        }
        ledger
            .record_soh_observation(
                LedgerSohObservation::new(0.9, DAY, Provenance::Measured, "bms-7".to_string())
                    .unwrap(),
            )
            .unwrap();
        let report = compute_headroom(&terms(), &ledger, 2 * DAY, &SohStalenessConfig::default())
            .unwrap();
        assert!(matches!(
            report.cycles,
            HeadroomStatus::Exceeded { overage, unit } if unit == "cycles" && (overage - 1_000.0).abs() < 1e-9
        ));
        assert!(matches!(
            report.throughput,
            HeadroomStatus::Exceeded { overage, unit } if unit == "MWh" && (overage - 80_000.0).abs() < 1e-9
        ));
        assert!(report.statement().contains("never penalized"));
    }

    #[test]
    fn headroom_is_not_creditable_without_soh_observations() {
        let ledger = ledger(DegradationChemistry::Lfp);
        let report = compute_headroom(&terms(), &ledger, DAY, &SohStalenessConfig::default())
            .unwrap();
        assert!(matches!(
            report.capacity_retention,
            HeadroomStatus::NotCreditable { .. }
        ));
        assert!(report.capacity_detail.is_none());
    }

    #[test]
    fn headroom_is_not_creditable_with_unknown_provenance_counters() {
        let mut ledger = ledger(DegradationChemistry::Lfp);
        ledger
            .record_throughput(500.0, 500.0, Provenance::Unknown)
            .unwrap();
        ledger
            .record_soh_observation(
                LedgerSohObservation::new(0.9, DAY, Provenance::Measured, "bms-7".to_string())
                    .unwrap(),
            )
            .unwrap();
        let report = compute_headroom(&terms(), &ledger, 2 * DAY, &SohStalenessConfig::default())
            .unwrap();
        // Capacity is creditable (fresh measured SoH), but the counters are
        // not: unknown narrows, never widens (P3.02).
        assert!(matches!(report.capacity_retention, HeadroomStatus::Evaluated(_)));
        assert!(matches!(
            report.cycles,
            HeadroomStatus::NotCreditable { .. }
        ));
        assert!(matches!(
            report.throughput,
            HeadroomStatus::NotCreditable { .. }
        ));
    }

    #[test]
    fn headroom_is_not_creditable_with_a_stale_soh_observation() {
        let mut ledger = ledger(DegradationChemistry::Lfp);
        ledger
            .record_soh_observation(
                LedgerSohObservation::new(0.9, DAY, Provenance::Measured, "bms-7".to_string())
                    .unwrap(),
            )
            .unwrap();
        // Evaluate 30 days later: the observation is stale under the 7-day
        // default bound (P15.09) — never silently credited.
        let report = compute_headroom(
            &terms(),
            &ledger,
            30 * DAY,
            &SohStalenessConfig::default(),
        )
        .unwrap();
        assert!(matches!(
            report.capacity_retention,
            HeadroomStatus::NotCreditable { .. }
        ));
    }

    #[test]
    fn window_status_is_recorded_per_evaluation_time() {
        let terms = terms();
        let ledger = ledger(DegradationChemistry::Lfp);
        let config = SohStalenessConfig::default();
        assert_eq!(
            compute_headroom(&terms, &ledger, -1, &config).unwrap().window,
            WindowStatus::NotYetEffective
        );
        assert_eq!(
            compute_headroom(&terms, &ledger, 10 * WARRANTY_YEAR_MILLIS, &config)
                .unwrap()
                .window,
            WindowStatus::Expired
        );
        assert_eq!(
            compute_headroom(&terms, &ledger, WARRANTY_YEAR_MILLIS, &config)
                .unwrap()
                .window,
            WindowStatus::Within
        );
    }

    #[test]
    fn rte_and_availability_are_declared_only_records() {
        let ledger = ledger(DegradationChemistry::Lfp);
        let report = compute_headroom(&terms(), &ledger, DAY, &SohStalenessConfig::default())
            .unwrap();
        let rte = report.rte_declared_only.as_ref().expect("rte declared");
        assert!((rte.min_round_trip_efficiency_fraction - 0.86).abs() < 1e-12);
        let availability = report
            .availability_declared_only
            .as_ref()
            .expect("availability declared");
        assert!((availability.min_availability_fraction - 0.97).abs() < 1e-12);
    }

    // --- augmentation events ---------------------------------------------------

    #[test]
    fn augmentation_events_validate() {
        assert!(matches!(
            AugmentationEvent::new(0, 0.0, "note".to_string(), Provenance::Configured),
            Err(WarrantyError::NonPositiveValue { field: "added_usable_energy_mwh", .. })
        ));
        assert!(matches!(
            AugmentationEvent::new(0, f64::NAN, "note".to_string(), Provenance::Configured),
            Err(WarrantyError::NonFiniteValue { field: "added_usable_energy_mwh", .. })
        ));
        assert!(matches!(
            AugmentationEvent::new(0, 5.0, " ".to_string(), Provenance::Configured),
            Err(WarrantyError::EmptyField { field: "note" })
        ));
        let event =
            AugmentationEvent::new(1_000, 2.5, "declared augmentation".to_string(), Provenance::Configured)
                .unwrap();
        assert_eq!(event.added_usable_energy_mwh, 2.5);
    }

    // --- reference-structure honesty ----------------------------------------------

    #[test]
    fn the_reference_structure_statement_carries_no_conformance_claim() {
        assert!(WARRANTY_REFERENCE_STRUCTURE_STATEMENT.contains("NO conformance"));
        assert!(WARRANTY_REFERENCE_STRUCTURE_STATEMENT.contains("UNVERIFIED"));
        assert!(WARRANTY_HEADROOM_STATEMENT.contains("never asserts a claim"));
    }
}
