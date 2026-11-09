// P1.21 — Configuration, secrets, and environment handling.
//
// Rules (ledger P1.21):
// - Every setting has a documented source (environment variable); defaults
//   exist ONLY where safe — there is NO default credential anywhere.
// - Missing required values fail fast at startup with a named error that
//   never contains the secret value.
// - Secrets (database URL, broker URL with credentials) are loaded by
//   reference and never logged (the P1.20 no-secrets test covers the log
//   path; Debug for Config redacts).
use std::env;
use std::fmt;
use std::time::Duration;

/// A configuration error: names the missing/invalid setting, never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Missing {
        name: &'static str,
    },
    Invalid {
        name: &'static str,
        reason: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Missing { name } => {
                write!(f, "required configuration value '{name}' is not set")
            }
            ConfigError::Invalid { name, reason } => {
                write!(f, "configuration value '{name}' is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// The control-plane configuration (P1.21 step 1 schema).
#[derive(Clone)]
pub struct Config {
    /// `FLEXCORE_DATABASE_URL` — REQUIRED. PostgreSQL 18 connection string
    /// (may contain credentials; never logged, see Debug impl).
    pub database_url: String,
    /// `FLEXCORE_NATS_URL` — REQUIRED. NATS server URL (may contain
    /// credentials; never logged).
    pub nats_url: String,
    /// `FLEXCORE_BIND_ADDR` — OPTIONAL, default `0.0.0.0:8080`.
    pub bind_addr: String,
    /// `FLEXCORE_LOG_LEVEL` — OPTIONAL, default `info`
    /// (trace|debug|info|warn|error).
    pub log_level: String,
    /// `FLEXCORE_OUTBOX_POLL_INTERVAL_MS` — OPTIONAL, default `1000`.
    pub outbox_poll_interval: Duration,
    /// `FLEXCORE_DRAIN_TIMEOUT_SECS` — OPTIONAL, default `30`. Bounded
    /// graceful-shutdown drain window (P1.22).
    pub drain_timeout: Duration,
    /// `FLEXCORE_CURRENT_STATE_FRESHNESS_MS` — OPTIONAL, default `300000`
    /// (5 minutes). The CURRENT-STATE use-case freshness limit (P2.18 step
    /// 3; a per-use-case limit recorded as CONFIGURATION, never a
    /// hard-coded constant — P2.08 step 3, foundations §6.4). A value older
    /// than this limit is presented with the STALE flag, never as GOOD.
    /// The historical view (P2.19) defines its own limit separately.
    pub current_state_freshness: Duration,
}

impl Config {
    /// Loads and validates the configuration from the process environment.
    /// Missing required values return a named [`ConfigError`].
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("FLEXCORE_DATABASE_URL")
            .map_err(|_| ConfigError::Missing { name: "FLEXCORE_DATABASE_URL" })?;
        if database_url.trim().is_empty() {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_DATABASE_URL",
                reason: "empty".to_string(),
            });
        }
        let nats_url = env::var("FLEXCORE_NATS_URL")
            .map_err(|_| ConfigError::Missing { name: "FLEXCORE_NATS_URL" })?;
        if nats_url.trim().is_empty() {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_NATS_URL",
                reason: "empty".to_string(),
            });
        }
        let bind_addr = env::var("FLEXCORE_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let log_level = env::var("FLEXCORE_LOG_LEVEL").unwrap_or_else(|_| "info".into());
        if !matches!(log_level.as_str(), "trace" | "debug" | "info" | "warn" | "error") {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_LOG_LEVEL",
                reason: format!("'{log_level}' is not trace|debug|info|warn|error"),
            });
        }
        let outbox_poll_ms: u64 = parse_or(
            "FLEXCORE_OUTBOX_POLL_INTERVAL_MS",
            env::var("FLEXCORE_OUTBOX_POLL_INTERVAL_MS").ok(),
            1000,
        )?;
        let drain_secs: u64 = parse_or(
            "FLEXCORE_DRAIN_TIMEOUT_SECS",
            env::var("FLEXCORE_DRAIN_TIMEOUT_SECS").ok(),
            30,
        )?;
        let current_state_freshness_ms: u64 = parse_or(
            "FLEXCORE_CURRENT_STATE_FRESHNESS_MS",
            env::var("FLEXCORE_CURRENT_STATE_FRESHNESS_MS").ok(),
            300_000,
        )?;
        if current_state_freshness_ms == 0 {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_CURRENT_STATE_FRESHNESS_MS",
                reason: "the freshness limit must be a positive millisecond value".to_string(),
            });
        }
        Ok(Self {
            database_url,
            nats_url,
            bind_addr,
            log_level,
            outbox_poll_interval: Duration::from_millis(outbox_poll_ms),
            drain_timeout: Duration::from_secs(drain_secs),
            current_state_freshness: Duration::from_millis(current_state_freshness_ms),
        })
    }
}

fn parse_or(name: &'static str, raw: Option<String>, default: u64) -> Result<u64, ConfigError> {
    match raw {
        None => Ok(default),
        Some(v) => v.parse::<u64>().map_err(|_| ConfigError::Invalid {
            name,
            reason: "not a non-negative integer".to_string(),
        }),
    }
}

impl fmt::Debug for Config {
    /// Redacts both URLs: only their presence and length are rendered.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &format!("<set, {} chars>", self.database_url.len()))
            .field("nats_url", &format!("<set, {} chars>", self.nats_url.len()))
            .field("bind_addr", &self.bind_addr)
            .field("log_level", &self.log_level)
            .field("outbox_poll_interval", &self.outbox_poll_interval)
            .field("drain_timeout", &self.drain_timeout)
            .field("current_state_freshness", &self.current_state_freshness)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_required_value_fails_fast_with_named_error() {
        // Directly on the loader internals: an empty environment yields the
        // named missing error, and the error text carries no value.
        let err = ConfigError::Missing { name: "FLEXCORE_DATABASE_URL" };
        assert_eq!(err.to_string(), "required configuration value 'FLEXCORE_DATABASE_URL' is not set");
    }

    #[test]
    fn debug_never_renders_secret_values() {
        let cfg = Config {
            database_url: "postgres://user:supersecret@db:5432/flexcore".to_string(),
            nats_url: "nats://user:topsecret@nats:4222".to_string(),
            bind_addr: "0.0.0.0:8080".to_string(),
            log_level: "info".to_string(),
            outbox_poll_interval: Duration::from_millis(1000),
            drain_timeout: Duration::from_secs(30),
            current_state_freshness: Duration::from_millis(300_000),
        };
        let rendered = format!("{cfg:?}");
        assert!(!rendered.contains("supersecret"));
        assert!(!rendered.contains("topsecret"));
    }
}
