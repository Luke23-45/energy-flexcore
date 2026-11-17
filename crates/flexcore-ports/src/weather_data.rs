// P15.03 — The weather-data ingestion PORT (the PV-forecast weather seam).
//
// Phase 15 plan §2 row P15.03; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 03_p15_03_pv_weather_seam.md`.
//
// WHAT THIS IS: the capability interface through which weather /
// irradiance data (observations and forecast series) enters FlexCore as
// INGESTED DATA. The build-vs-buy question (P15-E-1, registered OPEN with
// the DEFAULT/RECOMMENDATION: buy a commercial irradiance/power service)
// is decided by the human owner; this port is vendor-neutral BY
// CONSTRUCTION — the port crate forbids vendor and product names in source
// text (the lib.rs grep gate), and every concrete technology (HTTP client,
// vendor SDK, API key vault) belongs to an adapter implementing this port.
//
// RULES THIS PORT ENCODES (mirroring the crate's recorded rules):
//
// 1. **Data, never commands.** Weather data is an INPUT to forecasting
//    (ADR-010 extension, conventions §6.3). The port has no write path to
//    any device, no dispatch vocabulary, and no permission beyond reading
//    the data a provider supplies. A weather datum is never a measurement
//    of the grid and never a market price.
// 2. **Provenance travels with every value** (the P3.02
//    unknown-narrows discipline and the P3.31 labeling discipline,
//    applied to bought data): every sample carries its closed
//    [`WeatherDataProvenance`] class and a non-empty source reference.
//    There is no unprovenanced weather sample type — construction is
//    Result-based and refuses the empty reference (no partial records).
// 3. **Tenant scoping is structural** (P0.16 rules 1 and 5): every port
//    method takes the domain `TenantContext`; weather data licensed for
//    one tenant is readable only through that tenant's context.
// 4. **Sync traits** (the crate rule): no future, no async runtime; an
//    adapter that must bridge asynchrony does so on its own side.
// 5. **Canonical units declared, never assumed** (§10.2 freeze applies to
//    the power/energy quantities derived FROM weather; the weather
//    quantities themselves carry their own closed unit vocabulary below —
//    each quantity names its unit; a mismatched unit is a named refusal,
//    never a silent conversion).
// 6. **Quality is carried, not invented**: weather samples carry the
//    platform's six-state telemetry quality vocabulary
//    (`flexcore-contracts::quality::TelemetryQuality`) so a bought datum
//    that is late, estimated, or suspect is HONEST in-band — the staleness
//    rules (P15.16) consume exactly this.
//
// WHAT IS A RECORDED SEAM (NOT implemented): no adapter ships here; no
// vendor account, token, or endpoint exists (the API key is a HUMAN GATE
// input, UNSIGNED until registered — P15-E-1). The PV forecasting service
// that consumes this port is the P15.03/P15.16 design; nothing here
// produces a forecast.

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::SiteId;
use flexcore_domain::tenant_context::TenantContext;

/// The closed weather-quantity vocabulary with its declared unit. Every
/// quantity names the unit its value carries; nothing is unit-free, and no
/// conversion happens in the port (adapters convert at their boundary and
/// declare the conversion — the P14.07 `units_map` pattern, mirrored).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherQuantity {
    /// Global horizontal irradiance — watts per square metre (`W/m2`).
    GlobalHorizontalIrradiance,
    /// Direct normal irradiance — watts per square metre (`W/m2`).
    DirectNormalIrradiance,
    /// Diffuse horizontal irradiance — watts per square metre (`W/m2`).
    DiffuseHorizontalIrradiance,
    /// Air temperature at 2 m — degrees Celsius (`degC`).
    AirTemperatureDegC,
    /// Wind speed at 10 m — metres per second (`m/s`).
    WindSpeedMs,
    /// Total cloud cover — a fraction in [0, 1] (declared fraction, like
    /// SOC and availability; not a percentage).
    CloudCoverFraction,
}

impl WeatherQuantity {
    /// The unit this quantity's value carries (declared, never assumed).
    pub const fn unit(self) -> &'static str {
        match self {
            WeatherQuantity::GlobalHorizontalIrradiance => "W/m2",
            WeatherQuantity::DirectNormalIrradiance => "W/m2",
            WeatherQuantity::DiffuseHorizontalIrradiance => "W/m2",
            WeatherQuantity::AirTemperatureDegC => "degC",
            WeatherQuantity::WindSpeedMs => "m/s",
            WeatherQuantity::CloudCoverFraction => "fraction",
        }
    }

    /// The wire spelling of the quantity (records and audit details).
    pub const fn wire_name(self) -> &'static str {
        match self {
            WeatherQuantity::GlobalHorizontalIrradiance => "ghi",
            WeatherQuantity::DirectNormalIrradiance => "dni",
            WeatherQuantity::DiffuseHorizontalIrradiance => "dhi",
            WeatherQuantity::AirTemperatureDegC => "air_temperature",
            WeatherQuantity::WindSpeedMs => "wind_speed",
            WeatherQuantity::CloudCoverFraction => "cloud_cover",
        }
    }
}

/// The closed provenance class of a weather datum (P3.02 provenance
/// discipline applied to bought data: the class travels with the value;
/// unknown provenance narrows what the datum may be used for, never
/// widens it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherDataProvenance {
    /// Bought from a commercial weather/irradiance service and ingested as
    /// DATA (the P15-E-1 DEFAULT/RECOMMENDATION; the vendor is named only
    /// in the deployment configuration, never in this port).
    CommercialService,
    /// Measured at the site (a site-owned sensor). The sensor's own
    /// telemetry provenance is a telemetry-layer concern; this class
    /// records only that the datum is site-measured.
    SiteMeasured,
    /// Derived from a public/open weather feed and ingested as DATA.
    PublicFeed,
}

impl WeatherDataProvenance {
    /// The canonical label spelling (records, audit details, artifacts).
    pub const fn as_str(self) -> &'static str {
        match self {
            WeatherDataProvenance::CommercialService => "commercial-service",
            WeatherDataProvenance::SiteMeasured => "site-measured",
            WeatherDataProvenance::PublicFeed => "public-feed",
        }
    }
}

/// One weather sample at one site and one instant. Result-constructed:
/// there is no way to hold an unvalidated sample (no partial records — the
/// `NormalizedTelemetry` posture, mirrored).
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSample {
    /// The site the datum describes.
    pub site_id: SiteId,
    /// The quantity and its declared unit.
    pub quantity: WeatherQuantity,
    /// The value in the declared unit. Validated finite at construction.
    pub value: f64,
    /// The instant the datum is valid for (event time, UTC).
    pub valid_at: EventTime,
    /// The closed provenance class (travels with the value — never stored
    /// apart where it can drift, the P8.32 linked-provenance rule).
    pub provenance: WeatherDataProvenance,
    /// The provider/source reference (non-empty; a REFERENCE to the
    /// provider's dataset/query identity, never credentials).
    pub source_ref: String,
    /// The six-state quality of the datum (bought data can be SUSPECT /
    /// STALE / ESTIMATED exactly like telemetry; the consumer decides
    /// what to trust, the port only carries the truth).
    pub quality: TelemetryQuality,
}

/// Why a weather sample was refused. Every variant is a named, matchable
/// error; refusals never degrade into a default sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeatherSampleError {
    /// The value was NaN or infinite.
    NonFiniteValue,
    /// A fraction-quantity value (cloud cover) was outside [0, 1]
    /// (§10.2 boundary rule: out-of-range is a refusal, never clipped).
    FractionOutOfRange { value: f64 },
    /// An irradiance value was negative (irradiance is non-negative by
    /// definition; a negative value is a data error, not an absorption).
    NegativeIrradiance { value: f64 },
    /// The source reference was empty or whitespace.
    EmptySourceRef,
}

impl WeatherSampleError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            WeatherSampleError::NonFiniteValue => "WEATHER_SAMPLE_NON_FINITE_VALUE",
            WeatherSampleError::FractionOutOfRange { .. } => "WEATHER_SAMPLE_FRACTION_OUT_OF_RANGE",
            WeatherSampleError::NegativeIrradiance { .. } => "WEATHER_SAMPLE_NEGATIVE_IRRADIANCE",
            WeatherSampleError::EmptySourceRef => "WEATHER_SAMPLE_EMPTY_SOURCE_REF",
        }
    }
}

impl std::fmt::Display for WeatherSampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherSampleError::NonFiniteValue => {
                write!(f, "weather value must be finite (no NaN or infinity)")
            }
            WeatherSampleError::FractionOutOfRange { value } => write!(
                f,
                "fraction-quantity value {value} is outside [0, 1]; out-of-range is a \
                 refusal, never clipped"
            ),
            WeatherSampleError::NegativeIrradiance { value } => write!(
                f,
                "irradiance value {value} W/m2 is negative; a negative irradiance is a \
                 data error, never absorbed"
            ),
            WeatherSampleError::EmptySourceRef => {
                write!(f, "the source reference must be a non-empty string")
            }
        }
    }
}

impl std::error::Error for WeatherSampleError {}

impl WeatherSample {
    /// Validates and constructs one sample (fail-closed; the unit the
    /// value carries is the quantity's declared unit — no separate unit
    /// field exists to disagree with it).
    pub fn new(
        site_id: SiteId,
        quantity: WeatherQuantity,
        value: f64,
        valid_at: EventTime,
        provenance: WeatherDataProvenance,
        source_ref: String,
        quality: TelemetryQuality,
    ) -> Result<Self, WeatherSampleError> {
        if !value.is_finite() {
            return Err(WeatherSampleError::NonFiniteValue);
        }
        match quantity {
            WeatherQuantity::CloudCoverFraction => {
                if !(0.0..=1.0).contains(&value) {
                    return Err(WeatherSampleError::FractionOutOfRange { value });
                }
            }
            WeatherQuantity::GlobalHorizontalIrradiance
            | WeatherQuantity::DirectNormalIrradiance
            | WeatherQuantity::DiffuseHorizontalIrradiance => {
                if value < 0.0 {
                    return Err(WeatherSampleError::NegativeIrradiance { value });
                }
            }
            WeatherQuantity::AirTemperatureDegC | WeatherQuantity::WindSpeedMs => {}
        }
        if source_ref.trim().is_empty() {
            return Err(WeatherSampleError::EmptySourceRef);
        }
        Ok(Self {
            site_id,
            quantity,
            value,
            valid_at,
            provenance,
            source_ref,
            quality,
        })
    }
}

/// The weather-data ingestion port (P15.03). The port defines the
/// interface ONLY; the implementing adapter is the composition root's
/// choice once P15-E-1 is closed (DEFAULT/RECOMMENDATION: a commercial
/// irradiance/power service, ingested as DATA with provenance). No
/// implementation ships in this crate, and no vendor, product, protocol,
/// or endpoint is named here — the crate's vendor-name grep gate holds.
///
/// Control status: an INPUT seam for forecasting (ADR-010 extension,
/// conventions §6.3). Nothing here authorizes, issues, or dispatches
/// anything; a weather datum is never a command, a grid measurement, or a
/// market price.
pub trait WeatherDataProvider {
    /// The latest observation per quantity for one site, tenant-scoped.
    /// An absent quantity is an empty result — never a fabricated sample.
    fn latest_observation(
        &self,
        ctx: &TenantContext,
        site_id: &SiteId,
        quantity: WeatherQuantity,
    ) -> Result<Option<WeatherSample>, WeatherPortError>;

    /// The forecast series for one site and quantity: samples in
    /// valid-at order over the requested horizon. The horizon START
    /// (UTC millis) and END (UTC millis, exclusive) are caller-declared;
    /// a start-after-end window is a named refusal. Samples inside the
    /// series are validated (each is a [`WeatherSample`]); the port does
    /// NOT validate forecast accuracy — accuracy is the evaluation
    /// framework's (P15.05) business, and no skill is claimed anywhere.
    fn forecast_series(
        &self,
        ctx: &TenantContext,
        site_id: &SiteId,
        quantity: WeatherQuantity,
        valid_from_millis: i64,
        valid_until_millis: i64,
    ) -> Result<Vec<WeatherSample>, WeatherPortError>;
}

/// The weather-port error surface. Distinct from the sample-validation
/// errors: this is the port/adaptor boundary (the `BackendError`
/// posture — adapters report opaque code + message, never raw vendor
/// text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeatherPortError {
    /// The port's backing adapter is unavailable (fail-closed: a missing
    /// weather provider is never answered with synthetic weather).
    ProviderUnavailable { code: String, message: String },
    /// The requested window is inverted or non-positive-length.
    InvalidWindow { from_millis: i64, until_millis: i64 },
    /// The provider returned data that failed validation; nothing is
    /// partially accepted (the whole series is refused, never trimmed).
    ProviderDataRefused { reason: String },
}

impl WeatherPortError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            WeatherPortError::ProviderUnavailable { .. } => "WEATHER_PROVIDER_UNAVAILABLE",
            WeatherPortError::InvalidWindow { .. } => "WEATHER_PORT_INVALID_WINDOW",
            WeatherPortError::ProviderDataRefused { .. } => "WEATHER_PROVIDER_DATA_REFUSED",
        }
    }
}

impl std::fmt::Display for WeatherPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherPortError::ProviderUnavailable { code, message } => write!(
                f,
                "weather provider unavailable ({code}): {message} — a missing provider is \
                 never answered with synthetic weather (fail-closed)"
            ),
            WeatherPortError::InvalidWindow {
                from_millis,
                until_millis,
            } => write!(
                f,
                "invalid forecast window [{from_millis}, {until_millis}): start must be \
                 before end (UTC millis)"
            ),
            WeatherPortError::ProviderDataRefused { reason } => {
                write!(f, "provider data refused in full: {reason}")
            }
        }
    }
}

impl std::error::Error for WeatherPortError {}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the validation rules or a literal copied from
// this module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteId {
        SiteId::new(
            flexcore_contracts::ids::FlexUuid::parse("0192a800-0000-7000-8000-000000000001")
                .expect("fixture site id is a valid uuid"),
        )
    }

    fn valid_at() -> EventTime {
        EventTime::from_unix_millis(1_780_272_000_000).expect("fixture instant is representable")
    }

    #[test]
    fn quantity_units_are_the_declared_vocabulary() {
        assert_eq!(WeatherQuantity::GlobalHorizontalIrradiance.unit(), "W/m2");
        assert_eq!(WeatherQuantity::AirTemperatureDegC.unit(), "degC");
        assert_eq!(WeatherQuantity::WindSpeedMs.unit(), "m/s");
        assert_eq!(WeatherQuantity::CloudCoverFraction.unit(), "fraction");
        assert_eq!(WeatherQuantity::GlobalHorizontalIrradiance.wire_name(), "ghi");
        assert_eq!(WeatherQuantity::CloudCoverFraction.wire_name(), "cloud_cover");
    }

    #[test]
    fn provenance_labels_are_stable() {
        assert_eq!(WeatherDataProvenance::CommercialService.as_str(), "commercial-service");
        assert_eq!(WeatherDataProvenance::SiteMeasured.as_str(), "site-measured");
        assert_eq!(WeatherDataProvenance::PublicFeed.as_str(), "public-feed");
    }

    #[test]
    fn a_well_formed_sample_constructs() {
        let sample = WeatherSample::new(
            site(),
            WeatherQuantity::GlobalHorizontalIrradiance,
            612.5,
            valid_at(),
            WeatherDataProvenance::CommercialService,
            "weather://fixture/query-1".to_string(),
            TelemetryQuality::Good,
        )
        .expect("valid sample");
        assert_eq!(sample.quantity.unit(), "W/m2");
        assert_eq!(sample.provenance.as_str(), "commercial-service");
    }

    #[test]
    fn non_finite_values_are_refused() {
        let err = WeatherSample::new(
            site(),
            WeatherQuantity::AirTemperatureDegC,
            f64::NAN,
            valid_at(),
            WeatherDataProvenance::SiteMeasured,
            "weather://fixture/q".to_string(),
            TelemetryQuality::Good,
        )
        .expect_err("NaN refused");
        assert_eq!(err.name(), "WEATHER_SAMPLE_NON_FINITE_VALUE");
    }

    #[test]
    fn cloud_cover_outside_unit_interval_is_refused_never_clipped() {
        let err = WeatherSample::new(
            site(),
            WeatherQuantity::CloudCoverFraction,
            1.5,
            valid_at(),
            WeatherDataProvenance::CommercialService,
            "weather://fixture/q".to_string(),
            TelemetryQuality::Good,
        )
        .expect_err("1.5 refused");
        assert_eq!(err.name(), "WEATHER_SAMPLE_FRACTION_OUT_OF_RANGE");
        // The closed-interval endpoints are accepted.
        for value in [0.0, 1.0] {
            WeatherSample::new(
                site(),
                WeatherQuantity::CloudCoverFraction,
                value,
                valid_at(),
                WeatherDataProvenance::CommercialService,
                "weather://fixture/q".to_string(),
                TelemetryQuality::Good,
            )
            .expect("boundary fractions are valid");
        }
    }

    #[test]
    fn negative_irradiance_is_refused_as_a_data_error() {
        let err = WeatherSample::new(
            site(),
            WeatherQuantity::DirectNormalIrradiance,
            -1.0,
            valid_at(),
            WeatherDataProvenance::CommercialService,
            "weather://fixture/q".to_string(),
            TelemetryQuality::Good,
        )
        .expect_err("negative irradiance refused");
        assert_eq!(err.name(), "WEATHER_SAMPLE_NEGATIVE_IRRADIANCE");
    }

    #[test]
    fn an_empty_source_ref_is_refused() {
        let err = WeatherSample::new(
            site(),
            WeatherQuantity::WindSpeedMs,
            3.2,
            valid_at(),
            WeatherDataProvenance::PublicFeed,
            "   ".to_string(),
            TelemetryQuality::Good,
        )
        .expect_err("empty source ref refused");
        assert_eq!(err.name(), "WEATHER_SAMPLE_EMPTY_SOURCE_REF");
    }

    #[test]
    fn every_error_renders_non_empty_prose() {
        let errors = [
            WeatherSampleError::NonFiniteValue,
            WeatherSampleError::FractionOutOfRange { value: 2.0 },
            WeatherSampleError::NegativeIrradiance { value: -1.0 },
            WeatherSampleError::EmptySourceRef,
            WeatherPortError::ProviderUnavailable {
                code: "E-TEST".to_string(),
                message: "fixture".to_string(),
            },
            WeatherPortError::InvalidWindow {
                from_millis: 10,
                until_millis: 5,
            },
            WeatherPortError::ProviderDataRefused {
                reason: "fixture".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
