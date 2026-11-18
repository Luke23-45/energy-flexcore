// P16.08 — Telemetry export pipelines (DESIGN): per-market export
//         postures, cadence budgets, and the adapter seam. RECORD +
//         DESIGN; no pipeline transmits.
//
// Phase 16 plan §2 row P16.08; sources: [S-90] (PJM FERC 2222
// participation model overview — the ≥10 MW aggregation telemetry
// posture), [S-100] (ERCOT ADER pilot — 2-second ICCP telemetry),
// [S-101] (ERCOT ADER Phase 3.3 report — net-consumer normalization and
// ramp-rate aggregation context), [S-104] (ISO-NE metering & telemetry
// checklist — the metering/telemetry posture class), [S-105] (NYISO
// Metering Fundamentals Dec 2024), [S-106] (NYISO DER 6-second telemetry
// MIWG deck).
//
// WHAT THIS IS (decided once):
//
// - **A DESIGN RECORD in code, not a transmitting pipeline.** Every type
//   here declares a posture, a budget, or a seam. No socket, no ICCP/TASE.2
//   stack, no SCADA link, no scheduler wiring exists anywhere — the
//   first live export is a human-gated lab process (conventions §7.3;
//   certification/conformance is NEVER claimed by this platform).
// - **The single-ingestion-path rule holds.** Export batches are
//   composed from the EXISTING Phase-2 telemetry pipeline records
//   (`flexcore_ports::telemetry_source::NormalizedTelemetry`) — no
//   parallel ingestion path, no second decoder, no side store. Export
//   reads what the platform already ingests; it never re-collects.
// - **Cadence budgets are DECLARED data, not measured claims** (the
//   declared-bounds discipline; X-8 pre-registration carryover): each
//   budget constant cites its source and carries the re-verify-at-use
//   rule. The observed export rate is a SEPARATE, structurally-distinct
//   measured value that only exists after a human-gated lab run.
// - **Jurisdiction-specific postures** (ADR-011): the three modeled
//   postures are per-market records with named sources. There is no
//   universal telemetry-export engine; a new market is a new posture
//   record, never a hard-coded branch.
// - **Aggregation math is the DECLARED FlexCore export posture**: the
//   net-consumer normalization and capacity-weighted ramp-rate
//   aggregation below are the formulas FlexCore DECLARES it would use to
//   prepare export batches. They are NOT a claim about how any ISO
//   computes its own aggregates (ISO-side rule details are UNVERIFIED;
//   re-verify against the market's manual at use).
//
// Control status: pure declarations and pure functions over constructed
// inputs for the simulation-only 0.x line. No pipeline transmits; no
// export has ever been sent; no market telemetry requirement is claimed
// as met.

use flexcore_contracts::units::ActivePowerMW;
use flexcore_ports::telemetry_source::NormalizedTelemetry;

/// The export-pipeline design schema version (a change is a NEW version).
pub const TELEMETRY_EXPORT_DESIGN_SCHEMA_VERSION: u32 = 1;

/// The no-transmission statement (pinned by tests).
pub const NO_PIPELINE_TRANSMITS_STATEMENT: &str = "This module is a DESIGN record: it \
     declares per-market export postures, cadence budgets, and an adapter seam. No export \
     pipeline transmits — no socket, no ICCP/TASE.2 stack, no SCADA link, and no \
     scheduler wiring exists anywhere in this program. The first live export is a \
     human-gated lab process; no conformance, qualification, or certification claim is \
     made by or for FlexCore.";

/// The single-ingestion-path statement (pinned by tests).
pub const EXPORT_READS_EXISTING_TELEMETRY_STATEMENT: &str = "Export batches are composed \
     from the EXISTING Phase-2 telemetry pipeline records (NormalizedTelemetry over the \
     TelemetrySource port). There is no parallel ingestion path: no second decoder, no \
     side collection, no separate subscription. Export reads what the platform already \
     ingests.";

/// The declared-not-measured statement for budgets (pinned by tests).
pub const BUDGETS_ARE_DECLARED_STATEMENT: &str = "Every cadence budget is DECLARED data \
     citing its source, subject to the re-verify-at-use rule. No budget is a measured \
     or met claim: observed export rates exist only as separate, structurally-distinct \
     values recorded after a human-gated lab run (X-8 pre-registration carryover).";

/// The market the posture serves (closed; per-jurisdiction by design —
/// ADR-011). A new market is a NEW posture record, never a code branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportMarket {
    /// ERCOT ADER pilot posture: 2-second ICCP telemetry [S-100][S-101].
    ErcotAder,
    /// NYISO ESR posture: 6-second SCADA telemetry including SOC
    /// [S-105][S-106].
    NyisoEsr,
    /// PJM aggregation posture: telemetry for aggregations of
    /// ≥10 MW [S-90].
    PjmAggregation,
}

impl ExportMarket {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            ExportMarket::ErcotAder => "ercot-ader",
            ExportMarket::NyisoEsr => "nyiso-esr",
            ExportMarket::PjmAggregation => "pjm-aggregation",
        }
    }
}

impl std::fmt::Display for ExportMarket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// --- Declared cadence budgets (cited, re-verify at use) ----------------------

/// ERCOT ADER: the 2-second ICCP telemetry budget, in milliseconds
/// ([S-100][S-101]; "4-second"-class references are superseded — the
/// 2-second ICCP is the confirmed figure in the gap analysis §11 track E;
/// re-verify against the ADER pilot documents at use).
pub const ERCOT_ICCP_BUDGET_MILLIS: i64 = 2_000;

/// NYISO ESR: the 6-second SCADA telemetry budget, in milliseconds
/// ([S-105][S-106]; re-verify at use).
pub const NYISO_SCADA_BUDGET_MILLIS: i64 = 6_000;

/// PJM: the ≥10 MW aggregation threshold (MW) for the telemetry posture
/// ([S-90]; re-verify at use). Power is a physical quantity — the float
/// is the units-crate convention and is NOT money (the no-float ban is a
/// money rule).
pub const PJM_MIN_AGGREGATION_MW: f64 = 10.0;

/// The quantities a market's posture requires (closed per posture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportQuantity {
    /// Active power at the aggregation point (MW).
    ActivePower,
    /// State of charge as a fraction in [0, 1] (NYISO ESR postures carry
    /// SOC with SCADA telemetry [S-105][S-106]; the canonical-unit rule —
    /// SOC is a fraction at the API boundary — holds in export).
    Soc,
}

impl ExportQuantity {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ExportQuantity::ActivePower => "active_power",
            ExportQuantity::Soc => "soc",
        }
    }
}

/// One per-market telemetry export posture RECORD (a declared design, not
/// a live configuration). Constructed fail-closed: a budget must be
/// positive, the quantity set non-empty, and the transport reference and
/// source non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPostureRecord {
    /// The market this posture serves.
    pub market: ExportMarket,
    /// The declared export cadence budget in milliseconds (one of the
    /// cited budget constants; the caller binds it explicitly so the
    /// budget's source is always visible at the construction site).
    pub cadence_budget_millis: i64,
    /// The quantities the posture requires, in declared order.
    pub quantities: Vec<ExportQuantity>,
    /// The transport seam reference (e.g. the ICCP/TASE.2 class for ERCOT
    /// [S-100]; the SCADA/MIWG class for NYISO [S-106]) — a REFERENCE
    /// string, never a connection. Wire details UNVERIFIED until the
    /// human-gated adapter lands.
    pub transport_ref: String,
    /// The recorded source of the posture (the [S-n] citation chain).
    pub source: String,
}

/// Why a posture or batch was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportDesignError {
    /// The cadence budget was not positive.
    NonPositiveBudget { millis: i64 },
    /// The posture required no quantities (a posture that exports nothing
    /// is not recordable).
    EmptyQuantitySet,
    /// The transport reference or posture source was empty.
    EmptyReference { field: &'static str },
    /// The batch was empty — an empty export batch is a refusal, never a
    /// silent no-op (fail-closed, the P4.30 closing-rule discipline
    /// mirrored).
    EmptyBatch,
    /// A component value was non-finite.
    NonFiniteValue { field: &'static str },
    /// The aggregation weights summed to zero (a weighted aggregate over
    /// zero weight is undefined — refused, never defaulted).
    ZeroTotalWeight,
}

impl ExportDesignError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ExportDesignError::NonPositiveBudget { .. } => "EXPORT_DESIGN_NON_POSITIVE_BUDGET",
            ExportDesignError::EmptyQuantitySet => "EXPORT_DESIGN_EMPTY_QUANTITY_SET",
            ExportDesignError::EmptyReference { .. } => "EXPORT_DESIGN_EMPTY_REFERENCE",
            ExportDesignError::EmptyBatch => "EXPORT_DESIGN_EMPTY_BATCH",
            ExportDesignError::NonFiniteValue { .. } => "EXPORT_DESIGN_NON_FINITE_VALUE",
            ExportDesignError::ZeroTotalWeight => "EXPORT_DESIGN_ZERO_TOTAL_WEIGHT",
        }
    }
}

impl std::fmt::Display for ExportDesignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportDesignError::NonPositiveBudget { millis } => {
                write!(f, "the cadence budget must be positive; got {millis} ms")
            }
            ExportDesignError::EmptyQuantitySet => {
                write!(f, "the posture must require at least one quantity")
            }
            ExportDesignError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            ExportDesignError::EmptyBatch => {
                write!(f, "an export batch is non-empty or refused — never a silent no-op")
            }
            ExportDesignError::NonFiniteValue { field } => {
                write!(f, "the '{field}' value is not finite")
            }
            ExportDesignError::ZeroTotalWeight => write!(
                f,
                "the aggregation weights sum to zero; a weighted aggregate over zero \
                 weight is undefined and is refused, never defaulted"
            ),
        }
    }
}

impl std::error::Error for ExportDesignError {}

impl ExportPostureRecord {
    /// Validates and constructs one posture record (fail-closed).
    pub fn new(
        market: ExportMarket,
        cadence_budget_millis: i64,
        quantities: Vec<ExportQuantity>,
        transport_ref: String,
        source: String,
    ) -> Result<Self, ExportDesignError> {
        if cadence_budget_millis <= 0 {
            return Err(ExportDesignError::NonPositiveBudget {
                millis: cadence_budget_millis,
            });
        }
        if quantities.is_empty() {
            return Err(ExportDesignError::EmptyQuantitySet);
        }
        if transport_ref.trim().is_empty() {
            return Err(ExportDesignError::EmptyReference {
                field: "transport_ref",
            });
        }
        if source.trim().is_empty() {
            return Err(ExportDesignError::EmptyReference { field: "source" });
        }
        Ok(Self {
            market,
            cadence_budget_millis,
            quantities,
            transport_ref,
            source,
        })
    }
}

/// The declared sign convention of an export view (the reactive-sign /
/// declared-convention pattern applied to the export boundary). The
/// canonical platform convention is P > 0 = EXPORT at the point of
/// connection (AGENTS.md §4); some market telemetry views are defined
/// consumption-positive — the convention is DECLARED per posture, never
/// assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportSignConvention {
    /// P > 0 means export at the point of connection (the platform
    /// canonical convention).
    ExportPositive,
    /// P > 0 means consumption (net load) — the net-consumer view.
    ConsumptionPositive,
}

impl ExportSignConvention {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ExportSignConvention::ExportPositive => "export-positive",
            ExportSignConvention::ConsumptionPositive => "consumption-positive",
        }
    }
}

/// Net-consumer normalization (the DECLARED export posture [S-101]):
/// aggregates per-asset signed active power (canonical convention: P > 0
/// = export at each asset's point of connection) into the aggregation's
/// net value, then renders it in the DECLARED export convention. An
/// export-positive request returns the sum unchanged; a
/// consumption-positive request returns the negated sum (net load). The
/// batch must be non-empty and every value finite (fail-closed).
pub fn net_consumer_normalization(
    records: &[NormalizedTelemetry],
    convention: ExportSignConvention,
) -> Result<ActivePowerMW, ExportDesignError> {
    if records.is_empty() {
        return Err(ExportDesignError::EmptyBatch);
    }
    let mut sum = 0.0_f64;
    for record in records {
        let Some(value) = record.value() else {
            // MISSING gap markers carry no value and are skipped; the
            // MISSING state is the pipeline's own supervision.
            continue;
        };
        if let flexcore_ports::telemetry_source::QuantityValue::Power(p) = value {
            if !p.value().is_finite() {
                return Err(ExportDesignError::NonFiniteValue {
                    field: "active_power",
                });
            }
            sum += p.value();
        }
        // Non-power quantities are skipped: the net-consumer lens is a
        // power lens, and the caller declares per-quantity lenses.
    }
    let signed = match convention {
        ExportSignConvention::ExportPositive => sum,
        ExportSignConvention::ConsumptionPositive => -sum,
    };
    ActivePowerMW::new(signed)
        .map_err(|_| ExportDesignError::NonFiniteValue {
            field: "normalized_net_power",
        })
}

/// One per-asset ramp-rate component with its aggregation weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampRateComponent {
    /// The asset's ramp rate (MW/s) in the platform convention.
    pub rate_mw_per_s: f64,
    /// The aggregation weight (declared — capacity-weighted posture:
    /// the asset's rated MW). Must be finite and positive to contribute.
    pub weight_mw: f64,
}

/// Capacity-weighted ramp-rate aggregation (the DECLARED export posture
/// [S-101]): the weighted mean of the per-asset ramp rates with the
/// declared weights. This is the formula FlexCore declares it would use
/// to PREPARE an export value — NOT a claim about how any ISO computes
/// its own aggregate (ISO-side rules are UNVERIFIED; re-verify at use).
/// Zero total weight is a named refusal; non-finite components are named
/// refusals; nothing is silently dropped or defaulted.
pub fn weighted_ramp_rate_aggregation(
    components: &[RampRateComponent],
) -> Result<f64, ExportDesignError> {
    if components.is_empty() {
        return Err(ExportDesignError::EmptyBatch);
    }
    let mut weight_sum = 0.0_f64;
    let mut weighted = 0.0_f64;
    for component in components {
        if !component.rate_mw_per_s.is_finite() {
            return Err(ExportDesignError::NonFiniteValue {
                field: "rate_mw_per_s",
            });
        }
        if !component.weight_mw.is_finite() {
            return Err(ExportDesignError::NonFiniteValue { field: "weight_mw" });
        }
        weight_sum += component.weight_mw;
        weighted += component.rate_mw_per_s * component.weight_mw;
    }
    if weight_sum == 0.0 {
        return Err(ExportDesignError::ZeroTotalWeight);
    }
    Ok(weighted / weight_sum)
}

/// The telemetry export adapter SEAM (declared, UNWIRED — see
/// [`NO_PIPELINE_TRANSMITS_STATEMENT`]). The concrete per-market
/// transport (ICCP/TASE.2 class for ERCOT [S-100], the SCADA class for
/// NYISO [S-106]) implements this trait at the composition root after a
/// jurisdiction decision (D1, OPEN; ADR-011) and the human-gated lab
/// process. No implementation ships in this crate beyond the named
/// unwired refusal; sync trait (the crate rule).
pub trait TelemetryExportSink {
    /// The market the sink serves (one sink per posture — a sink that
    /// served two markets would blur two jurisdictions' audit trails).
    fn market(&self) -> ExportMarket;

    /// Exports one prepared batch. Every call on an UNWIRED sink answers
    /// the named `SeamNotWired` refusal — nothing is ever sent.
    fn export_batch(
        &self,
        posture: &ExportPostureRecord,
        records: &[NormalizedTelemetry],
    ) -> Result<(), ExportSeamError>;
}

/// The unwired-seam error (the protocol-entsoe `SeamNotWired` posture
/// mirrored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportSeamError {
    /// The sink is unwired: no transport exists in this program.
    SeamNotWired { market: ExportMarket },
    /// The posture did not match the sink's market (a mismatched pair is
    /// a named refusal, never a silent re-target).
    PostureMarketMismatch { sink: ExportMarket, posture: ExportMarket },
}

impl ExportSeamError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ExportSeamError::SeamNotWired { .. } => "EXPORT_SEAM_NOT_WIRED",
            ExportSeamError::PostureMarketMismatch { .. } => "EXPORT_SEAM_POSTURE_MISMATCH",
        }
    }
}

impl std::fmt::Display for ExportSeamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportSeamError::SeamNotWired { market } => write!(
                f,
                "the {market} telemetry export seam is NOT WIRED: no transport exists in \
                 this program and nothing is ever sent (the first live export is a \
                 human-gated lab process)"
            ),
            ExportSeamError::PostureMarketMismatch { sink, posture } => write!(
                f,
                "the posture serves {posture} but the sink serves {sink}; a mismatched \
                 pair is refused, never silently re-targeted"
            ),
        }
    }
}

impl std::error::Error for ExportSeamError {}

/// The UNWIRED export sink: answers every export call with the named
/// [`ExportSeamError::SeamNotWired`] refusal. This is the composition
/// root's default binding until a human-gated adapter lands — the
/// refusal IS the honesty guarantee (nothing can silently transmit).
#[derive(Debug, Clone, Copy)]
pub struct UnwiredTelemetryExportSink {
    market: ExportMarket,
}

impl UnwiredTelemetryExportSink {
    /// Binds the unwired sink to one market.
    pub fn new(market: ExportMarket) -> Self {
        Self { market }
    }
}

impl TelemetryExportSink for UnwiredTelemetryExportSink {
    fn market(&self) -> ExportMarket {
        self.market
    }

    fn export_batch(
        &self,
        posture: &ExportPostureRecord,
        _records: &[NormalizedTelemetry],
    ) -> Result<(), ExportSeamError> {
        if posture.market != self.market {
            return Err(ExportSeamError::PostureMarketMismatch {
                sink: self.market,
                posture: posture.market,
            });
        }
        Err(ExportSeamError::SeamNotWired { market: self.market })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the declared formulas over CONSTRUCTED inputs.
// Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
    use flexcore_domain::ids::{AssetId, TenantId};
    use flexcore_ports::telemetry_source::{NormalizedTelemetryParts, QuantityValue};

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("valid test time")
    }

    fn it(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).expect("valid test ingest time")
    }

    fn power_record(tag: u128, millis: i64, mw: f64) -> NormalizedTelemetry {
        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: TenantId::new(uid(0xA100)),
            asset_id: AssetId::new(uid(tag)),
            quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(
                ActivePowerMW::new(mw).expect("finite test power"),
            )),
            event_time: t(millis),
            ingest_time: it(millis),
            source_sequence: 1,
            source_id: "sim://test".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid constructed record")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(NO_PIPELINE_TRANSMITS_STATEMENT.contains("No export pipeline transmits"));
        assert!(NO_PIPELINE_TRANSMITS_STATEMENT.contains("human-gated lab process"));
        assert!(EXPORT_READS_EXISTING_TELEMETRY_STATEMENT.contains("no parallel ingestion path"));
        assert!(BUDGETS_ARE_DECLARED_STATEMENT.contains("re-verify-at-use"));
    }

    #[test]
    fn declared_budgets_match_the_cited_figures() {
        // ERCOT 2-second ICCP [S-100][S-101]; NYISO 6-second SCADA
        // [S-105][S-106]; PJM ≥10 MW [S-90]. Re-verify at use.
        assert_eq!(ERCOT_ICCP_BUDGET_MILLIS, 2_000);
        assert_eq!(NYISO_SCADA_BUDGET_MILLIS, 6_000);
        assert_eq!(PJM_MIN_AGGREGATION_MW, 10.0);
    }

    #[test]
    fn postures_construct_fail_closed() {
        let posture = ExportPostureRecord::new(
            ExportMarket::ErcotAder,
            ERCOT_ICCP_BUDGET_MILLIS,
            vec![ExportQuantity::ActivePower],
            "iccp-tase2://fixture-class".to_string(),
            "[S-100][S-101]".to_string(),
        )
        .expect("valid posture");
        assert_eq!(posture.market.as_str(), "ercot-ader");
        assert_eq!(posture.cadence_budget_millis, 2_000);

        let err = ExportPostureRecord::new(
            ExportMarket::NyisoEsr,
            0,
            vec![ExportQuantity::ActivePower, ExportQuantity::Soc],
            "scada://fixture-class".to_string(),
            "[S-105][S-106]".to_string(),
        )
        .expect_err("zero budget refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_NON_POSITIVE_BUDGET");
        let err = ExportPostureRecord::new(
            ExportMarket::PjmAggregation,
            60_000,
            Vec::new(),
            "scada://fixture-class".to_string(),
            "[S-90]".to_string(),
        )
        .expect_err("empty quantity set refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_EMPTY_QUANTITY_SET");
        let err = ExportPostureRecord::new(
            ExportMarket::PjmAggregation,
            60_000,
            vec![ExportQuantity::ActivePower],
            "  ".to_string(),
            "[S-90]".to_string(),
        )
        .expect_err("empty transport ref refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_EMPTY_REFERENCE");
    }

    #[test]
    fn nyiso_posture_carries_soc_with_power() {
        // The NYISO ESR posture requires SOC alongside SCADA power
        // [S-105][S-106]; the SOC export stays a fraction in [0, 1].
        let posture = ExportPostureRecord::new(
            ExportMarket::NyisoEsr,
            NYISO_SCADA_BUDGET_MILLIS,
            vec![ExportQuantity::ActivePower, ExportQuantity::Soc],
            "scada://fixture-class".to_string(),
            "[S-105][S-106]".to_string(),
        )
        .expect("valid posture");
        assert!(posture.quantities.contains(&ExportQuantity::Soc));
    }

    #[test]
    fn net_consumer_normalization_is_convention_declared() {
        // Two assets: +2 MW export and +1 MW export (canonical P>0
        // export) -> net +3 MW export-positive.
        let records = vec![
            power_record(0xC001, 0, 2.0),
            power_record(0xC002, 0, 1.0),
        ];
        let net = net_consumer_normalization(&records, ExportSignConvention::ExportPositive)
            .expect("normalized");
        assert_eq!(net.value(), 3.0);
        // The net-consumer view (consumption-positive) negates the sum.
        let net = net_consumer_normalization(&records, ExportSignConvention::ConsumptionPositive)
            .expect("normalized");
        assert_eq!(net.value(), -3.0);
        // A net-consuming aggregation (−4 MW export = 4 MW import) reads
        // as +4 consumption in the net-consumer view [S-101].
        let records = vec![
            power_record(0xC001, 0, -3.5),
            power_record(0xC002, 0, -0.5),
        ];
        let net = net_consumer_normalization(&records, ExportSignConvention::ConsumptionPositive)
            .expect("normalized");
        assert_eq!(net.value(), 4.0);
    }

    #[test]
    fn net_consumer_normalization_refuses_empty_and_skips_missing() {
        let err = net_consumer_normalization(&[], ExportSignConvention::ExportPositive)
            .expect_err("empty batch refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_EMPTY_BATCH");
        // A MISSING gap marker (value None) is skipped, not zero-filled.
        let missing = NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: TenantId::new(uid(0xA100)),
            asset_id: AssetId::new(uid(0xC003)),
            quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
            value: None,
            event_time: t(0),
            ingest_time: it(0),
            source_sequence: 1,
            source_id: "flexcore:derived:test".to_string(),
            quality: TelemetryQuality::Missing,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid gap marker");
        let records = vec![missing, power_record(0xC001, 0, 2.0)];
        let net = net_consumer_normalization(&records, ExportSignConvention::ExportPositive)
            .expect("missing marker skipped");
        assert_eq!(net.value(), 2.0);
    }

    #[test]
    fn weighted_ramp_rate_aggregation_matches_hand_computation() {
        // Weights 100 MW and 50 MW; rates 1.0 and 2.0 MW/s:
        // (100*1.0 + 50*2.0) / 150 = 200/150 = 1.3333... MW/s.
        let components = vec![
            RampRateComponent {
                rate_mw_per_s: 1.0,
                weight_mw: 100.0,
            },
            RampRateComponent {
                rate_mw_per_s: 2.0,
                weight_mw: 50.0,
            },
        ];
        let aggregate = weighted_ramp_rate_aggregation(&components).expect("aggregate");
        let expected = 200.0 / 150.0;
        assert!((aggregate - expected).abs() < 1e-12);
        // Zero total weight is refused, never defaulted.
        let zero = vec![RampRateComponent {
            rate_mw_per_s: 1.0,
            weight_mw: 0.0,
        }];
        let err = weighted_ramp_rate_aggregation(&zero).expect_err("zero weight refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_ZERO_TOTAL_WEIGHT");
        // Non-finite components are named refusals.
        let bad = vec![RampRateComponent {
            rate_mw_per_s: f64::NAN,
            weight_mw: 1.0,
        }];
        let err = weighted_ramp_rate_aggregation(&bad).expect_err("NaN refused");
        assert_eq!(err.name(), "EXPORT_DESIGN_NON_FINITE_VALUE");
    }

    #[test]
    fn the_unwired_sink_never_transmits_and_refuses_mismatches() {
        let sink = UnwiredTelemetryExportSink::new(ExportMarket::ErcotAder);
        let posture = ExportPostureRecord::new(
            ExportMarket::ErcotAder,
            ERCOT_ICCP_BUDGET_MILLIS,
            vec![ExportQuantity::ActivePower],
            "iccp-tase2://fixture-class".to_string(),
            "[S-100][S-101]".to_string(),
        )
        .expect("valid posture");
        let err = sink
            .export_batch(&posture, &[power_record(0xC001, 0, 1.0)])
            .expect_err("unwired seam refused");
        assert_eq!(err.name(), "EXPORT_SEAM_NOT_WIRED");
        // A mismatched posture is refused before the seam answer.
        let wrong = ExportPostureRecord::new(
            ExportMarket::NyisoEsr,
            NYISO_SCADA_BUDGET_MILLIS,
            vec![ExportQuantity::ActivePower],
            "scada://fixture-class".to_string(),
            "[S-105][S-106]".to_string(),
        )
        .expect("valid posture");
        let err = sink
            .export_batch(&wrong, &[power_record(0xC001, 0, 1.0)])
            .expect_err("market mismatch refused");
        assert_eq!(err.name(), "EXPORT_SEAM_POSTURE_MISMATCH");
    }

    #[test]
    fn market_and_convention_labels_are_stable() {
        assert_eq!(ExportMarket::ErcotAder.as_str(), "ercot-ader");
        assert_eq!(ExportMarket::NyisoEsr.as_str(), "nyiso-esr");
        assert_eq!(ExportMarket::PjmAggregation.as_str(), "pjm-aggregation");
        assert_eq!(ExportSignConvention::ExportPositive.as_str(), "export-positive");
        assert_eq!(
            ExportSignConvention::ConsumptionPositive.as_str(),
            "consumption-positive"
        );
        assert_eq!(ExportQuantity::ActivePower.as_str(), "active_power");
        assert_eq!(ExportQuantity::Soc.as_str(), "soc");
    }
}
