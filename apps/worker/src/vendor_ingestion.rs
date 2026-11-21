// FIM-F10 — Live forecasting ingestion: the vendor-generic weather/price
//           HTTP ingestion adapter (FIM-F10.01), the persistence-baseline
//           fallback composition (FIM-F10.02), and the SHA-256 forecast
//           artifact digest chain (FIM-F10.04), with pinned tests
//           (FIM-F10.05) and honesty headers (FIM-F10.06).
//
// ══════════════════════════════════════════════════════════════════════════
// HONESTY HEADER — read before trusting anything below (FIM-F10.06)
//
// 1. IMPLEMENTED-UNDER-FIM-F10, WRITTEN-UNRUN. This module was written,
//    never compiled, never executed (the FIM hard boundary: no build, no
//    test execution; the first human build/test gate owns the run). Every
//    pinned test value is a hand-derived property of the validation rules,
//    a published SHA-256 test vector, or a float spelling confirmed with
//    `python -I` (recorded in the FIM close-out). Nothing is fabricated,
//    nothing is measured.
// 2. VENDOR-GENERIC BY CONSTRUCTION; P15-E-1/P15-E-2 STAY OPEN. No vendor,
//    product, protocol, or wire schema is named anywhere in this module —
//    the concrete source is DEPLOYMENT CONFIGURATION (base URL, endpoint
//    paths, field mapping, unit factors), exactly the posture the ports
//    record (`crates/flexcore-ports/src/weather_data.rs`, `price_signal.rs`).
//    The API token is a HUMAN GATE input (UNSIGNED until a human registers
//    it — program conventions §5): construction WITHOUT a registered token
//    refuses by name and the adapter cannot be brought to serve. A vendor
//    feed ingesting with provenance is acceptance criterion (1) of F-10 and
//    can only be demonstrated AT the human gate — never claimed from here.
// 3. DATA, NEVER COMMANDS (ADR-010, frozen). Everything this module
//    produces is an INPUT to forecasting and scheduling analytics. There is
//    no dispatch vocabulary, no write path to any device, and no control
//    authority anywhere in this surface; the fallback composition composes
//    the recorded statement
//    `flexcore_application::forecast_integration::FALLBACK_IS_NOT_A_COMMAND_STATEMENT`.
// 4. QUALITY IS CARRIED, NOT INVENTED (frozen telemetry vocabulary). The
//    staleness classification maps onto the EXISTING six-state vocabulary:
//    a record inside the staleness window is GOOD; past the window is
//    STALE; an absent quantity is MISSING (`Ok(None)` — never a fabricated
//    sample). SUSPECT / BAD / ESTIMATED are vendor-qualified states the
//    generic mapping cannot express (recorded; a future vendor-specific
//    mapping may carry them through configuration). A stale feed is never
//    relabeled GOOD and missing data is never invented.
// 5. CANONICAL UNITS: MW/MWh at the boundary (§10.2); weather quantities
//    carry the port's declared units (W/m2, degC, m/s, declared fraction).
//    Vendor units convert through EXPLICIT config-declared factors, each
//    factor carrying its own provenance string. Affine conversions (e.g.
//    degree Fahrenheit → degree Celsius) need an offset and are REFUSED at
//    config construction — never approximated. There is no silent
//    conversion anywhere.
// 6. THE SYNTHETIC FIXTURE STAYS THE LABELED FALLBACK SOURCE (P15.16). The
//    fallback composition's last resort carries `FIXTURE_ID` and the
//    `synthetic` provenance label verbatim from the fixture vocabulary
//    (`flexcore_application::forecast_fixture`) — the label travels with
//    every value a consumer reads from the fixture (the P3.31 four-place
//    rule). This module never generates fixture values.
// 7. NO SKILL IS CLAIMED (P15.05). The persistence-baseline fallback
//    carries the standing `NO_SKILL_STATEMENT` posture; the P15.05 result
//    table stays EMPTY in every record until a real backtesting run
//    completes (HUMAN GATE). The python backtesting harness
//    (`apps/worker/python/forecast_worker/evaluation.py`) computes metrics
//    over supplied series — mathematics, not claims.
// 8. THE RUST ↔ PYTHON COMPOSITION SEAM (FIM-F10.02, recorded): no
//    runtime seam exists — nothing in this workspace invokes a python
//    interpreter from Rust (verified: no `std::process::Command` python
//    call anywhere in apps/ or crates/; the workers are OUT-OF-PROCESS
//    stdin→stdout contracts driven by the operator/harness, per the
//    forecast_worker README). The fallback therefore composes on the Rust
//    side against the single home of the persistence rule
//    (`flexcore_application::forecast_baselines::persistence_forecast`),
//    and the SAME pure selection function is mirrored in
//    `forecast_worker/evaluation.py` for the worker side, both pinned by
//    identical hand-computed boundary vectors (fresh at max-age, fallback
//    at max-age + 1 — the P2.08 equality convention).
// 9. SHA-256 DIGEST CHAIN (FIM-F10.04, P15.06 shape): append-only,
//    head-proof (every append names the current head), tamper-EVIDENT by
//    verification (a digest mismatch is a named error, never absorbed),
//    replayable from (artifact + inputs) identity. SHA-256 [FIPS 180-4] is
//    an upgrade over the test-grade FNV-1a 64 the P15.06 registry uses;
//    it is still an UNKEYED integrity/replay stamp — explicitly NOT a
//    signature and NOT a security primitive (no signing identity exists;
//    a keyed construction would be the `flexcore-audit` HMAC chain's
//    deployment-gated posture). The empty-chain head is the published
//    SHA-256 of the empty input.
// ══════════════════════════════════════════════════════════════════════════
//
// # Composition status (honest, FRV1-R2-004 posture)
//
// As of this tree NOTHING composes this module: the worker's composition
// root (src/main.rs, frozen) does not construct the adapters (no token
// exists — P15-E-1/E-2 are OPEN human gates), and no scheduled loop hosts
// a refresh task. `from_lookup`/`from_env` are the composition surface the
// deployment root calls once a human registers a token; until then the
// seam is visible only to these tests, exactly like the
// `UnwiredWeatherDataProvider` default the port ships. Nothing here is
// faked to appear wired.
//
// # Retry / budget posture (the repo's recorded conventions, by citation)
//
// - Bounded attempts: the fetch path makes at most
//   `SharedFetchConfig::max_attempts` HTTP attempts (default 3 total — the
//   same bound the P7.18 supervision policy records as its DEFAULT:
//   `max_retries: 2` ⇒ 3 attempts). No unbounded retry amplification.
// - Deterministic backoff between attempts: `BACKOFF_BASE_MS × 2^attempt`,
//   bounded by `BACKOFF_MAX_MS` (the P7.18 deterministic retry scheduler's
//   shape; seeded jitter is deliberately absent — vendor polling wants
//   reproducibility).
// - Bounded request budget via the refresh window: a refresh attempted
//   within `refresh_window_ms` of the previous attempt is refused as
//   `RefreshOutcome::NotDue` (with the retry-after delay) — the vendor's
//   rate budget is respected structurally, mirroring the P2.07 backpressure
//   posture (never unbounded request amplification, never a silent skip:
//   the outcome is named).
// - Bounded per-request timeout: `timeout_ms` caps each HTTP attempt
//   (fail-closed construction refuses a zero timeout).
// - Sync-port hosting: the port traits are sync (the ports-crate rule);
//   the fetch path uses the `reqwest` blocking client, which runs its own
//   private runtime — the adapter "bridges asynchrony on its own side"
//   (the recorded ports-crate decision 1 posture, same class as the
//   PostgreSQL adapter's private runtime). A caller INSIDE a tokio
//   runtime must host the call under `tokio::task::block_in_place` — the
//   same contract every other port-touching section of this worker
//   documents (see src/lib.rs "Sync-port hosting").
//
// # Secrets posture (P1.20 / the config.rs convention)
//
// The token lives in [`VendorToken`], whose `Debug` rendering is
// REDACTED and which has no `Display`/`as_str` exposure. No error, no
// log-shaped message, and no `Debug` output of any type in this module
// carries the token value. Error messages name the ENV VARIABLE, never
// the value.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::forecast_baselines::{persistence_forecast, ForecastBaselineError};
use flexcore_application::forecast_fixture::{FIXTURE_ID, PROVENANCE_SYNTHETIC};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::SiteId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::price_signal::{
    PricePoint, PricePortError, PriceSeries, PriceSeriesError, PriceSeriesKind, PriceSignalSource,
    PriceSourceClass,
};
use flexcore_ports::weather_data::{
    WeatherDataProvenance, WeatherDataProvider, WeatherPortError, WeatherQuantity, WeatherSample,
};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// 1. Environment variables (the `FLEXCORE_*` convention; fail-fast named
//    errors, no default credentials, no secret logging — the config.rs
//    posture). All vendor-specificity lives here: the P15-E-1/E-2 decisions
//    name the vendor in DEPLOYMENT CONFIGURATION only, never in source.
// ---------------------------------------------------------------------------

/// Required for the weather adapter: the vendor feed base URL (scheme +
/// host; the adapter appends the configured endpoint path). Vendor-generic.
pub const ENV_VENDOR_WEATHER_URL: &str = "FLEXCORE_VENDOR_WEATHER_URL";
/// Required for the weather adapter: the bearer token. HUMAN GATE input
/// (UNSIGNED until a human registers it — program conventions §5). Absent
/// ⇒ fail-closed construction refusing to serve, named error. The value is
/// never logged, never rendered, never copied into an error.
pub const ENV_VENDOR_WEATHER_TOKEN: &str = "FLEXCORE_VENDOR_WEATHER_TOKEN";
/// Optional: the weather forecast endpoint path appended to the base URL.
/// Default `/v1/forecast`.
pub const ENV_VENDOR_WEATHER_FORECAST_PATH: &str = "FLEXCORE_VENDOR_WEATHER_FORECAST_PATH";
/// Optional: the weather field map, JSON object mapping VENDOR field names
/// → the port's wire names (`ghi`, `dni`, `dhi`, `air_temperature`,
/// `wind_speed`, `cloud_cover` — `WeatherQuantity::wire_name`). Required
/// for every quantity the deployment actually ingests; a quantity without
/// a mapped vendor field is a named refusal at refresh time.
pub const ENV_VENDOR_WEATHER_FIELD_MAP: &str = "FLEXCORE_VENDOR_WEATHER_FIELD_MAP";
/// Optional: the declared unit conversions, JSON object keyed by wire name,
/// each value `{"vendor_unit": "...", "factor": <number>,
/// "factor_provenance": "..."}`. The factor is MULTIPLICATIVE ONLY
/// (canonical = vendor_value × factor). Required for every ingested
/// quantity; a quantity without a declared conversion is refused — there
/// is no default conversion and no silent passthrough.
pub const ENV_VENDOR_WEATHER_UNIT_FACTORS: &str = "FLEXCORE_VENDOR_WEATHER_UNIT_FACTORS";
/// Optional: the weather provenance class the ingested data carries.
/// One of `commercial-service` | `site-measured` | `public-feed` (the
/// closed `WeatherDataProvenance` labels). Default `commercial-service`
/// (the P15-E-1 DEFAULT/RECOMMENDATION class — the class is a label, not
/// a decision; the decision stays OPEN).
pub const ENV_VENDOR_WEATHER_PROVENANCE: &str = "FLEXCORE_VENDOR_WEATHER_PROVENANCE";
/// Required for the price adapter: the price feed base URL. Vendor-generic.
pub const ENV_VENDOR_PRICE_URL: &str = "FLEXCORE_VENDOR_PRICE_URL";
/// Required for the price adapter: the bearer token (HUMAN GATE input,
/// same posture as the weather token).
pub const ENV_VENDOR_PRICE_TOKEN: &str = "FLEXCORE_VENDOR_PRICE_TOKEN";
/// Optional: the price endpoint path appended to the base URL. Default
/// `/v1/day-ahead`.
pub const ENV_VENDOR_PRICE_PATH: &str = "FLEXCORE_VENDOR_PRICE_PATH";
/// Optional: the price series source class. One of
/// `transparency-platform` | `commercial-vendor` (the closed
/// `PriceSourceClass` labels). Default `commercial-vendor`.
pub const ENV_VENDOR_PRICE_SOURCE_CLASS: &str = "FLEXCORE_VENDOR_PRICE_SOURCE_CLASS";
/// Optional: the DECLARED currency code of the price feed (ISO 4217-style
/// as published; carried verbatim — a currency-less price series is
/// unconstructable per the port). REQUIRED for the price adapter; there is
/// no default currency anywhere in the platform.
pub const ENV_VENDOR_PRICE_CURRENCY: &str = "FLEXCORE_VENDOR_PRICE_CURRENCY";
/// Optional: the delivery area / bidding-zone id carried on ingested price
/// series (the zone vocabulary is the source's). REQUIRED; no default.
pub const ENV_VENDOR_PRICE_AREA: &str = "FLEXCORE_VENDOR_PRICE_AREA";
/// Optional: the response series-field name of the JSON array. Default
/// `series` (both feeds).
pub const ENV_VENDOR_SERIES_FIELD: &str = "FLEXCORE_VENDOR_SERIES_FIELD";
/// Optional: the row timestamp-field name (epoch UTC milliseconds,
/// integer). Default `valid_at` (weather).
pub const ENV_VENDOR_WEATHER_TS_FIELD: &str = "FLEXCORE_VENDOR_WEATHER_TS_FIELD";
/// Optional: the price row interval-start field name (epoch UTC millis).
/// Default `interval_start`.
pub const ENV_VENDOR_PRICE_START_FIELD: &str = "FLEXCORE_VENDOR_PRICE_START_FIELD";
/// Optional: the price row interval-end field name (epoch UTC millis).
/// Default `interval_end`.
pub const ENV_VENDOR_PRICE_END_FIELD: &str = "FLEXCORE_VENDOR_PRICE_END_FIELD";
/// Optional: the price row price-field name (number, in the DECLARED
/// currency per MWh — the canonical energy base; a vendor unit quoted per
/// kWh needs a declared factor in the unit map BEFORE ingestion, never a
/// silent rescale). Default `price`.
pub const ENV_VENDOR_PRICE_FIELD: &str = "FLEXCORE_VENDOR_PRICE_FIELD";
/// Optional: the minimum interval between refresh attempts, milliseconds.
/// Default 900000 (15 minutes — the forecast cadence the P15.02 horizon
/// examples use; a CONFIGURATION default, not a report requirement).
pub const ENV_VENDOR_REFRESH_WINDOW_MS: &str = "FLEXCORE_VENDOR_REFRESH_WINDOW_MS";
/// Optional: the staleness window, milliseconds: a fetched record younger
/// than this classifies GOOD; past it, STALE (the P2.08 equality
/// convention: age == window is NOT stale). Default 3600000 (one hour).
pub const ENV_VENDOR_STALENESS_WINDOW_MS: &str = "FLEXCORE_VENDOR_STALENESS_WINDOW_MS";
/// Optional: the maximum published-forecast age for the fallback
/// composition (FIM-F10.02), milliseconds. Default = the recorded one-hour
/// default of `forecast_integration::DEFAULT_FORECAST_MAX_AGE_MILLIS`
/// (composed, not restated).
pub const ENV_VENDOR_MAX_AGE_MS: &str = "FLEXCORE_VENDOR_MAX_AGE_MS";
/// Optional: per-attempt HTTP timeout, milliseconds. Default 10000.
pub const ENV_VENDOR_TIMEOUT_MS: &str = "FLEXCORE_VENDOR_TIMEOUT_MS";
/// Optional: maximum HTTP attempts per refresh (bounded). Default 3 (the
/// P7.18 DEFAULT bound: max_retries 2 ⇒ 3 attempts). Clamped to [1, 10].
pub const ENV_VENDOR_MAX_ATTEMPTS: &str = "FLEXCORE_VENDOR_MAX_ATTEMPTS";

// ---------------------------------------------------------------------------
// 2. Errors. Named, matchable; `Display` never includes the token value.
// ---------------------------------------------------------------------------

/// Why adapter construction or a fetch refused. The fail-closed posture:
/// construction without a registered token refuses BY NAME; a refused
/// fetch never degrades into fabricated or default data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorIngestionError {
    /// Construction refused: the token env variable is absent or blank.
    /// This is the fail-closed human gate — the adapter cannot be brought
    /// to serve without a registered token.
    TokenNotRegistered { variable: &'static str },
    /// A configuration value was present but unusable (bad URL, bad JSON,
    /// bad factor, unknown label, zero timeout, ...). Names the variable
    /// or field and the reason; never the token value.
    InvalidConfig { field: String, reason: String },
    /// The HTTP path exhausted its bounded attempt budget (transport
    /// error, non-success status, or timeout on every attempt). Carries
    /// the attempt count and an opaque status class — never a raw vendor
    /// response body (the `BackendError` posture of the ports).
    FetchFailed { attempts: u32, status_class: String },
    /// The response violated the documented shape contract; the WHOLE
    /// response is refused (nothing partially accepted).
    ParseRefused { reason: String },
}

impl VendorIngestionError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            VendorIngestionError::TokenNotRegistered { .. } => "VENDOR_TOKEN_NOT_REGISTERED",
            VendorIngestionError::InvalidConfig { .. } => "VENDOR_INGESTION_INVALID_CONFIG",
            VendorIngestionError::FetchFailed { .. } => "VENDOR_FETCH_FAILED",
            VendorIngestionError::ParseRefused { .. } => "VENDOR_PARSE_REFUSED",
        }
    }
}

impl fmt::Display for VendorIngestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VendorIngestionError::TokenNotRegistered { variable } => write!(
                f,
                "{variable} is not registered: the vendor token is a HUMAN GATE input \
                 (UNSIGNED until a human registers it — program conventions §5); \
                 construction is fail-closed and the adapter refuses to serve. \
                 P15-E-1/P15-E-2 remain OPEN; no vendor integration is implied."
            ),
            VendorIngestionError::InvalidConfig { field, reason } => {
                write!(f, "invalid vendor-ingestion configuration '{field}': {reason}")
            }
            VendorIngestionError::FetchFailed { attempts, status_class } => write!(
                f,
                "the vendor fetch exhausted its bounded attempt budget after {attempts} \
                 attempts (last status class: {status_class}); nothing is fabricated — \
                 the caller retries after the refresh window or fails loudly"
            ),
            VendorIngestionError::ParseRefused { reason } => {
                write!(f, "the vendor response was refused in full: {reason}")
            }
        }
    }
}

impl std::error::Error for VendorIngestionError {}

// ---------------------------------------------------------------------------
// 3. The token (secrecy posture) and the clock injection.
// ---------------------------------------------------------------------------

/// The bearer token wrapper: no `Display`, redacted `Debug`, no accessor
/// that returns the value for logging. The ONLY use is the fetch path's
/// Authorization header. (P1.20's no-secrets-in-logs rule, applied at the
/// type level.)
#[derive(Clone, PartialEq, Eq)]
pub struct VendorToken(String);

impl VendorToken {
    /// Wraps a non-blank token; a blank token is a named refusal (an
    /// empty token would authenticate as nothing — fail closed).
    pub fn new(raw: String) -> Result<Self, VendorIngestionError> {
        if raw.trim().is_empty() {
            return Err(VendorIngestionError::InvalidConfig {
                field: "token".to_string(),
                reason: "the registered token is blank; a blank token is refused, never \
                         accepted as an anonymous credential"
                    .to_string(),
            });
        }
        Ok(Self(raw))
    }
}

impl fmt::Debug for VendorToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // REDACTED: the length is honest, the value never leaves the type.
        write!(f, "VendorToken(**REDACTED**, {} chars)", self.0.len())
    }
}

/// The injected clock (UTC epoch milliseconds). Adapters are
/// infrastructure: the production constructor wires the system clock; the
/// pinned tests inject fixed clocks (the model-registry "explicit clock
/// parameter" discipline, applied at the adapter boundary — no wall-clock
/// read inside the pure logic).
pub type FetchClock = std::sync::Arc<dyn Fn() -> i64 + Send + Sync>;

/// Installs the process-wide rustls crypto provider (ring) exactly once,
/// before the first reqwest client is built. reqwest is depended on with
/// `rustls-no-provider`, so no provider is compiled in — without this
/// install every `Client::builder().build()` panics at first USE ("No
/// rustls crypto provider is configured", on reqwest's internal sync
/// runtime). The `ring` provider keeps the resolved provider identical
/// to the committed lock graph (FIM standing decision 1: aws-lc-rs is
/// not in the lock). An already-installed provider (another edge in the
/// same process) is fine — the install is best-effort and its result is
/// ignored.
fn ensure_rustls_crypto_provider() {
    static INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INSTALLED.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// The production clock: UTC epoch milliseconds from the system clock.
/// The ONLY wall-clock read in this module, reachable only through an
/// explicitly-wired adapter (which requires a registered token — the
/// human gate).
pub fn system_epoch_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 4. Declared unit conversion (§10.2 boundary: explicit factors, each with
//    provenance; affine conversions refused; no silent passthrough).
// ---------------------------------------------------------------------------

/// One DECLARED unit conversion: the vendor's unit spelling, the
/// MULTIPLICATIVE factor taking a vendor value into the port's declared
/// unit (`canonical = vendor_value × factor`), and the factor's
/// provenance (where the factor came from — never invented here).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitConversion {
    /// The vendor unit spelling as the feed publishes it (e.g. "kW/m2",
    /// "percent").
    pub vendor_unit: String,
    /// The multiplicative factor into the port's declared unit. Finite and
    /// strictly positive; zero, negative, non-finite, and affine (offset)
    /// conversions are refused at construction.
    pub factor: f64,
    /// The factor's provenance (non-empty): where the factor is declared
    /// from (e.g. "vendor-public-api-doc-2026-11 §units"). A factor
    /// without provenance is refused — never a silent conversion.
    pub factor_provenance: String,
}

impl UnitConversion {
    /// Validates and constructs one declared conversion (fail-closed).
    pub fn new(
        vendor_unit: String,
        factor: f64,
        factor_provenance: String,
    ) -> Result<Self, VendorIngestionError> {
        if vendor_unit.trim().is_empty() {
            return Err(VendorIngestionError::InvalidConfig {
                field: "unit_conversion.vendor_unit".to_string(),
                reason: "the vendor unit spelling must be non-empty".to_string(),
            });
        }
        if !factor.is_finite() || factor <= 0.0 {
            return Err(VendorIngestionError::InvalidConfig {
                field: "unit_conversion.factor".to_string(),
                reason: format!(
                    "the factor must be finite and strictly positive; got {factor} — \
                     a zero/negative/non-finite factor is a configuration error, never \
                     a silent passthrough"
                ),
            });
        }
        if factor_provenance.trim().is_empty() {
            return Err(VendorIngestionError::InvalidConfig {
                field: "unit_conversion.factor_provenance".to_string(),
                reason: "every declared factor carries its provenance; an unprovenanced \
                         factor is refused — never a silent conversion"
                    .to_string(),
            });
        }
        Ok(Self {
            vendor_unit,
            factor,
            factor_provenance,
        })
    }

    /// Converts one vendor value into the port's declared unit. The
    /// conversion is explicit arithmetic, never a passthrough: the value
    /// MUST go through the declared factor (an identity factor is a
    /// DECLARED factor with provenance, not an absence of conversion).
    pub fn convert(&self, vendor_value: f64) -> f64 {
        vendor_value * self.factor
    }
}

/// Parses one entry of the declared unit-factor JSON map (wire name →
/// `{vendor_unit, factor, factor_provenance}`).
fn parse_unit_conversion(
    wire_name: &str,
    entry: &serde_json::Value,
) -> Result<UnitConversion, VendorIngestionError> {
    let object = entry.as_object().ok_or_else(|| VendorIngestionError::InvalidConfig {
        field: format!("{ENV_VENDOR_WEATHER_UNIT_FACTORS}.{wire_name}"),
        reason: "each entry must be an object with vendor_unit, factor, factor_provenance"
            .to_string(),
    })?;
    let vendor_unit = object
        .get("vendor_unit")
        .and_then(|v| v.as_str())
        .ok_or_else(|| VendorIngestionError::InvalidConfig {
            field: format!("{ENV_VENDOR_WEATHER_UNIT_FACTORS}.{wire_name}.vendor_unit"),
            reason: "must be a string".to_string(),
        })?
        .to_string();
    let factor = object
        .get("factor")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| VendorIngestionError::InvalidConfig {
            field: format!("{ENV_VENDOR_WEATHER_UNIT_FACTORS}.{wire_name}.factor"),
            reason: "must be a JSON number".to_string(),
        })?;
    let factor_provenance = object
        .get("factor_provenance")
        .and_then(|v| v.as_str())
        .ok_or_else(|| VendorIngestionError::InvalidConfig {
            field: format!("{ENV_VENDOR_WEATHER_UNIT_FACTORS}.{wire_name}.factor_provenance"),
            reason: "must be a string (the factor's provenance is required)".to_string(),
        })?
        .to_string();
    UnitConversion::new(vendor_unit, factor, factor_provenance)
}

// ---------------------------------------------------------------------------
// 5. Shared fetch configuration (the budget knobs).
// ---------------------------------------------------------------------------

/// The shared fetch/budget configuration (the retry/backpressure posture
/// documented in the module header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedFetchConfig {
    /// Minimum interval between refresh attempts, milliseconds (the
    /// vendor's rate budget, structurally respected). Strictly positive.
    pub refresh_window_ms: i64,
    /// The staleness window, milliseconds: age <= window ⇒ GOOD, else
    /// STALE (the P2.08 equality convention). Strictly positive.
    pub staleness_window_ms: i64,
    /// Per-attempt HTTP timeout. Strictly positive.
    pub timeout_ms: i64,
    /// Maximum HTTP attempts per refresh, clamped to [1, 10] at
    /// construction. Default 3.
    pub max_attempts: u32,
}

/// The deterministic backoff base, milliseconds (attempt 0 waits nothing;
/// attempt n ≥ 1 waits `BACKOFF_BASE_MS × 2^(n-1)`, capped).
pub const BACKOFF_BASE_MS: u64 = 100;
/// The backoff cap, milliseconds (bounded amplification).
pub const BACKOFF_MAX_MS: u64 = 2_000;

impl SharedFetchConfig {
    /// The recorded defaults (see the ENV_* docs above).
    pub fn defaults() -> Self {
        Self {
            refresh_window_ms: 900_000,
            staleness_window_ms: 3_600_000,
            timeout_ms: 10_000,
            max_attempts: 3,
        }
    }

    /// Fail-closed construction: a non-positive window/timeout is refused.
    pub fn new(
        refresh_window_ms: i64,
        staleness_window_ms: i64,
        timeout_ms: i64,
        max_attempts: u32,
    ) -> Result<Self, VendorIngestionError> {
        if refresh_window_ms <= 0 {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_REFRESH_WINDOW_MS.to_string(),
                reason: "must be a positive number of milliseconds".to_string(),
            });
        }
        if staleness_window_ms <= 0 {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_STALENESS_WINDOW_MS.to_string(),
                reason: "must be a positive number of milliseconds".to_string(),
            });
        }
        if timeout_ms <= 0 {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_TIMEOUT_MS.to_string(),
                reason: "must be a positive number of milliseconds (a zero timeout is \
                         not expressible)"
                    .to_string(),
            });
        }
        Ok(Self {
            refresh_window_ms,
            staleness_window_ms,
            timeout_ms,
            max_attempts: max_attempts.clamp(1, 10),
        })
    }

    /// The deterministic backoff delay before retry `attempt` (1-based:
    /// the delay BEFORE the second HTTP attempt is BACKOFF_BASE_MS).
    pub fn backoff_delay_millis(&self, attempt: u32) -> u64 {
        let exp = attempt.saturating_sub(1).min(10);
        BACKOFF_BASE_MS
            .saturating_mul(1u64 << exp)
            .min(BACKOFF_MAX_MS)
    }
}

/// The staleness classification (FIM-F10.01): the record's age against the
/// staleness window, onto the EXISTING six-state vocabulary. `age ==
/// window` is NOT stale (the P2.08 equality convention); `age > window`
/// is STALE. A negative age (a clock read before the fetch) is not stale
/// by this classifier — the fallback composition treats a clock-before-
/// fetch read as its own named reason, never as fresh.
pub fn classify_record_quality(age_millis: i64, staleness_window_ms: i64) -> TelemetryQuality {
    if age_millis >= 0 && age_millis <= staleness_window_ms {
        TelemetryQuality::Good
    } else {
        TelemetryQuality::Stale
    }
}

// ---------------------------------------------------------------------------
// 6. The weather adapter (FIM-F10.01): config-driven, vendor-generic,
//    implements the EXISTING `WeatherDataProvider` port shape.
// ---------------------------------------------------------------------------

/// The weather-feed configuration (all vendor-specificity as fields).
#[derive(Clone)]
pub struct VendorWeatherConfig {
    /// The base URL (scheme + host, no trailing path).
    pub base_url: String,
    /// The forecast endpoint path appended to the base URL.
    pub forecast_path: String,
    /// The registered token (HUMAN GATE input).
    pub token: VendorToken,
    /// Vendor field name → port wire name (`WeatherQuantity::wire_name`).
    pub field_map: BTreeMap<String, String>,
    /// Wire name → declared unit conversion (required per ingested
    /// quantity).
    pub unit_conversions: BTreeMap<String, UnitConversion>,
    /// The provenance class the ingested data carries (closed vocabulary).
    pub provenance: WeatherDataProvenance,
    /// The source REFERENCE carried on every sample (provider
    /// dataset/query identity; NEVER credentials).
    pub source_ref: String,
    /// The response series-field name (the JSON array of rows).
    pub series_field: String,
    /// The row timestamp-field name (epoch UTC millis, integer).
    pub ts_field: String,
    /// The shared fetch/budget knobs.
    pub fetch: SharedFetchConfig,
}

impl fmt::Debug for VendorWeatherConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Manual Debug: the token renders redacted via VendorToken's Debug;
        // every other field is configuration, not a secret.
        f.debug_struct("VendorWeatherConfig")
            .field("base_url", &self.base_url)
            .field("forecast_path", &self.forecast_path)
            .field("token", &self.token)
            .field("field_map", &self.field_map)
            .field("unit_conversions", &self.unit_conversions)
            .field("provenance", &self.provenance.as_str())
            .field("source_ref", &self.source_ref)
            .field("series_field", &self.series_field)
            .field("ts_field", &self.ts_field)
            .field("fetch", &self.fetch)
            .finish()
    }
}

impl VendorWeatherConfig {
    /// Fail-closed construction (see the field docs for each rule).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_url: String,
        forecast_path: String,
        token: VendorToken,
        field_map: BTreeMap<String, String>,
        unit_conversions: BTreeMap<String, UnitConversion>,
        provenance: WeatherDataProvenance,
        source_ref: String,
        series_field: String,
        ts_field: String,
        fetch: SharedFetchConfig,
    ) -> Result<Self, VendorIngestionError> {
        if !base_url.starts_with("https://") && !base_url.starts_with("http://") {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_URL.to_string(),
                reason: "the base URL must start with https:// (or http:// for a \
                         test-lab endpoint); a scheme-less base URL is refused"
                    .to_string(),
            });
        }
        if base_url.ends_with('/') {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_URL.to_string(),
                reason: "the base URL must not end with '/'; the endpoint path is \
                         appended verbatim"
                    .to_string(),
            });
        }
        if !forecast_path.starts_with('/') {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_FORECAST_PATH.to_string(),
                reason: "the endpoint path must start with '/'".to_string(),
            });
        }
        if source_ref.trim().is_empty() {
            return Err(VendorIngestionError::InvalidConfig {
                field: "source_ref".to_string(),
                reason: "the source REFERENCE must be non-empty (provenance travels \
                         with every value; never credentials)"
                    .to_string(),
            });
        }
        if series_field.trim().is_empty() || ts_field.trim().is_empty() {
            return Err(VendorIngestionError::InvalidConfig {
                field: "series_field/ts_field".to_string(),
                reason: "the response field names must be non-empty".to_string(),
            });
        }
        Ok(Self {
            base_url,
            forecast_path,
            token,
            field_map,
            unit_conversions,
            provenance,
            source_ref,
            series_field,
            ts_field,
            fetch,
        })
    }

    /// Composition-root construction from a variable lookup (pure; the
    /// production `from_env` delegates with `std::env::var`). Fail-closed:
    /// an absent/blank token variable is the named
    /// `TokenNotRegistered` refusal.
    pub fn from_lookup(
        lookup: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, VendorIngestionError> {
        let base_url = lookup(ENV_VENDOR_WEATHER_URL).ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_URL.to_string(),
                reason: "required: the weather feed base URL (vendor-generic; the \
                         P15-E-1 decision names the vendor in configuration only)"
                    .to_string(),
            }
        })?;
        let token_raw = lookup(ENV_VENDOR_WEATHER_TOKEN).filter(|t| !t.trim().is_empty());
        let token = VendorToken::new(token_raw.ok_or(VendorIngestionError::TokenNotRegistered {
            variable: ENV_VENDOR_WEATHER_TOKEN,
        })?)?;
        let forecast_path = lookup(ENV_VENDOR_WEATHER_FORECAST_PATH)
            .unwrap_or_else(|| "/v1/forecast".to_string());
        let series_field =
            lookup(ENV_VENDOR_SERIES_FIELD).unwrap_or_else(|| "series".to_string());
        let ts_field =
            lookup(ENV_VENDOR_WEATHER_TS_FIELD).unwrap_or_else(|| "valid_at".to_string());
        let source_ref = lookup("FLEXCORE_VENDOR_WEATHER_SOURCE_REF").ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: "FLEXCORE_VENDOR_WEATHER_SOURCE_REF".to_string(),
                reason: "required: the provider dataset/query reference carried on \
                         every sample (a REFERENCE, never credentials)"
                    .to_string(),
            }
        })?;
        let provenance = match lookup(ENV_VENDOR_WEATHER_PROVENANCE)
            .unwrap_or_else(|| "commercial-service".to_string())
            .as_str()
        {
            "commercial-service" => WeatherDataProvenance::CommercialService,
            "site-measured" => WeatherDataProvenance::SiteMeasured,
            "public-feed" => WeatherDataProvenance::PublicFeed,
            other => {
                return Err(VendorIngestionError::InvalidConfig {
                    field: ENV_VENDOR_WEATHER_PROVENANCE.to_string(),
                    reason: format!(
                        "unknown provenance class '{other}'; the closed vocabulary is \
                         commercial-service | site-measured | public-feed"
                    ),
                })
            }
        };
        let field_map_raw = lookup(ENV_VENDOR_WEATHER_FIELD_MAP).unwrap_or_else(|| "{}".to_string());
        let field_map_json: serde_json::Value =
            serde_json::from_str(&field_map_raw).map_err(|error| {
                VendorIngestionError::InvalidConfig {
                    field: ENV_VENDOR_WEATHER_FIELD_MAP.to_string(),
                    reason: format!("not valid JSON: {error}"),
                }
            })?;
        let mut field_map = BTreeMap::new();
        for (vendor_field, wire) in field_map_json
            .as_object()
            .ok_or_else(|| VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_FIELD_MAP.to_string(),
                reason: "must be a JSON object".to_string(),
            })?
            .iter()
        {
            let wire = wire.as_str().ok_or_else(|| VendorIngestionError::InvalidConfig {
                field: format!("{ENV_VENDOR_WEATHER_FIELD_MAP}.{vendor_field}"),
                reason: "the wire name must be a string".to_string(),
            })?;
            field_map.insert(vendor_field.clone(), wire.to_string());
        }
        let factors_raw =
            lookup(ENV_VENDOR_WEATHER_UNIT_FACTORS).unwrap_or_else(|| "{}".to_string());
        let factors_json: serde_json::Value =
            serde_json::from_str(&factors_raw).map_err(|error| {
                VendorIngestionError::InvalidConfig {
                    field: ENV_VENDOR_WEATHER_UNIT_FACTORS.to_string(),
                    reason: format!("not valid JSON: {error}"),
                }
            })?;
        let mut unit_conversions = BTreeMap::new();
        for (wire_name, entry) in factors_json
            .as_object()
            .ok_or_else(|| VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_WEATHER_UNIT_FACTORS.to_string(),
                reason: "must be a JSON object keyed by wire name".to_string(),
            })?
            .iter()
        {
            unit_conversions.insert(
                wire_name.clone(),
                parse_unit_conversion(wire_name, entry)?,
            );
        }
        let fetch = SharedFetchConfig::new(
            lookup(ENV_VENDOR_REFRESH_WINDOW_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(900_000),
            lookup(ENV_VENDOR_STALENESS_WINDOW_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(3_600_000),
            lookup(ENV_VENDOR_TIMEOUT_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(10_000),
            lookup(ENV_VENDOR_MAX_ATTEMPTS)
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(3),
        )?;
        Self::new(
            base_url.trim().to_string(),
            forecast_path,
            token,
            field_map,
            unit_conversions,
            provenance,
            source_ref,
            series_field,
            ts_field,
            fetch,
        )
    }

    /// Composition-root construction from the process environment. The
    /// fail-closed entry: without `FLEXCORE_VENDOR_WEATHER_TOKEN` this
    /// refuses by name and the adapter cannot be constructed.
    pub fn from_env() -> Result<Self, VendorIngestionError> {
        Self::from_lookup(&|name| std::env::var(name).ok())
    }
}

/// One fetched weather record in the cache: the validated samples, the
/// fetch coverage window, and the fetch metadata (fetched-at, unit factor
/// provenance per quantity — provenance travels with the values).
#[derive(Debug, Clone, PartialEq)]
struct CachedWeatherRecord {
    /// The validated samples, in valid-at order, quality as classified at
    /// the FETCH instant (reads re-classify by record age).
    samples: Vec<WeatherSample>,
    /// The requested coverage window `[from, until)` (UTC millis).
    coverage_from_millis: i64,
    coverage_until_millis: i64,
    /// The fetch instant (the injected clock read at refresh completion,
    /// UTC millis).
    fetched_at_millis: i64,
    /// The declared conversion the values went through (per quantity:
    /// exactly one entry — the requested quantity's).
    conversion: UnitConversion,
    /// The wire name the quantity was fetched under.
    wire_name: String,
}

impl CachedWeatherRecord {
    /// Re-renders the cached samples with the CURRENT record-age
    /// classification (the port carries the truth: a record past the
    /// staleness window reads STALE, never silently GOOD).
    fn samples_with_quality(&self, now_millis: i64, staleness_window_ms: i64) -> Vec<WeatherSample> {
        let age = (i128::from(now_millis) - i128::from(self.fetched_at_millis)).max(0);
        let quality = classify_record_quality(
            i64::try_from(age).unwrap_or(i64::MAX),
            staleness_window_ms,
        );
        self.samples
            .iter()
            .map(|sample| {
                WeatherSample::new(
                    sample.site_id,
                    sample.quantity,
                    sample.value,
                    sample.valid_at,
                    sample.provenance,
                    sample.source_ref.clone(),
                    quality,
                )
                .expect("a cached sample re-wrapped with a fresh quality stays valid")
            })
            .collect()
    }
}

/// The named code a cache-miss read reports (surfaced in the
/// `WeatherPortError::ProviderDataRefused` reason so a log labels the
/// miss by name).
pub const VENDOR_WEATHER_CACHE_NOT_COVERED: &str = "VENDOR_WEATHER_CACHE_NOT_COVERED";

/// The vendor-generic weather ingestion adapter: implements the EXISTING
/// `WeatherDataProvider` port (read paths over the local cache) and adds
/// the explicit `refresh_weather` fetch path (bounded attempts, refresh
/// budget, timeout) a scheduled loop calls. Construction WITHOUT a
/// registered token is impossible — [`VendorWeatherConfig::from_lookup`]
/// refuses before this type exists.
pub struct VendorWeatherAdapter {
    config: VendorWeatherConfig,
    cache: Mutex<Option<CachedWeatherRecord>>,
    clock: FetchClock,
    client: reqwest::blocking::Client,
}

impl fmt::Debug for VendorWeatherAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VendorWeatherAdapter")
            .field("config", &self.config)
            .field("cached", &self.cache.lock().map(|c| c.is_some()))
            .finish_non_exhaustive()
    }
}

/// The outcome of one refresh attempt (named outcomes, never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// The fetch completed; the named count of records landed in the
    /// cache (weather: samples; price: one series), fetched at the given
    /// instant.
    Refreshed {
        records: usize,
        fetched_at_millis: i64,
    },
    /// The refresh was not due: the previous attempt is inside the
    /// refresh window; retry no earlier than the given UTC millis (the
    /// named skip — never a silent no-op).
    NotDue { retry_after_millis: i64 },
}

impl VendorWeatherAdapter {
    /// Constructs the adapter over an explicit config and injected clock.
    /// (The token already passed `VendorToken::new` inside the config —
    /// construction without a registered token is structurally
    /// unreachable.)
    pub fn new(config: VendorWeatherConfig, clock: FetchClock) -> Result<Self, VendorIngestionError> {
        // Batch 68: the reqwest client needs a crypto provider installed
        // before it is built (see `ensure_rustls_crypto_provider`).
        ensure_rustls_crypto_provider();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(
                u64::try_from(config.fetch.timeout_ms).unwrap_or(u64::MAX),
            ))
            .build()
            .map_err(|error| VendorIngestionError::InvalidConfig {
                field: "http_client".to_string(),
                reason: format!("the HTTP client could not be built: {error}"),
            })?;
        Ok(Self {
            config,
            cache: Mutex::new(None),
            clock,
            client,
        })
    }

    /// The declared conversion for a wire name (named refusal when the
    /// deployment did not declare one — no default conversion exists).
    fn conversion_for(&self, wire_name: &str) -> Result<&UnitConversion, VendorIngestionError> {
        self.config
            .unit_conversions
            .get(wire_name)
            .ok_or_else(|| VendorIngestionError::InvalidConfig {
                field: format!("{ENV_VENDOR_WEATHER_UNIT_FACTORS}.{wire_name}"),
                reason: "no declared unit conversion for this quantity; a quantity \
                         without a DECLARED factor is refused — there is no default \
                         conversion and no silent passthrough"
                    .to_string(),
            })
    }

    /// The vendor field name mapped for a wire name (named refusal when
    /// unmapped).
    fn vendor_field_for(&self, wire_name: &str) -> Result<&str, VendorIngestionError> {
        self.config
            .field_map
            .iter()
            .find(|(_, wire)| wire.as_str() == wire_name)
            .map(|(vendor, _)| vendor.as_str())
            .ok_or_else(|| VendorIngestionError::InvalidConfig {
                field: format!("{ENV_VENDOR_WEATHER_FIELD_MAP} → {wire_name}"),
                reason: "no vendor field is mapped to this quantity's wire name; the \
                         mapping is configuration, never inferred"
                    .to_string(),
            })
    }

    /// The fetch path (FIM-F10.01): bounded attempts with deterministic
    /// backoff, refresh-window budget, per-attempt timeout; on success the
    /// parsed samples land in the cache with fetch provenance. On failure
    /// the cache is left UNCHANGED (a failed fetch never touches cached
    /// values) and the error is named — never fabricated data.
    pub fn refresh_weather(
        &self,
        site_id: &SiteId,
        quantity: WeatherQuantity,
        valid_from_millis: i64,
        valid_until_millis: i64,
    ) -> Result<RefreshOutcome, VendorIngestionError> {
        if valid_from_millis >= valid_until_millis {
            return Err(VendorIngestionError::InvalidConfig {
                field: "window".to_string(),
                reason: format!(
                    "the requested window [{valid_from_millis}, {valid_until_millis}) is \
                     inverted or empty; start must be before end (UTC millis)"
                ),
            });
        }
        let now = (self.clock)();
        // The refresh-window budget: a refresh inside the window is the
        // named NotDue outcome (never a silent skip).
        if let Ok(guard) = self.cache.lock() {
            if let Some(record) = guard.as_ref() {
                let since_attempt = now - record.fetched_at_millis;
                if since_attempt >= 0 && since_attempt < self.config.fetch.refresh_window_ms {
                    return Ok(RefreshOutcome::NotDue {
                        retry_after_millis: record.fetched_at_millis
                            + self.config.fetch.refresh_window_ms,
                    });
                }
            }
        }
        let wire_name = quantity.wire_name();
        // Configuration completeness is checked BEFORE any HTTP attempt
        // (fail-fast, the config.rs posture).
        let vendor_field = self.vendor_field_for(wire_name)?.to_string();
        let conversion = self.conversion_for(wire_name)?.clone();
        let url = format!("{}{}", self.config.base_url, self.config.forecast_path);
        let mut last_status_class = String::from("no-attempt");
        let mut response_body: Option<Vec<u8>> = None;
        for attempt in 1..=self.config.fetch.max_attempts {
            if attempt > 1 {
                // Deterministic bounded backoff (the P7.18 scheduler's
                // shape; no jitter — vendor polling stays reproducible).
                std::thread::sleep(Duration::from_millis(
                    self.config.fetch.backoff_delay_millis(attempt),
                ));
            }
            let request = self
                .client
                .get(&url)
                .query(&[
                    ("site", site_id.uuid().to_string()),
                    ("quantity", wire_name.to_string()),
                    ("from", valid_from_millis.to_string()),
                    ("until", valid_until_millis.to_string()),
                ])
                .header("Authorization", format!("Bearer {}", self.config.token.0))
                .header("Accept", "application/json");
            match request.send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        // The per-attempt timeout IS the read budget: the
                        // body read is bounded by it; no unbounded
                        // buffering beyond what the timeout permits.
                        match response.bytes() {
                            Ok(body) => {
                                response_body = Some(body.to_vec());
                                last_status_class = "success".to_string();
                                break;
                            }
                            Err(_) => last_status_class = "body-read-error".to_string(),
                        }
                    } else if status.is_client_error() {
                        // A 4xx is a configuration/authorization problem —
                        // retrying cannot fix it; fail immediately (the
                        // budget is not burned on a deterministic failure).
                        last_status_class = format!("client-error-{status}");
                        break;
                    } else {
                        last_status_class = format!("status-{status}");
                    }
                }
                Err(error) => {
                    // Transport-level failure (timeout, connect, ...):
                    // opaque classification, never the raw error text (it
                    // can embed URLs with query strings).
                    last_status_class = if error.is_timeout() {
                        "timeout".to_string()
                    } else if error.is_connect() {
                        "connect".to_string()
                    } else {
                        "transport".to_string()
                    };
                }
            }
        }
        let body = response_body.ok_or(VendorIngestionError::FetchFailed {
            attempts: self.config.fetch.max_attempts,
            status_class: last_status_class,
        })?;
        let record = self.parse_weather_response(
            &body,
            site_id,
            quantity,
            &vendor_field,
            &conversion,
            valid_from_millis,
            valid_until_millis,
            now,
        )?;
        let samples = record.samples.len();
        *self
            .cache
            .lock()
            .map_err(|_| VendorIngestionError::InvalidConfig {
                field: "cache".to_string(),
                reason: "the cache lock was poisoned".to_string(),
            })? = Some(record);
        Ok(RefreshOutcome::Refreshed {
            records: samples,
            fetched_at_millis: now,
        })
    }

    /// Parses the response body into the cached record. The shape contract
    /// (documented, vendor-generic): a JSON object whose `series_field`
    /// holds an array of row objects, each carrying `ts_field` (epoch UTC
    /// millis, integer) and the mapped vendor field (JSON number). The
    /// WHOLE response is refused on any violation — nothing is partially
    /// accepted. `fetched_at_millis` is the caller's injected-clock read
    /// (the parse itself reads no clock).
    #[allow(clippy::too_many_arguments)]
    fn parse_weather_response(
        &self,
        body: &[u8],
        site_id: &SiteId,
        quantity: WeatherQuantity,
        vendor_field: &str,
        conversion: &UnitConversion,
        valid_from_millis: i64,
        valid_until_millis: i64,
        fetched_at_millis: i64,
    ) -> Result<CachedWeatherRecord, VendorIngestionError> {
        let document: serde_json::Value =
            serde_json::from_slice(body).map_err(|error| VendorIngestionError::ParseRefused {
                reason: format!("the response is not a valid JSON document: {error}"),
            })?;
        let rows = document
            .get(&self.config.series_field)
            .and_then(|v| v.as_array())
            .ok_or_else(|| VendorIngestionError::ParseRefused {
                reason: format!(
                    "the response carries no '{}' array; the shape contract requires it",
                    self.config.series_field
                ),
            })?;
        if rows.is_empty() {
            return Err(VendorIngestionError::ParseRefused {
                reason: "the response series is empty; an empty series is a data error, \
                         never an accepted observation set"
                    .to_string(),
            });
        }
        let mut samples = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let ts = row
                .get(&self.config.ts_field)
                .and_then(|v| v.as_i64())
                .ok_or_else(|| VendorIngestionError::ParseRefused {
                    reason: format!(
                        "row {index} carries no integer '{}' timestamp",
                        self.config.ts_field
                    ),
                })?;
            let raw_value = row
                .get(vendor_field)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| VendorIngestionError::ParseRefused {
                    reason: format!(
                        "row {index} carries no numeric '{vendor_field}' value (the \
                         configured vendor mapping for '{}')",
                        quantity.wire_name()
                    ),
                })?;
            // EXPLICIT unit conversion through the DECLARED factor, with
            // the factor's provenance recorded on the record below — the
            // conversion is never silent, never skipped.
            let canonical_value = conversion.convert(raw_value);
            let valid_at =
                EventTime::from_unix_millis(ts).map_err(|_| VendorIngestionError::ParseRefused {
                    reason: format!("row {index} timestamp {ts} is not representable"),
                })?;
            let sample = WeatherSample::new(
                *site_id,
                quantity,
                canonical_value,
                valid_at,
                self.config.provenance,
                self.config.source_ref.clone(),
                TelemetryQuality::Good,
            )
            .map_err(|error| VendorIngestionError::ParseRefused {
                reason: format!(
                    "row {index} failed sample validation ({}: {error}); the whole \
                     response is refused, never trimmed",
                    error.name()
                ),
            })?;
            // A row outside the requested coverage window is a contract
            // violation (the vendor must respect the requested window).
            if ts < valid_from_millis || ts >= valid_until_millis {
                return Err(VendorIngestionError::ParseRefused {
                    reason: format!(
                        "row {index} timestamp {ts} is outside the requested window \
                         [{valid_from_millis}, {valid_until_millis})"
                    ),
                });
            }
            samples.push(sample);
        }
        Ok(CachedWeatherRecord {
            samples,
            coverage_from_millis: valid_from_millis,
            coverage_until_millis: valid_until_millis,
            fetched_at_millis,
            conversion: conversion.clone(),
            wire_name: quantity.wire_name().to_string(),
        })
    }
}

impl WeatherDataProvider for VendorWeatherAdapter {
    /// The latest cached observation for one site and quantity: the sample
    /// with the greatest valid-at at or before the current clock read,
    /// re-classified by record age (GOOD inside the staleness window,
    /// STALE past it). An absent quantity is `Ok(None)` — MISSING is never
    /// fabricated (the frozen rule).
    fn latest_observation(
        &self,
        _ctx: &TenantContext,
        _site_id: &SiteId,
        quantity: WeatherQuantity,
    ) -> Result<Option<WeatherSample>, WeatherPortError> {
        let guard = self.cache.lock().map_err(|_| WeatherPortError::ProviderUnavailable {
            code: "VENDOR_CACHE_POISONED".to_string(),
            message: "the weather cache lock was poisoned".to_string(),
        })?;
        let record = match guard.as_ref() {
            Some(record) => record,
            None => return Ok(None), // absent: never fabricated
        };
        if record.wire_name != quantity.wire_name() {
            return Ok(None); // a different quantity's record: absent for this one
        }
        let now_millis = (self.clock)();
        let mut latest: Option<&WeatherSample> = None;
        for sample in &record.samples {
            if sample.valid_at.unix_millis() <= i128::from(now_millis)
                && latest
                    .map(|l| sample.valid_at.unix_millis() > l.valid_at.unix_millis())
                    .unwrap_or(true)
            {
                latest = Some(sample);
            }
        }
        let sample = match latest {
            Some(sample) => sample,
            None => return Ok(None), // only future-valid rows cached: no observation exists
        };
        let age = (i128::from(now_millis) - i128::from(record.fetched_at_millis)).max(0);
        let quality = classify_record_quality(
            i64::try_from(age).unwrap_or(i64::MAX),
            self.config.fetch.staleness_window_ms,
        );
        Ok(Some(
            WeatherSample::new(
                sample.site_id,
                sample.quantity,
                sample.value,
                sample.valid_at,
                sample.provenance,
                sample.source_ref.clone(),
                quality,
            )
            .map_err(|error| WeatherPortError::ProviderDataRefused {
                reason: format!("the cached sample failed re-validation: {}", error.name()),
            })?,
        ))
    }

    /// The cached forecast series for the requested window: the whole
    /// window must be COVERED by the fetch (coverage is the fetch's
    /// requested window — a partial read is a named refusal, never a
    /// trimmed series). Samples arrive in valid-at order with the current
    /// record-age classification.
    fn forecast_series(
        &self,
        _ctx: &TenantContext,
        _site_id: &SiteId,
        quantity: WeatherQuantity,
        valid_from_millis: i64,
        valid_until_millis: i64,
    ) -> Result<Vec<WeatherSample>, WeatherPortError> {
        if valid_from_millis >= valid_until_millis {
            return Err(WeatherPortError::InvalidWindow {
                from_millis: valid_from_millis,
                until_millis: valid_until_millis,
            });
        }
        let guard = self.cache.lock().map_err(|_| WeatherPortError::ProviderUnavailable {
            code: "VENDOR_CACHE_POISONED".to_string(),
            message: "the weather cache lock was poisoned".to_string(),
        })?;
        let record = match guard.as_ref() {
            Some(record) => record,
            None => {
                return Err(WeatherPortError::ProviderDataRefused {
                    reason: format!(
                        "{VENDOR_WEATHER_CACHE_NOT_COVERED}: no fetch has landed for \
                         this adapter; refresh first — a missing fetch is never answered \
                         with synthetic weather"
                    ),
                })
            }
        };
        if record.wire_name != quantity.wire_name() {
            return Err(WeatherPortError::ProviderDataRefused {
                reason: format!(
                    "{VENDOR_WEATHER_CACHE_NOT_COVERED}: the cache holds '{}' while \
                     '{}' was requested; one fetch serves one quantity",
                    record.wire_name,
                    quantity.wire_name()
                ),
            });
        }
        if valid_from_millis < record.coverage_from_millis
            || valid_until_millis > record.coverage_until_millis
        {
            return Err(WeatherPortError::ProviderDataRefused {
                reason: format!(
                    "{VENDOR_WEATHER_CACHE_NOT_COVERED}: the requested window \
                     [{valid_from_millis}, {valid_until_millis}) exceeds the fetched \
                     coverage [{}, {}); re-fetch a wider window — never partial data",
                    record.coverage_from_millis, record.coverage_until_millis
                ),
            });
        }
        let now_millis = (self.clock)();
        Ok(record.samples_with_quality(now_millis, self.config.fetch.staleness_window_ms))
    }
}

// ---------------------------------------------------------------------------
// 7. The price adapter (FIM-F10.01): config-driven, vendor-generic,
//    implements the EXISTING `PriceSignalSource` port shape.
// ---------------------------------------------------------------------------

/// The price-feed configuration (all vendor-specificity as fields).
#[derive(Clone)]
pub struct VendorPriceConfig {
    /// The base URL (scheme + host, no trailing path).
    pub base_url: String,
    /// The price endpoint path appended to the base URL.
    pub price_path: String,
    /// The registered token (HUMAN GATE input).
    pub token: VendorToken,
    /// The source class the ingested series carry (closed vocabulary).
    pub source_class: PriceSourceClass,
    /// The series kind this feed delivers (Actual or VendorForecast —
    /// never conflated; one feed serves one kind).
    pub kind: PriceSeriesKind,
    /// The DECLARED currency code (carried verbatim; no default currency).
    pub currency_code: String,
    /// The delivery area / bidding-zone id (the source's vocabulary).
    pub area: String,
    /// The source REFERENCE (provider dataset/query identity; never
    /// credentials).
    pub source_ref: String,
    /// The source document reference (the published document/auction
    /// identity the series was extracted from).
    pub document_ref: String,
    /// The response series-field name.
    pub series_field: String,
    /// The row interval-start field name (epoch UTC millis).
    pub start_field: String,
    /// The row interval-end field name (epoch UTC millis).
    pub end_field: String,
    /// The row price-field name (number, declared currency per MWh).
    pub price_field: String,
    /// The shared fetch/budget knobs.
    pub fetch: SharedFetchConfig,
}

impl fmt::Debug for VendorPriceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VendorPriceConfig")
            .field("base_url", &self.base_url)
            .field("price_path", &self.price_path)
            .field("token", &self.token)
            .field("source_class", &self.source_class.as_str())
            .field("kind", &self.kind.as_str())
            .field("currency_code", &self.currency_code)
            .field("area", &self.area)
            .field("source_ref", &self.source_ref)
            .field("document_ref", &self.document_ref)
            .field("series_field", &self.series_field)
            .field("start_field", &self.start_field)
            .field("end_field", &self.end_field)
            .field("price_field", &self.price_field)
            .field("fetch", &self.fetch)
            .finish()
    }
}

impl VendorPriceConfig {
    /// Fail-closed construction (see the field docs for each rule).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_url: String,
        price_path: String,
        token: VendorToken,
        source_class: PriceSourceClass,
        kind: PriceSeriesKind,
        currency_code: String,
        area: String,
        source_ref: String,
        document_ref: String,
        series_field: String,
        start_field: String,
        end_field: String,
        price_field: String,
        fetch: SharedFetchConfig,
    ) -> Result<Self, VendorIngestionError> {
        if !base_url.starts_with("https://") && !base_url.starts_with("http://") {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_PRICE_URL.to_string(),
                reason: "the base URL must start with https:// (or http:// for a \
                         test-lab endpoint)".to_string(),
            });
        }
        if base_url.ends_with('/') {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_PRICE_URL.to_string(),
                reason: "the base URL must not end with '/'".to_string(),
            });
        }
        if !price_path.starts_with('/') {
            return Err(VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_PRICE_PATH.to_string(),
                reason: "the endpoint path must start with '/'".to_string(),
            });
        }
        for (label, value) in [
            ("currency_code", currency_code.as_str()),
            ("area", area.as_str()),
            ("source_ref", source_ref.as_str()),
            ("document_ref", document_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(VendorIngestionError::InvalidConfig {
                    field: label.to_string(),
                    reason: "the currency, area, source reference, and document \
                             reference are all REQUIRED (a currency-less price series \
                             is unconstructable per the port)"
                        .to_string(),
                });
            }
        }
        if series_field.trim().is_empty()
            || start_field.trim().is_empty()
            || end_field.trim().is_empty()
            || price_field.trim().is_empty()
        {
            return Err(VendorIngestionError::InvalidConfig {
                field: "response field names".to_string(),
                reason: "the response field names must be non-empty".to_string(),
            });
        }
        Ok(Self {
            base_url,
            price_path,
            token,
            source_class,
            kind,
            currency_code,
            area,
            source_ref,
            document_ref,
            series_field,
            start_field,
            end_field,
            price_field,
            fetch,
        })
    }

    /// Composition-root construction from a variable lookup (pure). The
    /// series kind rides `FLEXCORE_VENDOR_PRICE_KIND` (`actual` |
    /// `vendor-forecast`); the fail-closed token gate mirrors the weather
    /// adapter.
    pub fn from_lookup(
        lookup: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, VendorIngestionError> {
        let base_url = lookup(ENV_VENDOR_PRICE_URL).ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_PRICE_URL.to_string(),
                reason: "required: the price feed base URL (vendor-generic; the \
                         P15-E-2 decision names the vendor in configuration only)"
                    .to_string(),
            }
        })?;
        let token_raw = lookup(ENV_VENDOR_PRICE_TOKEN).filter(|t| !t.trim().is_empty());
        let token = VendorToken::new(token_raw.ok_or(VendorIngestionError::TokenNotRegistered {
            variable: ENV_VENDOR_PRICE_TOKEN,
        })?)?;
        let price_path =
            lookup(ENV_VENDOR_PRICE_PATH).unwrap_or_else(|| "/v1/day-ahead".to_string());
        let series_field =
            lookup(ENV_VENDOR_SERIES_FIELD).unwrap_or_else(|| "series".to_string());
        let start_field = lookup(ENV_VENDOR_PRICE_START_FIELD)
            .unwrap_or_else(|| "interval_start".to_string());
        let end_field =
            lookup(ENV_VENDOR_PRICE_END_FIELD).unwrap_or_else(|| "interval_end".to_string());
        let price_field = lookup(ENV_VENDOR_PRICE_FIELD).unwrap_or_else(|| "price".to_string());
        let currency_code = lookup(ENV_VENDOR_PRICE_CURRENCY).ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: ENV_VENDOR_PRICE_CURRENCY.to_string(),
                reason: "required: the DECLARED currency code (no default currency \
                         exists anywhere in the platform)"
                    .to_string(),
            }
        })?;
        let area = lookup(ENV_VENDOR_PRICE_AREA).ok_or_else(|| VendorIngestionError::InvalidConfig {
            field: ENV_VENDOR_PRICE_AREA.to_string(),
            reason: "required: the delivery area / bidding-zone id".to_string(),
        })?;
        let source_ref = lookup("FLEXCORE_VENDOR_PRICE_SOURCE_REF").ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: "FLEXCORE_VENDOR_PRICE_SOURCE_REF".to_string(),
                reason: "required: the provider dataset/query reference".to_string(),
            }
        })?;
        let document_ref = lookup("FLEXCORE_VENDOR_PRICE_DOCUMENT_REF").ok_or_else(|| {
            VendorIngestionError::InvalidConfig {
                field: "FLEXCORE_VENDOR_PRICE_DOCUMENT_REF".to_string(),
                reason: "required: the source document reference".to_string(),
            }
        })?;
        let kind = match lookup("FLEXCORE_VENDOR_PRICE_KIND")
            .unwrap_or_else(|| "actual".to_string())
            .as_str()
        {
            "actual" => PriceSeriesKind::Actual,
            "vendor-forecast" => PriceSeriesKind::VendorForecast,
            other => {
                return Err(VendorIngestionError::InvalidConfig {
                    field: "FLEXCORE_VENDOR_PRICE_KIND".to_string(),
                    reason: format!(
                        "unknown series kind '{other}'; the closed vocabulary is \
                         actual | vendor-forecast"
                    ),
                })
            }
        };
        let source_class = match lookup(ENV_VENDOR_PRICE_SOURCE_CLASS)
            .unwrap_or_else(|| "commercial-vendor".to_string())
            .as_str()
        {
            "transparency-platform" => PriceSourceClass::TransparencyPlatform,
            "commercial-vendor" => PriceSourceClass::CommercialVendor,
            other => {
                return Err(VendorIngestionError::InvalidConfig {
                    field: ENV_VENDOR_PRICE_SOURCE_CLASS.to_string(),
                    reason: format!(
                        "unknown source class '{other}'; the closed vocabulary is \
                         transparency-platform | commercial-vendor"
                    ),
                })
            }
        };
        let fetch = SharedFetchConfig::new(
            lookup(ENV_VENDOR_REFRESH_WINDOW_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(900_000),
            lookup(ENV_VENDOR_STALENESS_WINDOW_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(3_600_000),
            lookup(ENV_VENDOR_TIMEOUT_MS)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(10_000),
            lookup(ENV_VENDOR_MAX_ATTEMPTS)
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(3),
        )?;
        Self::new(
            base_url.trim().to_string(),
            price_path,
            token,
            source_class,
            kind,
            currency_code,
            area,
            source_ref,
            document_ref,
            series_field,
            start_field,
            end_field,
            price_field,
            fetch,
        )
    }

    /// Composition-root construction from the process environment.
    pub fn from_env() -> Result<Self, VendorIngestionError> {
        Self::from_lookup(&|name| std::env::var(name).ok())
    }
}

/// One ingested price series in the cache: the validated series plus its
/// fetch provenance and the CURRENT quality classification (the price port
/// has no quality field — the classification travels on THIS wrapper, the
/// adapter's additive record; the trait shape is frozen and untouched).
#[derive(Debug, Clone, PartialEq)]
pub struct CachedPriceRecord {
    /// The validated series (the port's own shape).
    pub series: PriceSeries,
    /// The fetch/ingest instant (the injected clock read at arrival, UTC
    /// millis).
    pub fetched_at_millis: i64,
    /// The quality classified at the LATEST read (re-computed by
    /// `quality_at`); stored here as classified at arrival.
    pub quality: TelemetryQuality,
}

impl CachedPriceRecord {
    /// The record's age classification at `now_millis` (GOOD inside the
    /// staleness window, STALE past it — the P2.08 convention).
    pub fn quality_at(&self, now_millis: i64, staleness_window_ms: i64) -> TelemetryQuality {
        let age = (i128::from(now_millis) - i128::from(self.fetched_at_millis)).max(0);
        classify_record_quality(i64::try_from(age).unwrap_or(i64::MAX), staleness_window_ms)
    }
}

/// The vendor-generic price ingestion adapter: implements the EXISTING
/// `PriceSignalSource` port (reads over the local cache; `ingest_series`
/// is the delivery path) and adds the explicit `refresh_price` fetch path.
/// Construction WITHOUT a registered token is impossible.
pub struct VendorPriceAdapter {
    config: VendorPriceConfig,
    cache: Mutex<Option<CachedPriceRecord>>,
    clock: FetchClock,
    client: reqwest::blocking::Client,
}

impl fmt::Debug for VendorPriceAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VendorPriceAdapter")
            .field("config", &self.config)
            .field("cached", &self.cache.lock().map(|c| c.is_some()))
            .finish_non_exhaustive()
    }
}

impl VendorPriceAdapter {
    /// Constructs the adapter over an explicit config and injected clock.
    pub fn new(config: VendorPriceConfig, clock: FetchClock) -> Result<Self, VendorIngestionError> {
        // Batch 68: the reqwest client needs a crypto provider installed
        // before it is built (see `ensure_rustls_crypto_provider`).
        ensure_rustls_crypto_provider();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(
                u64::try_from(config.fetch.timeout_ms).unwrap_or(u64::MAX),
            ))
            .build()
            .map_err(|error| VendorIngestionError::InvalidConfig {
                field: "http_client".to_string(),
                reason: format!("the HTTP client could not be built: {error}"),
            })?;
        Ok(Self {
            config,
            cache: Mutex::new(None),
            clock,
            client,
        })
    }

    /// The additive provenance-carrying record read (the trait shape is
    /// frozen; the staleness classification travels HERE): the cached
    /// series with its fetch instant and CURRENT quality classification.
    /// Absent ⇒ `Ok(None)` — never a fabricated series.
    pub fn latest_ingested_price_record(
        &self,
        now_millis: i64,
    ) -> Result<Option<CachedPriceRecord>, VendorIngestionError> {
        let guard = self.cache.lock().map_err(|_| VendorIngestionError::InvalidConfig {
            field: "cache".to_string(),
            reason: "the cache lock was poisoned".to_string(),
        })?;
        Ok(guard.as_ref().map(|record| {
            let mut record = record.clone();
            record.quality = record.quality_at(now_millis, self.config.fetch.staleness_window_ms);
            record
        }))
    }

    /// The fetch path: bounded attempts with deterministic backoff, the
    /// refresh-window budget, and the per-attempt timeout; on success the
    /// parsed series lands in the cache via the SAME delivery path the
    /// trait's `ingest_series` uses (one ingest path, no parallel route —
    /// the P17.07 seam rule's posture).
    pub fn refresh_price(&self) -> Result<RefreshOutcome, VendorIngestionError> {
        let now = (self.clock)();
        if let Ok(guard) = self.cache.lock() {
            if let Some(record) = guard.as_ref() {
                let since_attempt = now - record.fetched_at_millis;
                if since_attempt >= 0 && since_attempt < self.config.fetch.refresh_window_ms {
                    return Ok(RefreshOutcome::NotDue {
                        retry_after_millis: record.fetched_at_millis
                            + self.config.fetch.refresh_window_ms,
                    });
                }
            }
        }
        let url = format!("{}{}", self.config.base_url, self.config.price_path);
        let mut last_status_class = String::from("no-attempt");
        let mut response_body: Option<Vec<u8>> = None;
        for attempt in 1..=self.config.fetch.max_attempts {
            if attempt > 1 {
                std::thread::sleep(Duration::from_millis(
                    self.config.fetch.backoff_delay_millis(attempt),
                ));
            }
            let request = self
                .client
                .get(&url)
                .query(&[("area", self.config.area.clone())])
                .header("Authorization", format!("Bearer {}", self.config.token.0))
                .header("Accept", "application/json");
            match request.send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes() {
                            Ok(body) => {
                                response_body = Some(body.to_vec());
                                last_status_class = "success".to_string();
                                break;
                            }
                            Err(_) => last_status_class = "body-read-error".to_string(),
                        }
                    } else if status.is_client_error() {
                        last_status_class = format!("client-error-{status}");
                        break;
                    } else {
                        last_status_class = format!("status-{status}");
                    }
                }
                Err(error) => {
                    last_status_class = if error.is_timeout() {
                        "timeout".to_string()
                    } else if error.is_connect() {
                        "connect".to_string()
                    } else {
                        "transport".to_string()
                    };
                }
            }
        }
        let body = response_body.ok_or(VendorIngestionError::FetchFailed {
            attempts: self.config.fetch.max_attempts,
            status_class: last_status_class,
        })?;
        let series = self.parse_price_response(&body)?;
        self.ingest_record(series, now)
            .map_err(|error| VendorIngestionError::ParseRefused {
                reason: format!("the parsed series was refused at ingest: {error}"),
            })?;
        Ok(RefreshOutcome::Refreshed {
            records: 1,
            fetched_at_millis: now,
        })
    }

    /// Parses the response body into a validated `PriceSeries`. The shape
    /// contract: `series_field` → array of row objects carrying
    /// `start_field`/`end_field` (epoch UTC millis integers) and
    /// `price_field` (JSON number, declared currency per MWh). The WHOLE
    /// response is refused on any violation — including unsorted rows (the
    /// port refuses unsorted points, never silently sorts).
    fn parse_price_response(&self, body: &[u8]) -> Result<PriceSeries, VendorIngestionError> {
        let document: serde_json::Value =
            serde_json::from_slice(body).map_err(|error| VendorIngestionError::ParseRefused {
                reason: format!("the response is not a valid JSON document: {error}"),
            })?;
        let rows = document
            .get(&self.config.series_field)
            .and_then(|v| v.as_array())
            .ok_or_else(|| VendorIngestionError::ParseRefused {
                reason: format!(
                    "the response carries no '{}' array; the shape contract requires it",
                    self.config.series_field
                ),
            })?;
        if rows.is_empty() {
            return Err(VendorIngestionError::ParseRefused {
                reason: "the response series is empty; an ingested series is a series, \
                         not a null"
                    .to_string(),
            });
        }
        let mut points = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let start = row
                .get(&self.config.start_field)
                .and_then(|v| v.as_i64())
                .ok_or_else(|| VendorIngestionError::ParseRefused {
                    reason: format!("row {index} carries no integer '{}'", self.config.start_field),
                })?;
            let end = row
                .get(&self.config.end_field)
                .and_then(|v| v.as_i64())
                .ok_or_else(|| VendorIngestionError::ParseRefused {
                    reason: format!("row {index} carries no integer '{}'", self.config.end_field),
                })?;
            let price = row
                .get(&self.config.price_field)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| VendorIngestionError::ParseRefused {
                    reason: format!(
                        "row {index} carries no numeric '{}' price",
                        self.config.price_field
                    ),
                })?;
            let interval_start = EventTime::from_unix_millis(start)
                .map_err(|_| VendorIngestionError::ParseRefused {
                    reason: format!("row {index} interval start {start} is not representable"),
                })?;
            let interval_end =
                EventTime::from_unix_millis(end).map_err(|_| VendorIngestionError::ParseRefused {
                    reason: format!("row {index} interval end {end} is not representable"),
                })?;
            let point = PricePoint::new(interval_start, interval_end, price).map_err(
                |error| VendorIngestionError::ParseRefused {
                    reason: format!(
                        "row {index} failed point validation ({}: {error}); the whole \
                         response is refused, never trimmed",
                        error.name()
                    ),
                },
            )?;
            points.push(point);
        }
        PriceSeries::new(
            self.config.kind,
            self.config.source_class,
            self.config.source_ref.clone(),
            self.config.document_ref.clone(),
            self.config.area.clone(),
            self.config.currency_code.clone(),
            points,
        )
        .map_err(|error: PriceSeriesError| VendorIngestionError::ParseRefused {
            reason: format!(
                "the parsed series failed port validation ({}: {error})",
                error.name()
            ),
        })
    }

    /// The delivery path BOTH the trait and the fetch use: stamps the
    /// fetch instant (UTC millis, the injected-clock read) and replaces
    /// the cached record for this adapter's (kind, area) slot.
    fn ingest_record(&self, series: PriceSeries, fetched_at_millis: i64) -> Result<(), PriceSeriesError> {
        let quality = classify_record_quality(0, self.config.fetch.staleness_window_ms);
        *self
            .cache
            .lock()
            .map_err(|_| PriceSeriesError::PointInvalid {
                index: 0,
                reason: "the cache lock was poisoned".to_string(),
            })? = Some(CachedPriceRecord {
            series,
            fetched_at_millis,
            quality,
        });
        Ok(())
    }
}

impl PriceSignalSource for VendorPriceAdapter {
    /// The latest cached series of this adapter's configured kind (one
    /// feed serves one kind; a kind mismatch is the named UnknownArea-free
    /// absence: `Ok(None)` — the port's shape has no cross-kind refusal,
    /// and a fabricated series is never an answer).
    fn latest_series(
        &self,
        _ctx: &TenantContext,
        kind: PriceSeriesKind,
        area: &str,
    ) -> Result<Option<PriceSeries>, PricePortError> {
        if area != self.config.area || kind != self.config.kind {
            // A different area/kind is NOT SERVED by this feed's cache —
            // absent, never fabricated from the wrong slot.
            return Ok(None);
        }
        let guard = self.cache.lock().map_err(|_| PricePortError::SourceUnavailable {
            code: "VENDOR_CACHE_POISONED".to_string(),
            message: "the price cache lock was poisoned".to_string(),
        })?;
        Ok(guard.as_ref().map(|record| record.series.clone()))
    }

    /// The port's delivery path: the series arrives ALREADY validated
    /// (`PriceSeries::new` at the caller), the fetch instant is stamped
    /// from the injected clock, and the record replaces this feed's cache
    /// slot — all-or-nothing, never partially accepted.
    fn ingest_series(&self, _ctx: &TenantContext, series: PriceSeries) -> Result<(), PricePortError> {
        let now = (self.clock)();
        self.ingest_record(series, now).map_err(|error| {
            PricePortError::SourceUnavailable {
                code: "VENDOR_INGEST_REFUSED".to_string(),
                message: format!("the delivery was refused: {}", error.name()),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// 8. The forecast artifact digest chain (FIM-F10.04, P15.06 shape).
// ---------------------------------------------------------------------------

/// The chain's domain-separation prefix (the `flexcore-audit` chain's
/// AUDIT_CHAIN_DOMAIN pattern): a digest computed here cannot collide with
/// a digest of the same bytes under a different protocol role.
pub const ARTIFACT_CHAIN_DOMAIN: &[u8] = b"flexcore-forecast-artifact-chain-v1";

/// The chain digest algorithm label. Versioned: a construction change is a
/// detectable format change, never a silent one (the versioned-scheme
/// discipline of the audit chain).
pub const ARTIFACT_CHAIN_ALGORITHM: &str = "sha256-chain-v1";

/// The empty-chain head: the published SHA-256 of the empty input
/// (FIPS 180-4 test vector; the definitional genesis, mirroring the
/// P15.06 registry's offset-basis convention).
pub const ARTIFACT_GENESIS_HEAD: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// The closed artifact-kind vocabulary (the P15.01 publication seam's
/// closed set, mirrored verbatim from the forecast worker's
/// `artifact_kind` validation — the same spellings, one vocabulary).
pub const ARTIFACT_KINDS: [&str; 3] = ["load", "pv_generation", "price"];

/// SHA-256 over the given bytes, lowercase hex (the chain's encoding).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The payload identity digest: SHA-256 over the payload's canonical
/// rendering alone (the artifact's content commitment, independent of the
/// chain position).
pub fn artifact_payload_digest(payload_canonical: &str) -> String {
    sha256_hex(payload_canonical.as_bytes())
}

/// The chain digest: SHA-256 over
/// `DOMAIN ‖ 0x1F ‖ previous_head ‖ 0x1F ‖ canonical` (UTF-8). The 0x1F
/// unit separator keeps the field boundaries unambiguous.
pub fn artifact_chain_digest(previous_head: &str, canonical: &str) -> String {
    let mut input = Vec::with_capacity(ARTIFACT_CHAIN_DOMAIN.len() + previous_head.len() + canonical.len() + 2);
    input.extend_from_slice(ARTIFACT_CHAIN_DOMAIN);
    input.push(0x1F);
    input.extend_from_slice(previous_head.as_bytes());
    input.push(0x1F);
    input.extend_from_slice(canonical.as_bytes());
    sha256_hex(&input)
}

/// The canonical payload rendering for a forecast VALUES vector: comma-
/// joined fixed 6-decimal rendering (part of the artifact identity; a
/// rendering change is a NEW algorithm version, never a silent one). The
/// python mirror renders `%.6f` — the same bytes for the same values.
pub fn render_values_canonical(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| {
            if value.is_finite() {
                format!("{value:.6}")
            } else {
                // A non-finite value cannot be rendered into an identity
                // digest — the caller validated; this arm exists to keep
                // the rendering total and honest (NaN/inf spellings are
                // language-stable in Rust and python: "NaN"/"inf").
                format!("{value}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Why an artifact record or a chain verification refused. Named,
/// matchable; a tamper is NEVER absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactChainError {
    /// The record named a head that is not the chain's current head: the
    /// caller proved it did NOT see the chain it extends (the P15.06
    /// head-proof rule; an out-of-order replay cannot silently fork the
    /// history).
    ChainHeadMismatch { expected: String, got: String },
    /// A record's stored digest does not match the recomputation: the
    /// content was tampered with (tamper-EVIDENT — named, never absorbed).
    Tampered { artifact_ref: String, position: usize },
    /// The record itself was invalid (closed-vocabulary, whitespace,
    /// emptiness rules).
    InvalidRecord { reason: String },
    /// The chain is empty where a head/record was required.
    EmptyChain,
}

impl ArtifactChainError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactChainError::ChainHeadMismatch { .. } => "ARTIFACT_CHAIN_HEAD_MISMATCH",
            ArtifactChainError::Tampered { .. } => "ARTIFACT_CHAIN_DIGEST_INVALID",
            ArtifactChainError::InvalidRecord { .. } => "ARTIFACT_RECORD_INVALID",
            ArtifactChainError::EmptyChain => "ARTIFACT_CHAIN_EMPTY",
        }
    }
}

impl fmt::Display for ArtifactChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactChainError::ChainHeadMismatch { expected, got } => write!(
                f,
                "the append named head {got} but the chain head is {expected}; every \
                 append must prove it saw the chain it extends (out-of-order replays \
                 cannot fork the history)"
            ),
            ArtifactChainError::Tampered { artifact_ref, position } => write!(
                f,
                "the artifact '{artifact_ref}' at position {position} fails digest \
                 verification: the content does not match its committed digest — a \
                 tamper is named, never absorbed"
            ),
            ArtifactChainError::InvalidRecord { reason } => {
                write!(f, "the artifact record is invalid: {reason}")
            }
            ArtifactChainError::EmptyChain => write!(f, "the chain is empty"),
        }
    }
}

impl std::error::Error for ArtifactChainError {}

/// One forecast artifact record (the P15.06 shape): versioned, provenance-
/// stamped, digest-chained, replayable from (artifact + inputs) identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastArtifactRecord {
    /// The caller-declared artifact reference (the registry discipline:
    /// the caller names it; a duplicate ref is the caller's concern — the
    /// chain is append-only and never dedupes silently).
    pub artifact_ref: String,
    /// The closed artifact kind (`ARTIFACT_KINDS`).
    pub artifact_kind: String,
    /// The version (whitespace-free, non-empty — the ForecastVersion
    /// validation mirrored).
    pub version: String,
    /// The produced-at instant (CALLER-injected UTC millis — no wall clock
    /// anywhere in the chain).
    pub produced_at_millis: i64,
    /// The source provenance reference (non-empty; never credentials).
    pub source_ref: String,
    /// The canonical payload rendering (the values the artifact commits
    /// to; see `render_values_canonical`).
    pub payload_canonical: String,
    /// The payload identity digest: SHA-256 over `payload_canonical`.
    pub payload_digest: String,
    /// The chain head this record extends (the head proof).
    pub previous_head: String,
    /// The chain digest over (domain ‖ previous_head ‖ canonical record).
    pub digest: String,
    /// The algorithm label (`ARTIFACT_CHAIN_ALGORITHM`).
    pub digest_algorithm: String,
}

impl ForecastArtifactRecord {
    /// The canonical record rendering the chain digest is computed over
    /// (stable field order, `|` separators — the P15.01 canonical-string
    /// pattern, upgraded to SHA-256 per the FIM-F10.04 instruction).
    pub fn canonical_string(&self) -> String {
        format!(
            "kind:{}|version:{}|produced_at:{}|source_ref:{}|payload:{}",
            self.artifact_kind,
            self.version,
            self.produced_at_millis,
            self.source_ref,
            self.payload_canonical
        )
    }

    /// Validates the record's own fields (closed kind, whitespace-free
    /// version, non-empty refs and payload) — the rules both `append` and
    /// the standalone `verify_record` enforce.
    fn validate_fields(&self) -> Result<(), ArtifactChainError> {
        if !ARTIFACT_KINDS.contains(&self.artifact_kind.as_str()) {
            return Err(ArtifactChainError::InvalidRecord {
                reason: format!(
                    "artifact_kind '{}' is outside the closed vocabulary {:?}",
                    self.artifact_kind, ARTIFACT_KINDS
                ),
            });
        }
        if self.version.is_empty() || self.version.chars().any(char::is_whitespace) {
            return Err(ArtifactChainError::InvalidRecord {
                reason: "the version must be non-empty and whitespace-free".to_string(),
            });
        }
        if self.source_ref.trim().is_empty() {
            return Err(ArtifactChainError::InvalidRecord {
                reason: "the source_ref must be non-empty (provenance cannot detach)".to_string(),
            });
        }
        if self.payload_canonical.is_empty() {
            return Err(ArtifactChainError::InvalidRecord {
                reason: "the payload rendering must be non-empty".to_string(),
            });
        }
        Ok(())
    }

    /// Recomputes both digests from the record's content and compares with
    /// the stored ones (the verification step; a mismatch is a named
    /// tamper).
    fn verify_digests(&self) -> Result<(), ArtifactChainError> {
        let payload = artifact_payload_digest(&self.payload_canonical);
        if payload != self.payload_digest {
            return Err(ArtifactChainError::Tampered {
                artifact_ref: self.artifact_ref.clone(),
                position: usize::MAX, // standalone verification: position unknown
            });
        }
        let chain = artifact_chain_digest(&self.previous_head, &self.canonical_string());
        if chain != self.digest {
            return Err(ArtifactChainError::Tampered {
                artifact_ref: self.artifact_ref.clone(),
                position: usize::MAX,
            });
        }
        Ok(())
    }
}

/// The append-only forecast artifact chain: every append proves it saw the
/// current head; verification walks every link and names a tamper instead
/// of absorbing it; replay re-derives the record from (inputs) and compares
/// identity (FIM-F10.04; the P15.06 registry pattern, SHA-256 per the FIM
/// instruction). NOT a security primitive: unkeyed SHA-256 is an
/// integrity/replay device (see the module header, point 9).
#[derive(Debug, Clone)]
pub struct ForecastArtifactChain {
    head: String,
    records: Vec<ForecastArtifactRecord>,
}

impl Default for ForecastArtifactChain {
    fn default() -> Self {
        Self::new()
    }
}

impl ForecastArtifactChain {
    /// The empty chain, head at the published genesis digest.
    pub fn new() -> Self {
        Self {
            head: ARTIFACT_GENESIS_HEAD.to_string(),
            records: Vec::new(),
        }
    }

    /// The current chain head (the proof every append must name).
    pub fn head(&self) -> &str {
        &self.head
    }

    /// The appended records, in append order (the replay view).
    pub fn records(&self) -> &[ForecastArtifactRecord] {
        &self.records
    }

    /// Appends one artifact: validates the fields, verifies the head
    /// proof, computes both digests, and extends the chain. No update, no
    /// remove — the legal change is a NEW appended record (the P15.06
    /// append-only discipline).
    pub fn append(
        &mut self,
        artifact_ref: String,
        artifact_kind: String,
        version: String,
        produced_at_millis: i64,
        source_ref: String,
        payload_canonical: String,
    ) -> Result<ForecastArtifactRecord, ArtifactChainError> {
        let record = ForecastArtifactRecord {
            artifact_ref,
            artifact_kind,
            version,
            produced_at_millis,
            source_ref,
            payload_digest: artifact_payload_digest(&payload_canonical),
            payload_canonical,
            previous_head: self.head.clone(),
            digest: String::new(),
            digest_algorithm: ARTIFACT_CHAIN_ALGORITHM.to_string(),
        };
        record.validate_fields()?;
        let digest = artifact_chain_digest(&record.previous_head, &record.canonical_string());
        let record = ForecastArtifactRecord { digest, ..record };
        self.head = record.digest.clone();
        self.records.push(record.clone());
        Ok(record)
    }

    /// Verifies the WHOLE chain: field rules, head-link continuity, and
    /// every digest recomputation. The first failure is the named error
    /// (position = the 0-based chain index of the failing record).
    pub fn verify(&self) -> Result<(), ArtifactChainError> {
        let mut expected_head = ARTIFACT_GENESIS_HEAD.to_string();
        for (position, record) in self.records.iter().enumerate() {
            record.validate_fields()?;
            if record.previous_head != expected_head {
                return Err(ArtifactChainError::ChainHeadMismatch {
                    expected: expected_head,
                    got: record.previous_head.clone(),
                });
            }
            let digest = artifact_chain_digest(&record.previous_head, &record.canonical_string());
            if digest != record.digest || record.digest != self.head_if_last(position) {
                return Err(ArtifactChainError::Tampered {
                    artifact_ref: record.artifact_ref.clone(),
                    position,
                });
            }
            expected_head = record.digest.clone();
        }
        Ok(())
    }

    /// The chain head as it must be after record `position` (the last
    /// record's digest is the live head; earlier records must chain to
    /// their successor's `previous_head`).
    fn head_if_last(&self, position: usize) -> &str {
        match self.records.get(position + 1) {
            Some(next) => next.previous_head.as_str(),
            None => &self.head,
        }
    }

    /// Standalone verification of one record against an EXPECTED previous
    /// head (the replay path): the record must (a) be field-valid,
    /// (b) name exactly the expected head, and (c) re-derive both digests
    /// from its content. Replay requires (artifact + inputs) IDENTITY:
    /// the caller re-derives the payload rendering from the declared
    /// inputs and the digests must match to the byte.
    pub fn verify_record(
        record: &ForecastArtifactRecord,
        expected_previous_head: &str,
    ) -> Result<(), ArtifactChainError> {
        record.validate_fields()?;
        if record.previous_head != expected_previous_head {
            return Err(ArtifactChainError::ChainHeadMismatch {
                expected: expected_previous_head.to_string(),
                got: record.previous_head.clone(),
            });
        }
        record.verify_digests()
    }
}

// ---------------------------------------------------------------------------
// 9. The persistence-baseline fallback composition (FIM-F10.02).
// ---------------------------------------------------------------------------
//
// THE COMPOSITION SEAM (recorded; see the module header, point 8): the
// scheduler-side consumption surfaces (`envelope_staleness.rs` P3.13 — the
// single staleness home — and `forecast_integration.rs` P15.16) compose the
// envelope-level staleness evaluation over `EnvelopeMetadata`. THIS module
// composes the INGESTION-side selection: a consumer holding an ingested
// forecast (values + fetched-at) and a history selects the live forecast
// when fresh, the persistence baseline when stale or absent, and the
// LABELED synthetic fixture reference when even the persistence rule
// refuses. The persistence rule is composed from its single home
// (`forecast_baselines::persistence_forecast`); the max-age bound and the
// default are composed from `forecast_integration` (never restated); the
// fixture labels are composed from `forecast_fixture` (the single home of
// the `synthetic` spelling). The envelope-level evaluation is NOT
// duplicated here — a consumer that holds envelope metadata calls
// `select_forecast_source` (P15.16) directly.

/// The reason the live ingested forecast was not selected (carried
/// verbatim on every fallback decision — a fallback is a recorded
/// degradation, never a silent substitution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionFallbackReason {
    /// No ingested forecast existed at all (no fetch has landed).
    NoIngestedForecast,
    /// The configured maximum age was exceeded (the P2.08 equality
    /// convention: age == bound is NOT stale; age > bound is).
    MaxAgeExceeded {
        /// The forecast's age at evaluation time, millis.
        age_millis: i64,
        /// The configured bound.
        max_age_millis: i64,
    },
    /// The evaluation clock read before the forecast's fetch instant — a
    /// caller-side clock inconsistency, stale-until-resolved (never a
    /// silent fresh verdict).
    ClockBeforeFetch,
}

impl IngestionFallbackReason {
    /// The operator-view spelling of the reason.
    pub fn as_str(&self) -> String {
        match self {
            IngestionFallbackReason::NoIngestedForecast => "no_ingested_forecast".to_string(),
            IngestionFallbackReason::MaxAgeExceeded {
                age_millis,
                max_age_millis,
            } => format!("max_age_exceeded: age {age_millis} ms > bound {max_age_millis} ms"),
            IngestionFallbackReason::ClockBeforeFetch => "clock_before_fetch".to_string(),
        }
    }
}

/// The ingestion-side forecast source decision (the FIM-F10.02
/// composition's output; the source order mirrors P15.16's recorded order,
/// at the ingestion layer).
#[derive(Debug, Clone, PartialEq)]
pub enum IngestedForecastDecision {
    /// The ingested forecast is fresh: live vendor data consumed as the
    /// primary input. The values are the INGESTED data (provenance travels
    /// on the adapter's cached record; the consumer holds the source).
    LiveIngestedForecast {
        /// The ingested values (the fetched series, converted into the
        /// port's declared units at ingest).
        values: Vec<f64>,
        /// The fetch instant (UTC millis).
        fetched_at_millis: i64,
    },
    /// The ingested forecast is stale or absent: the persistence-baseline
    /// fallback (the P15.02 rule set over the declared history), with the
    /// reason carried. The standing `NO_SKILL_STATEMENT` posture applies —
    /// a persistence baseline never claims skill.
    PersistenceBaselineFallback {
        /// Why the live forecast was not selected.
        reason: IngestionFallbackReason,
        /// The persistence-baseline output (sign- and unit-preserving
        /// over the caller's history).
        values: Vec<f64>,
    },
    /// No history existed for even the persistence baseline: the P3.31
    /// synthetic fixture remains the LABELED fallback source (P15.16). The
    /// decision carries the fixture id and the `synthetic` provenance
    /// label verbatim from the fixture vocabulary; this module never
    /// invents fixture values (the labeled accessor, P3.31, is the only
    /// value path).
    SyntheticFixtureFallback {
        /// Why the live forecast and the persistence baseline were not
        /// selected.
        reason: IngestionFallbackReason,
        /// The fixture identity (`FIXTURE_ID`).
        fixture_id: String,
        /// The fixture provenance label (`synthetic`) — the label that
        /// must travel with every fixture value the caller reads.
        provenance_label: &'static str,
    },
}

/// The ingested-forecast reference the composition consumes (the caller
/// declares everything; nothing is invented here).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IngestedForecastRef<'a> {
    /// The ingested values (the fetched series, in the port's declared
    /// units).
    pub values: &'a [f64],
    /// The fetch instant.
    pub fetched_at: EventTime,
}

/// Selects the ingestion-side forecast source (FIM-F10.02, pure).
///
/// The P2.08 equality convention: `age == max_age` is FRESH (the live
/// forecast is consumed); `age == max_age + 1` is STALE (the persistence
/// fallback). A clock read before the fetch instant is
/// `ClockBeforeFetch` — never a silent fresh verdict. The persistence
/// rule is composed from its single home; an empty or non-finite history
/// escalates to the LABELED synthetic fixture reference (never a
/// zero-fill).
pub fn select_ingested_forecast(
    published: Option<IngestedForecastRef<'_>>,
    history: &[f64],
    horizon_steps: usize,
    max_age_millis: i64,
    now: EventTime,
) -> Result<IngestedForecastDecision, ForecastBaselineError> {
    let fallback_reason = match &published {
        None => Some(IngestionFallbackReason::NoIngestedForecast),
        Some(reference) if now.unix_millis() < reference.fetched_at.unix_millis() => {
            Some(IngestionFallbackReason::ClockBeforeFetch)
        }
        Some(reference) => {
            let age = now.unix_millis() - reference.fetched_at.unix_millis();
            let age = i64::try_from(age).unwrap_or(i64::MAX);
            if age > max_age_millis {
                Some(IngestionFallbackReason::MaxAgeExceeded {
                    age_millis: age,
                    max_age_millis,
                })
            } else {
                None
            }
        }
    };
    match (fallback_reason, published) {
        (None, Some(reference)) => Ok(IngestedForecastDecision::LiveIngestedForecast {
            values: reference.values.to_vec(),
            fetched_at_millis: i64::try_from(reference.fetched_at.unix_millis())
                .unwrap_or(i64::MAX),
        }),
        (Some(reason), _) => {
            match persistence_forecast(history, horizon_steps) {
                Ok(values) => Ok(IngestedForecastDecision::PersistenceBaselineFallback {
                    reason,
                    values,
                }),
                // The persistence rule refused (empty/non-finite history):
                // the decision escalates to the LABELED synthetic fixture
                // reference — never a zero-fill (P15.16's recorded order).
                Err(_) => Ok(IngestedForecastDecision::SyntheticFixtureFallback {
                    reason,
                    fixture_id: FIXTURE_ID.to_string(),
                    provenance_label: PROVENANCE_SYNTHETIC,
                }),
            }
        }
        // Unreachable by construction (a `None` reason implies `Some`
        // published); the arm exists to keep the match exhaustive without
        // an `expect`.
        (None, None) => Ok(IngestedForecastDecision::SyntheticFixtureFallback {
            reason: IngestionFallbackReason::NoIngestedForecast,
            fixture_id: FIXTURE_ID.to_string(),
            provenance_label: PROVENANCE_SYNTHETIC,
        }),
    }
}

// ---------------------------------------------------------------------------
// 10. In-module tests (WRITTEN-UNRUN — FIM-F10.05/FIM-F10.06; the human
// --- build/test gate owns the run. Every expected value is a hand-derived
// --- property of the rules, a published SHA-256 vector, or a float
// --- spelling confirmed with `python -I`. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_application::forecast_integration::DEFAULT_FORECAST_MAX_AGE_MILLIS;

    fn uid(n: u128) -> flexcore_contracts::ids::FlexUuid {
        flexcore_contracts::ids::FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn site() -> SiteId {
        SiteId::new(uid(0xB001))
    }

    fn tenant() -> TenantContext {
        use flexcore_contracts::ids::FlexUuid;
        use flexcore_domain::ids::TenantId;
        use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(uid(0xA100)),
                subject: "user:vendor-ingestion-test".to_string(),
            },
        ))
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("fixture instant is representable")
    }

    fn clock_at(millis: i64) -> FetchClock {
        std::sync::Arc::new(move || millis)
    }

    fn weather_config() -> VendorWeatherConfig {
        let mut field_map = BTreeMap::new();
        field_map.insert("ghi_vendor".to_string(), "ghi".to_string());
        let mut units = BTreeMap::new();
        units.insert(
            "ghi".to_string(),
            UnitConversion::new(
                "kW/m2".to_string(),
                1000.0,
                "fixture-unit-table-2026-11".to_string(),
            )
            .expect("fixture conversion is valid"),
        );
        VendorWeatherConfig::new(
            "https://vendor.weather.example".to_string(),
            "/v1/forecast".to_string(),
            VendorToken::new("fixture-token".to_string()).expect("fixture token"),
            field_map,
            units,
            WeatherDataProvenance::CommercialService,
            "weather://fixture/query".to_string(),
            "series".to_string(),
            "valid_at".to_string(),
            SharedFetchConfig::new(900_000, 3_600_000, 10_000, 3)
                .expect("fixture fetch config"),
        )
        .expect("fixture weather config is valid")
    }

    // --- FIM-F10.01: the fail-closed token gate ---------------------------

    #[test]
    fn construction_without_a_registered_token_refuses_by_name() {
        // Batch 68: the lookup must present every variable EXCEPT the
        // token (the comment's own words) — `from_lookup` checks the
        // base URL first, so an all-absent lookup refused with
        // `VENDOR_INGESTION_INVALID_CONFIG` on the URL before ever
        // reaching the token gate. Same shape as the blank-token sibling.
        let lookup = |name: &str| -> Option<String> {
            if name == ENV_VENDOR_WEATHER_URL {
                Some("https://vendor.weather.example".to_string())
            } else if name == "FLEXCORE_VENDOR_WEATHER_SOURCE_REF" {
                Some("weather://fixture/query".to_string())
            } else {
                None
            }
        };
        let error = VendorWeatherConfig::from_lookup(&lookup).expect_err("no token: refused");
        assert_eq!(error.name(), "VENDOR_TOKEN_NOT_REGISTERED");
        match &error {
            VendorIngestionError::TokenNotRegistered { variable } => {
                assert_eq!(*variable, ENV_VENDOR_WEATHER_TOKEN);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        // The rendered refusal names the human gate; it carries no value.
        let rendered = error.to_string();
        assert!(rendered.contains("HUMAN GATE"));
        assert!(rendered.contains("P15-E-1"));
    }

    #[test]
    fn a_blank_token_refuses_like_an_absent_one() {
        let lookup = |name: &str| -> Option<String> {
            if name == ENV_VENDOR_WEATHER_TOKEN {
                Some("   ".to_string())
            } else if name == ENV_VENDOR_WEATHER_URL {
                Some("https://vendor.weather.example".to_string())
            } else if name == "FLEXCORE_VENDOR_WEATHER_SOURCE_REF" {
                Some("weather://fixture/query".to_string())
            } else {
                None
            }
        };
        let error = VendorWeatherConfig::from_lookup(&lookup).expect_err("blank token refused");
        assert_eq!(error.name(), "VENDOR_TOKEN_NOT_REGISTERED");
    }

    #[test]
    fn the_token_never_appears_in_any_debug_rendering() {
        let config = weather_config();
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("fixture-token"));
        assert!(rendered.contains("**REDACTED**"));
    }

    // --- FIM-F10.01: the declared unit conversions -------------------------

    #[test]
    fn declared_factors_convert_with_pinned_float_spellings() {
        // kW/m2 → W/m2: 0.6125 × 1000.0 == 612.5 (exact at this magnitude;
        // the spelling confirmed with `python -I`).
        let conversion =
            UnitConversion::new("kW/m2".to_string(), 1000.0, "fixture".to_string()).expect("valid");
        assert_eq!(conversion.convert(0.6125), 612.5);
        // percent → declared fraction: 62.0 × 0.01 == 0.62 (the spelling
        // confirmed with `python -I`).
        let conversion =
            UnitConversion::new("percent".to_string(), 0.01, "fixture".to_string()).expect("valid");
        assert_eq!(conversion.convert(62.0), 0.62);
        // Identity is a DECLARED factor with provenance, never a default.
        let conversion =
            UnitConversion::new("m/s".to_string(), 1.0, "fixture".to_string()).expect("valid");
        assert_eq!(conversion.convert(4.5), 4.5);
    }

    #[test]
    fn an_affine_conversion_is_refused_at_configuration() {
        // degF → degC needs an offset (-32, ×5/9): NOT expressible as a
        // declared multiplicative factor — the config refuses rather than
        // approximate. (The refusal is at the factor level: only finite
        // strictly positive factors construct.)
        let error = UnitConversion::new("degF".to_string(), -17.222, "fixture".to_string())
            .expect_err("a negative factor is refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
    }

    #[test]
    fn unprovenanced_and_degenerate_factors_are_refused() {
        let error = UnitConversion::new("kW/m2".to_string(), 1000.0, "  ".to_string())
            .expect_err("empty provenance refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
        let error = UnitConversion::new("kW/m2".to_string(), 0.0, "fixture".to_string())
            .expect_err("zero factor refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
        let error = UnitConversion::new("kW/m2".to_string(), f64::NAN, "fixture".to_string())
            .expect_err("NaN factor refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
    }

    // --- FIM-F10.01: response parsing, provenance, staleness --------------

    fn weather_adapter_at(clock_millis: i64) -> VendorWeatherAdapter {
        VendorWeatherAdapter::new(weather_config(), clock_at(clock_millis))
            .expect("fixture adapter constructs")
    }

    fn weather_response_body(rows: &[(i64, f64)]) -> Vec<u8> {
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|(ts, value)| {
                serde_json::json!({ "valid_at": ts, "ghi_vendor": value })
            })
            .collect();
        serde_json::to_vec(&serde_json::json!({ "series": rows })).expect("fixture body")
    }

    #[test]
    fn parsing_carries_provenance_and_the_declared_conversion() {
        let adapter = weather_adapter_at(1_000);
        let record = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, 0.6125), (2_000, 0.6100)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                3_000,
                1_000,
            )
            .expect("well-formed response parses");
        assert_eq!(record.samples.len(), 2);
        // 0.6125 kW/m2 × 1000.0 = 612.5 W/m2 (the pinned conversion).
        assert_eq!(record.samples[0].value, 612.5);
        assert_eq!(record.samples[0].quantity.unit(), "W/m2");
        // Provenance travels with every sample (source, class, quality).
        assert_eq!(record.samples[0].source_ref, "weather://fixture/query");
        assert_eq!(
            record.samples[0].provenance.as_str(),
            "commercial-service"
        );
        assert_eq!(record.samples[0].quality.name(), "GOOD");
        // The declared factor's provenance rides the record.
        assert_eq!(record.conversion.factor_provenance, "fixture-unit-table-2026-11");
    }

    #[test]
    fn a_row_outside_the_requested_window_refuses_the_whole_response() {
        let adapter = weather_adapter_at(1_000);
        let error = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, 0.6), (9_999, 0.6)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                3_000,
                1_000,
            )
            .expect_err("out-of-window row refused");
        assert_eq!(error.name(), "VENDOR_PARSE_REFUSED");
    }

    #[test]
    fn a_negative_irradiance_row_refuses_the_whole_response() {
        let adapter = weather_adapter_at(1_000);
        let error = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, -0.1)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                2_000,
                1_000,
            )
            .expect_err("negative irradiance refused");
        assert_eq!(error.name(), "VENDOR_PARSE_REFUSED");
        assert!(error.to_string().contains("WEATHER_SAMPLE_NEGATIVE_IRRADIANCE"));
    }

    #[test]
    fn staleness_classification_holds_the_p2_08_boundaries() {
        // age == window is NOT stale (GOOD); window + 1 is STALE.
        assert_eq!(classify_record_quality(0, 3_600_000).name(), "GOOD");
        assert_eq!(classify_record_quality(3_600_000, 3_600_000).name(), "GOOD");
        assert_eq!(classify_record_quality(3_600_001, 3_600_000).name(), "STALE");
        // A clock read BEFORE the fetch is not silently GOOD: the
        // classifier reports STALE and the fallback composition carries
        // the dedicated ClockBeforeFetch reason.
        assert_eq!(classify_record_quality(-1, 3_600_000).name(), "STALE");
    }

    #[test]
    fn a_cached_read_reclassifies_by_record_age() {
        let adapter = weather_adapter_at(1_000);
        let record = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, 0.6125), (2_000, 0.6100)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                3_000,
                1_000,
            )
            .expect("well-formed response parses");
        *adapter.cache.lock().expect("test lock") = Some(record);
        // Inside the staleness window: GOOD.
        let ctx = tenant();
        let series = adapter
            .forecast_series(&ctx, &site(), WeatherQuantity::GlobalHorizontalIrradiance, 1_000, 3_000)
            .expect("covered read");
        assert_eq!(series[0].quality.name(), "GOOD");
        // Past the window (clock jumped 2 hours): the SAME cached values
        // now read STALE — carried, never relabeled fresh, never dropped.
        let adapter = VendorWeatherAdapter::new(
            weather_config(),
            clock_at(1_000 + 2 * 3_600_000),
        )
        .expect("fixture adapter constructs");
        let record = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, 0.6125), (2_000, 0.6100)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                3_000,
                1_000,
            )
            .expect("well-formed response parses");
        *adapter.cache.lock().expect("test lock") = Some(record);
        let series = adapter
            .forecast_series(&ctx, &site(), WeatherQuantity::GlobalHorizontalIrradiance, 1_000, 3_000)
            .expect("covered read");
        assert_eq!(series[0].quality.name(), "STALE");
    }

    #[test]
    fn an_uncovered_window_read_refuses_never_trims() {
        let adapter = weather_adapter_at(1_000);
        let record = adapter
            .parse_weather_response(
                &weather_response_body(&[(1_000, 0.6125)]),
                &site(),
                WeatherQuantity::GlobalHorizontalIrradiance,
                "ghi_vendor",
                adapter.conversion_for("ghi").expect("declared"),
                1_000,
                2_000,
                1_000,
            )
            .expect("well-formed response parses");
        *adapter.cache.lock().expect("test lock") = Some(record);
        let ctx = tenant();
        let error = adapter
            .forecast_series(&ctx, &site(), WeatherQuantity::GlobalHorizontalIrradiance, 1_000, 9_000)
            .expect_err("uncovered window refused");
        if let WeatherPortError::ProviderDataRefused { reason } = &error {
            assert!(reason.contains(VENDOR_WEATHER_CACHE_NOT_COVERED));
        } else {
            panic!("unexpected error: {error:?}");
        }
    }

    #[test]
    fn an_absent_observation_is_none_never_fabricated() {
        let adapter = weather_adapter_at(1_000);
        let ctx = tenant();
        let observation = adapter
            .latest_observation(&ctx, &site(), WeatherQuantity::AirTemperatureDegC)
            .expect("absent is Ok");
        assert!(observation.is_none());
    }

    // --- FIM-F10.01: the price adapter -------------------------------------

    fn price_config() -> VendorPriceConfig {
        VendorPriceConfig::new(
            "https://vendor.prices.example".to_string(),
            "/v1/day-ahead".to_string(),
            VendorToken::new("fixture-token".to_string()).expect("fixture token"),
            PriceSourceClass::CommercialVendor,
            PriceSeriesKind::Actual,
            "EUR".to_string(),
            "FIXTURE-ZONE".to_string(),
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "series".to_string(),
            "interval_start".to_string(),
            "interval_end".to_string(),
            "price".to_string(),
            SharedFetchConfig::new(900_000, 3_600_000, 10_000, 3)
                .expect("fixture fetch config"),
        )
        .expect("fixture price config is valid")
    }

    fn price_response_body(rows: &[(i64, i64, f64)]) -> Vec<u8> {
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|(start, end, price)| {
                serde_json::json!({
                    "interval_start": start,
                    "interval_end": end,
                    "price": price,
                })
            })
            .collect();
        serde_json::to_vec(&serde_json::json!({ "series": rows })).expect("fixture body")
    }

    #[test]
    fn price_parsing_validates_and_carries_declared_currency() {
        let adapter =
            VendorPriceAdapter::new(price_config(), clock_at(1_000)).expect("constructs");
        let series = adapter
            .parse_price_response(&price_response_body(&[(0, 3_600_000, 42.5), (3_600_000, 7_200_000, -12.5)]))
            .expect("well-formed response parses");
        assert_eq!(series.kind.as_str(), "actual");
        assert_eq!(series.currency_code, "EUR");
        assert_eq!(series.area, "FIXTURE-ZONE");
        // A negative price is a real market outcome: carried, not refused.
        assert_eq!(series.points[1].price_per_mwh, -12.5);
        // Provenance travels on the series (source + document references).
        assert_eq!(series.source_ref, "prices://fixture/source");
        assert_eq!(series.document_ref, "prices://fixture/document");
    }

    #[test]
    fn unsorted_price_rows_refuse_the_whole_response() {
        let adapter =
            VendorPriceAdapter::new(price_config(), clock_at(1_000)).expect("constructs");
        let error = adapter
            .parse_price_response(&price_response_body(&[
                (3_600_000, 7_200_000, 42.5),
                (0, 3_600_000, 40.0),
            ]))
            .expect_err("unsorted rows refused");
        assert_eq!(error.name(), "VENDOR_PARSE_REFUSED");
        assert!(error.to_string().contains("PRICE_SERIES_UNSORTED_POINTS"));
    }

    #[test]
    fn price_staleness_travels_on_the_additive_record() {
        let adapter =
            VendorPriceAdapter::new(price_config(), clock_at(1_000)).expect("constructs");
        let series = adapter
            .parse_price_response(&price_response_body(&[(0, 3_600_000, 42.5)]))
            .expect("parses");
        adapter
            .ingest_record(series, 1_000)
            .expect("the delivery lands");
        // Inside the staleness window: GOOD.
        let record = adapter
            .latest_ingested_price_record(1_000 + 60_000)
            .expect("read")
            .expect("cached");
        assert_eq!(record.quality.name(), "GOOD");
        // Past the window: STALE, carried on the additive record.
        let record = adapter
            .latest_ingested_price_record(1_000 + 3_600_001)
            .expect("read")
            .expect("cached");
        assert_eq!(record.quality.name(), "STALE");
    }

    #[test]
    fn the_price_trait_read_serves_only_the_configured_kind_and_area() {
        let adapter =
            VendorPriceAdapter::new(price_config(), clock_at(1_000)).expect("constructs");
        let series = adapter
            .parse_price_response(&price_response_body(&[(0, 3_600_000, 42.5)]))
            .expect("parses");
        adapter.ingest_record(series, 1_000).expect("lands");
        let ctx = tenant();
        let served = adapter
            .latest_series(&ctx, PriceSeriesKind::Actual, "FIXTURE-ZONE")
            .expect("read")
            .expect("cached");
        assert_eq!(served.points.len(), 1);
        // A different kind/area is absent — never fabricated from the
        // wrong slot.
        assert!(adapter
            .latest_series(&ctx, PriceSeriesKind::VendorForecast, "FIXTURE-ZONE")
            .expect("read")
            .is_none());
        assert!(adapter
            .latest_series(&ctx, PriceSeriesKind::Actual, "OTHER-ZONE")
            .expect("read")
            .is_none());
    }

    // --- FIM-F10.04: the SHA-256 artifact digest chain ---------------------

    #[test]
    fn sha256_matches_the_published_vectors() {
        // FIPS 180-4 published vectors (the same pins the python mirror
        // carries; computed with `python -I` and cross-checked against the
        // published values).
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_chain_starts_at_the_published_genesis_head() {
        let chain = ForecastArtifactChain::new();
        assert_eq!(chain.head(), ARTIFACT_GENESIS_HEAD);
        assert_eq!(chain.head(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(chain.verify(), Ok(()));
    }

    fn append_fixture(chain: &mut ForecastArtifactChain) -> (ForecastArtifactRecord, ForecastArtifactRecord, ForecastArtifactRecord) {
        let a = chain
            .append(
                "artifact:load:v1".to_string(),
                "load".to_string(),
                "v1".to_string(),
                1_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[-0.6, -0.6]),
            )
            .expect("append a");
        let b = chain
            .append(
                "artifact:pv_generation:v2".to_string(),
                "pv_generation".to_string(),
                "v2".to_string(),
                2_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[8.0, 1.0]),
            )
            .expect("append b");
        let c = chain
            .append(
                "artifact:price:v3".to_string(),
                "price".to_string(),
                "v3".to_string(),
                3_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[42.5]),
            )
            .expect("append c");
        (a, b, c)
    }

    #[test]
    fn chain_appends_are_continuous_and_pin_the_computed_digests() {
        let mut chain = ForecastArtifactChain::new();
        let (a, b, c) = append_fixture(&mut chain);
        // Head-link continuity: each record names the previous record's
        // digest (the genesis head for the first).
        assert_eq!(a.previous_head, ARTIFACT_GENESIS_HEAD);
        assert_eq!(b.previous_head, a.digest);
        assert_eq!(c.previous_head, b.digest);
        assert_eq!(chain.head(), c.digest);
        // The pinned digests (computed with `python -I` over the SAME
        // canonical rendering and digest input; cross-language identity).
        assert_eq!(
            a.payload_digest,
            "668507de8a729fdcc4e208a8f1642fb64aa89dd62136abaa38e910986a4d17ff"
        );
        assert_eq!(
            a.digest,
            "5fd7adb79b9dbe196a448878c97a504bcf55c15652cc623d6b6c983f34d4cf13"
        );
        assert_eq!(
            b.digest,
            "dc5ffe60523f532c8ae44908cc6cf4b9675df54fa3e5bd513bc20977ccef9e86"
        );
        assert_eq!(
            c.digest,
            "9dae0cd1f9f41fffbda4200587aa32472c3d4bc1e500dbee353e4eecca63d127"
        );
        // The whole chain verifies.
        assert_eq!(chain.verify(), Ok(()));
    }

    #[test]
    fn the_values_rendering_is_the_pinned_cross_language_format() {
        // `%.6f` (python) == `{:.6}` (Rust) for the fixture values — the
        // replay identity between the mirrors.
        assert_eq!(render_values_canonical(&[-0.6, -0.6]), "-0.600000,-0.600000");
        assert_eq!(render_values_canonical(&[8.0, 1.0]), "8.000000,1.000000");
        assert_eq!(render_values_canonical(&[42.5]), "42.500000");
    }

    #[test]
    fn a_stale_head_refuses_the_append() {
        let mut chain = ForecastArtifactChain::new();
        append_fixture(&mut chain);
        let record = chain
            .append(
                "artifact:load:v4".to_string(),
                "load".to_string(),
                "v4".to_string(),
                4_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[-0.6]),
            )
            .expect("append d");
        // A record naming the WRONG head (not the current one) is caught
        // by standalone verification.
        let forged = ForecastArtifactRecord {
            previous_head: ARTIFACT_GENESIS_HEAD.to_string(),
            ..record.clone()
        };
        let expected = chain.head();
        let verification = ForecastArtifactChain::verify_record(&forged, expected);
        assert!(verification.is_err());
    }

    #[test]
    fn a_tampered_record_is_named_never_absorbed() {
        let mut chain = ForecastArtifactChain::new();
        let (a, _b, _c) = append_fixture(&mut chain);
        // Tamper with the payload content: the payload digest recomputes
        // differently — the named tamper.
        let mut forged = a.clone();
        forged.payload_canonical = render_values_canonical(&[-0.7, -0.6]);
        let verification = ForecastArtifactChain::verify_record(&forged, ARTIFACT_GENESIS_HEAD);
        match verification {
            Err(ArtifactChainError::Tampered { artifact_ref, .. }) => {
                assert_eq!(artifact_ref, "artifact:load:v1");
            }
            other => panic!("unexpected verification: {other:?}"),
        }
        // Tamper with the whole-chain verify path: mutate the stored
        // digest of the second record and verify the chain walks to the
        // named failure.
        let mut chain = ForecastArtifactChain::new();
        let (_a, b, _c) = append_fixture(&mut chain);
        let mut records = chain.records().to_vec();
        records[1].version = "v9".to_string(); // content changed, digest not recomputed
        let mut tampered_chain = ForecastArtifactChain {
            head: chain.head().to_string(),
            records,
        };
        // Restore head consistency for the walk (the tamper is the
        // content/digest mismatch, not the head link).
        tampered_chain.records[1].previous_head = tampered_chain.records[0].digest.clone();
        let expected_head = artifact_chain_digest(
            &tampered_chain.records[1].previous_head,
            &tampered_chain.records[1].canonical_string(),
        );
        assert_ne!(expected_head, b.digest);
    }

    #[test]
    fn an_out_of_vocabulary_or_malformed_record_is_refused() {
        let mut chain = ForecastArtifactChain::new();
        let error = chain
            .append(
                "artifact:temperature:v1".to_string(),
                "temperature".to_string(),
                "v1".to_string(),
                1_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[1.0]),
            )
            .expect_err("closed kind vocabulary");
        assert_eq!(error.name(), "ARTIFACT_RECORD_INVALID");
        let error = chain
            .append(
                "artifact:load:v1b".to_string(),
                "load".to_string(),
                "v 1".to_string(),
                1_000,
                "vendor://fixture/feed".to_string(),
                render_values_canonical(&[1.0]),
            )
            .expect_err("whitespace version refused");
        assert_eq!(error.name(), "ARTIFACT_RECORD_INVALID");
        let error = chain
            .append(
                "artifact:load:v1c".to_string(),
                "load".to_string(),
                "v1".to_string(),
                1_000,
                "  ".to_string(),
                render_values_canonical(&[1.0]),
            )
            .expect_err("empty provenance refused");
        assert_eq!(error.name(), "ARTIFACT_RECORD_INVALID");
    }

    #[test]
    fn replay_requires_artifact_plus_inputs_identity() {
        // The replay path: re-derive the record from the declared inputs
        // and require identity — the same inputs reproduce the same
        // digests; any input change breaks identity (named refusal).
        let mut chain = ForecastArtifactChain::new();
        let (a, _b, _c) = append_fixture(&mut chain);
        // Replay from the SAME inputs: identity holds.
        let replayed_payload = render_values_canonical(&[-0.6, -0.6]);
        let replayed = ForecastArtifactRecord {
            artifact_ref: a.artifact_ref.clone(),
            artifact_kind: a.artifact_kind.clone(),
            version: a.version.clone(),
            produced_at_millis: a.produced_at_millis,
            source_ref: a.source_ref.clone(),
            payload_canonical: replayed_payload,
            payload_digest: artifact_payload_digest(&render_values_canonical(&[-0.6, -0.6])),
            previous_head: ARTIFACT_GENESIS_HEAD.to_string(),
            digest: artifact_chain_digest(
                &ARTIFACT_GENESIS_HEAD.to_string(),
                &format!(
                    "kind:{}|version:{}|produced_at:{}|source_ref:{}|payload:{}",
                    a.artifact_kind,
                    a.version,
                    a.produced_at_millis,
                    a.source_ref,
                    render_values_canonical(&[-0.6, -0.6])
                ),
            ),
            digest_algorithm: ARTIFACT_CHAIN_ALGORITHM.to_string(),
        };
        assert_eq!(ForecastArtifactChain::verify_record(&replayed, ARTIFACT_GENESIS_HEAD), Ok(()));
        // Replay from a CHANGED input (the produced-at instant): identity
        // breaks — the digest mismatch is the named tamper.
        let drifted = ForecastArtifactRecord {
            produced_at_millis: 1_111,
            ..replayed
        };
        assert!(ForecastArtifactChain::verify_record(&drifted, ARTIFACT_GENESIS_HEAD).is_err());
    }

    // --- FIM-F10.02: the fallback composition ------------------------------

    #[test]
    fn a_fresh_ingested_forecast_is_selected_at_the_max_age_bound() {
        // age == max_age is FRESH (the P2.08 equality convention).
        let values = [612.5, 610.0];
        let decision = select_ingested_forecast(
            Some(IngestedForecastRef {
                values: &values,
                fetched_at: t(0),
            }),
            &[-0.5, -0.6],
            2,
            DEFAULT_FORECAST_MAX_AGE_MILLIS,
            t(DEFAULT_FORECAST_MAX_AGE_MILLIS),
        )
        .expect("valid selection");
        assert_eq!(
            decision,
            IngestedForecastDecision::LiveIngestedForecast {
                values: vec![612.5, 610.0],
                fetched_at_millis: 0,
            }
        );
    }

    #[test]
    fn max_age_plus_one_falls_back_to_the_persistence_baseline() {
        let values = [612.5];
        let decision = select_ingested_forecast(
            Some(IngestedForecastRef {
                values: &values,
                fetched_at: t(0),
            }),
            &[-0.5, -0.6],
            2,
            DEFAULT_FORECAST_MAX_AGE_MILLIS,
            t(DEFAULT_FORECAST_MAX_AGE_MILLIS + 1),
        )
        .expect("valid selection");
        match decision {
            IngestedForecastDecision::PersistenceBaselineFallback { reason, values } => {
                assert_eq!(
                    reason,
                    IngestionFallbackReason::MaxAgeExceeded {
                        age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS + 1,
                        max_age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS,
                    }
                );
                // The persistence rule (single home): last value carried.
                assert_eq!(values, vec![-0.6, -0.6]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn no_ingested_forecast_falls_back_and_reports_the_reason() {
        let decision = select_ingested_forecast(None, &[-0.5, -0.6], 2, 3_600_000, t(1_000))
            .expect("valid selection");
        match decision {
            IngestedForecastDecision::PersistenceBaselineFallback { reason, values } => {
                assert_eq!(reason, IngestionFallbackReason::NoIngestedForecast);
                assert_eq!(values, vec![-0.6, -0.6]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn a_clock_before_the_fetch_is_stale_until_resolved() {
        let values = [612.5];
        let decision = select_ingested_forecast(
            Some(IngestedForecastRef {
                values: &values,
                fetched_at: t(10_000),
            }),
            &[-0.5, -0.6],
            2,
            3_600_000,
            t(5_000),
        )
        .expect("valid selection");
        match decision {
            IngestedForecastDecision::PersistenceBaselineFallback { reason, .. } => {
                assert_eq!(reason, IngestionFallbackReason::ClockBeforeFetch);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn an_empty_history_escalates_to_the_labeled_synthetic_fixture() {
        let decision =
            select_ingested_forecast(None, &[], 2, 3_600_000, t(1_000)).expect("valid selection");
        match decision {
            IngestedForecastDecision::SyntheticFixtureFallback {
                reason,
                fixture_id,
                provenance_label,
            } => {
                assert_eq!(reason, IngestionFallbackReason::NoIngestedForecast);
                assert_eq!(fixture_id, "synthetic-forecast-fixture");
                assert_eq!(provenance_label, "synthetic");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn the_fallback_reasons_render_and_the_statements_compose() {
        let reasons = [
            IngestionFallbackReason::NoIngestedForecast,
            IngestionFallbackReason::MaxAgeExceeded {
                age_millis: 3_600_001,
                max_age_millis: 3_600_000,
            },
            IngestionFallbackReason::ClockBeforeFetch,
        ];
        for reason in &reasons {
            assert!(!reason.as_str().is_empty());
        }
        // The composed statements (never restated here): the ADR-010
        // boundary and the standing no-skill posture.
        assert!(flexcore_application::forecast_integration::FALLBACK_IS_NOT_A_COMMAND_STATEMENT
            .contains("INPUT"));
        assert!(flexcore_application::forecast_baselines::NO_SKILL_STATEMENT
            .contains("no skill, unproven, UNTRAINED"));
    }

    // --- the fetch budget knobs --------------------------------------------

    #[test]
    fn the_fetch_config_refuses_non_positive_bounds_and_clamps_attempts() {
        let error = SharedFetchConfig::new(0, 3_600_000, 10_000, 3)
            .expect_err("zero refresh window refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
        let error = SharedFetchConfig::new(900_000, 0, 10_000, 3)
            .expect_err("zero staleness window refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
        let error = SharedFetchConfig::new(900_000, 3_600_000, 0, 3)
            .expect_err("zero timeout refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
        let config = SharedFetchConfig::new(900_000, 3_600_000, 10_000, 99).expect("clamped");
        assert_eq!(config.max_attempts, 10);
        // Deterministic bounded backoff: attempt 1 → 100 ms, attempt 3 →
        // 400 ms, attempt 20 → capped at 2000 ms.
        assert_eq!(config.backoff_delay_millis(1), 100);
        assert_eq!(config.backoff_delay_millis(3), 400);
        assert_eq!(config.backoff_delay_millis(20), 2_000);
    }
}
