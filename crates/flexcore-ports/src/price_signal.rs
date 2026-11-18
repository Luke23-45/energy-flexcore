// P15.04 — The price-signal ingestion PORT (the day-ahead price seam).
//
// Phase 15 plan §2 row P15.04; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 04_p15_04_price_ingestion_entsoe.md`.
//
// WHAT THIS IS: the capability interface through which market PRICE
// signals (day-ahead and intra-day price series) enter FlexCore as
// INGESTED DATA. The concrete source — the ENTSO-E Transparency Platform
// REST adapter (A44 day-ahead price documents, [S-111]) and any
// vendor-forecast seam (P15-E-2, registered OPEN; [S-112]) — lives in an
// adapter; this port is source-neutral BY CONSTRUCTION (the port crate
// forbids vendor, product, protocol, and wire-schema names in source
// text — the lib.rs grep gate). The adapter implements THIS port; the
// port never names the source.
//
// RULES THIS PORT ENCODES:
//
// 1. **Prices are DATA, never bid inputs.** Phase 16 owns any market
//    participation; this port has no bid, offer, or order vocabulary
//    anywhere in its surface (structural, not conventional — the same
//    posture as the `dispatch`-absence rule). A price series is an input
//    to later-phase analytics; nothing in this port can place, shape, or
//    influence a market action, and nothing here can influence a dispatch
//    command (ADR-010 extension, conventions §6.3).
// 2. **A price is not a forecast claim.** An ingested price series
//    (historical/actual) and a vendor price FORECAST series are distinct
//    kinds ([`PriceSeriesKind`]); a forecast series carries its explicit
//    no-skill posture — the platform never re-states a vendor's accuracy
//    claim (honesty rules §7.2; UNVERIFIED register discipline).
// 3. **Currency is declared per series, never assumed** (the reactive-
//    sign / declared-convention pattern): a price point carries the
//    currency code and the per-energy unit ("currency per MWh", the
//    canonical MWh base); a currency-less price is unconstructable.
// 4. **Provenance travels with every series** (the linked-provenance
//    rule, P8.32 pattern): source class, source reference, and the
//    document reference travel IN the series record.
// 5. **Tenant scoping is structural** (P0.16 rules 1 and 5): every port
//    method takes the domain `TenantContext`.
// 6. **Sync traits** (the crate rule): no futures, no async runtime.
//
// WHAT IS A RECORDED SEAM (NOT implemented): no adapter ships here; the
// API token for the concrete source is a HUMAN GATE input (UNSIGNED until
// a human registers it — conventions §5); the market-jurisdiction
// register entry is a human-gate program boundary. Nothing here bids,
// settles, or claims market qualification.

use flexcore_contracts::time_model::EventTime;
use flexcore_domain::tenant_context::TenantContext;

/// The kind of an ingested price series. Historical/actual and forecast
/// series are NEVER conflated: a consumer must name the kind it wants,
/// and a forecast series is delivered with its explicit no-skill posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceSeriesKind {
    /// Actual/historical prices (e.g. a day-ahead auction's cleared
    /// prices, published by the transparency source [S-111]).
    Actual,
    /// A vendor price FORECAST series (P15-E-2 seam; ingested as DATA
    /// with provenance; no accuracy claim is carried or re-stated by
    /// FlexCore [S-112]).
    VendorForecast,
}

impl PriceSeriesKind {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            PriceSeriesKind::Actual => "actual",
            PriceSeriesKind::VendorForecast => "vendor-forecast",
        }
    }
}

/// The closed source-class vocabulary of an ingested price series (the
/// provenance class travels with the series — never stored apart).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceSourceClass {
    /// The European transparency platform's published day-ahead prices
    /// (the [S-111] adapter is the concrete source; the port carries only
    /// the class).
    TransparencyPlatform,
    /// A commercial vendor's price-forecast feed (P15-E-2; named only in
    /// deployment configuration, never in this port).
    CommercialVendor,
}

impl PriceSourceClass {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            PriceSourceClass::TransparencyPlatform => "transparency-platform",
            PriceSourceClass::CommercialVendor => "commercial-vendor",
        }
    }
}

/// One price point of an ingested series: the price for one delivery
/// interval, in the series' declared currency per MWh. Result-constructed
/// (no partial records).
#[derive(Debug, Clone, PartialEq)]
pub struct PricePoint {
    /// Start of the delivery interval (UTC, start-inclusive).
    pub interval_start: EventTime,
    /// End of the delivery interval (UTC, end-exclusive — the P3.03
    /// interval semantics applied to market intervals).
    pub interval_end: EventTime,
    /// The price for the interval, in `currency_code` per MWh. Validated
    /// finite at construction; sign is NOT constrained (negative prices
    /// are real market outcomes in several jurisdictions — UNVERIFIED for
    /// any specific market; the value is carried as published).
    pub price_per_mwh: f64,
}

/// Why a price point was refused. Named, matchable, never degraded into
/// a default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PricePointError {
    /// The price was NaN or infinite.
    NonFinitePrice,
    /// The interval is inverted or empty (start must be before end).
    InvertedInterval,
}

impl PricePointError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PricePointError::NonFinitePrice => "PRICE_POINT_NON_FINITE",
            PricePointError::InvertedInterval => "PRICE_POINT_INVERTED_INTERVAL",
        }
    }
}

impl std::fmt::Display for PricePointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PricePointError::NonFinitePrice => {
                write!(f, "price must be finite (no NaN or infinity)")
            }
            PricePointError::InvertedInterval => write!(
                f,
                "the delivery interval is inverted or empty (start must be before end)"
            ),
        }
    }
}

impl std::error::Error for PricePointError {}

impl PricePoint {
    /// Validates and constructs one price point (fail-closed).
    pub fn new(
        interval_start: EventTime,
        interval_end: EventTime,
        price_per_mwh: f64,
    ) -> Result<Self, PricePointError> {
        if !price_per_mwh.is_finite() {
            return Err(PricePointError::NonFinitePrice);
        }
        if interval_start.unix_millis() >= interval_end.unix_millis() {
            return Err(PricePointError::InvertedInterval);
        }
        Ok(Self {
            interval_start,
            interval_end,
            price_per_mwh,
        })
    }
}

/// One ingested price series with its declared currency and full
/// provenance. The currency is DECLARED per series (no default currency
/// exists anywhere in the platform); the per-energy base is MWh (§10.2
/// canonical energy unit) — a document quoted per kWh is converted at the
/// adapter boundary, never here.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceSeries {
    /// The kind (actual vs vendor forecast — never conflated).
    pub kind: PriceSeriesKind,
    /// The source class (closed vocabulary).
    pub source_class: PriceSourceClass,
    /// The source REFERENCE (provider dataset/query identity; never
    /// credentials — the P7.05 secret-reference rule).
    pub source_ref: String,
    /// The source document reference (e.g. the published document /
    /// auction identity the series was extracted from; a reference, never
    /// the document payload).
    pub document_ref: String,
    /// The delivery area / bidding-zone identifier as published by the
    /// source (carried verbatim; the zone vocabulary is the source's, not
    /// the platform's — UNVERIFIED until the adapter's zone map is
    /// lab-checked against [S-111]).
    pub area: String,
    /// The DECLARED currency code (ISO 4217-style as published; UNVERIFIED
    /// per source until the adapter documents the actual code set).
    pub currency_code: String,
    /// The price points, in interval-start order (the consumer sorts
    /// before delivery; the port validates ordering at construction).
    pub points: Vec<PricePoint>,
}

/// Why a price series was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriceSeriesError {
    /// The source or document reference was empty or whitespace.
    EmptyReference { field: &'static str },
    /// The area or currency was empty or whitespace.
    EmptyAreaOrCurrency,
    /// The points list was empty (an ingested series is a series, not a
    /// null).
    NoPoints,
    /// The points were not in ascending interval-start order (the port
    /// refuses unsorted input rather than silently sorting — the caller's
    /// ordering intent is part of the record).
    UnsortedPoints { index: usize },
    /// A point failed its own validation (the point error's name rides).
    PointInvalid { index: usize, reason: String },
}

impl PriceSeriesError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PriceSeriesError::EmptyReference { .. } => "PRICE_SERIES_EMPTY_REFERENCE",
            PriceSeriesError::EmptyAreaOrCurrency => "PRICE_SERIES_EMPTY_AREA_OR_CURRENCY",
            PriceSeriesError::NoPoints => "PRICE_SERIES_NO_POINTS",
            PriceSeriesError::UnsortedPoints { .. } => "PRICE_SERIES_UNSORTED_POINTS",
            PriceSeriesError::PointInvalid { .. } => "PRICE_SERIES_POINT_INVALID",
        }
    }
}

impl std::fmt::Display for PriceSeriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriceSeriesError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            PriceSeriesError::EmptyAreaOrCurrency => {
                write!(f, "the delivery area and currency code must be non-empty")
            }
            PriceSeriesError::NoPoints => {
                write!(f, "an ingested price series carries at least one point")
            }
            PriceSeriesError::UnsortedPoints { index } => write!(
                f,
                "the points are not in ascending interval-start order (first violation at \
                 index {index}); the port refuses unsorted input, never silently sorts"
            ),
            PriceSeriesError::PointInvalid { index, reason } => {
                write!(f, "price point {index} is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for PriceSeriesError {}

impl PriceSeries {
    /// Validates and constructs one series (fail-closed: empty refs, empty
    /// area/currency, empty point list, unsorted points, and invalid
    /// points are each named refusals — nothing is partially accepted).
    pub fn new(
        kind: PriceSeriesKind,
        source_class: PriceSourceClass,
        source_ref: String,
        document_ref: String,
        area: String,
        currency_code: String,
        points: Vec<PricePoint>,
    ) -> Result<Self, PriceSeriesError> {
        if source_ref.trim().is_empty() {
            return Err(PriceSeriesError::EmptyReference {
                field: "source_ref",
            });
        }
        if document_ref.trim().is_empty() {
            return Err(PriceSeriesError::EmptyReference {
                field: "document_ref",
            });
        }
        if area.trim().is_empty() || currency_code.trim().is_empty() {
            return Err(PriceSeriesError::EmptyAreaOrCurrency);
        }
        if points.is_empty() {
            return Err(PriceSeriesError::NoPoints);
        }
        for (index, pair) in points.windows(2).enumerate() {
            if pair[0].interval_start.unix_millis() > pair[1].interval_start.unix_millis() {
                return Err(PriceSeriesError::UnsortedPoints { index: index + 1 });
            }
        }
        Ok(Self {
            kind,
            source_class,
            source_ref,
            document_ref,
            area,
            currency_code,
            points,
        })
    }
}

/// The price-signal ingestion port (P15.04). READ-ONLY over ingested
/// price data: there is no write path, no bid/offer/order vocabulary, and
/// no dispatch vocabulary anywhere in this trait's surface — prices are
/// INGESTED DATA for later phases (Phase 16 consumes them; this phase
/// produces only the ingestion seam). No ADAPTER implementation ships in
/// this crate — the only shipped implementation is the fail-closed
/// [`UnwiredPriceSignalSource`] default (FRV1-R1-003 recorded deferral),
/// which refuses every operation by name — and no concrete source
/// (platform or vendor) is named here.
///
/// Control status: an INPUT seam (ADR-010 extension, conventions §6.3).
/// Nothing here authorizes, issues, dispatches, or bids; an ingested
/// price is never a command, a grid measurement, or a FlexCore accuracy
/// claim.
pub trait PriceSignalSource {
    /// The latest ingested series of the requested kind for one area,
    /// tenant-scoped. An absent series is an empty result — never a
    /// fabricated series and never a default price.
    fn latest_series(
        &self,
        ctx: &TenantContext,
        kind: PriceSeriesKind,
        area: &str,
    ) -> Result<Option<PriceSeries>, PricePortError>;

    /// Ingests one validated series (the adapter's delivery path). The
    /// series arrives ALREADY validated ([`PriceSeries::new`]); the port
    /// refuses it whole on any store-side failure — never partially.
    fn ingest_series(&self, ctx: &TenantContext, series: PriceSeries)
        -> Result<(), PricePortError>;
}

/// The price-port error surface (the `BackendError` posture: opaque code
/// + message at the adapter boundary, never raw vendor text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PricePortError {
    /// The port's backing adapter is unavailable (fail-closed: a missing
    /// price source is never answered with a fabricated series).
    SourceUnavailable { code: String, message: String },
    /// The area is unknown to the configured source (the zone vocabulary
    /// is the source's; an unknown area is a named refusal, never a
    /// silent empty).
    UnknownArea { area: String },
}

impl PricePortError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PricePortError::SourceUnavailable { .. } => "PRICE_SOURCE_UNAVAILABLE",
            PricePortError::UnknownArea { .. } => "PRICE_SOURCE_UNKNOWN_AREA",
        }
    }
}

impl std::fmt::Display for PricePortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PricePortError::SourceUnavailable { code, message } => write!(
                f,
                "price source unavailable ({code}): {message} — a missing source is never \
                 answered with a fabricated series (fail-closed)"
            ),
            PricePortError::UnknownArea { area } => write!(
                f,
                "the delivery area '{area}' is unknown to the configured price source; \
                 unknown areas are named refusals, never silent empties"
            ),
        }
    }
}

impl std::error::Error for PricePortError {}

// --- The unwired shipped default ---------------------------------------------

/// The name the unwired default reports in every refusal code (surfaced in
/// the [`PricePortError::SourceUnavailable`] code so a log or metric
/// labels the unwired seam by name).
pub const PRICE_SOURCE_SEAM_NOT_WIRED: &str = "PRICE_SOURCE_SEAM_NOT_WIRED";

/// The shipped default [`PriceSignalSource`] implementation: refuses every
/// operation with the named `PRICE_SOURCE_SEAM_NOT_WIRED` refusal
/// (FRV1-R1-003 recorded deferral; the fail-closed `Unwired*` seam idiom
/// this workspace already uses at the protocol-port layer, lifted to this
/// port). No adapter implements the price port anywhere in the 0.x
/// baseline — no price source is configured, no delivery contract exists,
/// and no concrete source integration is invented here (a price source is
/// named only in deployment configuration, never in this port). A wired
/// source can only arrive as a deliberate, composed adapter
/// implementation — never as a silent default.
///
/// Fail-closed by construction: both operations answer
/// [`PricePortError::SourceUnavailable`] (a missing price source is never
/// answered with a fabricated series), so a composition that wires this
/// default by mistake refuses loudly instead of feeding invented prices
/// into the later-phase consumers.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnwiredPriceSignalSource;

impl PriceSignalSource for UnwiredPriceSignalSource {
    fn latest_series(
        &self,
        _ctx: &TenantContext,
        _kind: PriceSeriesKind,
        _area: &str,
    ) -> Result<Option<PriceSeries>, PricePortError> {
        Err(PricePortError::SourceUnavailable {
            code: PRICE_SOURCE_SEAM_NOT_WIRED.to_string(),
            message: "no adapter implements the PriceSignalSource port in this \
                      baseline (FRV1-R1-003 recorded deferral; no price source \
                      is configured in 0.x) — ingested price data lands with the \
                      first adapter composition"
                .to_string(),
        })
    }

    fn ingest_series(
        &self,
        _ctx: &TenantContext,
        _series: PriceSeries,
    ) -> Result<(), PricePortError> {
        Err(PricePortError::SourceUnavailable {
            code: PRICE_SOURCE_SEAM_NOT_WIRED.to_string(),
            message: "no adapter implements the PriceSignalSource port in this \
                      baseline (FRV1-R1-003 recorded deferral; no price source \
                      is configured in 0.x) — ingested price data lands with the \
                      first adapter composition"
                .to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the validation rules or a literal copied from
// this module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn instant(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("fixture instant is representable")
    }

    fn point(start_millis: i64, price: f64) -> PricePoint {
        PricePoint::new(instant(start_millis), instant(start_millis + 3_600_000), price)
            .expect("fixture point is valid")
    }

    #[test]
    fn kind_and_source_labels_are_stable() {
        assert_eq!(PriceSeriesKind::Actual.as_str(), "actual");
        assert_eq!(PriceSeriesKind::VendorForecast.as_str(), "vendor-forecast");
        assert_eq!(PriceSourceClass::TransparencyPlatform.as_str(), "transparency-platform");
        assert_eq!(PriceSourceClass::CommercialVendor.as_str(), "commercial-vendor");
    }

    #[test]
    fn a_well_formed_series_constructs() {
        let series = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            vec![point(0, 42.5), point(3_600_000, 38.0)],
        )
        .expect("valid series");
        assert_eq!(series.kind.as_str(), "actual");
        assert_eq!(series.currency_code, "EUR");
        assert_eq!(series.points.len(), 2);
    }

    #[test]
    fn a_non_finite_price_point_is_refused() {
        let err = PricePoint::new(instant(0), instant(1), f64::NAN).expect_err("NaN refused");
        assert_eq!(err.name(), "PRICE_POINT_NON_FINITE");
    }

    #[test]
    fn negative_prices_are_carried_not_refused() {
        // Negative prices are real market outcomes in several
        // jurisdictions (UNVERIFIED for any specific market); the port
        // carries the published value, sign included.
        let point = PricePoint::new(instant(0), instant(1), -12.5).expect("negative is carried");
        assert_eq!(point.price_per_mwh, -12.5);
    }

    #[test]
    fn an_inverted_interval_is_refused() {
        let err =
            PricePoint::new(instant(10), instant(10), 1.0).expect_err("empty interval refused");
        assert_eq!(err.name(), "PRICE_POINT_INVERTED_INTERVAL");
    }

    #[test]
    fn empty_references_area_currency_and_points_are_refused() {
        let err = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "  ".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            vec![point(0, 1.0)],
        )
        .expect_err("empty source ref refused");
        assert_eq!(err.name(), "PRICE_SERIES_EMPTY_REFERENCE");

        let err = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "".to_string(),
            "EUR".to_string(),
            vec![point(0, 1.0)],
        )
        .expect_err("empty area refused");
        assert_eq!(err.name(), "PRICE_SERIES_EMPTY_AREA_OR_CURRENCY");

        let err = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            Vec::new(),
        )
        .expect_err("empty point list refused");
        assert_eq!(err.name(), "PRICE_SERIES_NO_POINTS");
    }

    #[test]
    fn unsorted_points_are_refused_never_silently_sorted() {
        let err = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            vec![point(3_600_000, 1.0), point(0, 2.0)],
        )
        .expect_err("unsorted points refused");
        assert_eq!(err.name(), "PRICE_SERIES_UNSORTED_POINTS");
    }

    #[test]
    fn every_error_renders_non_empty_prose() {
        // Three different error types, so the prose is collected as
        // Strings: one array cannot hold them together.
        let errors = [
            PricePointError::NonFinitePrice.to_string(),
            PricePointError::InvertedInterval.to_string(),
            PriceSeriesError::EmptyReference { field: "source_ref" }.to_string(),
            PriceSeriesError::EmptyAreaOrCurrency.to_string(),
            PriceSeriesError::NoPoints.to_string(),
            PriceSeriesError::UnsortedPoints { index: 1 }.to_string(),
            PriceSeriesError::PointInvalid {
                index: 0,
                reason: "fixture".to_string(),
            }
            .to_string(),
            PricePortError::SourceUnavailable {
                code: "E-TEST".to_string(),
                message: "fixture".to_string(),
            }
            .to_string(),
            PricePortError::UnknownArea { area: "X".to_string() }.to_string(),
        ];
        for error in errors {
            assert!(!error.is_empty());
        }
    }

    /// The only path a read can take to a tenant scope: authenticated
    /// claims → tenant context. There is no other constructor.
    fn tenant_context() -> TenantContext {
        use flexcore_contracts::ids::FlexUuid;
        use flexcore_domain::ids::TenantId;
        use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(
                    FlexUuid::parse("0192a800-0000-7000-8000-00000000000a")
                        .expect("fixture tenant id is a valid uuid"),
                ),
                subject: "user:price-port-test".to_string(),
            },
        ))
    }

    // FRV1-R1-003 — the unwired default is a named fail-closed refusal,
    // never a fabricated series and never a silently-accepted ingest.
    #[test]
    fn the_unwired_default_refuses_every_operation_by_name() {
        let source = UnwiredPriceSignalSource;
        let err = source
            .latest_series(&tenant_context(), PriceSeriesKind::Actual, "FIXTURE-ZONE")
            .expect_err("the unwired default refuses");
        assert_eq!(err.name(), "PRICE_SOURCE_UNAVAILABLE");
        let rendered = err.to_string();
        assert!(rendered.contains(PRICE_SOURCE_SEAM_NOT_WIRED));
        assert!(rendered.contains("FRV1-R1-003"));

        let series = PriceSeries::new(
            PriceSeriesKind::Actual,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            vec![point(0, 42.5)],
        )
        .expect("valid series");
        let err = source
            .ingest_series(&tenant_context(), series)
            .expect_err("the unwired default refuses");
        assert_eq!(err.name(), "PRICE_SOURCE_UNAVAILABLE");
        assert!(err.to_string().contains(PRICE_SOURCE_SEAM_NOT_WIRED));
    }
}
