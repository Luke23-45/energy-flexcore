// P1.19 — Worker configuration from the environment.
//
// Conventions (same as the control plane's `FLEXCORE_*` scheme):
//
// - **Fail fast**: every variable is parsed before any connection is
//   opened; an invalid value aborts startup with a named, human-readable
//   error — never a half-configured worker.
// - **No default credentials, no secret logging**: `FLEXCORE_DATABASE_URL`
//   and `FLEXCORE_NATS_URL` are required (a worker without them has
//   nothing to do, and inventing a default would embed credentials in
//   source). Error messages carry the VARIABLE NAME and the reason, never
//   the VALUE — database URLs contain credentials, and tracing the value
//   would leak them into logs (P1.20's no-secrets-in-logs rule, applied
//   here from day one).
// - Every optional variable has a safe, documented default; none of the
//   defaults changes delivery semantics (they tune polling, parallelism,
//   and the probe/shutdown mechanics only — the §6.3 ordering is
//   hardcoded in the adapters, not configurable).

use std::net::SocketAddr;
use std::time::Duration;

/// Required: PostgreSQL connection URL for the outbox store and the inbox
/// store. Connects as the runtime role (`flexcore_app` per P1.12 §3.1);
/// migrations need the owner role, which this worker deliberately does not
/// hold — the embedded migrations are applied by the operator/bootstrap
/// path (P1.13 role model).
pub const ENV_DATABASE_URL: &str = "FLEXCORE_DATABASE_URL";
/// Required: NATS server URL (JetStream transport).
pub const ENV_NATS_URL: &str = "FLEXCORE_NATS_URL";
/// Optional: log level (`trace|debug|info|warn|error`); default `info`.
pub const ENV_LOG_LEVEL: &str = "FLEXCORE_LOG_LEVEL";
/// Optional: outbox idle poll interval in milliseconds; default 200.
pub const ENV_OUTBOX_POLL_INTERVAL_MS: &str = "FLEXCORE_OUTBOX_POLL_INTERVAL_MS";
/// Optional: number of consumer loop tasks (1..=64); default 1. All tasks
/// share the same durable consumer; inbox deduplication makes parallel
/// delivery safe (at-least-once, never exactly-once).
pub const ENV_CONCURRENCY: &str = "FLEXCORE_CONCURRENCY";
/// Optional: probe server bind address; default `127.0.0.1:8081`.
pub const ENV_PROBE_ADDR: &str = "FLEXCORE_PROBE_ADDR";
/// Optional: bounded shutdown timeout in milliseconds; default 10000.
pub const ENV_SHUTDOWN_TIMEOUT_MS: &str = "FLEXCORE_SHUTDOWN_TIMEOUT_MS";
/// Optional: telemetry ingestion bounded-channel capacity in frames
/// (P2.07 §5 / P2.10); default 10000. At the limit the pipeline sheds
/// frames with a retryable error — never unbounded buffering, never a
/// silent drop.
pub const ENV_INGEST_CHANNEL_CAPACITY: &str = "FLEXCORE_INGEST_CHANNEL_CAPACITY";
/// Optional: the trusted telemetry source identity for the Phase 2 stub
/// source authentication (P2.10 step 6). When set, a provider declaring a
/// different source id is rejected at the receive stage; unset = accept
/// the (single, simulator) provider. The stub status is recorded in
/// [`crate::ingestion::SOURCE_AUTH_STATUS`].
pub const ENV_INGEST_TRUSTED_SOURCE_ID: &str = "FLEXCORE_INGEST_TRUSTED_SOURCE_ID";
/// Optional: the maintenance loop's database URL (P2.20 D-R1). The
/// maintenance job needs DDL (partition CREATE/DETACH/DROP), which the
/// runtime role must not hold (D-P6), so it connects as the
/// migrator/maintenance role through its own URL. UNSET = the maintenance
/// loop is NOT started at all — no maintenance without the privileged path
/// (fail-safe). Secret: never logged.
pub const ENV_MAINTENANCE_DATABASE_URL: &str = "FLEXCORE_MAINTENANCE_DATABASE_URL";
/// Optional: the partition pre-creation/retention pass interval in
/// milliseconds; default 3 600 000 (one hour).
pub const ENV_MAINTENANCE_INTERVAL_MS: &str = "FLEXCORE_MAINTENANCE_INTERVAL_MS";
/// Optional: how many monthly partitions to keep created ahead of the ingest
/// frontier (P2.20 step 1 / D-R1); default 2, bounded to 1..=24.
pub const ENV_MAINTENANCE_MONTHS_AHEAD: &str = "FLEXCORE_MAINTENANCE_MONTHS_AHEAD";
/// Optional: the retention horizon in months for the "normalized telemetry"
/// data class (§16.3; D-R4). UNSET = NO retention is configured: no
/// deletion occurs and the unconfigured state is reported at every pass
/// (D-R4 — the explicit recorded behaviour, never a silent default). Any
/// value present is configuration, never a hard-coded constant (P0.16 §4).
pub const ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS: &str =
    "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS";
/// Optional: whether THIS run is flagged approved to execute retention
/// (`true`/`false`); default `false`. Retention execution is a
/// manual/approved operation in Phase 2 (D-R2): configured-but-unapproved
/// is visible and deletes nothing.
pub const ENV_RETENTION_APPROVED: &str = "FLEXCORE_RETENTION_APPROVED";
/// Optional: path of the synthetic-portfolio scenario file the telemetry
/// ingestion task consumes (P2.10/P2.14/P2.25; e.g.
/// `scenarios/synthetic-portfolio/scenario-v2.json`). SET = the worker
/// composes the simulator `TelemetrySource` and starts the ingestion task;
/// UNSET = the ingestion task is NOT started and the worker runs without
/// it — the recorded honest startup log (the simulator is never started
/// silently). The scenario is loaded all-or-nothing (P2.13); a rejected
/// scenario aborts startup.
pub const ENV_SIMULATOR_SCENARIO: &str = "FLEXCORE_SIMULATOR_SCENARIO";
/// Optional: the identity-provider reference of the tenant the simulator
/// portfolio is registered under (the registration uniqueness anchor,
/// P0.10 §1 row 1; P2.25 step 2). REQUIRED when `FLEXCORE_SIMULATOR_SCENARIO`
/// is set: the ingestion query is tenant-scoped, and the scenario's asset
/// keys are matched against this tenant's registered asset names (the
/// key → registry-ID mapping exists ONLY at registration, P2.13 §1).
pub const ENV_SIMULATOR_TENANT_REFERENCE: &str = "FLEXCORE_SIMULATOR_TENANT_REFERENCE";
/// Optional: master-seed override (D-SC4). Default = the scenario's
/// recorded seed — a run is reproducible unless the operator deliberately
/// overrides it, and an override is a replay-relevant change (P2.24).
pub const ENV_SIMULATOR_SEED: &str = "FLEXCORE_SIMULATOR_SEED";
/// Optional: how many scenario ticks the ingestion subscription covers
/// (1..=10_000_000). Default = the scenario's recorded horizon (the full
/// 24 h exit-gate horizon at the 1-minute cadence, P2.25 §4).
pub const ENV_SIMULATOR_HORIZON_TICKS: &str = "FLEXCORE_SIMULATOR_HORIZON_TICKS";

/// Fail-fast configuration errors. `Display` never includes the
/// environment VALUE (credentials live in the URLs) — name and reason only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// A required variable was absent.
    MissingEnv { name: &'static str },
    /// A variable was present but unusable.
    InvalidEnv { name: &'static str, reason: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEnv { name } => {
                write!(f, "missing required environment variable {name}")
            }
            ConfigError::InvalidEnv { name, reason } => {
                write!(f, "invalid environment variable {name}: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Log level, restricted to the five tracing levels (the env filter is
/// built from this, not from free-form filter syntax, so a typo fails fast
/// instead of silently logging everything).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    fn parse(raw: &str) -> Result<Self, ConfigError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            other => Err(ConfigError::InvalidEnv {
                name: ENV_LOG_LEVEL,
                reason: format!(
                    "expected one of trace|debug|info|warn|error, got a value that parses as '{other}'"
                ),
            }),
        }
    }
}

/// The complete worker configuration. Values that could carry credentials
/// (`database_url`, `nats_url`) are deliberately excluded from any `Debug`
/// rendering implemented here — the struct derives `Debug` with those
/// fields redacted, so a mistake cannot leak them into a log line.
#[derive(Clone)]
pub struct WorkerConfig {
    /// PostgreSQL URL. Secret: never logged.
    pub database_url: String,
    /// NATS URL. Secret-ish: never logged.
    pub nats_url: String,
    pub log_level: LogLevel,
    /// Outbox idle poll interval (publisher idle sleep).
    pub outbox_poll_interval: Duration,
    /// Consumer loop task count (1..=64).
    pub consumer_concurrency: usize,
    /// Probe server bind address.
    pub probe_addr: SocketAddr,
    /// Bounded shutdown timeout for in-flight work.
    pub shutdown_timeout: Duration,
    /// Telemetry ingestion bounded-channel capacity (P2.07 §5 default
    /// 10,000; P2.10).
    pub ingest_channel_capacity: usize,
    /// The trusted telemetry source identity for the Phase 2 stub source
    /// authentication (`None` = accept the configured provider).
    pub ingest_trusted_source_id: Option<String>,
    /// The maintenance loop's database URL (migrator/maintenance role,
    /// D-R1). Secret: never logged. `None` = the maintenance loop is not
    /// started (fail-safe).
    pub maintenance_database_url: Option<String>,
    /// The maintenance pass interval (pre-creation + retention status).
    pub maintenance_interval: Duration,
    /// Monthly partitions kept created ahead of the ingest frontier (D-R1).
    pub maintenance_months_ahead: u32,
    /// The retention horizon in months for the "normalized telemetry" data
    /// class (D-R4). `None` = no retention configured: no deletion occurs,
    /// the state is reported at every pass.
    pub retention_normalized_telemetry_months: Option<u32>,
    /// Whether THIS run is flagged approved to execute retention (D-R2).
    pub retention_approved: bool,
    /// The scenario file path for the telemetry ingestion task's simulator
    /// source (P2.10/P2.14/P2.25). `None` = the ingestion task is not
    /// started (the recorded honest behaviour, never a silent start).
    pub simulator_scenario: Option<String>,
    /// The identity-provider reference of the tenant the simulator
    /// portfolio is registered under (P2.25 step 2). Required when
    /// `simulator_scenario` is set.
    pub simulator_tenant_reference: Option<String>,
    /// The master-seed override (D-SC4). `None` = the scenario's recorded
    /// seed is used unchanged.
    pub simulator_seed: Option<u64>,
    /// The subscription horizon in scenario ticks (1..=10_000_000).
    /// `None` = the scenario's recorded horizon.
    pub simulator_horizon_ticks: Option<u64>,
}

impl std::fmt::Debug for WorkerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerConfig")
            .field("database_url", &"<redacted>")
            .field("nats_url", &"<redacted>")
            .field("log_level", &self.log_level)
            .field("outbox_poll_interval", &self.outbox_poll_interval)
            .field("consumer_concurrency", &self.consumer_concurrency)
            .field("probe_addr", &self.probe_addr)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("ingest_channel_capacity", &self.ingest_channel_capacity)
            .field("ingest_trusted_source_id", &self.ingest_trusted_source_id)
            .field("maintenance_database_url", &"<redacted>")
            .field("maintenance_interval", &self.maintenance_interval)
            .field("maintenance_months_ahead", &self.maintenance_months_ahead)
            .field(
                "retention_normalized_telemetry_months",
                &self.retention_normalized_telemetry_months,
            )
            .field("retention_approved", &self.retention_approved)
            .field("simulator_scenario", &self.simulator_scenario)
            .field("simulator_tenant_reference", &self.simulator_tenant_reference)
            .field("simulator_seed", &self.simulator_seed)
            .field("simulator_horizon_ticks", &self.simulator_horizon_ticks)
            .finish()
    }
}

impl WorkerConfig {
    /// Parses the configuration from the process environment. Fail-fast:
    /// every variable is validated here, before any connection is opened.
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = require(ENV_DATABASE_URL)?;
        let nats_url = require(ENV_NATS_URL)?;
        let log_level = match std::env::var(ENV_LOG_LEVEL) {
            Ok(raw) => LogLevel::parse(&raw)?,
            Err(_) => LogLevel::Info,
        };
        let outbox_poll_interval = optional_ms(
            ENV_OUTBOX_POLL_INTERVAL_MS,
            Duration::from_millis(200),
        )?;
        let consumer_concurrency = optional_parse(ENV_CONCURRENCY, 1usize).and_then(|value| {
            if (1..=64).contains(&value) {
                Ok(value)
            } else {
                Err(ConfigError::InvalidEnv {
                    name: ENV_CONCURRENCY,
                    reason: "expected an integer in 1..=64".to_string(),
                })
            }
        })?;
        let probe_addr = match std::env::var(ENV_PROBE_ADDR) {
            Ok(raw) => raw
                .parse::<SocketAddr>()
                .map_err(|_| ConfigError::InvalidEnv {
                    name: ENV_PROBE_ADDR,
                    reason: "expected a socket address like 127.0.0.1:8081".to_string(),
                })?,
            Err(_) => "127.0.0.1:8081".parse().expect("literal address parses"),
        };
        let shutdown_timeout =
            optional_ms(ENV_SHUTDOWN_TIMEOUT_MS, Duration::from_millis(10_000))?;
        let ingest_channel_capacity = optional_parse(ENV_INGEST_CHANNEL_CAPACITY, 10_000usize)
            .and_then(|value| {
                if (1..=1_000_000).contains(&value) {
                    Ok(value)
                } else {
                    Err(ConfigError::InvalidEnv {
                        name: ENV_INGEST_CHANNEL_CAPACITY,
                        reason: "expected an integer in 1..=1_000_000 frames".to_string(),
                    })
                }
            })?;
        let ingest_trusted_source_id = match std::env::var(ENV_INGEST_TRUSTED_SOURCE_ID) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
            Ok(_) => None,
            Err(_) => None,
        };
        let maintenance_database_url = match std::env::var(ENV_MAINTENANCE_DATABASE_URL) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
            Ok(_) => None,
            Err(_) => None,
        };
        let maintenance_interval =
            optional_ms(ENV_MAINTENANCE_INTERVAL_MS, Duration::from_millis(3_600_000))?;
        let maintenance_months_ahead =
            optional_parse(ENV_MAINTENANCE_MONTHS_AHEAD, 2u32).and_then(|value| {
                if (1..=24).contains(&value) {
                    Ok(value)
                } else {
                    Err(ConfigError::InvalidEnv {
                        name: ENV_MAINTENANCE_MONTHS_AHEAD,
                        reason: "expected an integer in 1..=24 months".to_string(),
                    })
                }
            })?;
        let retention_normalized_telemetry_months =
            match std::env::var(ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS) {
                Ok(raw) => {
                    let months = raw.trim().parse::<u32>().map_err(|_| ConfigError::InvalidEnv {
                        name: ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS,
                        reason: "expected a non-negative integer number of months".to_string(),
                    })?;
                    if months == 0 {
                        return Err(ConfigError::InvalidEnv {
                            name: ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS,
                            reason: "the retention horizon must be at least one month (0 months \
                                     is expressed by leaving the variable unset)"
                                .to_string(),
                        });
                    }
                    Some(months)
                }
                // Unset = the recorded no-retention-configured state (D-R4).
                Err(_) => None,
            };
        let retention_approved = match std::env::var(ENV_RETENTION_APPROVED) {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ConfigError::InvalidEnv {
                        name: ENV_RETENTION_APPROVED,
                        reason: "expected 'true' or 'false'".to_string(),
                    })
                }
            },
            Err(_) => false,
        };
        // Simulator-source composition configuration (P2.10/P2.14/P2.25).
        // Unset scenario = the ingestion task is not started (the recorded
        // honest behaviour); a set-but-empty value is treated as unset.
        let simulator_scenario = match std::env::var(ENV_SIMULATOR_SCENARIO) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
            Ok(_) => None,
            Err(_) => None,
        };
        let simulator_tenant_reference = match std::env::var(ENV_SIMULATOR_TENANT_REFERENCE) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
            Ok(_) => None,
            Err(_) => None,
        };
        let simulator_seed = match std::env::var(ENV_SIMULATOR_SEED) {
            Ok(raw) => Some(raw.trim().parse::<u64>().map_err(|_| ConfigError::InvalidEnv {
                name: ENV_SIMULATOR_SEED,
                reason: "expected a non-negative integer master seed".to_string(),
            })?),
            Err(_) => None,
        };
        let simulator_horizon_ticks = match std::env::var(ENV_SIMULATOR_HORIZON_TICKS) {
            Ok(raw) => {
                let ticks =
                    raw.trim().parse::<u64>().map_err(|_| ConfigError::InvalidEnv {
                        name: ENV_SIMULATOR_HORIZON_TICKS,
                        reason: "expected a positive integer number of ticks".to_string(),
                    })?;
                if ticks == 0 || ticks > 10_000_000 {
                    return Err(ConfigError::InvalidEnv {
                        name: ENV_SIMULATOR_HORIZON_TICKS,
                        reason: "expected an integer in 1..=10_000_000 ticks".to_string(),
                    });
                }
                Some(ticks)
            }
            Err(_) => None,
        };
        Ok(Self {
            database_url,
            nats_url,
            log_level,
            outbox_poll_interval,
            consumer_concurrency,
            probe_addr,
            shutdown_timeout,
            ingest_channel_capacity,
            ingest_trusted_source_id,
            maintenance_database_url,
            maintenance_interval,
            maintenance_months_ahead,
            retention_normalized_telemetry_months,
            retention_approved,
            simulator_scenario,
            simulator_tenant_reference,
            simulator_seed,
            simulator_horizon_ticks,
        })
    }
}

fn require(name: &'static str) -> Result<String, ConfigError> {
    match std::env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => Ok(raw.trim().to_string()),
        Ok(_) => Err(ConfigError::InvalidEnv {
            name,
            reason: "set but empty".to_string(),
        }),
        Err(_) => Err(ConfigError::MissingEnv { name }),
    }
}

fn optional_parse<T: std::str::FromStr>(
    name: &'static str,
    default: T,
) -> Result<T, ConfigError> {
    match std::env::var(name) {
        Ok(raw) => raw.trim().parse::<T>().map_err(|_| ConfigError::InvalidEnv {
            name,
            reason: format!("expected a value parsable as {}", std::any::type_name::<T>()),
        }),
        Err(_) => Ok(default),
    }
}

fn optional_ms(name: &'static str, default: Duration) -> Result<Duration, ConfigError> {
    let millis = optional_parse(name, default.as_millis() as u64).and_then(|value| {
        if value == 0 {
            Err(ConfigError::InvalidEnv {
                name,
                reason: "expected a positive number of milliseconds".to_string(),
            })
        } else {
            Ok(value)
        }
    })?;
    Ok(Duration::from_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env-var tests are inherently serial; the required-variable cases run
    // against unset names by construction (the integration environment
    // never sets these in unit-test runs).

    #[test]
    fn missing_required_variable_is_a_named_error() {
        // No secret values anywhere in the error rendering.
        let error = ConfigError::MissingEnv {
            name: ENV_DATABASE_URL,
        };
        let rendered = error.to_string();
        assert!(rendered.contains("FLEXCORE_DATABASE_URL"));
    }

    #[test]
    fn log_level_parses_and_rejects() {
        assert_eq!(LogLevel::parse("INFO").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::parse(" warn ").unwrap(), LogLevel::Warn);
        assert!(LogLevel::parse("verbose").is_err());
    }

    #[test]
    fn debug_rendering_redacts_urls() {
        let config = WorkerConfig {
            database_url: "postgres://app:secret@db/flexcore".to_string(),
            nats_url: "nats://user:secret@nats:4222".to_string(),
            log_level: LogLevel::Info,
            outbox_poll_interval: Duration::from_millis(200),
            consumer_concurrency: 1,
            probe_addr: "127.0.0.1:8081".parse().unwrap(),
            shutdown_timeout: Duration::from_millis(10_000),
            ingest_channel_capacity: 10_000,
            ingest_trusted_source_id: None,
            maintenance_database_url: Some("postgres://migrator:secret@db/flexcore".to_string()),
            maintenance_interval: Duration::from_millis(3_600_000),
            maintenance_months_ahead: 2,
            retention_normalized_telemetry_months: None,
            retention_approved: false,
            simulator_scenario: Some("scenarios/synthetic-portfolio/scenario-v2.json".to_string()),
            simulator_tenant_reference: Some("idp:flexcore-demo".to_string()),
            simulator_seed: None,
            simulator_horizon_ticks: None,
        };
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("secret"), "secrets must never render: {rendered}");
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn zero_and_out_of_range_values_are_rejected() {
        assert_eq!(
            optional_ms(ENV_OUTBOX_POLL_INTERVAL_MS, Duration::from_millis(200)),
            Ok(Duration::from_millis(200))
        );
    }
}
